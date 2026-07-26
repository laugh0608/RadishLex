use std::fmt;

pub const QUALIFICATION_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QualificationRunState {
    Created = 1,
    Running = 2,
    Cancelling = 3,
    Completed = 4,
    Failed = 5,
    Cancelled = 6,
}

impl QualificationRunState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QualificationPhase {
    Created = 1,
    ValidateRequest = 2,
    PrepareWorkspace = 3,
    CreateDomain = 4,
    AuthorizeSecondDevice = 5,
    ClientAFirstSync = 6,
    PrepareConflict = 7,
    ClientBMerge = 8,
    ClientAConflictRecovery = 9,
    VerifyConvergence = 10,
    Cleanup = 11,
    Complete = 12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QualificationErrorCode {
    None = 0,
    InvalidRequest = 1,
    AlreadyRunning = 2,
    Unauthenticated = 3,
    TlsRejected = 4,
    TransportTimeout = 5,
    ServerUnavailable = 6,
    ProtocolRejected = 7,
    CryptoRejected = 8,
    LocalStorageFailed = 9,
    ConflictNotObserved = 10,
    ConvergenceFailed = 11,
    Cancelled = 12,
    Internal = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualificationError {
    pub code: QualificationErrorCode,
    pub phase: QualificationPhase,
    pub retryable: bool,
}

impl QualificationError {
    pub(crate) fn new(
        code: QualificationErrorCode,
        phase: QualificationPhase,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            phase,
            retryable,
        }
    }

    pub(crate) fn invalid_request() -> Self {
        Self::new(
            QualificationErrorCode::InvalidRequest,
            QualificationPhase::ValidateRequest,
            false,
        )
    }
}

impl fmt::Display for QualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Manager sync qualification failed with code {} at phase {}",
            self.code as u32, self.phase as u32
        )
    }
}

impl std::error::Error for QualificationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationRunSnapshot {
    pub version: u32,
    pub state: QualificationRunState,
    pub phase: QualificationPhase,
    pub discovered: usize,
    pub downloaded: usize,
    pub applied: usize,
    pub uploaded: usize,
    pub conflicts: usize,
    pub retries: usize,
    pub convergence_rounds: usize,
    pub temporary_files_cleaned: bool,
    pub worker_stopped: bool,
    pub transient_inputs_cleared: bool,
    pub error: Option<QualificationError>,
}

impl Default for QualificationRunSnapshot {
    fn default() -> Self {
        Self {
            version: QUALIFICATION_SNAPSHOT_VERSION,
            state: QualificationRunState::Created,
            phase: QualificationPhase::Created,
            discovered: 0,
            downloaded: 0,
            applied: 0,
            uploaded: 0,
            conflicts: 0,
            retries: 0,
            convergence_rounds: 0,
            temporary_files_cleaned: false,
            worker_stopped: false,
            transient_inputs_cleared: false,
            error: None,
        }
    }
}
