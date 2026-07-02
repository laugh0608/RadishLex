use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::model::{validate_non_empty_bytes, validate_required, CryptoError};

use super::{
    DevicePrivateKeyStoreStatus, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    DeviceSigningStorageBackend, DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1, ED25519_PUBLIC_KEY_LEN,
    ED25519_SIGNATURE_LEN, SIGNATURE_ALGORITHM_ED25519_V1,
};

mod jni_bridge;

pub const ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION: u16 = 1;
pub const ANDROID_KEYSTORE_PROVIDER: &str = "AndroidKeyStore";
pub const ANDROID_KEYSTORE_SIGNATURE_ALGORITHM: &str = "Ed25519";
pub const DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX: &str = "org.radishlex.sync.signing";

pub const ANDROID_KEYSTORE_JNI_BRIDGE_CLASS: &str =
    "org/radishlex/android/keystore/RadishLexAndroidKeystoreJniBridge";
pub const ANDROID_KEYSTORE_JNI_RESULT_CLASS: &str =
    "org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult";
pub const ANDROID_KEYSTORE_JNI_CREATE_SIGNING_KEY_METHOD: &str = "createSigningKey";
pub const ANDROID_KEYSTORE_JNI_LOAD_PUBLIC_KEY_METHOD: &str = "loadPublicKey";
pub const ANDROID_KEYSTORE_JNI_SIGN_METHOD: &str = "sign";
pub const ANDROID_KEYSTORE_JNI_DELETE_SIGNING_KEY_METHOD: &str = "deleteSigningKey";
pub const ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR: &str = concat!(
    "(ILjava/lang/String;Ljava/lang/String;)L",
    "org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult;"
);
pub const ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR: &str = concat!(
    "(ILjava/lang/String;Ljava/lang/String;[B)L",
    "org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult;"
);
pub const ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD: &str = "getPublicKey";
pub const ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD: &str = "getSignature";
pub const ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD: &str = "getErrorCode";
pub const ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR: &str = "()[B";
pub const ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR: &str = "()Ljava/lang/String;";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidKeystoreBridgeOperation {
    CreateSigningKey,
    LoadPublicKey,
    Sign,
    DeleteSigningKey,
}

impl AndroidKeystoreBridgeOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateSigningKey => "create_signing_key",
            Self::LoadPublicKey => "load_public_key",
            Self::Sign => "sign",
            Self::DeleteSigningKey => "delete_signing_key",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AndroidKeystoreBridgeRequest {
    pub contract_version: u16,
    pub operation: AndroidKeystoreBridgeOperation,
    pub signing_key_id: String,
    pub alias: String,
    pub canonical_bytes_len: usize,
}

