use radishlex_ime_userdb::{LearningStatusSummary, UserDb};

use crate::error::FfiError;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexLearningStatusSummary {
    pub schema_version: i64,
    pub plaintext_payload: u8,
    pub p1_raw_details: u8,
    pub context_stats: u8,
    pub active_user_terms: usize,
    pub suppressed_user_terms: usize,
    pub ranker_weights: usize,
    pub deleted_term_tombstones: usize,
    pub selection_events: usize,
    pub negative_feedback: usize,
    pub import_batches: usize,
    pub latest_user_term_updated_at_ms: i64,
    pub latest_user_term_updated_at_present: u8,
    pub latest_selection_event_at_ms: i64,
    pub latest_selection_event_at_present: u8,
    pub latest_negative_feedback_at_ms: i64,
    pub latest_negative_feedback_at_present: u8,
    pub latest_deleted_term_at_ms: i64,
    pub latest_deleted_term_at_present: u8,
    pub latest_import_batch_at_ms: i64,
    pub latest_import_batch_at_present: u8,
    pub latest_activity_at_ms: i64,
    pub latest_activity_at_present: u8,
}

impl RadishLexLearningStatusSummary {
    pub const fn empty() -> Self {
        Self {
            schema_version: 0,
            plaintext_payload: 0,
            p1_raw_details: 0,
            context_stats: 0,
            active_user_terms: 0,
            suppressed_user_terms: 0,
            ranker_weights: 0,
            deleted_term_tombstones: 0,
            selection_events: 0,
            negative_feedback: 0,
            import_batches: 0,
            latest_user_term_updated_at_ms: 0,
            latest_user_term_updated_at_present: 0,
            latest_selection_event_at_ms: 0,
            latest_selection_event_at_present: 0,
            latest_negative_feedback_at_ms: 0,
            latest_negative_feedback_at_present: 0,
            latest_deleted_term_at_ms: 0,
            latest_deleted_term_at_present: 0,
            latest_import_batch_at_ms: 0,
            latest_import_batch_at_present: 0,
            latest_activity_at_ms: 0,
            latest_activity_at_present: 0,
        }
    }
}

impl From<LearningStatusSummary> for RadishLexLearningStatusSummary {
    fn from(summary: LearningStatusSummary) -> Self {
        let (latest_user_term_updated_at_ms, latest_user_term_updated_at_present) =
            ffi_optional_ms(summary.latest_user_term_updated_at_ms);
        let (latest_selection_event_at_ms, latest_selection_event_at_present) =
            ffi_optional_ms(summary.latest_selection_event_at_ms);
        let (latest_negative_feedback_at_ms, latest_negative_feedback_at_present) =
            ffi_optional_ms(summary.latest_negative_feedback_at_ms);
        let (latest_deleted_term_at_ms, latest_deleted_term_at_present) =
            ffi_optional_ms(summary.latest_deleted_term_at_ms);
        let (latest_import_batch_at_ms, latest_import_batch_at_present) =
            ffi_optional_ms(summary.latest_import_batch_at_ms);
        let (latest_activity_at_ms, latest_activity_at_present) =
            ffi_optional_ms(summary.latest_activity_at_ms);

        Self {
            schema_version: summary.schema_version,
            plaintext_payload: 0,
            p1_raw_details: 0,
            context_stats: 0,
            active_user_terms: summary.active_user_terms,
            suppressed_user_terms: summary.suppressed_user_terms,
            ranker_weights: summary.ranker_weights,
            deleted_term_tombstones: summary.deleted_term_tombstones,
            selection_events: summary.selection_events,
            negative_feedback: summary.negative_feedback,
            import_batches: summary.import_batches,
            latest_user_term_updated_at_ms,
            latest_user_term_updated_at_present,
            latest_selection_event_at_ms,
            latest_selection_event_at_present,
            latest_negative_feedback_at_ms,
            latest_negative_feedback_at_present,
            latest_deleted_term_at_ms,
            latest_deleted_term_at_present,
            latest_import_batch_at_ms,
            latest_import_batch_at_present,
            latest_activity_at_ms,
            latest_activity_at_present,
        }
    }
}

pub fn learning_status_for_path(db_path: &str) -> Result<RadishLexLearningStatusSummary, FfiError> {
    let db = UserDb::open(db_path)?;
    Ok(db.learning_status_summary()?.into())
}

fn ffi_optional_ms(value: Option<i64>) -> (i64, u8) {
    (value.unwrap_or_default(), u8::from(value.is_some()))
}
