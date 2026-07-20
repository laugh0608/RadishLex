use std::fmt;

use radishlex_ime_crypto::SignedSyncObjectManifest;

use crate::{
    AssembledSyncObject, OpaqueSyncCursor, PlaintextSyncPayload, RemoteObjectVersion,
    SyncObjectType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncCyclePhase {
    Preflight,
    Discover,
    Download,
    Verify,
    DecryptAndDecode,
    ApplyAndAdvanceCursor,
    PlanUpload,
    PrepareSignedOutbox,
    Upload,
    AcknowledgeOutbox,
    Complete,
}

impl SyncCyclePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preflight => "preflight",
            Self::Discover => "discover",
            Self::Download => "download",
            Self::Verify => "verify",
            Self::DecryptAndDecode => "decrypt_and_decode",
            Self::ApplyAndAdvanceCursor => "apply_and_advance_cursor",
            Self::PlanUpload => "plan_upload",
            Self::PrepareSignedOutbox => "prepare_signed_outbox",
            Self::Upload => "upload",
            Self::AcknowledgeOutbox => "acknowledge_outbox",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncCycleOutcome {
    Completed,
    Cancelled,
    Blocked,
    Failed,
}

impl SyncCycleOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOrchestrationErrorCode {
    PolicyBlocked,
    BackendUnavailable,
    Unauthenticated,
    RevokedDevice,
    KeyEpochRejected,
    UnsupportedSchema,
    UnsupportedAlgorithm,
    InvalidMetadata,
    SignatureMismatch,
    CiphertextHashMismatch,
    AadMismatch,
    DecryptFailed,
    DecodeFailed,
    LocalTransactionFailed,
    CursorInvalid,
    ConflictRetryExhausted,
    TransportTimeout,
    ServerUnavailable,
    Cancelled,
}

impl SyncOrchestrationErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PolicyBlocked => "policy_blocked",
            Self::BackendUnavailable => "backend_unavailable",
            Self::Unauthenticated => "unauthenticated",
            Self::RevokedDevice => "revoked_device",
            Self::KeyEpochRejected => "key_epoch_rejected",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::UnsupportedAlgorithm => "unsupported_algorithm",
            Self::InvalidMetadata => "invalid_metadata",
            Self::SignatureMismatch => "signature_mismatch",
            Self::CiphertextHashMismatch => "ciphertext_hash_mismatch",
            Self::AadMismatch => "aad_mismatch",
            Self::DecryptFailed => "decrypt_failed",
            Self::DecodeFailed => "decode_failed",
            Self::LocalTransactionFailed => "local_transaction_failed",
            Self::CursorInvalid => "cursor_invalid",
            Self::ConflictRetryExhausted => "conflict_retry_exhausted",
            Self::TransportTimeout => "transport_timeout",
            Self::ServerUnavailable => "server_unavailable",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOrchestrationError {
    pub code: SyncOrchestrationErrorCode,
    pub phase: SyncCyclePhase,
    pub retryable: bool,
}

impl SyncOrchestrationError {
    pub fn new(code: SyncOrchestrationErrorCode, phase: SyncCyclePhase, retryable: bool) -> Self {
        Self {
            code,
            phase,
            retryable,
        }
    }
}

impl fmt::Display for SyncOrchestrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sync orchestration failed at {}: {}",
            self.phase.as_str(),
            self.code.as_str()
        )
    }
}

impl std::error::Error for SyncOrchestrationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCycleSummary {
    pub outcome: SyncCycleOutcome,
    pub final_phase: SyncCyclePhase,
    pub discovered: usize,
    pub downloaded: usize,
    pub applied: usize,
    pub uploaded: usize,
    pub conflicts: usize,
    pub retries: usize,
    pub last_success_at_ms: Option<i64>,
    pub error: Option<SyncOrchestrationError>,
}

