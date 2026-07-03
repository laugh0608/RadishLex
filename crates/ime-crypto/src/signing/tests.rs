use super::*;
use crate::{
    AlgorithmId, CiphertextHash, CryptoObjectType, KeyDescriptor, KeyRole, Nonce,
    XCHACHA20POLY1305_NONCE_LEN,
};

#[test]
fn test_memory_key_store_signs_and_verifies_without_debug_leaks() {
    let mut store = TestMemoryDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate().expect("test backend status");
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::TestMemoryV1
    );
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("test backend cannot sign production objects"),
        CryptoError::BackendCapabilityMismatch {
            backend: DEVICE_KEY_STORE_TEST_MEMORY_V1.to_owned(),
            message: "backend is not eligible for production signing".to_owned(),
        }
    );

    let public_key = store
        .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
        .expect("public key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    assert_eq!(
        handle.storage_backend,
        DeviceSigningStorageBackend::TestMemoryV1
    );
    assert!(handle.exportable);
    assert!(!handle.hardware_backed);
    assert!(!handle.user_presence_required);
    assert!(!handle.backup_migratable);
    assert_eq!(handle.revoked_at_ms, None);

    let canonical =
        canonical_signature_bytes("test_record", &[SignatureField::text("field", "value")]);

    let signature = store.sign(&handle, &canonical).expect("signature");
    signature
        .verify_at(&public_key, &canonical, 10)
        .expect("signature verifies");

    let debug = format!("{signature:?} {store:?}");
    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("070707"));
}

#[test]
fn unavailable_key_store_blocks_production_signing_and_key_operations() {
    let store = UnavailableDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate().expect("unavailable status");
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::Unavailable
    );
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("unavailable backend cannot sign"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        }
    );
    assert_eq!(
        store
            .create_signing_key("device-a", "signing-key-a", 10)
            .expect_err("unavailable backend cannot create signing key"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        }
    );
    assert_eq!(
        store
            .handle("device-a", "signing-key-a")
            .expect_err("unavailable backend has no handle"),
        CryptoError::PrivateKeyUnavailable {
            key_id: "signing-key-a".to_owned(),
        }
    );

    let handle =
        DeviceSigningKeyHandle::test_memory("device-a", "signing-key-a", 10).expect("handle");
    let canonical =
        canonical_signature_bytes("test_record", &[SignatureField::text("field", "value")]);
    assert_eq!(
        store
            .public_key(&handle)
            .expect_err("unavailable backend cannot load public key"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        }
    );
    assert_eq!(
        store
            .sign(&handle, &canonical)
            .expect_err("unavailable backend cannot sign"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        }
    );
    assert_eq!(
        store
            .delete_or_revoke(&handle, 20)
            .expect_err("unavailable backend cannot revoke local key"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_UNAVAILABLE.to_owned(),
        }
    );
}

#[test]
fn test_memory_key_store_revocation_blocks_signing_and_export() {
    let mut store = TestMemoryDeviceKeyStore::new();
    store
        .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
        .expect("public key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    store
        .delete_or_revoke(&handle, 20)
        .expect("revoke test key");

    let public_key = store.public_key(&handle).expect("public key");
    assert_eq!(public_key.revoked_at_ms, Some(20));
    assert!(!public_key.is_active_at(20));

    let canonical =
        canonical_signature_bytes("test_record", &[SignatureField::text("field", "value")]);
    assert_eq!(
        store
            .sign(&handle, &canonical)
            .expect_err("revoked key cannot sign"),
        CryptoError::PrivateKeyRevoked {
            key_id: "signing-key-a".to_owned(),
        }
    );
    assert_eq!(
        store
            .export_private_key_for_tests(&handle)
            .expect_err("revoked key cannot be exported"),
        CryptoError::PrivateKeyRevoked {
            key_id: "signing-key-a".to_owned(),
        }
    );
}

#[test]
fn backend_capabilities_gate_production_signing() {
    let platform_status = DevicePrivateKeyStoreStatus::platform(
        DeviceSigningStorageBackend::AppleKeychainV1,
        true,
        true,
        false,
    )
    .expect("platform status");
    platform_status.validate().expect("valid platform status");
    platform_status
        .ensure_production_signing_allowed()
        .expect("platform backend can sign production objects");
    assert_eq!(
        platform_status.capabilities.storage_backend,
        DeviceSigningStorageBackend::AppleKeychainV1
    );
    assert!(!platform_status.capabilities.exportable);
    assert!(platform_status.capabilities.hardware_backed);
    assert!(platform_status.capabilities.user_presence_required);
    assert!(!platform_status.capabilities.backup_migratable);

    assert_eq!(
        DeviceSigningBackendCapabilities::platform(
            DeviceSigningStorageBackend::TestMemoryV1,
            false,
            false,
            false,
        )
        .expect_err("test backend is not a production platform backend"),
        CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_TEST_MEMORY_V1.to_owned(),
        }
    );

    let capabilities = DeviceSigningBackendCapabilities::platform(
        DeviceSigningStorageBackend::AppleKeychainV1,
        false,
        false,
        false,
    )
    .expect("capabilities");
    assert_eq!(
        DeviceSigningKeyHandle::new(
            "device-a",
            "signing-key-a",
            SignatureAlgorithmId::ed25519_v1(),
            DeviceSigningStorageBackend::AndroidKeystoreV1,
            capabilities,
            10,
        )
        .expect_err("handle backend must match capabilities"),
        CryptoError::BackendCapabilityMismatch {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
            message: "capabilities must describe the handle storage backend".to_owned(),
        }
    );
}