impl AndroidKeystoreBridgeRequest {
    pub fn new(
        operation: AndroidKeystoreBridgeOperation,
        signing_key_id: impl Into<String>,
        alias: impl Into<String>,
        canonical_bytes_len: usize,
    ) -> Result<Self, CryptoError> {
        let request = Self {
            contract_version: ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION,
            operation,
            signing_key_id: signing_key_id.into(),
            alias: alias.into(),
            canonical_bytes_len,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.contract_version != ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION {
            return Err(CryptoError::invalid_field(
                "android_keystore_bridge_contract_version",
                format!("value must be {ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION}"),
            ));
        }
        validate_required("signing_key_id", &self.signing_key_id)?;
        validate_required("android_keystore_alias", &self.alias)?;
        match self.operation {
            AndroidKeystoreBridgeOperation::Sign => {
                if self.canonical_bytes_len == 0 {
                    return Err(CryptoError::invalid_field(
                        "canonical_bytes_len",
                        "sign operation requires non-empty canonical bytes",
                    ));
                }
            }
            _ => {
                if self.canonical_bytes_len != 0 {
                    return Err(CryptoError::invalid_field(
                        "canonical_bytes_len",
                        "only sign operation may carry canonical bytes",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for AndroidKeystoreBridgeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AndroidKeystoreBridgeRequest")
            .field("contract_version", &self.contract_version)
            .field("operation", &self.operation.as_str())
            .field("signing_key_id", &self.signing_key_id)
            .field("alias_len", &self.alias.len())
            .field("canonical_bytes_len", &self.canonical_bytes_len)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidKeystoreBridgeErrorCode {
    StorageBackendUnavailable,
    UnsupportedSignatureAlgorithm,
    UnsupportedStorageBackend,
    PrivateKeyUnavailable,
    PrivateKeyLocked,
    PrivateKeyAccessDenied,
    PrivateKeyUserPresenceRequired,
    PrivateKeyCorrupted,
}

impl AndroidKeystoreBridgeErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StorageBackendUnavailable => "storage_backend_unavailable",
            Self::UnsupportedSignatureAlgorithm => "unsupported_signature_algorithm",
            Self::UnsupportedStorageBackend => "unsupported_storage_backend",
            Self::PrivateKeyUnavailable => "private_key_unavailable",
            Self::PrivateKeyLocked => "private_key_locked",
            Self::PrivateKeyAccessDenied => "private_key_access_denied",
            Self::PrivateKeyUserPresenceRequired => "private_key_user_presence_required",
            Self::PrivateKeyCorrupted => "private_key_corrupted",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CryptoError> {
        match value {
            "storage_backend_unavailable" => Ok(Self::StorageBackendUnavailable),
            "unsupported_signature_algorithm" => Ok(Self::UnsupportedSignatureAlgorithm),
            "unsupported_storage_backend" => Ok(Self::UnsupportedStorageBackend),
            "private_key_unavailable" => Ok(Self::PrivateKeyUnavailable),
            "private_key_locked" => Ok(Self::PrivateKeyLocked),
            "private_key_access_denied" => Ok(Self::PrivateKeyAccessDenied),
            "private_key_user_presence_required" => Ok(Self::PrivateKeyUserPresenceRequired),
            "private_key_corrupted" => Ok(Self::PrivateKeyCorrupted),
            _ => Err(CryptoError::invalid_field(
                "android_keystore_bridge_error_code",
                format!("unsupported Android Keystore bridge error code {value}"),
            )),
        }
    }

    pub fn to_crypto_error(self, signing_key_id: &str) -> CryptoError {
        match self {
            Self::StorageBackendUnavailable => CryptoError::StorageBackendUnavailable {
                backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            },
            Self::UnsupportedSignatureAlgorithm => CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            },
            Self::UnsupportedStorageBackend => CryptoError::UnsupportedStorageBackend {
                backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            },
            Self::PrivateKeyUnavailable => CryptoError::PrivateKeyUnavailable {
                key_id: signing_key_id.to_owned(),
            },
            Self::PrivateKeyLocked => CryptoError::PrivateKeyLocked {
                key_id: signing_key_id.to_owned(),
            },
            Self::PrivateKeyAccessDenied => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
            },
            Self::PrivateKeyUserPresenceRequired => CryptoError::PrivateKeyUserPresenceRequired {
                key_id: signing_key_id.to_owned(),
            },
            Self::PrivateKeyCorrupted => CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            },
        }
    }
}

pub fn android_keystore_alias(
    alias_prefix: &str,
    signing_key_id: &str,
) -> Result<String, CryptoError> {
    validate_required("android_keystore_alias_prefix", alias_prefix)?;
    validate_required("signing_key_id", signing_key_id)?;
    Ok(format!("{alias_prefix}.{signing_key_id}"))
}

pub fn validate_android_keystore_public_key(
    signing_key_id: &str,
    public_key: impl Into<Vec<u8>>,
) -> Result<Vec<u8>, CryptoError> {
    validate_required("signing_key_id", signing_key_id)?;
    let public_key = public_key.into();
    if public_key.len() != ED25519_PUBLIC_KEY_LEN {
        return Err(CryptoError::PrivateKeyCorrupted {
            key_id: signing_key_id.to_owned(),
        });
    }
    Ok(public_key)
}

pub fn validate_android_keystore_signature(
    signing_key_id: &str,
    signature: impl Into<Vec<u8>>,
) -> Result<Vec<u8>, CryptoError> {
    validate_required("signing_key_id", signing_key_id)?;
    let signature = signature.into();
    if signature.len() != ED25519_SIGNATURE_LEN {
        return Err(CryptoError::PrivateKeyCorrupted {
            key_id: signing_key_id.to_owned(),
        });
    }
    Ok(signature)
}

pub struct AndroidKeystoreDeviceKeyStore {
    alias_prefix: String,
    bridge: Arc<dyn AndroidKeystoreBridge>,
    revoked_keys: Mutex<BTreeSet<(String, String)>>,
}

impl AndroidKeystoreDeviceKeyStore {
    pub fn new() -> Self {
        Self::with_bridge(
            DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX.to_owned(),
            default_android_keystore_bridge(),
        )
        .expect("default Android Keystore alias prefix is valid")
    }

    pub fn with_alias_prefix(alias_prefix: impl Into<String>) -> Result<Self, CryptoError> {
        Self::with_bridge(alias_prefix.into(), default_android_keystore_bridge())
    }

    fn with_bridge(
        alias_prefix: String,
        bridge: Arc<dyn AndroidKeystoreBridge>,
    ) -> Result<Self, CryptoError> {
        validate_required("android_keystore_alias_prefix", &alias_prefix)?;
        Ok(Self {
            alias_prefix,
            bridge,
            revoked_keys: Mutex::new(BTreeSet::new()),
        })
    }

    pub fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        self.bridge.backend_status()
    }

    pub fn create_signing_key(
        &self,
        device_id: &str,
        signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        self.ensure_not_revoked(device_id, signing_key_id)?;
        let alias = self.key_alias(signing_key_id)?;
        let public_key = self.bridge.create_signing_key(signing_key_id, &alias)?;
        let public_key = validate_android_keystore_public_key(signing_key_id, public_key)?;
        DeviceSigningPublicKey::new(
            device_id,
            signing_key_id,
            super::SignatureAlgorithmId::ed25519_v1(),
            public_key,
            _created_at_ms,
            None,
        )
    }

    pub fn handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        self.ensure_not_revoked(device_id, signing_key_id)?;
        let alias = self.key_alias(signing_key_id)?;
        let _public_key = self.bridge.public_key(signing_key_id, &alias)?;
        let _public_key = validate_android_keystore_public_key(signing_key_id, _public_key)?;
        DeviceSigningKeyHandle::android_keystore(device_id, signing_key_id, 0)
    }

    pub fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_android_handle(handle)?;
        self.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let alias = self.key_alias(&handle.signing_key_id)?;
        let public_key = self.bridge.public_key(&handle.signing_key_id, &alias)?;
        let public_key = validate_android_keystore_public_key(&handle.signing_key_id, public_key)?;
        DeviceSigningPublicKey::new(
            &handle.device_id,
            &handle.signing_key_id,
            super::SignatureAlgorithmId::ed25519_v1(),
            public_key,
            handle.created_at_ms,
            handle.revoked_at_ms,
        )
    }

    pub fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        validate_android_handle(handle)?;
        validate_non_empty_bytes("canonical_bytes", canonical_bytes)?;
        self.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let alias = self.key_alias(&handle.signing_key_id)?;
        let signature = self
            .bridge
            .sign(&handle.signing_key_id, &alias, canonical_bytes)?;
        let signature = validate_android_keystore_signature(&handle.signing_key_id, signature)?;
        DeviceSignature::new(
            handle.signing_key_id.clone(),
            handle.device_id.clone(),
            signature,
        )
    }

