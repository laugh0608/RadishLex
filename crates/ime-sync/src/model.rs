use std::fmt;

use radishlex_ime_crypto::{
    CryptoError, CryptoObjectType, EncryptedObjectEnvelope,
    ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256, ENVELOPE_SCHEMA_VERSION, XCHACHA20POLY1305_NONCE_LEN,
};

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

    pub fn from_crypto_object_type(object_type: CryptoObjectType) -> Self {
        match object_type {
            CryptoObjectType::DictionaryUserTerms => Self::DictionaryUserTerms,
            CryptoObjectType::DictionaryDeletedTerms => Self::DictionaryDeletedTerms,
            CryptoObjectType::RankerWeights => Self::RankerWeights,
            CryptoObjectType::SettingsProfile => Self::SettingsProfile,
            CryptoObjectType::SettingsSchema => Self::SettingsSchema,
            CryptoObjectType::BackupSnapshot => Self::BackupSnapshot,
        }
    }

    pub fn to_crypto_object_type(self) -> CryptoObjectType {
        match self {
            Self::DictionaryUserTerms => CryptoObjectType::DictionaryUserTerms,
            Self::DictionaryDeletedTerms => CryptoObjectType::DictionaryDeletedTerms,
            Self::RankerWeights => CryptoObjectType::RankerWeights,
            Self::SettingsProfile => CryptoObjectType::SettingsProfile,
            Self::SettingsSchema => CryptoObjectType::SettingsSchema,
            Self::BackupSnapshot => CryptoObjectType::BackupSnapshot,
        }
    }
}

