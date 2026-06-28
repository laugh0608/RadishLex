use super::*;
use crate::{
    AlgorithmId, CiphertextHash, CryptoObjectType, KeyDescriptor, KeyRole, Nonce,
    XCHACHA20POLY1305_NONCE_LEN,
};

#[test]
fn test_memory_key_store_signs_and_verifies_without_debug_leaks() {
    let mut store = TestMemoryDeviceKeyStore::new();
    let public_key = store
        .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
        .expect("public key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
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