impl SyncCycleSummary {
    pub fn blocked(error: SyncOrchestrationError) -> Self {
        Self {
            outcome: SyncCycleOutcome::Blocked,
            final_phase: error.phase,
            discovered: 0,
            downloaded: 0,
            applied: 0,
            uploaded: 0,
            conflicts: 0,
            retries: 0,
            last_success_at_ms: None,
            error: Some(error),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DecryptedSyncObject {
    pub remote: RemoteObjectVersion,
    pub plaintext_payload: Vec<u8>,
}

impl fmt::Debug for DecryptedSyncObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecryptedSyncObject")
            .field("remote", &self.remote)
            .field(
                "plaintext_payload",
                &format_args!("[redacted; {} bytes]", self.plaintext_payload.len()),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncApplyPageSummary {
    pub discovered: usize,
    pub applied_user_terms: usize,
    pub applied_deleted_terms: usize,
    pub applied_ranker_weights: usize,
}

impl SyncApplyPageSummary {
    pub fn applied_records(&self) -> usize {
        self.applied_user_terms + self.applied_deleted_terms + self.applied_ranker_weights
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LocalSyncSnapshot {
    pub domain_id: String,
    pub object_id: String,
    pub object_type: SyncObjectType,
    pub local_revision: u64,
    pub remote_base_version: Option<u64>,
    pub payload: PlaintextSyncPayload,
}

impl fmt::Debug for LocalSyncSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSyncSnapshot")
            .field("domain_id", &self.domain_id)
            .field("object_id", &self.object_id)
            .field("object_type", &self.object_type)
            .field("local_revision", &self.local_revision)
            .field("remote_base_version", &self.remote_base_version)
            .field("payload", &self.payload)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedSyncOutbox {
    pub domain_id: String,
    pub local_revision: u64,
    pub object: AssembledSyncObject,
    pub manifest: SignedSyncObjectManifest,
    pub attempt_count: u32,
}

impl fmt::Debug for PreparedSyncOutbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSyncOutbox")
            .field("domain_id", &self.domain_id)
            .field("object_id", &self.object.draft.object_id)
            .field("version", &self.object.draft.version)
            .field("local_revision", &self.local_revision)
            .field(
                "encrypted_payload_len",
                &self.object.draft.encrypted_payload_len,
            )
            .field("attempt_count", &self.attempt_count)
            .finish()
    }
}

pub trait SyncLocalRepository {
    fn begin_cycle(
        &mut self,
        domain_id: &str,
        started_at_ms: i64,
        lease_expires_at_ms: i64,
    ) -> Result<(), SyncOrchestrationError>;

    fn record_cycle_phase(
        &mut self,
        domain_id: &str,
        phase: SyncCyclePhase,
    ) -> Result<(), SyncOrchestrationError>;

    fn cycle_cancel_requested(&self, domain_id: &str) -> Result<bool, SyncOrchestrationError>;

    fn finish_cycle(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError>;

    fn current_cursor(
        &self,
        domain_id: &str,
    ) -> Result<Option<OpaqueSyncCursor>, SyncOrchestrationError>;

    fn apply_download_page(
        &mut self,
        domain_id: &str,
        next_cursor: &OpaqueSyncCursor,
        objects: &[DecryptedSyncObject],
    ) -> Result<SyncApplyPageSummary, SyncOrchestrationError>;

    fn outbound_snapshots(
        &mut self,
        domain_id: &str,
    ) -> Result<Vec<LocalSyncSnapshot>, SyncOrchestrationError>;

    fn prepared_outboxes(
        &self,
        domain_id: &str,
    ) -> Result<Vec<PreparedSyncOutbox>, SyncOrchestrationError>;

    fn store_prepared_outbox(
        &mut self,
        outbox: &PreparedSyncOutbox,
    ) -> Result<(), SyncOrchestrationError>;

    fn record_outbox_attempt(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        error_code: Option<SyncOrchestrationErrorCode>,
    ) -> Result<(), SyncOrchestrationError>;

    fn supersede_outbox(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        ciphertext_hash: &str,
    ) -> Result<(), SyncOrchestrationError>;

    fn acknowledge_outbox(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        ciphertext_hash: &str,
        change_sequence: u64,
        acknowledged_at_ms: i64,
    ) -> Result<(), SyncOrchestrationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_and_errors_have_stable_redacted_codes() {
        let error = SyncOrchestrationError::new(
            SyncOrchestrationErrorCode::SignatureMismatch,
            SyncCyclePhase::Verify,
            false,
        );
        assert_eq!(
            error.to_string(),
            "sync orchestration failed at verify: signature_mismatch"
        );
        assert_eq!(
            SyncCyclePhase::PrepareSignedOutbox.as_str(),
            "prepare_signed_outbox"
        );
        assert_eq!(
            SyncOrchestrationErrorCode::KeyEpochRejected.as_str(),
            "key_epoch_rejected"
        );
        assert_eq!(SyncCycleOutcome::Blocked.as_str(), "blocked");
    }

    #[test]
    fn blocked_summary_contains_no_payload_or_provider_message() {
        let summary = SyncCycleSummary::blocked(SyncOrchestrationError::new(
            SyncOrchestrationErrorCode::BackendUnavailable,
            SyncCyclePhase::Preflight,
            false,
        ));
        let debug = format!("{summary:?}");
        assert!(debug.contains("BackendUnavailable"));
        assert!(!debug.contains("payload"));
        assert!(!debug.contains("private"));
    }
}
