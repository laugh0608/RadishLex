#![cfg(all(feature = "apple-keychain", target_os = "macos"))]

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_crypto::{
    canonical_signature_bytes, AppleKeychainDeviceKeyStore, AppleKeychainP256DeviceKeyStore,
    AppleSecureEnclaveP256DeviceKeyStore, CryptoError, DeviceSigningKeyHandle,
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
    let _cleanup = KeychainSmokeCleanup::new(device_id.clone(), signing_key_id.clone(), now_ms);

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

#[test]
#[ignore = "touches the local Secure Enclave and Keychain; run only after explicit approval"]
fn apple_secure_enclave_p256_smoke_creates_reloads_cross_verifies_proves_non_exportable_and_deletes_key(
) -> Result<(), CryptoError> {
    assert_eq!(
        std::env::var("RADISHLEX_RUN_APPLE_SECURE_ENCLAVE_P256_SMOKE").as_deref(),
        Ok("1"),
        "set RADISHLEX_RUN_APPLE_SECURE_ENCLAVE_P256_SMOKE=1 only after explicit approval"
    );
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_millis() as i64;
    let device_id = format!(
        "radishlex-secure-enclave-smoke-device-{}",
        std::process::id()
    );
    let signing_key_id = format!("radishlex-secure-enclave-smoke-key-{now_ms}");
    let _cleanup =
        SecureEnclaveSmokeCleanup::new(device_id.clone(), signing_key_id.clone(), now_ms);

    let cleanup_handle =
        DeviceSigningKeyHandle::apple_secure_enclave_p256(&device_id, &signing_key_id, now_ms)?;
    let cleanup_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let _ = cleanup_store.delete_or_revoke(&cleanup_handle, now_ms);

    let store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let public_key = store.create_signing_key(&device_id, &signing_key_id, now_ms)?;
    let reloaded_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let handle = reloaded_store.handle(&device_id, &signing_key_id)?;
    assert_eq!(
        handle.storage_backend,
        DeviceSigningStorageBackend::AppleSecureEnclaveP256V1
    );
    assert!(!handle.exportable);
    assert!(handle.hardware_backed);
    assert!(!handle.user_presence_required);
    assert!(!handle.backup_migratable);

    let loaded_public_key = reloaded_store.public_key(&handle)?;
    assert_eq!(loaded_public_key.public_key, public_key.public_key);
    let canonical = canonical_signature_bytes(
        "apple_secure_enclave_p256_smoke",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = reloaded_store.sign(&handle, &canonical)?;
    signature.verify_at(&loaded_public_key, &canonical, now_ms)?;
    verify_p256_signature_with_go(
        &loaded_public_key.public_key,
        &signature.signature,
        &canonical,
    )?;
    reloaded_store.verify_private_key_non_exportable(&handle)?;

    reloaded_store.delete_or_revoke(&handle, now_ms + 1)?;
    assert!(matches!(
        reloaded_store.sign(&handle, &canonical),
        Err(CryptoError::PrivateKeyRevoked { .. })
    ));
    let fresh_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    assert!(matches!(
        fresh_store.handle(&device_id, &signing_key_id),
        Err(CryptoError::PrivateKeyUnavailable { .. })
    ));
    Ok(())
}

#[test]
#[ignore = "touches the local macOS Keychain; run only after explicit approval"]
fn apple_keychain_p256_smoke_creates_reloads_cross_verifies_and_deletes_key(
) -> Result<(), CryptoError> {
    assert_eq!(
        std::env::var("RADISHLEX_RUN_APPLE_KEYCHAIN_P256_SMOKE").as_deref(),
        Ok("1"),
        "set RADISHLEX_RUN_APPLE_KEYCHAIN_P256_SMOKE=1 only after explicit Keychain approval"
    );
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_millis() as i64;
    let device_id = format!("radishlex-p256-smoke-device-{}", std::process::id());
    let signing_key_id = format!("radishlex-p256-smoke-signing-key-{now_ms}");
    let _cleanup = P256KeychainSmokeCleanup::new(device_id.clone(), signing_key_id.clone(), now_ms);

    let cleanup_handle =
        DeviceSigningKeyHandle::apple_keychain_p256(&device_id, &signing_key_id, now_ms)?;
    let cleanup_store = AppleKeychainP256DeviceKeyStore::new();
    let _ = cleanup_store.delete_or_revoke(&cleanup_handle, now_ms);

    let store = AppleKeychainP256DeviceKeyStore::new();
    let public_key = store.create_signing_key(&device_id, &signing_key_id, now_ms)?;
    let reloaded_store = AppleKeychainP256DeviceKeyStore::new();
    let handle = reloaded_store.handle(&device_id, &signing_key_id)?;
    assert_eq!(
        handle.storage_backend,
        DeviceSigningStorageBackend::AppleKeychainP256V1
    );
    assert!(handle.exportable);
    assert!(!handle.hardware_backed);
    assert!(!handle.user_presence_required);
    assert!(!handle.backup_migratable);

    let loaded_public_key = reloaded_store.public_key(&handle)?;
    assert_eq!(loaded_public_key.public_key, public_key.public_key);

    let canonical = canonical_signature_bytes(
        "apple_keychain_p256_smoke",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = reloaded_store.sign(&handle, &canonical)?;
    signature.verify_at(&loaded_public_key, &canonical, now_ms)?;
    verify_p256_signature_with_go(
        &loaded_public_key.public_key,
        &signature.signature,
        &canonical,
    )?;

    reloaded_store.delete_or_revoke(&handle, now_ms + 1)?;
    assert!(matches!(
        reloaded_store.sign(&handle, &canonical),
        Err(CryptoError::PrivateKeyRevoked { .. })
    ));
    let fresh_store = AppleKeychainP256DeviceKeyStore::new();
    assert!(matches!(
        fresh_store.handle(&device_id, &signing_key_id),
        Err(CryptoError::PrivateKeyUnavailable { .. })
    ));
    Ok(())
}

fn verify_p256_signature_with_go(
    public_key: &[u8],
    signature: &[u8],
    canonical: &[u8],
) -> Result<(), CryptoError> {
    let server_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../server/sync-server");
    let status = Command::new("go")
        .args([
            "test",
            "./internal/storage",
            "-run",
            "^TestExternalP256SignatureSmoke$",
            "-count=1",
        ])
        .current_dir(server_dir)
        .env("GOCACHE", "/tmp/radishlex-apple-p256-go-cache")
        .env("RADISHLEX_GO_P256_SMOKE", "1")
        .env("RADISHLEX_GO_P256_PUBLIC_KEY_HEX", hex(public_key))
        .env("RADISHLEX_GO_P256_SIGNATURE_HEX", hex(signature))
        .env("RADISHLEX_GO_P256_CANONICAL_HEX", hex(canonical))
        .status()
        .map_err(|_| CryptoError::SignatureVerificationFailed)?;
    if !status.success() {
        return Err(CryptoError::SignatureVerificationFailed);
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

struct KeychainSmokeCleanup {
    device_id: String,
    signing_key_id: String,
    created_at_ms: i64,
}

impl KeychainSmokeCleanup {
    fn new(device_id: String, signing_key_id: String, created_at_ms: i64) -> Self {
        Self {
            device_id,
            signing_key_id,
            created_at_ms,
        }
    }
}

impl Drop for KeychainSmokeCleanup {
    fn drop(&mut self) {
        if let Ok(handle) = DeviceSigningKeyHandle::apple_keychain(
            &self.device_id,
            &self.signing_key_id,
            self.created_at_ms,
        ) {
            let store = AppleKeychainDeviceKeyStore::new();
            let _ = store.delete_or_revoke(&handle, self.created_at_ms + 1);
        }
    }
}

struct P256KeychainSmokeCleanup {
    device_id: String,
    signing_key_id: String,
    created_at_ms: i64,
}

impl P256KeychainSmokeCleanup {
    fn new(device_id: String, signing_key_id: String, created_at_ms: i64) -> Self {
        Self {
            device_id,
            signing_key_id,
            created_at_ms,
        }
    }
}

impl Drop for P256KeychainSmokeCleanup {
    fn drop(&mut self) {
        if let Ok(handle) = DeviceSigningKeyHandle::apple_keychain_p256(
            &self.device_id,
            &self.signing_key_id,
            self.created_at_ms,
        ) {
            let store = AppleKeychainP256DeviceKeyStore::new();
            let _ = store.delete_or_revoke(&handle, self.created_at_ms + 1);
        }
    }
}

struct SecureEnclaveSmokeCleanup {
    device_id: String,
    signing_key_id: String,
    created_at_ms: i64,
}

impl SecureEnclaveSmokeCleanup {
    fn new(device_id: String, signing_key_id: String, created_at_ms: i64) -> Self {
        Self {
            device_id,
            signing_key_id,
            created_at_ms,
        }
    }
}

impl Drop for SecureEnclaveSmokeCleanup {
    fn drop(&mut self) {
        if let Ok(handle) = DeviceSigningKeyHandle::apple_secure_enclave_p256(
            &self.device_id,
            &self.signing_key_id,
            self.created_at_ms,
        ) {
            let store = AppleSecureEnclaveP256DeviceKeyStore::new();
            let _ = store.delete_or_revoke(&handle, self.created_at_ms + 1);
        }
    }
}