    pub fn delete_or_revoke(
        &self,
        handle: &DeviceSigningKeyHandle,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        validate_android_handle(handle)?;
        let alias = self.key_alias(&handle.signing_key_id)?;
        let delete_result = self
            .bridge
            .delete_signing_key(&handle.signing_key_id, &alias);
        self.mark_revoked(&handle.device_id, &handle.signing_key_id, revoked_at_ms)?;
        delete_result
    }

    fn key_alias(&self, signing_key_id: &str) -> Result<String, CryptoError> {
        android_keystore_alias(&self.alias_prefix, signing_key_id)
    }

    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError> {
        let revoked_keys =
            self.revoked_keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
        if revoked_keys.contains(&(device_id.to_owned(), signing_key_id.to_owned())) {
            return Err(CryptoError::PrivateKeyRevoked {
                key_id: signing_key_id.to_owned(),
            });
        }
        Ok(())
    }

    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        _revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        let mut revoked_keys =
            self.revoked_keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
        revoked_keys.insert((device_id.to_owned(), signing_key_id.to_owned()));
        Ok(())
    }
}

trait AndroidKeystoreBridge: fmt::Debug + Send + Sync {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus;

    fn create_signing_key(&self, signing_key_id: &str, alias: &str)
        -> Result<Vec<u8>, CryptoError>;

