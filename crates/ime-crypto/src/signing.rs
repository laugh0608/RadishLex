use std::collections::BTreeMap;
use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::device::RecoveryMaterial;
use crate::model::{
    push_aad_field, validate_non_empty_bytes, validate_required, CryptoError,
    EncryptedObjectEnvelope,
};

#[cfg(feature = "apple-keychain")]
mod apple_keychain;

#[cfg(feature = "android-keystore")]
mod android_keystore;

#[cfg(feature = "android-keystore")]
pub use android_keystore::{
    android_keystore_alias, validate_android_keystore_public_key,
    validate_android_keystore_signature, AndroidKeystoreBridgeErrorCode,
    AndroidKeystoreBridgeOperation, AndroidKeystoreBridgeRequest, AndroidKeystoreDeviceKeyStore,
    ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION, ANDROID_KEYSTORE_JNI_BRIDGE_CLASS,
    ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR,
    ANDROID_KEYSTORE_JNI_CREATE_SIGNING_KEY_METHOD, ANDROID_KEYSTORE_JNI_DELETE_SIGNING_KEY_METHOD,
    ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD,
    ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD, ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD,
    ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_JNI_LOAD_PUBLIC_KEY_METHOD,
    ANDROID_KEYSTORE_JNI_RESULT_CLASS, ANDROID_KEYSTORE_JNI_SIGN_METHOD,
    ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_PROVIDER,
    ANDROID_KEYSTORE_SIGNATURE_ALGORITHM, DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX,
};

#[cfg(feature = "apple-keychain")]
pub use apple_keychain::AppleKeychainDeviceKeyStore;

pub const SIGNATURE_SCHEMA_VERSION: u16 = 1;
pub const SIGNATURE_ALGORITHM_ED25519_V1: &str = "ed25519-v1";
pub const DEVICE_KEY_STORE_TEST_MEMORY_V1: &str = "test-memory-v1";
pub const DEVICE_KEY_STORE_UNAVAILABLE: &str = "unavailable";
pub const DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1: &str = "apple-keychain-v1";
pub const DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1: &str = "android-keystore-v1";
pub const DEVICE_KEY_STORE_WINDOWS_CNG_V1: &str = "windows-cng-v1";
pub const DEVICE_KEY_STORE_LINUX_SECRET_SERVICE_V1: &str = "linux-secret-service-v1";
pub const ED25519_PUBLIC_KEY_LEN: usize = 32;
pub const ED25519_SIGNATURE_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureAlgorithmId(String);

impl SignatureAlgorithmId {
    pub fn new(value: impl Into<String>) -> Result<Self, CryptoError> {
        let value = value.into();
        validate_required("signature_algorithm", &value)?;
        if value != SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(CryptoError::invalid_field(
                "signature_algorithm",
                format!("unsupported signature algorithm {value}"),
            ));
        }
        Ok(Self(value))
    }

    pub fn ed25519_v1() -> Self {
        Self(SIGNATURE_ALGORITHM_ED25519_V1.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SignatureAlgorithmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSigningStorageBackend {
    TestMemoryV1,
    Unavailable,
    AppleKeychainV1,
    AndroidKeystoreV1,
    WindowsCngV1,
    LinuxSecretServiceV1,
}

impl DeviceSigningStorageBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestMemoryV1 => DEVICE_KEY_STORE_TEST_MEMORY_V1,
            Self::Unavailable => DEVICE_KEY_STORE_UNAVAILABLE,
            Self::AppleKeychainV1 => DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1,
            Self::AndroidKeystoreV1 => DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
            Self::WindowsCngV1 => DEVICE_KEY_STORE_WINDOWS_CNG_V1,
            Self::LinuxSecretServiceV1 => DEVICE_KEY_STORE_LINUX_SECRET_SERVICE_V1,
        }
    }

    pub fn is_test_only(self) -> bool {
        self == Self::TestMemoryV1
    }

    pub fn is_platform_backend(self) -> bool {
        matches!(
            self,
            Self::AppleKeychainV1
                | Self::AndroidKeystoreV1
                | Self::WindowsCngV1
                | Self::LinuxSecretServiceV1
        )
    }
}

