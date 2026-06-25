use radishlex_ime_userdb::{SyncPreflightSummary, UserDb};

use crate::error::FfiError;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexSyncPreflightSummary {
    pub schema_version: i64,
    pub plaintext_payload: u8,
    pub syncable_user_terms: usize,
    pub syncable_ranker_weights: usize,
    pub syncable_deleted_terms: usize,
    pub local_selection_events: usize,
    pub local_negative_feedback: usize,
    pub local_import_batches: usize,
}

impl RadishLexSyncPreflightSummary {
    pub const fn empty() -> Self {
        Self {
            schema_version: 0,
            plaintext_payload: 0,
            syncable_user_terms: 0,
            syncable_ranker_weights: 0,
            syncable_deleted_terms: 0,
            local_selection_events: 0,
            local_negative_feedback: 0,
            local_import_batches: 0,
        }
    }
}

impl From<SyncPreflightSummary> for RadishLexSyncPreflightSummary {
    fn from(summary: SyncPreflightSummary) -> Self {
        Self {
            schema_version: summary.schema_version,
            plaintext_payload: 0,
            syncable_user_terms: summary.syncable_user_terms,
            syncable_ranker_weights: summary.syncable_ranker_weights,
            syncable_deleted_terms: summary.syncable_deleted_terms,
            local_selection_events: summary.local_selection_events,
            local_negative_feedback: summary.local_negative_feedback,
            local_import_batches: summary.local_import_batches,
        }
    }
}

pub fn sync_preflight_for_path(db_path: &str) -> Result<RadishLexSyncPreflightSummary, FfiError> {
    let db = UserDb::open(db_path)?;
    Ok(db.sync_preflight_summary()?.into())
}
