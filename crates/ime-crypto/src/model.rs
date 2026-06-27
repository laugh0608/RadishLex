use std::collections::BTreeSet;
use std::fmt;

pub const ENVELOPE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoObjectType {
    DictionaryUserTerms,
    DictionaryDeletedTerms,
    RankerWeights,
    SettingsProfile,
    SettingsSchema,
    BackupSnapshot,
}

impl CryptoObjectType {
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

impl fmt::Display for CryptoObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    ProfileRoot,
    SyncMaster,
    DeviceKeyPair,
    DeviceWrapping,
    ObjectKey,
}

impl KeyRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileRoot => "profile_root",
            Self::SyncMaster => "sync_master",
            Self::DeviceKeyPair => "device_key_pair",
            Self::DeviceWrapping => "device_wrapping",
            Self::ObjectKey => "object_key",
        }
    }
}

impl fmt::Display for KeyRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDescriptor {
    pub key_id: String,
    pub role: KeyRole,
    pub key_epoch: u64,
}

impl KeyDescriptor {
    pub fn new(
        key_id: impl Into<String>,
        role: KeyRole,
        key_epoch: u64,
    ) -> Result<Self, CryptoError> {
        let descriptor = Self {
            key_id: key_id.into(),
            role,
            key_epoch,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("key_id", &self.key_id)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlgorithmId(String);

impl AlgorithmId {
    pub fn new(value: impl Into<String>) -> Result<Self, CryptoError> {
        let value = value.into();
        validate_required("algorithm", &value)?;

        let normalized = value.trim().to_ascii_lowercase();
        if normalized == "none" || normalized == "plaintext" {
            return Err(CryptoError::invalid_field(
                "algorithm",
                "value must name an authenticated encryption algorithm",
            ));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AlgorithmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nonce(Vec<u8>);

impl Nonce {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Result<Self, CryptoError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(CryptoError::invalid_field("nonce", "value cannot be empty"));
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiphertextHash(String);

impl CiphertextHash {
    pub fn new(value: impl Into<String>) -> Result<Self, CryptoError> {
        let value = value.into();
        validate_required("ciphertext_hash", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CiphertextHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedObjectEnvelope {
    pub schema_version: u16,
    pub object_id: String,
    pub object_type: CryptoObjectType,
    pub owner_device_id: String,
    pub key_id: String,
    pub key_epoch: u64,
    pub algorithm: AlgorithmId,
    pub nonce: Nonce,
    pub version: u64,
    pub base_version: Option<u64>,
    pub encrypted_payload: Vec<u8>,
    pub ciphertext_hash: CiphertextHash,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl EncryptedObjectEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        object_id: impl Into<String>,
        object_type: CryptoObjectType,
        owner_device_id: impl Into<String>,
        object_key: &KeyDescriptor,
        algorithm: AlgorithmId,
        nonce: Nonce,
        version: u64,
        encrypted_payload: impl Into<Vec<u8>>,
        ciphertext_hash: CiphertextHash,
        timestamp_ms: i64,
    ) -> Result<Self, CryptoError> {
        if object_key.role != KeyRole::ObjectKey {
            return Err(CryptoError::invalid_field(
                "key_role",
                "envelope encryption requires an object key",
            ));
        }

        let envelope = Self {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            object_id: object_id.into(),
            object_type,
            owner_device_id: owner_device_id.into(),
            key_id: object_key.key_id.clone(),
            key_epoch: object_key.key_epoch,
            algorithm,
            nonce,
            version,
            base_version: None,
            encrypted_payload: encrypted_payload.into(),
            ciphertext_hash,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn with_base_version(mut self, base_version: u64) -> Self {
        self.base_version = Some(base_version);
        self
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("object_id", &self.object_id)?;
        validate_required("owner_device_id", &self.owner_device_id)?;
        validate_required("key_id", &self.key_id)?;

        if self.schema_version != ENVELOPE_SCHEMA_VERSION {
            return Err(CryptoError::invalid_field(
                "schema_version",
                format!("value must be {ENVELOPE_SCHEMA_VERSION}"),
            ));
        }
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        if self.version == 0 {
            return Err(CryptoError::invalid_field(
                "version",
                "value must be greater than 0",
            ));
        }
        if let Some(base_version) = self.base_version {
            if base_version >= self.version {
                return Err(CryptoError::invalid_field(
                    "base_version",
                    "value must be lower than version",
                ));
            }
        }
        if self.encrypted_payload.is_empty() {
            return Err(CryptoError::invalid_field(
                "encrypted_payload",
                "value cannot be empty",
            ));
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(CryptoError::invalid_field(
                "updated_at_ms",
                "value must be greater than or equal to created_at_ms",
            ));
        }

        Ok(())
    }

    pub fn associated_data(&self) -> AssociatedData {
        AssociatedData {
            schema_version: self.schema_version,
            object_id: self.object_id.clone(),
            object_type: self.object_type,
            owner_device_id: self.owner_device_id.clone(),
            key_id: self.key_id.clone(),
            key_epoch: self.key_epoch,
            version: self.version,
            base_version: self.base_version,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        }
    }

    pub fn validate_associated_data(&self, data: &AssociatedData) -> Result<(), CryptoError> {
        let expected = self.associated_data();
        expected.matches(data)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedData {
    pub schema_version: u16,
    pub object_id: String,
    pub object_type: CryptoObjectType,
    pub owner_device_id: String,
    pub key_id: String,
    pub key_epoch: u64,
    pub version: u64,
    pub base_version: Option<u64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl AssociatedData {
    fn matches(&self, actual: &Self) -> Result<(), CryptoError> {
        compare_aad_field("schema_version", self.schema_version, actual.schema_version)?;
        compare_aad_field("object_id", &self.object_id, &actual.object_id)?;
        compare_aad_field("object_type", self.object_type, actual.object_type)?;
        compare_aad_field(
            "owner_device_id",
            &self.owner_device_id,
            &actual.owner_device_id,
        )?;
        compare_aad_field("key_id", &self.key_id, &actual.key_id)?;
        compare_aad_field("key_epoch", self.key_epoch, actual.key_epoch)?;
        compare_aad_field("version", self.version, actual.version)?;
        compare_aad_field("base_version", self.base_version, actual.base_version)?;
        compare_aad_field("created_at_ms", self.created_at_ms, actual.created_at_ms)?;
        compare_aad_field("updated_at_ms", self.updated_at_ms, actual.updated_at_ms)?;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NonceTracker {
    seen: BTreeSet<NonceUse>,
}

impl NonceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, envelope: &EncryptedObjectEnvelope) -> Result<(), CryptoError> {
        let nonce_use = NonceUse {
            key_id: envelope.key_id.clone(),
            key_epoch: envelope.key_epoch,
            nonce: envelope.nonce.clone(),
        };

        if !self.seen.insert(nonce_use) {
            return Err(CryptoError::DuplicateNonce {
                key_id: envelope.key_id.clone(),
                key_epoch: envelope.key_epoch,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NonceUse {
    key_id: String,
    key_epoch: u64,
    nonce: Nonce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    InvalidField {
        field: &'static str,
        message: String,
    },
    AssociatedDataMismatch {
        field: &'static str,
    },
    DuplicateNonce {
        key_id: String,
        key_epoch: u64,
    },
}

impl CryptoError {
    fn invalid_field(field: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidField {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, message } => write!(f, "invalid {field}: {message}"),
            Self::AssociatedDataMismatch { field } => {
                write!(f, "associated data mismatch: {field}")
            }
            Self::DuplicateNonce { key_id, key_epoch } => {
                write!(f, "duplicate nonce for key {key_id} at epoch {key_epoch}")
            }
        }
    }
}

impl std::error::Error for CryptoError {}

fn validate_required(field: &'static str, value: &str) -> Result<(), CryptoError> {
    if value.trim().is_empty() {
        return Err(CryptoError::invalid_field(field, "value cannot be empty"));
    }
    Ok(())
}

fn compare_aad_field<T: PartialEq>(
    field: &'static str,
    expected: T,
    actual: T,
) -> Result<(), CryptoError> {
    if expected == actual {
        Ok(())
    } else {
        Err(CryptoError::AssociatedDataMismatch { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_roles_have_stable_identifiers() {
        assert_eq!(KeyRole::ProfileRoot.as_str(), "profile_root");
        assert_eq!(KeyRole::SyncMaster.as_str(), "sync_master");
        assert_eq!(KeyRole::DeviceKeyPair.as_str(), "device_key_pair");
        assert_eq!(KeyRole::DeviceWrapping.as_str(), "device_wrapping");
        assert_eq!(KeyRole::ObjectKey.as_str(), "object_key");
    }

    #[test]
    fn envelope_validates_required_metadata_and_aad() {
        let envelope = sample_envelope();

        envelope.validate().expect("envelope validates");
        let aad = envelope.associated_data();

        assert_eq!(aad.schema_version, ENVELOPE_SCHEMA_VERSION);
        assert_eq!(aad.object_id, "dictionary-user-terms-device-a");
        assert_eq!(aad.object_type, CryptoObjectType::DictionaryUserTerms);
        assert_eq!(aad.key_id, "object-key-a");
        assert_eq!(aad.key_epoch, 3);
        envelope
            .validate_associated_data(&aad)
            .expect("matching AAD is accepted");
    }

    #[test]
    fn envelope_rejects_non_object_key_roles() {
        let sync_master = KeyDescriptor::new("sync-master-a", KeyRole::SyncMaster, 1).expect("key");

        let error = EncryptedObjectEnvelope::new(
            "dictionary-user-terms-device-a",
            CryptoObjectType::DictionaryUserTerms,
            "device-a",
            &sync_master,
            algorithm(),
            nonce(1),
            1,
            b"ciphertext".to_vec(),
            ciphertext_hash("ciphertext-hash"),
            10,
        )
        .expect_err("sync master key must not encrypt object payload directly");

        assert!(error.to_string().contains("key_role"));
    }

    #[test]
    fn envelope_rejects_plaintext_algorithm_and_empty_nonce() {
        let algorithm = AlgorithmId::new("plaintext").expect_err("plaintext is not AEAD");
        assert!(algorithm.to_string().contains("algorithm"));

        let nonce = Nonce::new(Vec::<u8>::new()).expect_err("empty nonce fails");
        assert!(nonce.to_string().contains("nonce"));
    }

    #[test]
    fn envelope_rejects_invalid_versions_and_empty_ciphertext() {
        let object_key = object_key(3);
        let error = EncryptedObjectEnvelope::new(
            "dictionary-user-terms-device-a",
            CryptoObjectType::DictionaryUserTerms,
            "device-a",
            &object_key,
            algorithm(),
            nonce(1),
            0,
            b"ciphertext".to_vec(),
            ciphertext_hash("ciphertext-hash"),
            10,
        )
        .expect_err("zero version fails");
        assert!(error.to_string().contains("version"));

        let error = EncryptedObjectEnvelope::new(
            "dictionary-user-terms-device-a",
            CryptoObjectType::DictionaryUserTerms,
            "device-a",
            &object_key,
            algorithm(),
            nonce(1),
            1,
            Vec::<u8>::new(),
            ciphertext_hash("ciphertext-hash"),
            10,
        )
        .expect_err("empty ciphertext fails");
        assert!(error.to_string().contains("encrypted_payload"));
    }

    #[test]
    fn envelope_rejects_invalid_base_version_after_update() {
        let envelope = sample_envelope().with_base_version(1);
        envelope.validate().expect("lower base version is valid");

        let envelope = sample_envelope().with_base_version(2);
        let error = envelope.validate().expect_err("base version must be lower");
        assert!(error.to_string().contains("base_version"));
    }

    #[test]
    fn aad_binding_detects_metadata_changes() {
        let envelope = sample_envelope();
        let mut aad = envelope.associated_data();
        aad.version += 1;

        let error = envelope
            .validate_associated_data(&aad)
            .expect_err("changed version must fail AAD check");
        assert_eq!(
            error,
            CryptoError::AssociatedDataMismatch { field: "version" }
        );
    }

    #[test]
    fn ciphertext_hash_is_a_required_semantic_type() {
        let error = CiphertextHash::new("").expect_err("empty hash fails");
        assert!(error.to_string().contains("ciphertext_hash"));

        let hash = CiphertextHash::new("ciphertext-hash").expect("hash");
        assert_eq!(hash.as_str(), "ciphertext-hash");
    }

    #[test]
    fn nonce_tracker_rejects_duplicate_nonce_for_same_key_epoch() {
        let mut tracker = NonceTracker::new();
        let envelope = sample_envelope();
        let duplicate = sample_envelope();

        tracker.observe(&envelope).expect("first nonce use passes");
        let error = tracker
            .observe(&duplicate)
            .expect_err("same key and nonce must fail");

        assert_eq!(
            error,
            CryptoError::DuplicateNonce {
                key_id: "object-key-a".to_owned(),
                key_epoch: 3,
            }
        );
    }

    #[test]
    fn nonce_tracker_allows_same_nonce_after_key_epoch_changes() {
        let mut tracker = NonceTracker::new();
        let current_epoch = sample_envelope();
        let next_epoch = envelope_with_key_epoch(4, nonce(1));

        tracker
            .observe(&current_epoch)
            .expect("first nonce use passes");
        tracker
            .observe(&next_epoch)
            .expect("same nonce with different key epoch passes");
    }

    fn sample_envelope() -> EncryptedObjectEnvelope {
        envelope_with_key_epoch(3, nonce(1))
    }

    fn envelope_with_key_epoch(key_epoch: u64, nonce: Nonce) -> EncryptedObjectEnvelope {
        let object_key = object_key(key_epoch);
        EncryptedObjectEnvelope::new(
            "dictionary-user-terms-device-a",
            CryptoObjectType::DictionaryUserTerms,
            "device-a",
            &object_key,
            algorithm(),
            nonce,
            2,
            b"ciphertext".to_vec(),
            ciphertext_hash("ciphertext-hash"),
            10,
        )
        .expect("valid envelope")
        .with_base_version(1)
    }

    fn object_key(key_epoch: u64) -> KeyDescriptor {
        KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, key_epoch).expect("key")
    }

    fn algorithm() -> AlgorithmId {
        AlgorithmId::new("radishlex.local-test-aead-v1").expect("algorithm")
    }

    fn nonce(seed: u8) -> Nonce {
        Nonce::new(vec![seed; 24]).expect("nonce")
    }

    fn ciphertext_hash(value: &str) -> CiphertextHash {
        CiphertextHash::new(value).expect("ciphertext hash")
    }
}
