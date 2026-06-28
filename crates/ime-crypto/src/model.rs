use std::collections::BTreeSet;
use std::fmt;

use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

use crate::device::DeviceWrappingKeyMaterial;

pub const ENVELOPE_SCHEMA_VERSION: u16 = 1;
pub const OBJECT_KEY_LEN: usize = 32;
pub const XCHACHA20POLY1305_NONCE_LEN: usize = 24;
pub const ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256: &str = "xchacha20poly1305-hkdf-sha256-v1";

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

#[derive(Clone, PartialEq, Eq)]
pub struct SyncMasterKeyMaterial([u8; OBJECT_KEY_LEN]);

impl SyncMasterKeyMaterial {
    pub fn new(bytes: [u8; OBJECT_KEY_LEN]) -> Result<Self, CryptoError> {
        validate_key_material("sync_master_key", &bytes)?;
        Ok(Self(bytes))
    }

    pub fn derive_object_key(
        &self,
        object_key: &KeyDescriptor,
        object_type: CryptoObjectType,
        object_id: &str,
    ) -> Result<ObjectKeyMaterial, CryptoError> {
        if object_key.role != KeyRole::ObjectKey {
            return Err(CryptoError::invalid_field(
                "key_role",
                "object key derivation requires an object key descriptor",
            ));
        }
        validate_required("object_id", object_id)?;

        let hkdf = Hkdf::<Sha256>::new(Some(b"radishlex-sync-master-v1"), &self.0);
        let mut output = [0u8; OBJECT_KEY_LEN];
        hkdf.expand(
            &object_key_info(object_key, object_type, object_id),
            &mut output,
        )
        .map_err(|_| {
            CryptoError::invalid_field("key_derivation", "failed to expand object key material")
        })?;
        ObjectKeyMaterial::new(output)
    }

    pub fn derive_device_wrapping_key(
        &self,
        wrapping_key: &KeyDescriptor,
        device_id: &str,
    ) -> Result<DeviceWrappingKeyMaterial, CryptoError> {
        if wrapping_key.role != KeyRole::DeviceWrapping {
            return Err(CryptoError::invalid_field(
                "key_role",
                "device wrapping key derivation requires a device wrapping descriptor",
            ));
        }
        validate_required("device_id", device_id)?;

        let hkdf = Hkdf::<Sha256>::new(Some(b"radishlex-sync-master-v1"), &self.0);
        let mut output = [0u8; OBJECT_KEY_LEN];
        hkdf.expand(
            &device_wrapping_key_info(wrapping_key, device_id),
            &mut output,
        )
        .map_err(|_| {
            CryptoError::invalid_field(
                "key_derivation",
                "failed to expand device wrapping key material",
            )
        })?;
        DeviceWrappingKeyMaterial::new(output)
    }