impl fmt::Display for SyncObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<CryptoObjectType> for SyncObjectType {
    fn from(value: CryptoObjectType) -> Self {
        Self::from_crypto_object_type(value)
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
    pub schema_version: u16,
    pub object_id: String,
    pub object_type: SyncObjectType,
    pub owner_device_id: String,
    pub key_id: String,
    pub key_epoch: u64,
    pub algorithm: String,
    pub nonce: Vec<u8>,
    pub version: u64,
    pub base_version: Option<u64>,
    pub encrypted_payload_len: usize,
    pub ciphertext_hash: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl EncryptedSyncObjectDraft {
    pub fn from_crypto_envelope(
        envelope: &EncryptedObjectEnvelope,
    ) -> Result<Self, SyncPayloadError> {
        envelope
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;

        let object = Self {
            schema_version: envelope.schema_version,
            object_id: envelope.object_id.clone(),
            object_type: SyncObjectType::from_crypto_object_type(envelope.object_type),
            owner_device_id: envelope.owner_device_id.clone(),
            key_id: envelope.key_id.clone(),
            key_epoch: envelope.key_epoch,
            algorithm: envelope.algorithm.as_str().to_owned(),
            nonce: envelope.nonce.as_bytes().to_vec(),
            version: envelope.version,
            base_version: envelope.base_version,
            encrypted_payload_len: envelope.encrypted_payload.len(),
            ciphertext_hash: envelope.ciphertext_hash.as_str().to_owned(),
            created_at_ms: envelope.created_at_ms,
            updated_at_ms: envelope.updated_at_ms,
        };
        object.validate()?;
        Ok(object)
    }

    pub fn with_base_version(mut self, base_version: u64) -> Self {
        self.base_version = Some(base_version);
        self
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("object_id", &self.object_id)?;
        validate_required("owner_device_id", &self.owner_device_id)?;
        validate_required("key_id", &self.key_id)?;
        validate_required("algorithm", &self.algorithm)?;
        validate_required("ciphertext_hash", &self.ciphertext_hash)?;

        if self.schema_version != ENVELOPE_SCHEMA_VERSION {
            return Err(SyncPayloadError::InvalidField {
                field: "schema_version",
                message: format!("value must be {ENVELOPE_SCHEMA_VERSION}"),
            });
        }
        if self.key_epoch == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "key_epoch",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.algorithm != ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256 {
            return Err(SyncPayloadError::InvalidField {
                field: "algorithm",
                message: format!("value must be {ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256}"),
            });
        }
        if self.nonce.len() != XCHACHA20POLY1305_NONCE_LEN {
            return Err(SyncPayloadError::InvalidField {
                field: "nonce",
                message: format!("value must be {XCHACHA20POLY1305_NONCE_LEN}"),
            });
        }
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
    InvalidCryptoEnvelope {
        message: String,
    },
}

impl SyncPayloadError {
    pub(crate) fn from_crypto_error(error: CryptoError) -> Self {
        Self::InvalidCryptoEnvelope {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for SyncPayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, message } => write!(f, "invalid {field}: {message}"),
            Self::InvalidCryptoEnvelope { message } => {
                write!(f, "invalid crypto envelope: {message}")
            }
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
    use radishlex_ime_crypto::{
        KeyDescriptor, KeyRole, Nonce, ObjectKeyMaterial, PlaintextPayload,
    };

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
    fn sync_object_type_matches_crypto_object_type() {
        let pairs = [
            (
                SyncObjectType::DictionaryUserTerms,
                CryptoObjectType::DictionaryUserTerms,
            ),
            (
                SyncObjectType::DictionaryDeletedTerms,
                CryptoObjectType::DictionaryDeletedTerms,
            ),
            (
                SyncObjectType::RankerWeights,
                CryptoObjectType::RankerWeights,
            ),
            (
                SyncObjectType::SettingsProfile,
                CryptoObjectType::SettingsProfile,
            ),
            (
                SyncObjectType::SettingsSchema,
                CryptoObjectType::SettingsSchema,
            ),
            (
                SyncObjectType::BackupSnapshot,
                CryptoObjectType::BackupSnapshot,
            ),
        ];

        for (sync_type, crypto_type) in pairs {
            assert_eq!(
                SyncObjectType::from_crypto_object_type(crypto_type),
                sync_type
            );
            assert_eq!(sync_type.to_crypto_object_type(), crypto_type);
            assert_eq!(sync_type.as_str(), crypto_type.as_str());
        }
    }

    #[test]
    fn encrypted_sync_object_draft_copies_crypto_envelope_metadata() {
        let envelope = sample_crypto_envelope();
        let draft = EncryptedSyncObjectDraft::from_crypto_envelope(&envelope).expect("sync draft");

        assert_eq!(draft.schema_version, ENVELOPE_SCHEMA_VERSION);
        assert_eq!(draft.object_id, envelope.object_id);
        assert_eq!(
            draft.object_type,
            SyncObjectType::from_crypto_object_type(envelope.object_type)
        );
        assert_eq!(draft.owner_device_id, envelope.owner_device_id);
        assert_eq!(draft.key_id, envelope.key_id);
        assert_eq!(draft.key_epoch, envelope.key_epoch);
        assert_eq!(draft.algorithm, ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256);
        assert_eq!(draft.nonce.as_slice(), envelope.nonce.as_bytes());
        assert_eq!(draft.version, envelope.version);
        assert_eq!(draft.base_version, envelope.base_version);
        assert_eq!(
            draft.encrypted_payload_len,
            envelope.encrypted_payload.len()
        );
        assert_eq!(draft.ciphertext_hash, envelope.ciphertext_hash.as_str());
        assert_eq!(draft.created_at_ms, envelope.created_at_ms);
        assert_eq!(draft.updated_at_ms, envelope.updated_at_ms);
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn encrypted_sync_object_draft_rejects_invalid_versions() {
        let object = valid_draft().with_base_version(2);

        let error = object.validate().expect_err("base version must fail");
        assert!(error.to_string().contains("base_version"));
    }

    #[test]
    fn encrypted_sync_object_draft_requires_encrypted_payload_metadata() {
        let mut object = valid_draft();
        object.object_id.clear();

        let error = object.validate().expect_err("missing object id fails");
        assert!(error.to_string().contains("object_id"));

        let mut object = valid_draft();
        object.encrypted_payload_len = 0;

        let error = object.validate().expect_err("missing payload len fails");
        assert!(error.to_string().contains("encrypted_payload_len"));
    }

    #[test]
    fn encrypted_sync_object_draft_requires_ciphertext_hash() {
        let mut object = valid_draft();
        object.ciphertext_hash.clear();

        let error = object
            .validate()
            .expect_err("missing ciphertext hash fails");
        assert!(error.to_string().contains("ciphertext_hash"));
    }

    #[test]
    fn encrypted_sync_object_draft_requires_crypto_envelope_metadata() {
        let mut object = valid_draft();
        object.key_id.clear();

        let error = object.validate().expect_err("missing key id fails");
        assert!(error.to_string().contains("key_id"));

        let mut object = valid_draft();
        object.algorithm = "plaintext".to_owned();

        let error = object.validate().expect_err("unsupported algorithm fails");
        assert!(error.to_string().contains("algorithm"));

        let mut object = valid_draft();
        object.nonce.clear();

        let error = object.validate().expect_err("invalid nonce fails");
        assert!(error.to_string().contains("nonce"));
    }

    #[test]
    fn encrypted_sync_object_draft_rejects_invalid_crypto_envelope() {
        let mut envelope = sample_crypto_envelope();
        envelope.key_id.clear();

        let error = EncryptedSyncObjectDraft::from_crypto_envelope(&envelope)
            .expect_err("invalid crypto envelope fails");
        assert!(error.to_string().contains("crypto envelope"));
        assert!(error.to_string().contains("key_id"));
    }

    fn valid_draft() -> EncryptedSyncObjectDraft {
        EncryptedSyncObjectDraft::from_crypto_envelope(&sample_crypto_envelope()).expect("draft")
    }

    fn sample_crypto_envelope() -> EncryptedObjectEnvelope {
        let object_key = KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 3).expect("key");
        let object_key_material = ObjectKeyMaterial::new([7u8; 32]).expect("key material");
        let payload = PlaintextPayload::new(
            CryptoObjectType::DictionaryUserTerms,
            br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input":"luobo","text":"synthetic-term"}]}"#.to_vec(),
        )
        .expect("payload");

        EncryptedObjectEnvelope::encrypt_payload_with_nonce(
            "dictionary-user-terms-device-a",
            "device-a",
            &object_key,
            &object_key_material,
            2,
            Some(1),
            payload,
            10,
            Nonce::new(vec![9u8; XCHACHA20POLY1305_NONCE_LEN]).expect("nonce"),
        )
        .expect("envelope")
    }
}
