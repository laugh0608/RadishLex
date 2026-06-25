use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDataClass {
    P1LocalOnly,
    P2EncryptedSync,
    LocalAuditOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncObjectType {
    DictionaryUserTerms,
    DictionaryDeletedTerms,
    RankerWeights,
    SettingsProfile,
    SettingsSchema,
    BackupSnapshot,
}

impl SyncObjectType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DictionaryUserTerms => "dictionary.user_terms",
            Self::DictionaryDeletedTerms => "dictionary.deleted_terms",
            Self::RankerWeights => "ranker.weights",
            Self::SettingsProfile => "settings.profile",
            Self::SettingsSchema => "settings.schema",
            Self::BackupSnapshot => "backup.snapshot",
        }
    }
}

impl fmt::Display for SyncObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadSource {
    UserTerms,
    DeletedTerms,
    RankerWeights,
    SelectionEvents,
    NegativeFeedback,
    ImportBatches,
}

impl PayloadSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserTerms => "user_terms",
            Self::DeletedTerms => "deleted_terms",
            Self::RankerWeights => "ranker_weights",
            Self::SelectionEvents => "selection_events",
            Self::NegativeFeedback => "negative_feedback",
            Self::ImportBatches => "import_batches",
        }
    }

    pub fn data_class(self) -> LocalDataClass {
        match self {
            Self::UserTerms | Self::DeletedTerms | Self::RankerWeights => {
                LocalDataClass::P2EncryptedSync
            }
            Self::SelectionEvents | Self::NegativeFeedback => LocalDataClass::P1LocalOnly,
            Self::ImportBatches => LocalDataClass::LocalAuditOnly,
        }
    }

    pub fn sync_object_type(self) -> Option<SyncObjectType> {
        match self {
            Self::UserTerms => Some(SyncObjectType::DictionaryUserTerms),
            Self::DeletedTerms => Some(SyncObjectType::DictionaryDeletedTerms),
            Self::RankerWeights => Some(SyncObjectType::RankerWeights),
            Self::SelectionEvents | Self::NegativeFeedback | Self::ImportBatches => None,
        }
    }
}

impl fmt::Display for PayloadSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlanItem {
    pub source: PayloadSource,
    pub data_class: LocalDataClass,
    pub object_type: Option<SyncObjectType>,
    pub record_count: usize,
}

