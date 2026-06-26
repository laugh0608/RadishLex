use std::fmt;

use crate::error::{UserDbError, UserDbResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLevel {
    P0NeverLearn,
    P1LocalOnly,
    P2EncryptedSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermSource {
    EngineSelection,
    ManualImport,
    ManualAdd,
    PhraseLearning,
}

impl TermSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EngineSelection => "engine_selection",
            Self::ManualImport => "manual_import",
            Self::ManualAdd => "manual_add",
            Self::PhraseLearning => "phrase_learning",
        }
    }

    pub fn from_str(value: &str) -> UserDbResult<Self> {
        match value {
            "engine_selection" => Ok(Self::EngineSelection),
            "manual_import" => Ok(Self::ManualImport),
            "manual_add" => Ok(Self::ManualAdd),
            "phrase_learning" => Ok(Self::PhraseLearning),
            _ => Err(UserDbError::invalid_input(
                "source",
                format!("unknown term source {value}"),
            )),
        }
    }
}

impl fmt::Display for TermSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermStatus {
    Active,
    Suppressed,
    Deleted,
}

impl TermStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suppressed => "suppressed",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_str(value: &str) -> UserDbResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "suppressed" => Ok(Self::Suppressed),
            "deleted" => Ok(Self::Deleted),
            _ => Err(UserDbError::invalid_input(
                "status",
                format!("unknown term status {value}"),
            )),
        }
    }
}

impl fmt::Display for TermStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegativeFeedbackReason {
    ImmediateBackspace,
    ReselectSameCode,
    ManualSuppress,
    ManualDelete,
}

impl NegativeFeedbackReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImmediateBackspace => "immediate_backspace",
            Self::ReselectSameCode => "reselect_same_code",
            Self::ManualSuppress => "manual_suppress",
            Self::ManualDelete => "manual_delete",
        }
    }

    pub fn from_str(value: &str) -> UserDbResult<Self> {
        match value {
            "immediate_backspace" => Ok(Self::ImmediateBackspace),
            "reselect_same_code" => Ok(Self::ReselectSameCode),
            "manual_suppress" => Ok(Self::ManualSuppress),
            "manual_delete" => Ok(Self::ManualDelete),
            _ => Err(UserDbError::invalid_input(
                "reason",
                format!("unknown negative feedback reason {value}"),
            )),
        }
    }
}

impl fmt::Display for NegativeFeedbackReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserTerm {
    pub id: i64,
    pub text: String,
    pub reading: Option<String>,
    pub input_code: String,
    pub source: TermSource,
    pub weight: f64,
    pub status: TermStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_used_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryTermRecord {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
    pub source: TermSource,
    pub weight: f64,
    pub status: TermStatus,
}

impl DictionaryTermRecord {
    pub fn new(
        input_code: impl Into<String>,
        text: impl Into<String>,
        reading: Option<impl Into<String>>,
        source: TermSource,
        weight: f64,
        status: TermStatus,
    ) -> Self {
        Self {
            input_code: input_code.into(),
            text: text.into(),
            reading: reading.map(Into::into),
            source,
            weight,
            status,
        }
    }
}

impl From<&UserTerm> for DictionaryTermRecord {
    fn from(term: &UserTerm) -> Self {
        Self {
            input_code: term.input_code.clone(),
            text: term.text.clone(),
            reading: term.reading.clone(),
            source: term.source,
            weight: term.weight,
            status: term.status,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryTermsFormat {
    V1,
}

impl DictionaryTermsFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "radishlex-user-terms-v1",
        }
    }

    pub fn version_line(self) -> &'static str {
        match self {
            Self::V1 => "# radishlex-user-terms-v1",
        }
    }

    pub fn supported_versions() -> &'static [&'static str] {
        &["radishlex-user-terms-v1"]
    }

    pub fn from_version_line(line: &str) -> UserDbResult<Self> {
        let Some(version) = line.strip_prefix("# ") else {
            return Err(UserDbError::invalid_input(
                "import_file",
                "missing dictionary format version line",
            ));
        };

        match version.trim() {
            "radishlex-user-terms-v1" => Ok(Self::V1),
            unsupported => Err(UserDbError::invalid_input(
                "import_file",
                format!(
                    "unsupported dictionary format {unsupported}; supported versions: {}",
                    Self::supported_versions().join(", ")
                ),
            )),
        }
    }
}

impl fmt::Display for DictionaryTermsFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DictionaryTermsDocument {
    pub format: DictionaryTermsFormat,
    pub records: Vec<DictionaryTermRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictionaryImportSummary {
    pub import_batch_id: Option<i64>,
    pub total_records: usize,
    pub imported_terms: usize,
    pub inserted_terms: usize,
    pub updated_terms: usize,
    pub skipped_deleted_terms: usize,
    pub skipped_duplicate_terms: usize,
}

impl DictionaryImportSummary {
    pub fn empty(total_records: usize) -> Self {
        Self {
            import_batch_id: None,
            total_records,
            imported_terms: 0,
            inserted_terms: 0,
            updated_terms: 0,
            skipped_deleted_terms: 0,
            skipped_duplicate_terms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryImportBatch {
    pub id: i64,
    pub source_name: String,
    pub total_records: usize,
    pub imported_terms: usize,
    pub inserted_terms: usize,
    pub updated_terms: usize,
    pub skipped_deleted_terms: usize,
    pub skipped_duplicate_terms: usize,
    pub created_at_ms: i64,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPreflightSummary {
    pub schema_version: i64,
    pub syncable_user_terms: usize,
    pub syncable_ranker_weights: usize,
    pub syncable_deleted_terms: usize,
    pub local_selection_events: usize,
    pub local_negative_feedback: usize,
    pub local_import_batches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionEventDraft {
    pub session_id: String,
    pub input_code: String,
    pub selected_text: String,
    pub selected_reading: Option<String>,
    pub candidate_index: usize,
    pub candidate_count: usize,
    pub context_kind: String,
    pub privacy: PrivacyLevel,
}

impl SelectionEventDraft {
    pub fn new(
        session_id: impl Into<String>,
        input_code: impl Into<String>,
        selected_text: impl Into<String>,
        candidate_index: usize,
        candidate_count: usize,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            input_code: input_code.into(),
            selected_text: selected_text.into(),
            selected_reading: None,
            candidate_index,
            candidate_count,
            context_kind: "general".to_owned(),
            privacy: PrivacyLevel::P1LocalOnly,
        }
    }

    pub fn with_reading(mut self, reading: impl Into<String>) -> Self {
        self.selected_reading = Some(reading.into());
        self
    }

    pub fn with_context_kind(mut self, context_kind: impl Into<String>) -> Self {
        self.context_kind = context_kind.into();
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacyLevel) -> Self {
        self.privacy = privacy;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeFeedbackDraft {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
    pub reason: NegativeFeedbackReason,
    pub context_kind: String,
    pub privacy: PrivacyLevel,
}

impl NegativeFeedbackDraft {
    pub fn new(
        input_code: impl Into<String>,
        text: impl Into<String>,
        reason: NegativeFeedbackReason,
    ) -> Self {
        Self {
            input_code: input_code.into(),
            text: text.into(),
            reading: None,
            reason,
            context_kind: "general".to_owned(),
            privacy: PrivacyLevel::P1LocalOnly,
        }
    }

    pub fn with_reading(mut self, reading: impl Into<String>) -> Self {
        self.reading = Some(reading.into());
        self
    }

    pub fn with_context_kind(mut self, context_kind: impl Into<String>) -> Self {
        self.context_kind = context_kind.into();
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacyLevel) -> Self {
        self.privacy = privacy;
        self
    }
}