impl fmt::Display for DeviceSigningStorageBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSigningKeyHandle {
    pub device_id: String,
    pub signing_key_id: String,
    pub signature_algorithm: SignatureAlgorithmId,
    pub storage_backend: DeviceSigningStorageBackend,
    pub exportable: bool,
    pub hardware_backed: bool,
    pub user_presence_required: bool,
    pub backup_migratable: bool,
    pub created_at_ms: i64,
    pub last_used_at_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
}

impl DeviceSigningKeyHandle {
    pub fn new(
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        signature_algorithm: SignatureAlgorithmId,
        storage_backend: DeviceSigningStorageBackend,
        capabilities: DeviceSigningBackendCapabilities,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        if capabilities.storage_backend != storage_backend {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: storage_backend.as_str().to_owned(),
                message: "capabilities must describe the handle storage backend".to_owned(),
            });
        }
        let handle = Self {
            device_id: device_id.into(),
            signing_key_id: signing_key_id.into(),
            signature_algorithm,
            storage_backend,
            exportable: capabilities.exportable,
            hardware_backed: capabilities.hardware_backed,
            user_presence_required: capabilities.user_presence_required,
            backup_migratable: capabilities.backup_migratable,
            created_at_ms,
            last_used_at_ms: None,
            revoked_at_ms: None,
        };
        handle.validate()?;
        Ok(handle)
    }

    pub fn test_memory(
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        Self::new(
            device_id,
            signing_key_id,
            SignatureAlgorithmId::ed25519_v1(),
            DeviceSigningStorageBackend::TestMemoryV1,
            DeviceSigningBackendCapabilities::test_memory(),
            created_at_ms,
        )
    }

    pub fn apple_keychain(
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        Self::new(
            device_id,
            signing_key_id,
            SignatureAlgorithmId::ed25519_v1(),
            DeviceSigningStorageBackend::AppleKeychainV1,
            DeviceSigningBackendCapabilities::apple_keychain_v1(),
            created_at_ms,
        )
    }

    pub fn android_keystore(
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        Self::new(
            device_id,
            signing_key_id,
            SignatureAlgorithmId::ed25519_v1(),
            DeviceSigningStorageBackend::AndroidKeystoreV1,
            DeviceSigningBackendCapabilities::android_keystore_v1(),
            created_at_ms,
        )
    }

    pub fn revoked(mut self, revoked_at_ms: i64) -> Result<Self, CryptoError> {
        self.revoked_at_ms = Some(revoked_at_ms);
        self.validate()?;
        Ok(self)
    }

    pub fn is_revoked_at(&self, timestamp_ms: i64) -> bool {
        self.revoked_at_ms
            .map(|revoked_at_ms| timestamp_ms >= revoked_at_ms)
            .unwrap_or(false)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("signing_key_id", &self.signing_key_id)?;
        if self.signature_algorithm.as_str() != SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(CryptoError::invalid_field(
                "signature_algorithm",
                "value must be ed25519-v1",
            ));
        }
        if self.storage_backend == DeviceSigningStorageBackend::Unavailable {
            return Err(CryptoError::invalid_field(
                "storage_backend",
                "signing key handle cannot use unavailable backend",
            ));
        }
        if let Some(last_used_at_ms) = self.last_used_at_ms {
            if last_used_at_ms < self.created_at_ms {
                return Err(CryptoError::invalid_field(
                    "last_used_at_ms",
                    "value must be greater than or equal to created_at_ms",
                ));
            }
        }
        if let Some(revoked_at_ms) = self.revoked_at_ms {
            if revoked_at_ms < self.created_at_ms {
                return Err(CryptoError::invalid_field(
                    "revoked_at_ms",
                    "value must be greater than or equal to created_at_ms",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSigningBackendCapabilities {
    pub storage_backend: DeviceSigningStorageBackend,
    pub exportable: bool,
    pub hardware_backed: bool,
    pub user_presence_required: bool,
    pub backup_migratable: bool,
}

impl DeviceSigningBackendCapabilities {
    pub fn test_memory() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::TestMemoryV1,
            exportable: true,
            hardware_backed: false,
            user_presence_required: false,
            backup_migratable: false,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::Unavailable,
            exportable: false,
            hardware_backed: false,
            user_presence_required: false,
            backup_migratable: false,
        }
    }

    pub fn apple_keychain_v1() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::AppleKeychainV1,
            exportable: false,
            hardware_backed: false,
            user_presence_required: false,
            backup_migratable: false,
        }
    }

    pub fn android_keystore_v1() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::AndroidKeystoreV1,
            exportable: false,
            hardware_backed: false,
            user_presence_required: false,
            backup_migratable: false,
        }
    }

    pub fn platform(
        storage_backend: DeviceSigningStorageBackend,
        hardware_backed: bool,
        user_presence_required: bool,
        backup_migratable: bool,
    ) -> Result<Self, CryptoError> {
        if !storage_backend.is_platform_backend() {
            return Err(CryptoError::UnsupportedStorageBackend {
                backend: storage_backend.as_str().to_owned(),
            });
        }
        Ok(Self {
            storage_backend,
            exportable: false,
            hardware_backed,
            user_presence_required,
            backup_migratable,
        })
    }

    pub fn allows_production_signing(self) -> bool {
        self.storage_backend.is_platform_backend() && !self.exportable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePrivateKeyStoreStatus {
    pub storage_backend: DeviceSigningStorageBackend,
    pub available: bool,
    pub can_create_signing_keys: bool,
    pub can_sign: bool,
    pub capabilities: DeviceSigningBackendCapabilities,
}

impl DevicePrivateKeyStoreStatus {
    pub fn test_memory() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::TestMemoryV1,
            available: true,
            can_create_signing_keys: true,
            can_sign: true,
            capabilities: DeviceSigningBackendCapabilities::test_memory(),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            storage_backend: DeviceSigningStorageBackend::Unavailable,
            available: false,
            can_create_signing_keys: false,
            can_sign: false,
            capabilities: DeviceSigningBackendCapabilities::unavailable(),
        }
    }

    pub fn apple_keychain_v1() -> Self {
        // The native Apple Keychain Ed25519 path is wired for gated smoke only.
        // It must not advertise production readiness until the platform smoke passes.
        Self {
            storage_backend: DeviceSigningStorageBackend::AppleKeychainV1,
            available: false,
            can_create_signing_keys: false,
            can_sign: false,
            capabilities: DeviceSigningBackendCapabilities::apple_keychain_v1(),
        }
    }

    pub fn android_keystore_v1() -> Self {
        // The Android Keystore path is currently a feature-gated bridge boundary.
        // It must not advertise production readiness before JNI and device smoke pass.
        Self {
            storage_backend: DeviceSigningStorageBackend::AndroidKeystoreV1,
            available: false,
            can_create_signing_keys: false,
            can_sign: false,
            capabilities: DeviceSigningBackendCapabilities::android_keystore_v1(),
        }
    }

    pub fn platform(
        storage_backend: DeviceSigningStorageBackend,
        hardware_backed: bool,
        user_presence_required: bool,
        backup_migratable: bool,
    ) -> Result<Self, CryptoError> {
        let capabilities = DeviceSigningBackendCapabilities::platform(
            storage_backend,
            hardware_backed,
            user_presence_required,
            backup_migratable,
        )?;
        Ok(Self {
            storage_backend,
            available: true,
            can_create_signing_keys: true,
            can_sign: true,
            capabilities,
        })
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.storage_backend != self.capabilities.storage_backend {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: self.storage_backend.as_str().to_owned(),
                message: "status capabilities must describe the status backend".to_owned(),
            });
        }
        if self.storage_backend == DeviceSigningStorageBackend::Unavailable && self.available {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: self.storage_backend.as_str().to_owned(),
                message: "unavailable backend cannot be marked available".to_owned(),
            });
        }
        if !self.available && (self.can_create_signing_keys || self.can_sign) {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: self.storage_backend.as_str().to_owned(),
                message: "unavailable status cannot create keys or sign".to_owned(),
            });
        }
        Ok(())
    }

    pub fn ensure_production_signing_allowed(&self) -> Result<(), CryptoError> {
        self.validate()?;
        if !self.available || !self.can_sign {
            return Err(CryptoError::StorageBackendUnavailable {
                backend: self.storage_backend.as_str().to_owned(),
            });
        }
        if !self.capabilities.allows_production_signing() {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: self.storage_backend.as_str().to_owned(),
                message: "backend is not eligible for production signing".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSigningPublicKey {
    pub device_id: String,
    pub signing_key_id: String,
    pub signature_algorithm: SignatureAlgorithmId,
    pub public_key: Vec<u8>,
    pub created_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

impl DeviceSigningPublicKey {
    pub fn new(
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        signature_algorithm: SignatureAlgorithmId,
        public_key: impl Into<Vec<u8>>,
        created_at_ms: i64,
        revoked_at_ms: Option<i64>,
    ) -> Result<Self, CryptoError> {
        let public_key = Self {
            device_id: device_id.into(),
            signing_key_id: signing_key_id.into(),
            signature_algorithm,
            public_key: public_key.into(),
            created_at_ms,
            revoked_at_ms,
        };
        public_key.validate()?;
        Ok(public_key)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("signing_key_id", &self.signing_key_id)?;
        if self.signature_algorithm.as_str() != SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(CryptoError::invalid_field(
                "signature_algorithm",
                "value must be ed25519-v1",
            ));
        }
        if self.public_key.len() != ED25519_PUBLIC_KEY_LEN {
            return Err(CryptoError::invalid_field(
                "public_key",
                format!("value must be {ED25519_PUBLIC_KEY_LEN} bytes"),
            ));
        }
        if let Some(revoked_at_ms) = self.revoked_at_ms {
            if revoked_at_ms < self.created_at_ms {
                return Err(CryptoError::invalid_field(
                    "revoked_at_ms",
                    "value must be greater than or equal to created_at_ms",
                ));
            }
        }
        Ok(())
    }

    pub fn is_active_at(&self, timestamp_ms: i64) -> bool {
        timestamp_ms >= self.created_at_ms
            && self
                .revoked_at_ms
                .map(|revoked_at_ms| timestamp_ms < revoked_at_ms)
                .unwrap_or(true)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceSignature {
    pub signature_schema_version: u16,
    pub signature_algorithm: SignatureAlgorithmId,
    pub signature_key_id: String,
    pub signer_device_id: String,
    pub signature: Vec<u8>,
}

impl DeviceSignature {
    pub fn new(
        signature_key_id: impl Into<String>,
        signer_device_id: impl Into<String>,
        signature: impl Into<Vec<u8>>,
    ) -> Result<Self, CryptoError> {
        let signature = Self {
            signature_schema_version: SIGNATURE_SCHEMA_VERSION,
            signature_algorithm: SignatureAlgorithmId::ed25519_v1(),
            signature_key_id: signature_key_id.into(),
            signer_device_id: signer_device_id.into(),
            signature: signature.into(),
        };
        signature.validate()?;
        Ok(signature)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.signature_schema_version != SIGNATURE_SCHEMA_VERSION {
            return Err(CryptoError::invalid_field(
                "signature_schema_version",
                format!("value must be {SIGNATURE_SCHEMA_VERSION}"),
            ));
        }
        if self.signature_algorithm.as_str() != SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(CryptoError::invalid_field(
                "signature_algorithm",
                "value must be ed25519-v1",
            ));
        }
        validate_required("signature_key_id", &self.signature_key_id)?;
        validate_required("signer_device_id", &self.signer_device_id)?;
        if self.signature.len() != ED25519_SIGNATURE_LEN {
            return Err(CryptoError::invalid_field(
                "signature",
                format!("value must be {ED25519_SIGNATURE_LEN} bytes"),
            ));
        }
        Ok(())
    }

    pub fn verify_at(
        &self,
        public_key: &DeviceSigningPublicKey,
        canonical_bytes: &[u8],
        signed_at_ms: i64,
    ) -> Result<(), CryptoError> {
        self.validate()?;
        public_key.validate()?;
        if self.signature_key_id != public_key.signing_key_id {
            return Err(CryptoError::SignatureVerificationFailed);
        }
        if self.signer_device_id != public_key.device_id {
            return Err(CryptoError::SignatureVerificationFailed);
        }
        if !public_key.is_active_at(signed_at_ms) {
            return Err(CryptoError::SignatureVerificationFailed);
        }

        let public_key_bytes: [u8; ED25519_PUBLIC_KEY_LEN] =
            public_key.public_key.as_slice().try_into().map_err(|_| {
                CryptoError::invalid_field(
                    "public_key",
                    format!("value must be {ED25519_PUBLIC_KEY_LEN} bytes"),
                )
            })?;
        let signature_bytes: [u8; ED25519_SIGNATURE_LEN] =
            self.signature.as_slice().try_into().map_err(|_| {
                CryptoError::invalid_field(
                    "signature",
                    format!("value must be {ED25519_SIGNATURE_LEN} bytes"),
                )
            })?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|_| CryptoError::SignatureVerificationFailed)?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify_strict(canonical_bytes, &signature)
            .map_err(|_| CryptoError::SignatureVerificationFailed)
    }
}

impl fmt::Debug for DeviceSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceSignature")
            .field("signature_schema_version", &self.signature_schema_version)
            .field("signature_algorithm", &self.signature_algorithm)
            .field("signature_key_id", &self.signature_key_id)
            .field("signer_device_id", &self.signer_device_id)
            .field("signature", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureField {
    name: &'static str,
    value: Vec<u8>,
}

impl SignatureField {
    pub fn text(name: &'static str, value: &str) -> Self {
        Self {
            name,
            value: value.as_bytes().to_vec(),
        }
    }

    pub fn bytes(name: &'static str, value: &[u8]) -> Self {
        Self {
            name,
            value: value.to_vec(),
        }
    }

    pub fn u16(name: &'static str, value: u16) -> Self {
        Self::text(name, &value.to_string())
    }

    pub fn u64(name: &'static str, value: u64) -> Self {
        Self::text(name, &value.to_string())
    }

    pub fn i64(name: &'static str, value: i64) -> Self {
        Self::text(name, &value.to_string())
    }

    pub fn usize(name: &'static str, value: usize) -> Self {
        Self::text(name, &value.to_string())
    }

    pub fn optional_u64(name: &'static str, value: Option<u64>) -> Self {
        Self::text(
            name,
            &value.map(|value| value.to_string()).unwrap_or_default(),
        )
    }
}

pub fn canonical_signature_bytes(record_type: &str, fields: &[SignatureField]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_aad_field(&mut bytes, "domain_separator", b"radishlex-signature-v1");
    push_aad_field(&mut bytes, "record_type", record_type.as_bytes());
    for field in fields {
        push_aad_field(&mut bytes, field.name, &field.value);
    }
    bytes
}

#[derive(Default)]
pub struct TestMemoryDeviceKeyStore {
    keys: BTreeMap<(String, String), TestMemorySigningKey>,
}

impl TestMemoryDeviceKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::test_memory()
    }

    pub fn insert_signing_key(
        &mut self,
        device_id: impl Into<String>,
        signing_key_id: impl Into<String>,
        seed: [u8; ED25519_PUBLIC_KEY_LEN],
        created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        let device_id = device_id.into();
        let signing_key_id = signing_key_id.into();
        validate_required("device_id", &device_id)?;
        validate_required("signing_key_id", &signing_key_id)?;
        if seed.iter().all(|byte| *byte == 0) {
            return Err(CryptoError::invalid_field(
                "signing_key_seed",
                "value cannot be all zeroes",
            ));
        }

        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = DeviceSigningPublicKey::new(
            device_id.clone(),
            signing_key_id.clone(),
            SignatureAlgorithmId::ed25519_v1(),
            signing_key.verifying_key().to_bytes(),
            created_at_ms,
            None,
        )?;
        self.keys.insert(
            (device_id.clone(), signing_key_id.clone()),
            TestMemorySigningKey {
                handle: DeviceSigningKeyHandle::test_memory(
                    device_id,
                    signing_key_id,
                    created_at_ms,
                )?,
                signing_key,
                public_key: public_key.clone(),
            },
        );
        Ok(public_key)
    }

    pub fn handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        Ok(self.lookup(device_id, signing_key_id)?.handle.clone())
    }

    pub fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        handle.validate()?;
        Ok(self
            .lookup(&handle.device_id, &handle.signing_key_id)?
            .public_key
            .clone())
    }

    pub fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        handle.validate()?;
        validate_non_empty_bytes("canonical_bytes", canonical_bytes)?;
        let key = self.lookup(&handle.device_id, &handle.signing_key_id)?;
        if key.handle.revoked_at_ms.is_some() {
            return Err(CryptoError::PrivateKeyRevoked {
                key_id: handle.signing_key_id.clone(),
            });
        }
        let signature = key.signing_key.sign(canonical_bytes);
        DeviceSignature::new(
            handle.signing_key_id.clone(),
            handle.device_id.clone(),
            signature.to_bytes(),
        )
    }

    pub fn delete_or_revoke(
        &mut self,
        handle: &DeviceSigningKeyHandle,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        handle.validate()?;
        let key = self.lookup_mut(&handle.device_id, &handle.signing_key_id)?;
        key.handle = key.handle.clone().revoked(revoked_at_ms)?;
        key.public_key.revoked_at_ms = Some(revoked_at_ms);
        key.public_key.validate()?;
        Ok(())
    }

    pub fn export_private_key_for_tests(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<[u8; ED25519_PUBLIC_KEY_LEN], CryptoError> {
        handle.validate()?;
        if self
            .lookup(&handle.device_id, &handle.signing_key_id)?
            .handle
            .revoked_at_ms
            .is_some()
        {
            return Err(CryptoError::PrivateKeyRevoked {
                key_id: handle.signing_key_id.clone(),
            });
        }
        if !handle.exportable || handle.storage_backend != DeviceSigningStorageBackend::TestMemoryV1
        {
            return Err(CryptoError::PrivateKeyExportBlocked {
                key_id: handle.signing_key_id.clone(),
            });
        }
        Ok(self
            .lookup(&handle.device_id, &handle.signing_key_id)?
            .signing_key
            .to_bytes())
    }

    fn lookup(
        &self,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<&TestMemorySigningKey, CryptoError> {
        self.keys
            .get(&(device_id.to_owned(), signing_key_id.to_owned()))
            .ok_or_else(|| CryptoError::PrivateKeyUnavailable {
                key_id: signing_key_id.to_owned(),
            })
    }

    fn lookup_mut(
        &mut self,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<&mut TestMemorySigningKey, CryptoError> {
        self.keys
            .get_mut(&(device_id.to_owned(), signing_key_id.to_owned()))
            .ok_or_else(|| CryptoError::PrivateKeyUnavailable {
                key_id: signing_key_id.to_owned(),
            })
    }
}

impl fmt::Debug for TestMemoryDeviceKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestMemoryDeviceKeyStore")
            .field("key_count", &self.keys.len())
            .finish()
    }
}

