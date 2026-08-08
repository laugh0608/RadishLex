//! Compile-isolated L6 checkpoint and evidence controller.

#![forbid(unsafe_code)]

mod command;
mod controller;
mod evidence;
#[cfg(any(target_os = "linux", test))]
mod process;
mod scenario;

pub use command::{ControllerCommand, ControllerCommandError, WorkerCommand};
pub use controller::{
    run_checkpoint_controller, CheckpointNotification, CheckpointProcessBackend,
    CheckpointProcessError, ProcessGroupProof, WorkerTermination,
};
#[cfg(any(target_os = "linux", test))]
pub use evidence::write_system_checkpoint_evidence;
pub use evidence::{
    CheckpointEvidenceEnvelopeV1, CheckpointEvidenceError, L6_ACCEPTANCE_BUILD_IDENTITY,
    L6_CHECKPOINT_EVIDENCE_FORMAT, L6_EVIDENCE_ROOT,
};
#[cfg(any(target_os = "linux", test))]
pub use process::{run_checkpoint_worker, LinuxCheckpointProcessBackend};
pub use scenario::L6CrashScenario;
