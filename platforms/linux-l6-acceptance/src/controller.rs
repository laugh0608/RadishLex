use std::fmt;

use radishlex_linux_product_install::LinuxL6Checkpoint;

use crate::command::ControllerCommand;
use crate::evidence::CheckpointEvidenceEnvelopeV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointNotification {
    checkpoint: LinuxL6Checkpoint,
}

impl CheckpointNotification {
    pub const fn new(checkpoint: LinuxL6Checkpoint) -> Self {
        Self { checkpoint }
    }

    pub const fn checkpoint(self) -> LinuxL6Checkpoint {
        self.checkpoint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerTermination {
    signal: Option<i32>,
}

impl WorkerTermination {
    pub const fn signaled(signal: i32) -> Self {
        Self {
            signal: Some(signal),
        }
    }

    pub const fn exited() -> Self {
        Self { signal: None }
    }

    pub const fn signal(self) -> Option<i32> {
        self.signal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessGroupProof {
    member_count: usize,
    dpkg_member_count: usize,
    inspection_complete: bool,
}

impl ProcessGroupProof {
    pub const fn new(
        member_count: usize,
        dpkg_member_count: usize,
        inspection_complete: bool,
    ) -> Self {
        Self {
            member_count,
            dpkg_member_count,
            inspection_complete,
        }
    }

    pub const fn proves_empty_without_dpkg(self) -> bool {
        self.inspection_complete && self.member_count == 0 && self.dpkg_member_count == 0
    }
}

pub trait CheckpointProcessBackend {
    type Worker;

    fn spawn_worker(
        &mut self,
        command: &ControllerCommand,
    ) -> Result<Self::Worker, CheckpointProcessError>;

    fn wait_for_checkpoint(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<CheckpointNotification, CheckpointProcessError>;

    fn terminate_process_group(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<(), CheckpointProcessError>;

    fn wait_for_worker(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<WorkerTermination, CheckpointProcessError>;

    fn prove_process_group_empty(
        &mut self,
        worker: &mut Self::Worker,
    ) -> Result<ProcessGroupProof, CheckpointProcessError>;
}

pub fn run_checkpoint_controller(
    command: &ControllerCommand,
    backend: &mut impl CheckpointProcessBackend,
) -> Result<CheckpointEvidenceEnvelopeV1, CheckpointProcessError> {
    let mut worker = backend.spawn_worker(command)?;
    let notification = match backend.wait_for_checkpoint(&mut worker) {
        Ok(notification) => notification,
        Err(error) => {
            let _ = backend.terminate_process_group(&mut worker);
            let _ = backend.wait_for_worker(&mut worker);
            return Err(error);
        }
    };
    if notification.checkpoint() != command.scenario().checkpoint() {
        let _ = backend.terminate_process_group(&mut worker);
        let _ = backend.wait_for_worker(&mut worker);
        return Err(CheckpointProcessError::CheckpointMismatch);
    }
    backend.terminate_process_group(&mut worker)?;
    let termination = backend.wait_for_worker(&mut worker)?;
    let proof = backend.prove_process_group_empty(&mut worker)?;
    if !proof.proves_empty_without_dpkg() {
        return Err(CheckpointProcessError::ProcessGroupNotEmpty);
    }
    if termination.signal() != Some(9) {
        return Err(CheckpointProcessError::WorkerTerminationInvalid);
    }
    CheckpointEvidenceEnvelopeV1::new(command, termination, proof)
        .map_err(|_| CheckpointProcessError::EvidenceInvalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointProcessError {
    SpawnFailed,
    CheckpointUnavailable,
    CheckpointMismatch,
    TerminationFailed,
    WorkerTerminationInvalid,
    ProcessInspectionFailed,
    ProcessGroupNotEmpty,
    EvidenceInvalid,
}

impl fmt::Display for CheckpointProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::SpawnFailed => "L6 checkpoint worker could not start",
            Self::CheckpointUnavailable => "L6 checkpoint notification is unavailable",
            Self::CheckpointMismatch => "L6 checkpoint notification differs from the scenario",
            Self::TerminationFailed => "L6 process group termination failed",
            Self::WorkerTerminationInvalid => "L6 worker termination is not SIGKILL",
            Self::ProcessInspectionFailed => "L6 process group inspection failed",
            Self::ProcessGroupNotEmpty => "L6 process group or dpkg child remains",
            Self::EvidenceInvalid => "L6 checkpoint evidence is invalid",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CheckpointProcessError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend {
        checkpoint: LinuxL6Checkpoint,
        termination: WorkerTermination,
        proof: ProcessGroupProof,
        calls: Vec<&'static str>,
    }

    impl CheckpointProcessBackend for FakeBackend {
        type Worker = ();

        fn spawn_worker(
            &mut self,
            _command: &ControllerCommand,
        ) -> Result<Self::Worker, CheckpointProcessError> {
            self.calls.push("spawn");
            Ok(())
        }

        fn wait_for_checkpoint(
            &mut self,
            _worker: &mut Self::Worker,
        ) -> Result<CheckpointNotification, CheckpointProcessError> {
            self.calls.push("checkpoint");
            Ok(CheckpointNotification::new(self.checkpoint))
        }

        fn terminate_process_group(
            &mut self,
            _worker: &mut Self::Worker,
        ) -> Result<(), CheckpointProcessError> {
            self.calls.push("terminate_group");
            Ok(())
        }

        fn wait_for_worker(
            &mut self,
            _worker: &mut Self::Worker,
        ) -> Result<WorkerTermination, CheckpointProcessError> {
            self.calls.push("wait");
            Ok(self.termination)
        }

        fn prove_process_group_empty(
            &mut self,
            _worker: &mut Self::Worker,
        ) -> Result<ProcessGroupProof, CheckpointProcessError> {
            self.calls.push("prove_empty");
            Ok(self.proof)
        }
    }

    fn command() -> ControllerCommand {
        let arguments = vec![
            "crash".to_owned(),
            "--scenario".to_owned(),
            "install_prepared".to_owned(),
            "--repository-commit".to_owned(),
            "a".repeat(40),
            "--guest-identity-sha256".to_owned(),
            "b".repeat(64),
            "--snapshot-identity-sha256".to_owned(),
            "c".repeat(64),
            "--authorized-l6-crash".to_owned(),
            "--".to_owned(),
            "start".to_owned(),
            "--operation-id".to_owned(),
            "0123456789abcdef0123456789abcdef".to_owned(),
            "--kind".to_owned(),
            "install".to_owned(),
            "--target-deb".to_owned(),
            "/var/tmp/radishlex_1.0-1_arm64.deb".to_owned(),
            "--target-evidence".to_owned(),
            "/var/tmp/radishlex_1.0-1_arm64.deb.evidence.json".to_owned(),
            "--authorized-system-mutation".to_owned(),
            "--preserve-user-data".to_owned(),
        ];
        ControllerCommand::parse(&arguments).expect("parse controller")
    }

    #[test]
    fn controller_terminates_and_proves_the_whole_group_before_evidence() {
        let command = command();
        let mut backend = FakeBackend {
            checkpoint: command.scenario().checkpoint(),
            termination: WorkerTermination::signaled(9),
            proof: ProcessGroupProof::new(0, 0, true),
            calls: Vec::new(),
        };
        run_checkpoint_controller(&command, &mut backend).expect("run controller");
        assert_eq!(
            backend.calls,
            [
                "spawn",
                "checkpoint",
                "terminate_group",
                "wait",
                "prove_empty"
            ]
        );
    }

    #[test]
    fn controller_refuses_evidence_when_any_group_or_dpkg_member_remains() {
        let command = command();
        for proof in [
            ProcessGroupProof::new(1, 0, true),
            ProcessGroupProof::new(1, 1, true),
            ProcessGroupProof::new(0, 0, false),
        ] {
            let mut backend = FakeBackend {
                checkpoint: command.scenario().checkpoint(),
                termination: WorkerTermination::signaled(9),
                proof,
                calls: Vec::new(),
            };
            assert_eq!(
                run_checkpoint_controller(&command, &mut backend).unwrap_err(),
                CheckpointProcessError::ProcessGroupNotEmpty
            );
        }
    }

    #[test]
    fn controller_proves_the_group_empty_but_rejects_non_sigkill_termination() {
        let command = command();
        let mut backend = FakeBackend {
            checkpoint: command.scenario().checkpoint(),
            termination: WorkerTermination::exited(),
            proof: ProcessGroupProof::new(0, 0, true),
            calls: Vec::new(),
        };
        assert_eq!(
            run_checkpoint_controller(&command, &mut backend).unwrap_err(),
            CheckpointProcessError::WorkerTerminationInvalid
        );
        assert_eq!(backend.calls.last(), Some(&"prove_empty"));
    }
}