impl SyncPlanItem {
    pub fn new(source: PayloadSource, record_count: usize) -> Self {
        Self {
            source,
            data_class: source.data_class(),
            object_type: source.sync_object_type(),
            record_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPayloadPlan {
    pub items: Vec<SyncPlanItem>,
}

impl SyncPayloadPlan {
    pub fn new(items: Vec<SyncPlanItem>) -> Self {
        Self { items }
    }

    pub fn syncable_items(&self) -> impl Iterator<Item = &SyncPlanItem> {
        self.items
            .iter()
            .filter(|item| item.data_class == LocalDataClass::P2EncryptedSync)
    }

    pub fn local_only_items(&self) -> impl Iterator<Item = &SyncPlanItem> {
        self.items
            .iter()
            .filter(|item| item.data_class != LocalDataClass::P2EncryptedSync)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedSyncObjectDraft {
    pub object_id: String,
    pub object_type: SyncObjectType,
    pub owner_device_id: String,
    pub version: u64,
    pub base_version: Option<u64>,
    pub encrypted_payload_len: usize,
    pub payload_hash: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl EncryptedSyncObjectDraft {
    pub fn new(
        object_id: impl Into<String>,
        object_type: SyncObjectType,
        owner_device_id: impl Into<String>,
        version: u64,
        encrypted_payload_len: usize,
        payload_hash: impl Into<String>,
        timestamp_ms: i64,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            object_type,
            owner_device_id: owner_device_id.into(),
            version,
            base_version: None,
            encrypted_payload_len,
            payload_hash: payload_hash.into(),
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
        }
    }

    pub fn with_base_version(mut self, base_version: u64) -> Self {
        self.base_version = Some(base_version);
        self
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("object_id", &self.object_id)?;
        validate_required("owner_device_id", &self.owner_device_id)?;
        validate_required("payload_hash", &self.payload_hash)?;

        if self.version == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "version",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if let Some(base_version) = self.base_version {
            if base_version >= self.version {
                return Err(SyncPayloadError::InvalidField {
                    field: "base_version",
                    message: "value must be lower than version".to_owned(),
                });
            }
        }
        if self.encrypted_payload_len == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "encrypted_payload_len",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(SyncPayloadError::InvalidField {
                field: "updated_at_ms",
                message: "value must be greater than or equal to created_at_ms".to_owned(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncPayloadError {
    InvalidField {
        field: &'static str,
        message: String,
    },
}

impl fmt::Display for SyncPayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, message } => write!(f, "invalid {field}: {message}"),
        }
    }
}

impl std::error::Error for SyncPayloadError {}

fn validate_required(field: &'static str, value: &str) -> Result<(), SyncPayloadError> {
    if value.trim().is_empty() {
        return Err(SyncPayloadError::InvalidField {
            field,
            message: "value cannot be empty".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_sources_classify_p2_and_local_only_data() {
        assert_eq!(
            PayloadSource::UserTerms.sync_object_type(),
            Some(SyncObjectType::DictionaryUserTerms)
        );
        assert_eq!(
            PayloadSource::RankerWeights.sync_object_type(),
            Some(SyncObjectType::RankerWeights)
        );
        assert_eq!(PayloadSource::SelectionEvents.sync_object_type(), None);
        assert_eq!(PayloadSource::NegativeFeedback.sync_object_type(), None);
        assert_eq!(
            PayloadSource::SelectionEvents.data_class(),
            LocalDataClass::P1LocalOnly
        );
        assert_eq!(
            PayloadSource::ImportBatches.data_class(),
            LocalDataClass::LocalAuditOnly
        );
    }

    #[test]
    fn sync_payload_plan_separates_syncable_and_local_items() {
        let plan = SyncPayloadPlan::new(vec![
            SyncPlanItem::new(PayloadSource::UserTerms, 2),
            SyncPlanItem::new(PayloadSource::SelectionEvents, 5),
            SyncPlanItem::new(PayloadSource::ImportBatches, 1),
        ]);

        let syncable: Vec<_> = plan.syncable_items().map(|item| item.source).collect();
        let local_only: Vec<_> = plan.local_only_items().map(|item| item.source).collect();

        assert_eq!(syncable, vec![PayloadSource::UserTerms]);
        assert_eq!(
            local_only,
            vec![PayloadSource::SelectionEvents, PayloadSource::ImportBatches]
        );
    }

    #[test]
    fn encrypted_sync_object_draft_validates_metadata() {
        let object = EncryptedSyncObjectDraft::new(
            "dictionary-user-terms-device-a",
            SyncObjectType::DictionaryUserTerms,
            "device-a",
            2,
            128,
            "hash",
            10,
        )
        .with_base_version(1);

        assert!(object.validate().is_ok());
    }

    #[test]
    fn encrypted_sync_object_draft_rejects_invalid_versions() {
        let object = EncryptedSyncObjectDraft::new(
            "dictionary-user-terms-device-a",
            SyncObjectType::DictionaryUserTerms,
            "device-a",
            2,
            128,
            "hash",
            10,
        )
        .with_base_version(2);

        let error = object.validate().expect_err("base version must fail");
        assert!(error.to_string().contains("base_version"));
    }

    #[test]
    fn encrypted_sync_object_draft_requires_encrypted_payload_metadata() {
        let object = EncryptedSyncObjectDraft::new(
            "",
            SyncObjectType::DictionaryUserTerms,
            "device-a",
            1,
            0,
            "",
            10,
        );

        let error = object.validate().expect_err("missing object id fails");
        assert!(error.to_string().contains("object_id"));
    }
}
