use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::model::{validate_non_empty_bytes, validate_required, CryptoError};

use super::{
    DevicePrivateKeyStoreStatus, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    DeviceSigningStorageBackend, DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
};

const DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX: &str = "org.radishlex.sync.signing";

pub struct AndroidKeystoreDeviceKeyStore {
    alias_prefix: String,
    bridge: Arc<dyn AndroidKeystoreBridge>,
    revoked_keys: Mutex<BTreeSet<(String, String)>>,
}

impl AndroidKeystoreDeviceKeyStore {
    pub fn new() -> Self {
        Self::with_bridge(
            DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX.to_owned(),
            Arc::new(UnavailableAndroidKeystoreBridge),
        )
        .expect("default Android Keystore alias prefix is valid")
    }

    pub fn with_alias_prefix(alias_prefix: impl Into<String>) -> Result<Self, CryptoError> {
        Self::with_bridge(
            alias_prefix.into(),
            Arc::new(UnavailableAndroidKeystoreBridge),
        )
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
        let public_key = self
            .bridge
            .create_signing_key(signing_key_id, &self.key_alias(signing_key_id))?;
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
        let _public_key = self
            .bridge
            .public_key(signing_key_id, &self.key_alias(signing_key_id))?;
        DeviceSigningKeyHandle::android_keystore(device_id, signing_key_id, 0)
    }

    pub fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_android_handle(handle)?;
        self.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let public_key = self.bridge.public_key(
            &handle.signing_key_id,
            &self.key_alias(&handle.signing_key_id),
        )?;
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
        let signature = self.bridge.sign(
            &handle.signing_key_id,
            &self.key_alias(&handle.signing_key_id),
            canonical_bytes,
        )?;
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
        let delete_result = self.bridge.delete_signing_key(
            &handle.signing_key_id,
            &self.key_alias(&handle.signing_key_id),
        );
        self.mark_revoked(&handle.device_id, &handle.signing_key_id, revoked_at_ms)?;
        delete_result
    }

    fn key_alias(&self, signing_key_id: &str) -> String {
        format!("{}.{signing_key_id}", self.alias_prefix)
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