struct TestMemorySigningKey {
    handle: DeviceSigningKeyHandle,
    signing_key: SigningKey,
    public_key: DeviceSigningPublicKey,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableDeviceKeyStore;

impl UnavailableDeviceKeyStore {
    pub fn new() -> Self {
        Self
    }

    pub fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::unavailable()
    }

    pub fn create_signing_key(
        &self,
        _device_id: &str,
        _signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        })
    }

    pub fn handle(
        &self,
        _device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        Err(CryptoError::PrivateKeyUnavailable {
            key_id: signing_key_id.to_owned(),
        })
    }

    pub fn public_key(
        &self,
        _handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        })
    }

    pub fn sign(
        &self,
        _handle: &DeviceSigningKeyHandle,
        _canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        Err(CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        })
    }

    pub fn delete_or_revoke(
        &self,
        _handle: &DeviceSigningKeyHandle,
        _revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        Err(CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedSyncObjectManifest {
    pub signature: DeviceSignature,
    pub domain_id: String,
    pub object_id: String,
    pub object_type: String,
    pub version: u64,
    pub base_version: Option<u64>,
    pub key_id: String,
    pub key_epoch: u64,
    pub envelope_algorithm: String,
    pub nonce: Vec<u8>,
    pub encrypted_payload_len: usize,
    pub ciphertext_hash: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl SignedSyncObjectManifest {
    pub fn new(
        domain_id: impl Into<String>,
        envelope: &EncryptedObjectEnvelope,
        signature: DeviceSignature,
    ) -> Result<Self, CryptoError> {
        envelope.validate()?;
        let manifest = Self {
            signature,
            domain_id: domain_id.into(),
            object_id: envelope.object_id.clone(),
            object_type: envelope.object_type.as_str().to_owned(),
            version: envelope.version,
            base_version: envelope.base_version,
            key_id: envelope.key_id.clone(),
            key_epoch: envelope.key_epoch,
            envelope_algorithm: envelope.algorithm.as_str().to_owned(),
            nonce: envelope.nonce.as_bytes().to_vec(),
            encrypted_payload_len: envelope.encrypted_payload.len(),
            ciphertext_hash: envelope.ciphertext_hash.as_str().to_owned(),
            created_at_ms: envelope.created_at_ms,
            updated_at_ms: envelope.updated_at_ms,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("sync_object_manifest", &self.signature_fields())
    }

    pub fn signature_fields(&self) -> Vec<SignatureField> {
        vec![
            SignatureField::u16(
                "signature_schema_version",
                self.signature.signature_schema_version,
            ),
            SignatureField::text(
                "signature_algorithm",
                self.signature.signature_algorithm.as_str(),
            ),
            SignatureField::text("signature_key_id", &self.signature.signature_key_id),
            SignatureField::text("signer_device_id", &self.signature.signer_device_id),
            SignatureField::text("domain_id", &self.domain_id),
            SignatureField::text("object_id", &self.object_id),
            SignatureField::text("object_type", &self.object_type),
            SignatureField::u64("version", self.version),
            SignatureField::optional_u64("base_version", self.base_version),
            SignatureField::text("key_id", &self.key_id),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::text("envelope_algorithm", &self.envelope_algorithm),
            SignatureField::bytes("nonce", &self.nonce),
            SignatureField::usize("encrypted_payload_len", self.encrypted_payload_len),
            SignatureField::text("ciphertext_hash", &self.ciphertext_hash),
            SignatureField::i64("created_at_ms", self.created_at_ms),
            SignatureField::i64("updated_at_ms", self.updated_at_ms),
        ]
    }

    pub fn verify(&self, public_key: &DeviceSigningPublicKey) -> Result<(), CryptoError> {
        self.validate()?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.created_at_ms)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        self.signature.validate()?;
        validate_required("domain_id", &self.domain_id)?;
        validate_required("object_id", &self.object_id)?;
        validate_required("object_type", &self.object_type)?;
        validate_required("key_id", &self.key_id)?;
        validate_required("envelope_algorithm", &self.envelope_algorithm)?;
        validate_non_empty_bytes("nonce", &self.nonce)?;
        validate_required("ciphertext_hash", &self.ciphertext_hash)?;
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
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        if self.encrypted_payload_len == 0 {
            return Err(CryptoError::invalid_field(
                "encrypted_payload_len",
                "value must be greater than 0",
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRecoveryRecordManifest {
    pub signature: DeviceSignature,
    pub recovery_id: String,
    pub domain_id: String,
    pub key_epoch: u64,
    pub kdf_id: String,
    pub kdf_version: u16,
    pub salt: Vec<u8>,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
    pub envelope_algorithm: String,
    pub envelope_nonce: Vec<u8>,
    pub encrypted_recovery_key_len: usize,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl SignedRecoveryRecordManifest {
    pub fn new(
        material: &RecoveryMaterial,
        signature: DeviceSignature,
    ) -> Result<Self, CryptoError> {
        material.validate()?;
        let manifest = Self {
            signature,
            recovery_id: material.recovery_id.clone(),
            domain_id: material.domain_id.clone(),
            key_epoch: material.key_epoch,
            kdf_id: material.kdf_id.clone(),
            kdf_version: material.kdf_version,
            salt: material.salt.clone(),
            memory_kib: material.memory_kib,
            iterations: material.iterations,
            parallelism: material.parallelism,
            output_len: material.output_len,
            envelope_algorithm: material.envelope_algorithm.as_str().to_owned(),
            envelope_nonce: material.envelope_nonce.as_bytes().to_vec(),
            encrypted_recovery_key_len: material.encrypted_recovery_key.len(),
            created_at_ms: material.created_at_ms,
            updated_at_ms: material.updated_at_ms,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("recovery_record", &self.signature_fields())
    }

    pub fn signature_fields(&self) -> Vec<SignatureField> {
        vec![
            SignatureField::u16(
                "signature_schema_version",
                self.signature.signature_schema_version,
            ),
            SignatureField::text(
                "signature_algorithm",
                self.signature.signature_algorithm.as_str(),
            ),
            SignatureField::text("signature_key_id", &self.signature.signature_key_id),
            SignatureField::text("signer_device_id", &self.signature.signer_device_id),
            SignatureField::text("recovery_id", &self.recovery_id),
            SignatureField::text("domain_id", &self.domain_id),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::text("kdf_id", &self.kdf_id),
            SignatureField::u16("kdf_version", self.kdf_version),
            SignatureField::bytes("salt", &self.salt),
            SignatureField::u64("memory_kib", u64::from(self.memory_kib)),
            SignatureField::u64("iterations", u64::from(self.iterations)),
            SignatureField::u64("parallelism", u64::from(self.parallelism)),
            SignatureField::usize("output_len", self.output_len),
            SignatureField::text("envelope_algorithm", &self.envelope_algorithm),
            SignatureField::bytes("envelope_nonce", &self.envelope_nonce),
            SignatureField::usize(
                "encrypted_recovery_key_len",
                self.encrypted_recovery_key_len,
            ),
            SignatureField::i64("created_at_ms", self.created_at_ms),
            SignatureField::i64("updated_at_ms", self.updated_at_ms),
        ]
    }

    pub fn verify(&self, public_key: &DeviceSigningPublicKey) -> Result<(), CryptoError> {
        self.validate()?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.created_at_ms)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        self.signature.validate()?;
        validate_required("recovery_id", &self.recovery_id)?;
        validate_required("domain_id", &self.domain_id)?;
        validate_required("kdf_id", &self.kdf_id)?;
        validate_non_empty_bytes("salt", &self.salt)?;
        validate_required("envelope_algorithm", &self.envelope_algorithm)?;
        validate_non_empty_bytes("envelope_nonce", &self.envelope_nonce)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        if self.kdf_version == 0 || self.memory_kib == 0 || self.iterations == 0 {
            return Err(CryptoError::invalid_field(
                "kdf_parameters",
                "KDF version, memory and iterations must be greater than 0",
            ));
        }
        if self.parallelism == 0 || self.output_len == 0 {
            return Err(CryptoError::invalid_field(
                "kdf_parameters",
                "parallelism and output_len must be greater than 0",
            ));
        }
        if self.encrypted_recovery_key_len == 0 {
            return Err(CryptoError::invalid_field(
                "encrypted_recovery_key_len",
                "value must be greater than 0",
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
}

#[cfg(test)]
mod tests;