#[test]
fn apple_keychain_capabilities_keep_metadata_but_status_blocks_production() {
    let capabilities = DeviceSigningBackendCapabilities::apple_keychain_v1();
    assert_eq!(
        capabilities.storage_backend,
        DeviceSigningStorageBackend::AppleKeychainV1
    );
    assert!(!capabilities.exportable);
    assert!(!capabilities.hardware_backed);
    assert!(!capabilities.user_presence_required);
    assert!(!capabilities.backup_migratable);
    assert!(capabilities.allows_production_signing());

    let status = DevicePrivateKeyStoreStatus::apple_keychain_v1();
    status.validate().expect("apple keychain status");
    assert!(!status.available);
    assert!(!status.can_create_signing_keys);
    assert!(!status.can_sign);
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("apple keychain smoke blocker keeps backend out of production signing"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
        }
    );

    let handle = DeviceSigningKeyHandle::apple_keychain("device-a", "signing-key-a", 10)
        .expect("apple keychain handle");
    let debug = format!("{handle:?}");
    assert!(debug.contains("AppleKeychainV1"));
    assert!(!debug.contains("private"));
    assert!(!debug.contains("seed"));
}

#[test]
fn android_keystore_capabilities_keep_metadata_but_status_blocks_production() {
    let capabilities = DeviceSigningBackendCapabilities::android_keystore_v1();
    assert_eq!(
        capabilities.storage_backend,
        DeviceSigningStorageBackend::AndroidKeystoreV1
    );
    assert!(!capabilities.exportable);
    assert!(!capabilities.hardware_backed);
    assert!(!capabilities.user_presence_required);
    assert!(!capabilities.backup_migratable);
    assert!(capabilities.allows_production_signing());

    let status = DevicePrivateKeyStoreStatus::android_keystore_v1();
    status.validate().expect("android keystore status");
    assert!(!status.available);
    assert!(!status.can_create_signing_keys);
    assert!(!status.can_sign);
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("android keystore smoke blocker keeps backend out of production signing"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
        }
    );

    let handle = DeviceSigningKeyHandle::android_keystore("device-a", "signing-key-a", 10)
        .expect("android keystore handle");
    let debug = format!("{handle:?}");
    assert!(debug.contains("AndroidKeystoreV1"));
    assert!(!debug.contains("private"));
    assert!(!debug.contains("seed"));
}

#[cfg(feature = "apple-keychain")]
#[test]
fn apple_keychain_store_status_blocks_production_until_platform_strategy_is_resolved() {
    let store = AppleKeychainDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate().expect("apple keychain store status");
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::AppleKeychainV1
    );
    assert!(!status.available);
    assert!(!status.can_create_signing_keys);
    assert!(!status.can_sign);
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("apple keychain status must block production signing"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
        }
    );
}

#[cfg(feature = "android-keystore")]
#[test]
fn android_keystore_store_status_blocks_production_until_platform_bridge_is_verified() {
    let store = AndroidKeystoreDeviceKeyStore::new();
    let status = store.backend_status();
    status.validate().expect("android keystore store status");
    assert_eq!(
        status.storage_backend,
        DeviceSigningStorageBackend::AndroidKeystoreV1
    );
    assert!(!status.available);
    assert!(!status.can_create_signing_keys);
    assert!(!status.can_sign);
    assert_eq!(
        status
            .ensure_production_signing_allowed()
            .expect_err("android keystore status must block production signing"),
        CryptoError::StorageBackendUnavailable {
            backend: DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1.to_owned(),
        }
    );

    let handle = DeviceSigningKeyHandle::android_keystore("device-a", "signing-key-a", 10)
        .expect("android keystore handle");
    assert!(matches!(
        store.sign(&handle, b"synthetic-canonical-bytes"),
        Err(CryptoError::StorageBackendUnavailable { .. })
            | Err(CryptoError::UnsupportedStorageBackend { .. })
    ));
    let debug = format!("{store:?}");
    assert!(debug.contains("AndroidKeystoreDeviceKeyStore"));
    assert!(!debug.contains("org.radishlex.sync.signing"));
    assert!(!debug.contains("synthetic-canonical-bytes"));
}

