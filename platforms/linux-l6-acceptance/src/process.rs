use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use radishlex_linux_product_install::{
    run_linux_maintenance_with_l6_checkpoints, LinuxL6Checkpoint, LinuxL6CheckpointError,
    LinuxL6CheckpointSink,
};
use serde::{Deserialize, Serialize};

use crate::command::{ControllerCommand, WorkerCommand};
use crate::controller::{
    CheckpointNotification, CheckpointProcessBackend, CheckpointProcessError, ProcessGroupProof,
    WorkerTermination,
};

const KILL_PROGRAM: &str = "/usr/bin/kill";
const CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(16 * 60);
const PROCESS_GROUP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_WIRE_BYTES: usize = 4096;
const WIRE_FORMAT: &str = "radishlex-linux-l6-checkpoint-wire-v1";

pub struct LinuxCheckpointProcessBackend;

pub struct LinuxCheckpointWorker {
    child: Child,
    process_group_id: u32,
    notification: Receiver<Result<Vec<u8>, CheckpointProcessError>>,
    finished: bool,
}

impl Drop for LinuxCheckpointWorker {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let _ = terminate_group(self.process_group_id);
        let _ = self.child.wait();
        self.finished = true;
    }
}

impl CheckpointProcessBackend for LinuxCheckpointProcessBackend {
    type Worker = LinuxCheckpointWorker;