    fn public_key(&self, signing_key_id: &str, alias: &str) -> Result<Vec<u8>, CryptoError>;

    fn sign(
        &self,
        signing_key_id: &str,
        alias: &str,
        canonical_bytes: &[u8],
    ) -> Result<Vec<u8>, CryptoError>;

    fn delete_signing_key(&self, signing_key_id: &str, alias: &str) -> Result<(), CryptoError>;
}

#[derive(Debug)]
struct UnavailableAndroidKeystoreBridge;

impl AndroidKeystoreBridge for UnavailableAndroidKeystoreBridge {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::android_keystore_v1()
    }

    fn create_signing_key(
        &self,
        _signing_key_id: &str,
        _alias: &str,
    ) -> Result<Vec<u8>, CryptoError> {
        Err(android_keystore_unavailable())
    }

    fn public_key(&self, signing_key_id: &str, _alias: &str) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::PrivateKeyUnavailable {
            key_id: signing_key_id.to_owned(),
        })
    }

    fn sign(
        &self,
        _signing_key_id: &str,
        _alias: &str,
        _canonical_bytes: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Err(android_keystore_unavailable())
    }

    fn delete_signing_key(&self, _signing_key_id: &str, _alias: &str) -> Result<(), CryptoError> {
        Err(android_keystore_unavailable())
    }
}

impl Default for AndroidKeystoreDeviceKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AndroidKeystoreDeviceKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let revoked_key_count = self
            .revoked_keys
            .lock()
            .map(|revoked_keys| revoked_keys.len())
            .unwrap_or_default();
        f.debug_struct("AndroidKeystoreDeviceKeyStore")
            .field("storage_backend", &DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1)
            .field("alias_prefix_len", &self.alias_prefix.len())
            .field("revoked_key_count", &revoked_key_count)
            .finish()
    }
}

fn validate_android_handle(handle: &DeviceSigningKeyHandle) -> Result<(), CryptoError> {
    handle.validate()?;
    if handle.storage_backend != DeviceSigningStorageBackend::AndroidKeystoreV1 {
        return Err(CryptoError::BackendCapabilityMismatch {
            backend: handle.storage_backend.as_str().to_owned(),
            message: "handle must use android-keystore-v1 backend".to_owned(),
        });
    }
    Ok(())
}

fn android_keystore_unavailable() -> CryptoError {
    #[cfg(target_os = "android")]
    {
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
        }
    }
}

