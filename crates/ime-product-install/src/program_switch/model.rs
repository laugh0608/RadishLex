use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramSwitchErrorCode {
    UnsafeTargetParent,
    UnsafeTransactionDirectory,
    UnexpectedTransactionObject,
    ArtifactMissing,
    ArtifactIdentityChanged,
    CrossDevice,
    InvalidOperationState,
    Receipt,
    FaultInjected,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramSwitchError {
    code: ProgramSwitchErrorCode,
}

impl ProgramSwitchError {
    pub(super) const fn new(code: ProgramSwitchErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> ProgramSwitchErrorCode {
        self.code
    }
}

impl fmt::Display for ProgramSwitchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ProgramSwitchErrorCode::UnsafeTargetParent => "program target parent is unsafe",
            ProgramSwitchErrorCode::UnsafeTransactionDirectory => {
                "program transaction directory is unsafe"
            }
            ProgramSwitchErrorCode::UnexpectedTransactionObject => {
                "program transaction directory contains an unexpected object"
            }
            ProgramSwitchErrorCode::ArtifactMissing => "program artifact is missing",
            ProgramSwitchErrorCode::ArtifactIdentityChanged => {
                "program artifact filesystem identity changed"
            }
            ProgramSwitchErrorCode::CrossDevice => {
                "program artifact is not on the target filesystem"
            }
            ProgramSwitchErrorCode::InvalidOperationState => {
                "program switch is not valid in the current operation state"
            }
            ProgramSwitchErrorCode::Receipt => "program switch receipt update failed",
            ProgramSwitchErrorCode::FaultInjected => "program switch fault was injected",
            ProgramSwitchErrorCode::Io => "program switch filesystem operation failed",
        })
    }
}

impl std::error::Error for ProgramSwitchError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramSwitchAction {
    PreserveSource,
    CommitTarget,
    ReturnTargetToStage,
    RestoreSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramSwitchBoundary {
    BeforeRename,
    AfterRename,
    AfterTargetDirectorySync,
    AfterSourceDirectorySync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramSwitchFaultPoint {
    action: ProgramSwitchAction,
    boundary: ProgramSwitchBoundary,
}

impl ProgramSwitchFaultPoint {
    pub const fn new(action: ProgramSwitchAction, boundary: ProgramSwitchBoundary) -> Self {
        Self { action, boundary }
    }

    pub const fn action(self) -> ProgramSwitchAction {
        self.action
    }

    pub const fn boundary(self) -> ProgramSwitchBoundary {
        self.boundary
    }
}

pub trait ProgramSwitchFaultInjector {
    fn should_fail(&mut self, point: ProgramSwitchFaultPoint) -> bool;
}

#[derive(Debug, Default)]
pub struct NoProgramSwitchFaults;

impl ProgramSwitchFaultInjector for NoProgramSwitchFaults {
    fn should_fail(&mut self, _point: ProgramSwitchFaultPoint) -> bool {
        false
    }
}
