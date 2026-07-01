#![cfg(feature = "android-keystore")]

use radishlex_ime_crypto::{
    AndroidKeystoreDeviceKeyStore, CryptoError, DeviceSigningStorageBackend,
    DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
};

#[test]
#[ignore = "touches Android Keystore once the platform bridge exists; run only after explicit approval"]
fn android_keystore_smoke_requires_verified_platform_bridge_before_signing(
) -> Result<(), CryptoError> {
    let store = AndroidKeystoreDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate()?;
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::AndroidKeystoreV1
    );

    #[cfg(target_os = "android")]
    {
        status.ensure_production_signing_allowed()?;
        unreachable!("android-keystore-v1 smoke must create, sign, verify, and delete a real key");
    }

    #[cfg(not(target_os = "android"))]
    {
        assert_eq!(
            status
                .ensure_production_signing_allowed()
                .expect_err("host smoke cannot claim Android Keystore production readiness"),
            CryptoError::StorageBackendUnavailable {
                backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            }
        );
        Ok(())
    }
}
