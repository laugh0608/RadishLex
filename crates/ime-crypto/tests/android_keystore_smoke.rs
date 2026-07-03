#![cfg(feature = "android-keystore")]

#[cfg(target_os = "android")]
use radishlex_ime_crypto::DeviceSigningKeyHandle;
use radishlex_ime_crypto::{
    AndroidKeystoreDeviceKeyStore, CryptoError, DeviceSigningStorageBackend,
    DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
};

#[test]
#[ignore = "touches Android Keystore once the platform bridge exists; run only after explicit approval"]
fn android_keystore_smoke_creates_signs_verifies_and_deletes_key() -> Result<(), CryptoError> {
    let store = AndroidKeystoreDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate()?;
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::AndroidKeystoreV1
    );
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("real smoke must not flip production readiness automatically"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
        }
    );

    #[cfg(target_os = "android")]
    {
        use std::time::{SystemTime, UNIX_EPOCH};

        use radishlex_ime_crypto::{canonical_signature_bytes, SignatureField};

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_millis() as i64;
        let device_id = format!("radishlex-android-smoke-device-{}", std::process::id());
        let signing_key_id = format!("radishlex-android-smoke-signing-key-{now_ms}");
        let _cleanup =
            AndroidKeystoreSmokeCleanup::new(device_id.clone(), signing_key_id.clone(), now_ms);

        let cleanup_handle =
            DeviceSigningKeyHandle::android_keystore(&device_id, &signing_key_id, now_ms)?;
        let cleanup_store = AndroidKeystoreDeviceKeyStore::new();
        let _ = cleanup_store.delete_or_revoke(&cleanup_handle, now_ms);

        let public_key = store.create_signing_key(&device_id, &signing_key_id, now_ms)?;
        let handle = store.handle(&device_id, &signing_key_id)?;
        assert_eq!(
            handle.storage_backend,
            DeviceSigningStorageBackend::AndroidKeystoreV1
        );
        assert!(!handle.exportable);
        assert!(!handle.hardware_backed);
        assert!(!handle.user_presence_required);
        assert!(!handle.backup_migratable);

        let loaded_public_key = store.public_key(&handle)?;
        assert_eq!(loaded_public_key.public_key, public_key.public_key);

        let canonical = canonical_signature_bytes(
            "android_keystore_smoke",
            &[SignatureField::text("smoke", "synthetic")],
        );
        let signature = store.sign(&handle, &canonical)?;
        signature.verify_at(&loaded_public_key, &canonical, now_ms)?;

        store.delete_or_revoke(&handle, now_ms + 1)?;
        assert!(matches!(
            store.sign(&handle, &canonical),
            Err(CryptoError::PrivateKeyRevoked { .. })
        ));

        let fresh_store = AndroidKeystoreDeviceKeyStore::new();
        assert!(matches!(
            fresh_store.handle(&device_id, &signing_key_id),
            Err(CryptoError::PrivateKeyUnavailable { .. })
        ));
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        Ok(())
    }
}

#[cfg(target_os = "android")]
struct AndroidKeystoreSmokeCleanup {
    device_id: String,
    signing_key_id: String,
    created_at_ms: i64,
}

#[cfg(target_os = "android")]
impl AndroidKeystoreSmokeCleanup {
    fn new(device_id: String, signing_key_id: String, created_at_ms: i64) -> Self {
        Self {
            device_id,
            signing_key_id,
            created_at_ms,
        }
    }
}

#[cfg(target_os = "android")]
impl Drop for AndroidKeystoreSmokeCleanup {
    fn drop(&mut self) {
        if let Ok(handle) = DeviceSigningKeyHandle::android_keystore(
            &self.device_id,
            &self.signing_key_id,
            self.created_at_ms,
        ) {
            let store = AndroidKeystoreDeviceKeyStore::new();
            let _ = store.delete_or_revoke(&handle, self.created_at_ms + 1);
        }
    }
}
