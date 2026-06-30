#![cfg(all(feature = "apple-keychain", target_os = "macos"))]

use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_crypto::{
    canonical_signature_bytes, AppleKeychainDeviceKeyStore, CryptoError, DeviceSigningKeyHandle,
    DeviceSigningStorageBackend, SignatureField,
};

#[test]
#[ignore = "touches the local macOS Keychain; run only after explicit approval"]
fn apple_keychain_smoke_creates_signs_verifies_and_deletes_key() -> Result<(), CryptoError> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_millis() as i64;
    let device_id = format!("radishlex-smoke-device-{}", std::process::id());
    let signing_key_id = format!("radishlex-smoke-signing-key-{now_ms}");

    let cleanup_handle =
        DeviceSigningKeyHandle::apple_keychain(&device_id, &signing_key_id, now_ms)?;
    let cleanup_store = AppleKeychainDeviceKeyStore::new();
    let _ = cleanup_store.delete_or_revoke(&cleanup_handle, now_ms);

    let store = AppleKeychainDeviceKeyStore::new();
    let public_key = store.create_signing_key(&device_id, &signing_key_id, now_ms)?;
    let handle = store.handle(&device_id, &signing_key_id)?;
    assert_eq!(
        handle.storage_backend,
        DeviceSigningStorageBackend::AppleKeychainV1
    );
    assert!(!handle.exportable);
    assert!(!handle.hardware_backed);
    assert!(!handle.user_presence_required);
    assert!(!handle.backup_migratable);

    let loaded_public_key = store.public_key(&handle)?;
    assert_eq!(loaded_public_key.public_key, public_key.public_key);

    let canonical = canonical_signature_bytes(
        "apple_keychain_smoke",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = store.sign(&handle, &canonical)?;
    signature.verify_at(&loaded_public_key, &canonical, now_ms)?;

    store.delete_or_revoke(&handle, now_ms + 1)?;
    assert!(matches!(
        store.sign(&handle, &canonical),
        Err(CryptoError::PrivateKeyRevoked { .. })
    ));

    let fresh_store = AppleKeychainDeviceKeyStore::new();
    assert!(matches!(
        fresh_store.handle(&device_id, &signing_key_id),
        Err(CryptoError::PrivateKeyUnavailable { .. })
    ));
    Ok(())
}
