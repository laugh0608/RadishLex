use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::debian::{DebianCommandInvocation, DPKG_PROGRAM, NULL_DEVICE};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const WAIT_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianCommandTermination {
    Exited(i32),
    Signaled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebianCommandOutput {
    termination: DebianCommandTermination,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl DebianCommandOutput {
    pub fn new(termination: DebianCommandTermination, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            termination,
            stdout,
            stderr,
        }
    }

    pub const fn termination(&self) -> DebianCommandTermination {
        self.termination
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }

    pub const fn succeeded(&self) -> bool {
        matches!(self.termination, DebianCommandTermination::Exited(0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianExecutionErrorCode {
    ProgramIdentity,
    PermissionDenied,
    DiagnosticsOverflow,
    TimedOut,
    Io,
}

#[derive(Debug)]
pub struct DebianExecutionError {
    code: DebianExecutionErrorCode,
    message: &'static str,
    source: Option<io::Error>,
}

impl DebianExecutionError {
    pub fn new(code: DebianExecutionErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            source: None,
        }
    }

    fn io(message: &'static str, source: io::Error) -> Self {
        let code = if source.kind() == io::ErrorKind::PermissionDenied {
            DebianExecutionErrorCode::PermissionDenied
        } else {
            DebianExecutionErrorCode::Io
        };
        Self {
            code,
            message,
            source: Some(source),
        }
    }

    pub const fn code(&self) -> DebianExecutionErrorCode {
        self.code
    }
}

impl fmt::Display for DebianExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Some(source) => write!(formatter, "{}: {}", self.message, source),
            None => formatter.write_str(self.message),
        }
    }
}

impl std::error::Error for DebianExecutionError {}

pub trait DebianCommandExecutor {
    fn execute(
        &mut self,
        invocation: &DebianCommandInvocation,
    ) -> Result<DebianCommandOutput, DebianExecutionError>;
}

#[derive(Debug, Default)]
pub struct LinuxSystemCommandExecutor;

impl DebianCommandExecutor for LinuxSystemCommandExecutor {
    fn execute(
        &mut self,
        invocation: &DebianCommandInvocation,
    ) -> Result<DebianCommandOutput, DebianExecutionError> {
        validate_program_identity(invocation.program())?;
        if invocation.program() != DPKG_PROGRAM
            || invocation.standard_input().path() != NULL_DEVICE
            || !invocation.clears_environment()
        {
            return Err(DebianExecutionError::new(
                DebianExecutionErrorCode::ProgramIdentity,
                "Debian command invocation differs from the fixed contract",
            ));
        }
        let null = fs::File::open(NULL_DEVICE)
            .map_err(|error| DebianExecutionError::io("open null standard input", error))?;
        let mut command = Command::new(invocation.program());
        command
            .args(invocation.arguments())
            .env_clear()
            .envs(invocation.environment().iter().copied())
            .current_dir("/")
            .stdin(Stdio::from(null))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| DebianExecutionError::io("spawn fixed dpkg command", error))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            DebianExecutionError::new(
                DebianExecutionErrorCode::Io,
                "dpkg stdout pipe is unavailable",
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            DebianExecutionError::new(
                DebianExecutionErrorCode::Io,
                "dpkg stderr pipe is unavailable",
            )
        })?;
        let limits = invocation.diagnostics();
        let stdout_reader =
            thread::spawn(move || read_bounded_and_drain(stdout, limits.stdout_limit));
        let stderr_reader =
            thread::spawn(move || read_bounded_and_drain(stderr, limits.stderr_limit));
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| DebianExecutionError::io("wait for fixed dpkg command", error))?
            {
                break status;
            }
            if started.elapsed() >= COMMAND_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DebianExecutionError::new(
                    DebianExecutionErrorCode::TimedOut,
                    "fixed dpkg command exceeded the execution timeout",
                ));
            }
            thread::sleep(WAIT_INTERVAL);
        };
        let stdout = join_reader(stdout_reader, "read bounded dpkg stdout")?;
        let stderr = join_reader(stderr_reader, "read bounded dpkg stderr")?;
        if stdout.overflow || stderr.overflow {
            return Err(DebianExecutionError::new(
                DebianExecutionErrorCode::DiagnosticsOverflow,
                "dpkg diagnostics exceeded the fixed byte budget",
            ));
        }
        Ok(DebianCommandOutput::new(
            status.code().map_or(
                DebianCommandTermination::Signaled,
                DebianCommandTermination::Exited,
            ),
            stdout.value,
            stderr.value,
        ))
    }
}

fn validate_program_identity(program: &str) -> Result<(), DebianExecutionError> {
    if program != DPKG_PROGRAM {
        return Err(DebianExecutionError::new(
            DebianExecutionErrorCode::ProgramIdentity,
            "Debian command program is not the fixed dpkg path",
        ));
    }
    let path = Path::new(program);
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| DebianExecutionError::io("inspect fixed dpkg program", error))?;
    let canonical = fs::canonicalize(path)
        .map_err(|error| DebianExecutionError::io("canonicalize fixed dpkg program", error))?;
    if canonical != path
        || !metadata.file_type().is_file()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.mode() & 0o7777 != 0o755
        || metadata.nlink() != 1
    {
        return Err(DebianExecutionError::new(
            DebianExecutionErrorCode::ProgramIdentity,
            "fixed dpkg program identity is invalid",
        ));
    }
    Ok(())
}

struct BoundedStream {
    value: Vec<u8>,
    overflow: bool,
}

fn read_bounded_and_drain(mut reader: impl Read, limit: usize) -> Result<BoundedStream, io::Error> {
    let mut value = Vec::with_capacity(limit.min(64 * 1024));
    let mut overflow = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(value.len());
        let accepted = remaining.min(count);
        value.extend_from_slice(&buffer[..accepted]);
        overflow |= accepted != count;
    }
    Ok(BoundedStream { value, overflow })
}

fn join_reader(
    reader: thread::JoinHandle<Result<BoundedStream, io::Error>>,
    message: &'static str,
) -> Result<BoundedStream, DebianExecutionError> {
    reader
        .join()
        .map_err(|_| DebianExecutionError::new(DebianExecutionErrorCode::Io, message))?
        .map_err(|error| DebianExecutionError::io(message, error))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn bounded_reader_retains_only_the_budget_and_drains_the_stream() {
        let bytes = (0_u8..32).collect::<Vec<_>>();
        let mut reader = Cursor::new(bytes.clone());
        let output = read_bounded_and_drain(&mut reader, 11).expect("bounded diagnostic stream");
        assert_eq!(output.value, bytes[..11]);
        assert!(output.overflow);
        assert_eq!(reader.position(), bytes.len() as u64);
    }

    #[test]
    fn bounded_reader_accepts_the_exact_limit_without_overflow() {
        let bytes = b"bounded diagnostics".to_vec();
        let output = read_bounded_and_drain(Cursor::new(bytes.clone()), bytes.len())
            .expect("exact bounded diagnostic stream");
        assert_eq!(output.value, bytes);
        assert!(!output.overflow);
    }

    #[test]
    fn command_output_success_requires_an_exact_zero_exit() {
        assert!(DebianCommandOutput::new(
            DebianCommandTermination::Exited(0),
            Vec::new(),
            Vec::new(),
        )
        .succeeded());
        for termination in [
            DebianCommandTermination::Exited(1),
            DebianCommandTermination::Signaled,
        ] {
            assert!(!DebianCommandOutput::new(termination, Vec::new(), Vec::new()).succeeded());
        }
    }
}