    pub fn as_bytes(&self) -> &[u8; OBJECT_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for SyncMasterKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncMasterKeyMaterial([redacted])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ObjectKeyMaterial([u8; OBJECT_KEY_LEN]);

impl ObjectKeyMaterial {
    pub fn new(bytes: [u8; OBJECT_KEY_LEN]) -> Result<Self, CryptoError> {
        validate_key_material("object_key", &bytes)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; OBJECT_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for ObjectKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectKeyMaterial([redacted])")
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

    pub fn xchacha20poly1305_hkdf_sha256() -> Self {
        Self(ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256.to_owned())
    }

    pub fn is_supported(&self) -> bool {
        self.as_str() == ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256
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
        if bytes.len() != XCHACHA20POLY1305_NONCE_LEN {
            return Err(CryptoError::invalid_field(
                "nonce",
                format!("value must be {XCHACHA20POLY1305_NONCE_LEN} bytes"),
            ));
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
pub struct PlaintextPayload {
    pub object_type: CryptoObjectType,
    pub bytes: Vec<u8>,
}

impl PlaintextPayload {
    pub fn new(
        object_type: CryptoObjectType,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, CryptoError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(CryptoError::invalid_field(
                "plaintext_payload",
                "value cannot be empty",
            ));
        }
        Ok(Self { object_type, bytes })
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

    #[allow(clippy::too_many_arguments)]
    pub fn encrypt_payload(
        object_id: impl Into<String>,
        owner_device_id: impl Into<String>,
        object_key: &KeyDescriptor,
        object_key_material: &ObjectKeyMaterial,
        version: u64,
        base_version: Option<u64>,
        payload: PlaintextPayload,
        timestamp_ms: i64,
    ) -> Result<Self, CryptoError> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        Self::encrypt_payload_with_nonce(
            object_id,
            owner_device_id,
            object_key,
            object_key_material,
            version,
            base_version,
            payload,
            timestamp_ms,
            Nonce::new(nonce.to_vec())?,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encrypt_payload_with_nonce(
        object_id: impl Into<String>,
        owner_device_id: impl Into<String>,
        object_key: &KeyDescriptor,
        object_key_material: &ObjectKeyMaterial,
        version: u64,
        base_version: Option<u64>,
        payload: PlaintextPayload,
        timestamp_ms: i64,
        nonce: Nonce,
    ) -> Result<Self, CryptoError> {
        if object_key.role != KeyRole::ObjectKey {
            return Err(CryptoError::invalid_field(
                "key_role",
                "envelope encryption requires an object key",
            ));
        }

        let object_id = object_id.into();
        let owner_device_id = owner_device_id.into();
        validate_required("object_id", &object_id)?;
        validate_required("owner_device_id", &owner_device_id)?;

        let associated_data = AssociatedData {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            object_id: object_id.clone(),
            object_type: payload.object_type,
            owner_device_id: owner_device_id.clone(),
            key_id: object_key.key_id.clone(),
            key_epoch: object_key.key_epoch,
            version,
            base_version,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
        };
        let associated_data_bytes = associated_data.to_bytes();
        let encrypted_payload = encrypt_xchacha20poly1305(
            object_key_material,
            &nonce,
            &associated_data_bytes,
            &payload.bytes,
        )?;
        let ciphertext_hash = CiphertextHash::new(ciphertext_hash_hex(
            &associated_data_bytes,
            &encrypted_payload,
        ))?;

        let envelope = Self {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            object_id,
            object_type: payload.object_type,
            owner_device_id,
            key_id: object_key.key_id.clone(),
            key_epoch: object_key.key_epoch,
            algorithm: AlgorithmId::xchacha20poly1305_hkdf_sha256(),
            nonce,
            version,
            base_version,
            encrypted_payload,
            ciphertext_hash,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn decrypt_payload(
        &self,
        object_key_material: &ObjectKeyMaterial,
    ) -> Result<PlaintextPayload, CryptoError> {
        self.validate()?;
        ensure_supported_algorithm(&self.algorithm)?;

        let associated_data_bytes = self.associated_data().to_bytes();
        let expected_hash = CiphertextHash::new(ciphertext_hash_hex(
            &associated_data_bytes,
            &self.encrypted_payload,
        ))?;
        if expected_hash != self.ciphertext_hash {
            return Err(CryptoError::CiphertextHashMismatch);
        }

        let plaintext = decrypt_xchacha20poly1305(
            object_key_material,
            &self.nonce,
            &associated_data_bytes,
            &self.encrypted_payload,
        )?;
        PlaintextPayload::new(self.object_type, plaintext)
    }

    pub fn with_base_version(mut self, base_version: u64) -> Self {
        self.base_version = Some(base_version);
        self
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("object_id", &self.object_id)?;
        validate_required("owner_device_id", &self.owner_device_id)?;
        validate_required("key_id", &self.key_id)?;
        ensure_supported_algorithm(&self.algorithm)?;

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
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_aad_field(
            &mut bytes,
            "schema_version",
            self.schema_version.to_string().as_bytes(),
        );
        push_aad_field(&mut bytes, "object_id", self.object_id.as_bytes());
        push_aad_field(
            &mut bytes,
            "object_type",
            self.object_type.as_str().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "owner_device_id",
            self.owner_device_id.as_bytes(),
        );
        push_aad_field(&mut bytes, "key_id", self.key_id.as_bytes());
        push_aad_field(
            &mut bytes,
            "key_epoch",
            self.key_epoch.to_string().as_bytes(),
        );
        push_aad_field(&mut bytes, "version", self.version.to_string().as_bytes());
        push_aad_field(
            &mut bytes,
            "base_version",
            self.base_version
                .map(|value| value.to_string())
                .unwrap_or_default()
                .as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "created_at_ms",
            self.created_at_ms.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "updated_at_ms",
            self.updated_at_ms.to_string().as_bytes(),
        );
        bytes
    }

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
    CiphertextHashMismatch,
    KeyDerivationFailed,
    EncryptionFailed,
    DecryptionFailed,
}

impl CryptoError {
    pub(crate) fn invalid_field(field: &'static str, message: impl Into<String>) -> Self {
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
            Self::CiphertextHashMismatch => f.write_str("ciphertext hash mismatch"),
            Self::KeyDerivationFailed => f.write_str("key derivation failed"),
            Self::EncryptionFailed => f.write_str("encryption failed"),
            Self::DecryptionFailed => f.write_str("decryption failed"),
        }
    }
}

impl std::error::Error for CryptoError {}

pub(crate) fn validate_required(field: &'static str, value: &str) -> Result<(), CryptoError> {
    if value.trim().is_empty() {
        return Err(CryptoError::invalid_field(field, "value cannot be empty"));
    }
    Ok(())
}

pub(crate) fn validate_key_material(
    field: &'static str,
    bytes: &[u8; OBJECT_KEY_LEN],
) -> Result<(), CryptoError> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(CryptoError::invalid_field(
            field,
            "value cannot be all zeroes",
        ));
    }
    Ok(())
}

pub(crate) fn validate_non_empty_bytes(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), CryptoError> {
    if bytes.is_empty() {
        return Err(CryptoError::invalid_field(field, "value cannot be empty"));
    }
    Ok(())
}

fn ensure_supported_algorithm(algorithm: &AlgorithmId) -> Result<(), CryptoError> {
    if algorithm.is_supported() {
        Ok(())
    } else {
        Err(CryptoError::invalid_field(
            "algorithm",
            format!("unsupported algorithm {}", algorithm.as_str()),
        ))
    }
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

pub(crate) fn encrypt_xchacha20poly1305_raw(
    key: &[u8; OBJECT_KEY_LEN],
    nonce: &Nonce,
    associated_data: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::EncryptionFailed)?;
    cipher
        .encrypt(
            XNonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::EncryptionFailed)
}

pub(crate) fn decrypt_xchacha20poly1305_raw(
    key: &[u8; OBJECT_KEY_LEN],
    nonce: &Nonce,
    associated_data: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::DecryptionFailed)?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: ciphertext,
                aad: associated_data,
            },
        )
        .map_err(|_| CryptoError::DecryptionFailed)
}

fn encrypt_xchacha20poly1305(
    object_key_material: &ObjectKeyMaterial,
    nonce: &Nonce,
    associated_data: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    encrypt_xchacha20poly1305_raw(
        object_key_material.as_bytes(),
        nonce,
        associated_data,
        plaintext,
    )
}

fn decrypt_xchacha20poly1305(
    object_key_material: &ObjectKeyMaterial,
    nonce: &Nonce,
    associated_data: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    decrypt_xchacha20poly1305_raw(
        object_key_material.as_bytes(),
        nonce,
        associated_data,
        ciphertext,
    )
}

fn ciphertext_hash_hex(associated_data: &[u8], ciphertext: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"radishlex-ciphertext-hash-v1");
    hasher.update((associated_data.len() as u64).to_be_bytes());
    hasher.update(associated_data);
    hasher.update((ciphertext.len() as u64).to_be_bytes());
    hasher.update(ciphertext);
    hex_lower(&hasher.finalize())
}

fn object_key_info(
    object_key: &KeyDescriptor,
    object_type: CryptoObjectType,
    object_id: &str,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_aad_field(&mut bytes, "purpose", b"radishlex-object-key-v1");
    push_aad_field(&mut bytes, "key_id", object_key.key_id.as_bytes());
    push_aad_field(
        &mut bytes,
        "key_epoch",
        object_key.key_epoch.to_string().as_bytes(),
    );
    push_aad_field(&mut bytes, "object_type", object_type.as_str().as_bytes());
    push_aad_field(&mut bytes, "object_id", object_id.as_bytes());
    bytes
}

fn device_wrapping_key_info(wrapping_key: &KeyDescriptor, device_id: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_aad_field(&mut bytes, "purpose", b"radishlex-device-wrapping-key-v1");
    push_aad_field(&mut bytes, "key_id", wrapping_key.key_id.as_bytes());
    push_aad_field(
        &mut bytes,
        "key_epoch",
        wrapping_key.key_epoch.to_string().as_bytes(),
    );
    push_aad_field(&mut bytes, "device_id", device_id.as_bytes());
    bytes
}

pub(crate) fn push_aad_field(output: &mut Vec<u8>, name: &str, value: &[u8]) {
    output.extend_from_slice(name.as_bytes());
    output.push(b'=');
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
    output.push(0);
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests;