    fn spawn_worker(
        &mut self,
        command: &ControllerCommand,
    ) -> Result<Self::Worker, CheckpointProcessError> {
        let executable =
            std::env::current_exe().map_err(|_| CheckpointProcessError::SpawnFailed)?;
        let mut child = Command::new(executable)
            .args(command.worker_arguments())
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .process_group(0)
            .spawn()
            .map_err(|_| CheckpointProcessError::SpawnFailed)?;
        let process_group_id = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or(CheckpointProcessError::SpawnFailed)?;
        let (sender, notification) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut bytes = Vec::new();
            let result = match reader.read_until(b'\n', &mut bytes) {
                Ok(0) => Err(CheckpointProcessError::CheckpointUnavailable),
                Ok(_) if bytes.len() > MAX_WIRE_BYTES || !bytes.ends_with(b"\n") => {
                    Err(CheckpointProcessError::CheckpointUnavailable)
                }
                Ok(_) => Ok(bytes),
                Err(_) => Err(CheckpointProcessError::CheckpointUnavailable),
            };
            let _ = sender.send(result);
        });
        Ok(LinuxCheckpointWorker {
            child,
            process_group_id,
            notification,
            finished: false,
        })
    }

    fn wait_for_checkpoint(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<CheckpointNotification, CheckpointProcessError> {
        let bytes = worker
            .notification
            .recv_timeout(CHECKPOINT_TIMEOUT)
            .map_err(|_| CheckpointProcessError::CheckpointUnavailable)??;
        let wire: CheckpointWireV1 = serde_json::from_slice(&bytes)
            .map_err(|_| CheckpointProcessError::CheckpointUnavailable)?;
        if wire.canonical_bytes()? != bytes {
            return Err(CheckpointProcessError::CheckpointUnavailable);
        }
        Ok(CheckpointNotification::new(parse_checkpoint(
            &wire.checkpoint,
        )?))
    }

    fn terminate_process_group(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<(), CheckpointProcessError> {
        terminate_group(worker.process_group_id)
    }

    fn wait_for_worker(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<WorkerTermination, CheckpointProcessError> {
        let status = worker
            .child
            .wait()
            .map_err(|_| CheckpointProcessError::WorkerTerminationInvalid)?;
        worker.finished = true;
        Ok(status
            .signal()
            .map_or_else(WorkerTermination::exited, WorkerTermination::signaled))
    }

    fn prove_process_group_empty(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<ProcessGroupProof, CheckpointProcessError> {
        let deadline = Instant::now() + PROCESS_GROUP_TIMEOUT;
        let mut consecutive_empty = 0;
        loop {
            let proof = scan_process_group(worker.process_group_id)?;
            if proof.proves_empty_without_dpkg() {
                consecutive_empty += 1;
                if consecutive_empty == 2 {
                    return Ok(proof);
                }
            } else {
                consecutive_empty = 0;
            }
            if Instant::now() >= deadline {
                return Ok(proof);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub fn run_checkpoint_worker(command: WorkerCommand) -> Result<(), CheckpointProcessError> {
    let scenario = command.scenario();
    let mut checkpoints = BlockingCheckpointSink {
        target: scenario.checkpoint(),
    };
    run_linux_maintenance_with_l6_checkpoints(
        command.maintenance().clone(),
        &mut checkpoints,
        scenario.reject_target_validation(),
    )
    .map_err(|_| CheckpointProcessError::CheckpointUnavailable)?;
    Err(CheckpointProcessError::CheckpointUnavailable)
}

struct BlockingCheckpointSink {
    target: LinuxL6Checkpoint,
}

impl LinuxL6CheckpointSink for BlockingCheckpointSink {
    fn reached(&mut self, checkpoint: LinuxL6Checkpoint) -> Result<(), LinuxL6CheckpointError> {
        if checkpoint != self.target {
            return Ok(());
        }
        let wire = CheckpointWireV1 {
            format: WIRE_FORMAT.to_owned(),
            checkpoint: checkpoint.as_str().to_owned(),
        };
        let bytes = wire
            .canonical_bytes()
            .map_err(|_| LinuxL6CheckpointError::unavailable(checkpoint))?;
        std::io::stdout()
            .write_all(&bytes)
            .and_then(|()| std::io::stdout().flush())
            .map_err(|_| LinuxL6CheckpointError::unavailable(checkpoint))?;
        loop {
            std::thread::park();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointWireV1 {
    format: String,
    checkpoint: String,
}

impl CheckpointWireV1 {
    fn canonical_bytes(&self) -> Result<Vec<u8>, CheckpointProcessError> {
        if self.format != WIRE_FORMAT || parse_checkpoint(&self.checkpoint).is_err() {
            return Err(CheckpointProcessError::CheckpointUnavailable);
        }
        let mut bytes =
            serde_json::to_vec(self).map_err(|_| CheckpointProcessError::CheckpointUnavailable)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn terminate_group(process_group_id: u32) -> Result<(), CheckpointProcessError> {
    let status = Command::new(KILL_PROGRAM)
        .args(["-KILL", "--", &format!("-{process_group_id}")])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| CheckpointProcessError::TerminationFailed)?;
    if status.success() {
        Ok(())
    } else {
        Err(CheckpointProcessError::TerminationFailed)
    }
}

fn scan_process_group(process_group_id: u32) -> Result<ProcessGroupProof, CheckpointProcessError> {
    let mut members = 0usize;
    let mut dpkg_members = 0usize;
    for entry in
        fs::read_dir("/proc").map_err(|_| CheckpointProcessError::ProcessInspectionFailed)?
    {
        let entry = entry.map_err(|_| CheckpointProcessError::ProcessInspectionFailed)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let stat_path = entry.path().join("stat");
        let stat = match fs::read_to_string(stat_path) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(CheckpointProcessError::ProcessInspectionFailed),
        };
        let Some((group, command_name)) = parse_process_stat(&stat) else {
            return Err(CheckpointProcessError::ProcessInspectionFailed);
        };
        if group == process_group_id {
            members += 1;
            if command_name == "dpkg" || command_name.starts_with("dpkg-") {
                dpkg_members += 1;
            }
        }
    }
    Ok(ProcessGroupProof::new(members, dpkg_members, true))
}

fn parse_process_stat(value: &str) -> Option<(u32, &str)> {
    if value.len() > MAX_WIRE_BYTES || value.contains('\0') {
        return None;
    }
    let command_start = value.find('(')? + 1;
    let command_end = value.rfind(')')?;
    if command_end < command_start {
        return None;
    }
    let command_name = &value[command_start..command_end];
    let fields = value
        .get(command_end + 1..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    let process_group_id = fields.get(2)?.parse().ok()?;
    Some((process_group_id, command_name))
}

fn parse_checkpoint(value: &str) -> Result<LinuxL6Checkpoint, CheckpointProcessError> {
    use LinuxL6Checkpoint::*;
    match value {
        "prepared" => Ok(Prepared),
        "artifacts_staged" => Ok(ArtifactsStaged),
        "quiesced" => Ok(Quiesced),
        "package_mutating_before_dpkg" => Ok(PackageMutatingBeforeDpkg),
        "target_applied_before_proof" => Ok(TargetAppliedBeforeProof),
        "rollback_required" => Ok(RollbackRequired),
        "source_restoring_before_dpkg" => Ok(SourceRestoringBeforeDpkg),
        "source_applied_before_proof" => Ok(SourceAppliedBeforeProof),
        _ => Err(CheckpointProcessError::CheckpointUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_stat_parser_extracts_group_without_exposing_pid_or_raw_text() {
        let stat = "4242 (dpkg helper) S 1 4242 4242 0 -1 0\n";
        assert_eq!(parse_process_stat(stat), Some((4242, "dpkg helper")));
        assert!(parse_process_stat("malformed").is_none());
    }

    #[test]
    fn checkpoint_wire_is_canonical_and_bounded() {
        for scenario in crate::L6CrashScenario::ALL {
            let wire = CheckpointWireV1 {
                format: WIRE_FORMAT.to_owned(),
                checkpoint: scenario.checkpoint().as_str().to_owned(),
            };
            let bytes = wire.canonical_bytes().expect("canonical wire");
            assert!(bytes.len() < MAX_WIRE_BYTES);
            assert!(bytes.ends_with(b"\n"));
        }
    }
}