#[test]
fn signature_verification_rejects_wrong_key_and_revoked_key() {
    let mut store = TestMemoryDeviceKeyStore::new();
    let active_key = store
        .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
        .expect("active key");
    let wrong_key = store
        .insert_signing_key("device-b", "signing-key-b", [8u8; 32], 10)
        .expect("wrong key");
    let revoked_key = DeviceSigningPublicKey::new(
        "device-a",
        "signing-key-a",
        SignatureAlgorithmId::ed25519_v1(),
        active_key.public_key.clone(),
        10,
        Some(12),
    )
    .expect("revoked key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    let canonical =
        canonical_signature_bytes("test_record", &[SignatureField::text("field", "value")]);
    let signature = store.sign(&handle, &canonical).expect("signature");

    assert_eq!(
        signature
            .verify_at(&wrong_key, &canonical, 10)
            .expect_err("wrong public key fails"),
        CryptoError::SignatureVerificationFailed
    );
    assert_eq!(
        signature
            .verify_at(&revoked_key, &canonical, 12)
            .expect_err("revoked key fails"),
        CryptoError::SignatureVerificationFailed
    );
}

#[test]
fn sync_object_manifest_signature_covers_envelope_metadata() {
    let (store, public_key, handle) = signing_fixture();
    let envelope = sample_envelope();
    let signature = store
        .sign(
            &handle,
            &canonical_signature_bytes(
                "sync_object_manifest",
                &SignedSyncObjectManifest::new(
                    "domain-a",
                    &envelope,
                    empty_signature("signing-key-a", "device-a"),
                )
                .expect("unsigned-shaped manifest")
                .signature_fields(),
            ),
        )
        .expect("signature");
    let manifest =
        SignedSyncObjectManifest::new("domain-a", &envelope, signature).expect("manifest");

    manifest.verify(&public_key).expect("manifest verifies");

    let mut tampered = manifest.clone();
    tampered.ciphertext_hash = "changed-hash".to_owned();
    assert_eq!(
        tampered
            .verify(&public_key)
            .expect_err("tampered manifest fails"),
        CryptoError::SignatureVerificationFailed
    );
}

#[test]
fn recovery_record_signature_covers_kdf_and_ciphertext_metadata() {
    let (store, public_key, handle) = signing_fixture();
    let material = sample_recovery_material();
    let signature = store
        .sign(
            &handle,
            &canonical_signature_bytes(
                "recovery_record",
                &SignedRecoveryRecordManifest::new(
                    &material,
                    empty_signature("signing-key-a", "device-a"),
                )
                .expect("unsigned-shaped recovery record")
                .signature_fields(),
            ),
        )
        .expect("signature");
    let manifest =
        SignedRecoveryRecordManifest::new(&material, signature).expect("recovery manifest");

    manifest.verify(&public_key).expect("manifest verifies");

    let mut tampered = manifest.clone();
    tampered.memory_kib += 1;
    assert_eq!(
        tampered
            .verify(&public_key)
            .expect_err("tampered recovery record fails"),
        CryptoError::SignatureVerificationFailed
    );
}

fn signing_fixture() -> (
    TestMemoryDeviceKeyStore,
    DeviceSigningPublicKey,
    DeviceSigningKeyHandle,
) {
    let mut store = TestMemoryDeviceKeyStore::new();
    let public_key = store
        .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
        .expect("public key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    (store, public_key, handle)
}

fn empty_signature(key_id: &str, device_id: &str) -> DeviceSignature {
    DeviceSignature::new(key_id, device_id, vec![1u8; ED25519_SIGNATURE_LEN])
        .expect("empty placeholder signature")
}

fn sample_envelope() -> EncryptedObjectEnvelope {
    EncryptedObjectEnvelope::new(
        "dictionary-user-terms-device-a",
        CryptoObjectType::DictionaryUserTerms,
        "device-a",
        &KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 3).expect("key"),
        AlgorithmId::xchacha20poly1305_hkdf_sha256(),
        Nonce::new(vec![1u8; XCHACHA20POLY1305_NONCE_LEN]).expect("nonce"),
        2,
        b"encrypted-payload".to_vec(),
        CiphertextHash::new("ciphertext-hash").expect("hash"),
        20,
    )
    .expect("envelope")
    .with_base_version(1)
}

fn sample_recovery_material() -> RecoveryMaterial {
    RecoveryMaterial::new(
        "recovery-a",
        "domain-a",
        3,
        "argon2id-v1",
        1,
        b"0123456789abcdef",
        65_536,
        3,
        4,
        32,
        AlgorithmId::xchacha20poly1305_hkdf_sha256(),
        Nonce::new(vec![2u8; XCHACHA20POLY1305_NONCE_LEN]).expect("nonce"),
        b"encrypted-recovery-key",
        20,
        20,
    )
    .expect("recovery material")
}
