use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::manifest::VerifiedExecutable;

const HOST_TIMEOUT: Duration = Duration::from_secs(30);
const HOST_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_HOST_OUTPUT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostMode {
    Preflight,
    Candidate,
    PostSwitch,
}

#[derive(Debug)]
pub(crate) struct HostOutput {
    pub(crate) success: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(crate) trait ProductHostRunner {
    fn run(&mut self, executable: &VerifiedExecutable, mode: HostMode) -> HostOutput;
}

pub(crate) struct ProcessProductHostRunner;

impl ProductHostRunner for ProcessProductHostRunner {
    fn run(&mut self, executable: &VerifiedExecutable, mode: HostMode) -> HostOutput {
        if executable.revalidate().is_err() {
            return failed_output();
        }
        let mut command = Command::new(executable.path());
        if mode == HostMode::PostSwitch {
            command.arg("--post-switch");
        }
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("DYLD_") || key == "CFFIXED_USER_HOME" {
                command.env_remove(key);
            }
        }
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let Ok(mut child) = command.spawn() else {
            return failed_output();
        };
        let deadline = Instant::now() + HOST_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = read_bounded(child.stdout.take());
                    let stderr = read_bounded(child.stderr.take());
                    return HostOutput {
                        success: status.success()
                            && stdout
                                .as_ref()
                                .is_some_and(|value| value.len() <= MAX_HOST_OUTPUT_BYTES)
                            && stderr
                                .as_ref()
                                .is_some_and(|value| value.len() <= MAX_HOST_OUTPUT_BYTES),
                        stdout: stdout.unwrap_or_default(),
                        stderr: stderr.unwrap_or_default(),
                    };
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(HOST_POLL_INTERVAL),
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return failed_output();
                }
            }
        }
    }
}

fn read_bounded<R: Read>(stream: Option<R>) -> Option<Vec<u8>> {
    let stream = stream?;
    let mut bytes = Vec::new();
    stream
        .take((MAX_HOST_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

fn failed_output() -> HostOutput {
    HostOutput {
        success: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}