fn default_android_keystore_bridge() -> Arc<dyn AndroidKeystoreBridge> {
    #[cfg(target_os = "android")]
    {
        Arc::new(jni_bridge::JniAndroidKeystoreBridge::new())
    }

    #[cfg(not(target_os = "android"))]
    {
        Arc::new(UnavailableAndroidKeystoreBridge)
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AndroidKeystoreJniMethodSpec {
    name: &'static str,
    descriptor: &'static str,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn android_keystore_jni_method_spec(
    operation: AndroidKeystoreBridgeOperation,
) -> AndroidKeystoreJniMethodSpec {
    match operation {
        AndroidKeystoreBridgeOperation::CreateSigningKey => AndroidKeystoreJniMethodSpec {
            name: ANDROID_KEYSTORE_JNI_CREATE_SIGNING_KEY_METHOD,
            descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
        },
        AndroidKeystoreBridgeOperation::LoadPublicKey => AndroidKeystoreJniMethodSpec {
            name: ANDROID_KEYSTORE_JNI_LOAD_PUBLIC_KEY_METHOD,
            descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
        },
        AndroidKeystoreBridgeOperation::Sign => AndroidKeystoreJniMethodSpec {
            name: ANDROID_KEYSTORE_JNI_SIGN_METHOD,
            descriptor: ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR,
        },
        AndroidKeystoreBridgeOperation::DeleteSigningKey => AndroidKeystoreJniMethodSpec {
            name: ANDROID_KEYSTORE_JNI_DELETE_SIGNING_KEY_METHOD,
            descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::{
        canonical_signature_bytes, DeviceSigningStorageBackend, SignatureField,
        DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1, ED25519_PUBLIC_KEY_LEN, ED25519_SIGNATURE_LEN,
    };

    #[test]
    fn bridge_backed_store_creates_signs_verifies_and_deletes_key() {
        let bridge = Arc::new(SyntheticAndroidKeystoreBridge::new(
            DevicePrivateKeyStoreStatus::platform(
                DeviceSigningStorageBackend::AndroidKeystoreV1,
                false,
                false,
                false,
            )
            .expect("synthetic platform status"),
        ));
        let store = AndroidKeystoreDeviceKeyStore::with_bridge(
            "org.radishlex.test.signing".to_owned(),
            bridge,
        )
        .expect("store");
        let status = store.backend_status();
        status.validate().expect("status");
        assert_eq!(
            status.storage_backend,
            DeviceSigningStorageBackend::AndroidKeystoreV1
        );

        let public_key = store
            .create_signing_key("device-a", "signing-key-a", 10)
            .expect("public key");
        assert_eq!(public_key.public_key.len(), ED25519_PUBLIC_KEY_LEN);

        let handle = store.handle("device-a", "signing-key-a").expect("handle");
        assert_eq!(
            handle.storage_backend,
            DeviceSigningStorageBackend::AndroidKeystoreV1
        );
        assert!(!handle.exportable);

        let loaded_public_key = store.public_key(&handle).expect("loaded public key");
        assert_eq!(loaded_public_key.public_key, public_key.public_key);

        let canonical = canonical_signature_bytes(
            "android_keystore_bridge_smoke",
            &[SignatureField::text("fixture", "synthetic")],
        );
        let signature = store.sign(&handle, &canonical).expect("signature");
        assert_eq!(signature.signature.len(), ED25519_SIGNATURE_LEN);
        signature
            .verify_at(&loaded_public_key, &canonical, 10)
            .expect("signature verifies");

        let mut tampered = canonical.clone();
        tampered.push(1);
        assert_eq!(
            signature
                .verify_at(&loaded_public_key, &tampered, 10)
                .expect_err("tampered canonical bytes fail"),
            CryptoError::SignatureVerificationFailed
        );

        store
            .delete_or_revoke(&handle, 20)
            .expect("delete synthetic key");
        assert_eq!(
            store
                .sign(&handle, &canonical)
                .expect_err("revoked key cannot sign"),
            CryptoError::PrivateKeyRevoked {
                key_id: "signing-key-a".to_owned(),
            }
        );

        let debug = format!("{store:?}");
        assert!(debug.contains("AndroidKeystoreDeviceKeyStore"));
        assert!(debug.contains("alias_prefix_len"));
        assert!(!debug.contains("org.radishlex.test.signing"));
        assert!(!debug.contains("synthetic"));
    }

    #[test]
    fn default_bridge_stays_unavailable_and_does_not_fallback_to_test_memory() {
        let store = AndroidKeystoreDeviceKeyStore::new();
        let status = store.backend_status();
        status.validate().expect("status");
        assert_eq!(
            status.storage_backend,
            DeviceSigningStorageBackend::AndroidKeystoreV1
        );
        assert_eq!(
            status
                .ensure_production_signing_allowed()
                .expect_err("default bridge cannot sign production objects"),
            CryptoError::StorageBackendUnavailable {
                backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            }
        );
        assert!(matches!(
            store.create_signing_key("device-a", "signing-key-a", 10),
            Err(CryptoError::StorageBackendUnavailable { .. })
                | Err(CryptoError::UnsupportedStorageBackend { .. })
        ));
    }

    #[test]
    fn bridge_contract_request_validates_payload_shape_and_redacts_alias() {
        let alias =
            android_keystore_alias("org.radishlex.test.signing", "signing-key-a").expect("alias");
        let request = AndroidKeystoreBridgeRequest::new(
            AndroidKeystoreBridgeOperation::Sign,
            "signing-key-a",
            alias.clone(),
            72,
        )
        .expect("request");
        request.validate().expect("request validates");
        assert_eq!(
            request.contract_version,
            ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION
        );
        assert_eq!(
            request.operation.as_str(),
            AndroidKeystoreBridgeOperation::Sign.as_str()
        );

        let debug = format!("{request:?}");
        assert!(debug.contains("signing-key-a"));
        assert!(debug.contains("alias_len"));
        assert!(!debug.contains("org.radishlex.test.signing"));

        assert_eq!(
            AndroidKeystoreBridgeRequest::new(
                AndroidKeystoreBridgeOperation::Sign,
                "signing-key-a",
                &alias,
                0,
            )
            .expect_err("sign requires bytes"),
            CryptoError::InvalidField {
                field: "canonical_bytes_len",
                message: "sign operation requires non-empty canonical bytes".to_owned(),
            }
        );
        assert_eq!(
            AndroidKeystoreBridgeRequest::new(
                AndroidKeystoreBridgeOperation::LoadPublicKey,
                "signing-key-a",
                &alias,
                1,
            )
            .expect_err("load public key must not carry canonical bytes"),
            CryptoError::InvalidField {
                field: "canonical_bytes_len",
                message: "only sign operation may carry canonical bytes".to_owned(),
            }
        );
    }

    #[test]
    fn bridge_error_codes_map_to_crypto_errors_without_alias() {
        assert_eq!(
            AndroidKeystoreBridgeErrorCode::parse("private_key_locked").expect("error code"),
            AndroidKeystoreBridgeErrorCode::PrivateKeyLocked
        );
        assert_eq!(
            AndroidKeystoreBridgeErrorCode::PrivateKeyLocked.to_crypto_error("signing-key-a"),
            CryptoError::PrivateKeyLocked {
                key_id: "signing-key-a".to_owned(),
            }
        );
        assert_eq!(
            AndroidKeystoreBridgeErrorCode::StorageBackendUnavailable
                .to_crypto_error("signing-key-a"),
            CryptoError::StorageBackendUnavailable {
                backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            }
        );
        assert_eq!(
            AndroidKeystoreBridgeErrorCode::UnsupportedSignatureAlgorithm
                .to_crypto_error("signing-key-a"),
            CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            }
        );
        assert_eq!(
            AndroidKeystoreBridgeErrorCode::parse("raw_alias.org.radishlex")
                .expect_err("unknown bridge code is rejected"),
            CryptoError::InvalidField {
                field: "android_keystore_bridge_error_code",
                message: "unsupported Android Keystore bridge error code raw_alias.org.radishlex"
                    .to_owned(),
            }
        );
    }

    #[test]
    fn bridge_response_validation_rejects_malformed_key_material() {
        assert_eq!(
            validate_android_keystore_public_key("signing-key-a", vec![1u8; 31])
                .expect_err("public key length is fixed"),
            CryptoError::PrivateKeyCorrupted {
                key_id: "signing-key-a".to_owned(),
            }
        );
        assert_eq!(
            validate_android_keystore_signature("signing-key-a", vec![2u8; 63])
                .expect_err("signature length is fixed"),
            CryptoError::PrivateKeyCorrupted {
                key_id: "signing-key-a".to_owned(),
            }
        );
        assert_eq!(
            validate_android_keystore_public_key(
                "signing-key-a",
                vec![1u8; ED25519_PUBLIC_KEY_LEN]
            )
            .expect("public key")
            .len(),
            ED25519_PUBLIC_KEY_LEN
        );
        assert_eq!(
            validate_android_keystore_signature("signing-key-a", vec![2u8; ED25519_SIGNATURE_LEN])
                .expect("signature")
                .len(),
            ED25519_SIGNATURE_LEN
        );
    }

    #[test]
    fn jni_contract_matches_kotlin_facade_names_and_descriptors() {
        assert_eq!(
            ANDROID_KEYSTORE_JNI_BRIDGE_CLASS,
            "org/radishlex/android/keystore/RadishLexAndroidKeystoreJniBridge"
        );
        assert_eq!(
            ANDROID_KEYSTORE_JNI_RESULT_CLASS,
            "org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult"
        );
        assert_eq!(
            android_keystore_jni_method_spec(AndroidKeystoreBridgeOperation::CreateSigningKey),
            AndroidKeystoreJniMethodSpec {
                name: "createSigningKey",
                descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
            }
        );
        assert_eq!(
            android_keystore_jni_method_spec(AndroidKeystoreBridgeOperation::LoadPublicKey),
            AndroidKeystoreJniMethodSpec {
                name: "loadPublicKey",
                descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
            }
        );
        assert_eq!(
            android_keystore_jni_method_spec(AndroidKeystoreBridgeOperation::Sign),
            AndroidKeystoreJniMethodSpec {
                name: "sign",
                descriptor: ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR,
            }
        );
        assert_eq!(
            android_keystore_jni_method_spec(AndroidKeystoreBridgeOperation::DeleteSigningKey),
            AndroidKeystoreJniMethodSpec {
                name: "deleteSigningKey",
                descriptor: ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
            }
        );
        assert_eq!(
            ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR,
            "(ILjava/lang/String;Ljava/lang/String;)Lorg/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult;"
        );
        assert_eq!(
            ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR,
            "(ILjava/lang/String;Ljava/lang/String;[B)Lorg/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeResult;"
        );
        assert_eq!(ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD, "getPublicKey");
        assert_eq!(ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD, "getSignature");
        assert_eq!(ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD, "getErrorCode");
        assert_eq!(ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR, "()[B");
        assert_eq!(
            ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR,
            "()Ljava/lang/String;"
        );
    }

    struct SyntheticAndroidKeystoreBridge {
        status: DevicePrivateKeyStoreStatus,
        keys: Mutex<BTreeMap<String, SigningKey>>,
    }

    impl SyntheticAndroidKeystoreBridge {
        fn new(status: DevicePrivateKeyStoreStatus) -> Self {
            Self {
                status,
                keys: Mutex::new(BTreeMap::new()),
            }
        }
    }

    impl fmt::Debug for SyntheticAndroidKeystoreBridge {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("SyntheticAndroidKeystoreBridge")
        }
    }

    impl AndroidKeystoreBridge for SyntheticAndroidKeystoreBridge {
        fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
            self.status.clone()
        }

        fn create_signing_key(
            &self,
            _signing_key_id: &str,
            alias: &str,
        ) -> Result<Vec<u8>, CryptoError> {
            let signing_key = SigningKey::from_bytes(&synthetic_seed(alias));
            let public_key = signing_key.verifying_key().to_bytes().to_vec();
            self.keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: alias.to_owned(),
                })?
                .insert(alias.to_owned(), signing_key);
            Ok(public_key)
        }

        fn public_key(&self, signing_key_id: &str, alias: &str) -> Result<Vec<u8>, CryptoError> {
            let keys = self
                .keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
            let signing_key =
                keys.get(alias)
                    .ok_or_else(|| CryptoError::PrivateKeyUnavailable {
                        key_id: signing_key_id.to_owned(),
                    })?;
            Ok(signing_key.verifying_key().to_bytes().to_vec())
        }

        fn sign(
            &self,
            signing_key_id: &str,
            alias: &str,
            canonical_bytes: &[u8],
        ) -> Result<Vec<u8>, CryptoError> {
            let keys = self
                .keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
            let signing_key =
                keys.get(alias)
                    .ok_or_else(|| CryptoError::PrivateKeyUnavailable {
                        key_id: signing_key_id.to_owned(),
                    })?;
            Ok(signing_key.sign(canonical_bytes).to_bytes().to_vec())
        }

        fn delete_signing_key(&self, signing_key_id: &str, alias: &str) -> Result<(), CryptoError> {
            self.keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?
                .remove(alias);
            Ok(())
        }
    }

    fn synthetic_seed(alias: &str) -> [u8; ED25519_PUBLIC_KEY_LEN] {
        let mut seed = [17u8; ED25519_PUBLIC_KEY_LEN];
        for (index, byte) in alias.as_bytes().iter().enumerate() {
            seed[index % ED25519_PUBLIC_KEY_LEN] ^= *byte;
        }
        seed
    }
}
