use super::*;

#[test]
fn key_roles_have_stable_identifiers() {
    assert_eq!(KeyRole::ProfileRoot.as_str(), "profile_root");
    assert_eq!(KeyRole::SyncMaster.as_str(), "sync_master");
    assert_eq!(KeyRole::DeviceKeyPair.as_str(), "device_key_pair");
    assert_eq!(KeyRole::DeviceWrapping.as_str(), "device_wrapping");
    assert_eq!(KeyRole::ObjectKey.as_str(), "object_key");
}

#[test]
fn envelope_validates_required_metadata_and_aad() {
    let envelope = sample_envelope();

    envelope.validate().expect("envelope validates");
    let aad = envelope.associated_data();

    assert_eq!(aad.schema_version, ENVELOPE_SCHEMA_VERSION);
    assert_eq!(aad.object_id, "dictionary-user-terms-device-a");
    assert_eq!(aad.object_type, CryptoObjectType::DictionaryUserTerms);
    assert_eq!(aad.key_id, "object-key-a");
    assert_eq!(aad.key_epoch, 3);
    envelope
        .validate_associated_data(&aad)
        .expect("matching AAD is accepted");
}

#[test]
fn envelope_rejects_non_object_key_roles() {
    let sync_master = KeyDescriptor::new("sync-master-a", KeyRole::SyncMaster, 1).expect("key");

    let error = EncryptedObjectEnvelope::new(
        "dictionary-user-terms-device-a",
        CryptoObjectType::DictionaryUserTerms,
        "device-a",
        &sync_master,
        algorithm(),
        nonce(1),
        1,
        b"ciphertext".to_vec(),
        ciphertext_hash("ciphertext-hash"),
        10,
    )
    .expect_err("sync master key must not encrypt object payload directly");

    assert!(error.to_string().contains("key_role"));
}

#[test]
fn envelope_rejects_plaintext_algorithm_and_empty_nonce() {
    let algorithm = AlgorithmId::new("plaintext").expect_err("plaintext is not AEAD");
    assert!(algorithm.to_string().contains("algorithm"));

    let nonce = Nonce::new(Vec::<u8>::new()).expect_err("empty nonce fails");
    assert!(nonce.to_string().contains("nonce"));
}

#[test]
fn envelope_rejects_invalid_versions_and_empty_ciphertext() {
    let object_key = object_key(3);
    let error = EncryptedObjectEnvelope::new(
        "dictionary-user-terms-device-a",
        CryptoObjectType::DictionaryUserTerms,
        "device-a",
        &object_key,
        algorithm(),
        nonce(1),
        0,
        b"ciphertext".to_vec(),
        ciphertext_hash("ciphertext-hash"),
        10,
    )
    .expect_err("zero version fails");
    assert!(error.to_string().contains("version"));

    let error = EncryptedObjectEnvelope::new(
        "dictionary-user-terms-device-a",
        CryptoObjectType::DictionaryUserTerms,
        "device-a",
        &object_key,
        algorithm(),
        nonce(1),
        1,
        Vec::<u8>::new(),
        ciphertext_hash("ciphertext-hash"),
        10,
    )
    .expect_err("empty ciphertext fails");
    assert!(error.to_string().contains("encrypted_payload"));
}

#[test]
fn envelope_rejects_invalid_base_version_after_update() {
    let envelope = sample_envelope().with_base_version(1);
    envelope.validate().expect("lower base version is valid");

    let envelope = sample_envelope().with_base_version(2);
    let error = envelope.validate().expect_err("base version must be lower");
    assert!(error.to_string().contains("base_version"));
}

#[test]
fn aad_binding_detects_metadata_changes() {
    let envelope = sample_envelope();
    let mut aad = envelope.associated_data();
    aad.version += 1;

    let error = envelope
        .validate_associated_data(&aad)
        .expect_err("changed version must fail AAD check");
    assert_eq!(
        error,
        CryptoError::AssociatedDataMismatch { field: "version" }
    );
}

#[test]
fn ciphertext_hash_is_a_required_semantic_type() {
    let error = CiphertextHash::new("").expect_err("empty hash fails");
    assert!(error.to_string().contains("ciphertext_hash"));

    let hash = CiphertextHash::new("ciphertext-hash").expect("hash");
    assert_eq!(hash.as_str(), "ciphertext-hash");
}

#[test]
fn nonce_tracker_rejects_duplicate_nonce_for_same_key_epoch() {
    let mut tracker = NonceTracker::new();
    let envelope = sample_envelope();
    let duplicate = sample_envelope();

    tracker.observe(&envelope).expect("first nonce use passes");
    let error = tracker
        .observe(&duplicate)
        .expect_err("same key and nonce must fail");

    assert_eq!(
        error,
        CryptoError::DuplicateNonce {
            key_id: "object-key-a".to_owned(),
            key_epoch: 3,
        }
    );
}

#[test]
fn nonce_tracker_allows_same_nonce_after_key_epoch_changes() {
    let mut tracker = NonceTracker::new();
    let current_epoch = sample_envelope();
    let next_epoch = envelope_with_key_epoch(4, nonce(1));

    tracker
        .observe(&current_epoch)
        .expect("first nonce use passes");
    tracker
        .observe(&next_epoch)
        .expect("same nonce with different key epoch passes");
}

#[test]
fn derives_object_key_material_from_sync_master_key() {
    let master = sync_master_key();
    let object_key = object_key(3);
    let first = master
        .derive_object_key(
            &object_key,
            CryptoObjectType::DictionaryUserTerms,
            "dictionary-user-terms-device-a",
        )
        .expect("first object key");
    let second = master
        .derive_object_key(
            &object_key,
            CryptoObjectType::DictionaryDeletedTerms,
            "dictionary-deleted-terms-device-a",
        )
        .expect("second object key");

    assert_ne!(first, second);
}

#[test]
fn encrypts_and_decrypts_synthetic_payload_with_bound_aad() {
    let object_key = object_key(3);
    let object_key_material = object_key_material(&object_key);
    let payload = PlaintextPayload::new(
        CryptoObjectType::DictionaryUserTerms,
        br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input":"luobo","text":"synthetic-term"}]}"#.to_vec(),
    )
    .expect("payload");

    let envelope = EncryptedObjectEnvelope::encrypt_payload_with_nonce(
        "dictionary-user-terms-device-a",
        "device-a",
        &object_key,
        &object_key_material,
        2,
        Some(1),
        payload.clone(),
        10,
        nonce(7),
    )
    .expect("encrypts");

    assert_ne!(envelope.encrypted_payload, payload.bytes);
    assert_eq!(
        envelope.algorithm.as_str(),
        ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256
    );
    assert_eq!(envelope.nonce.as_bytes().len(), XCHACHA20POLY1305_NONCE_LEN);

    let decrypted = envelope
        .decrypt_payload(&object_key_material)
        .expect("decrypts");
    assert_eq!(decrypted, payload);
}

#[test]
fn decrypt_rejects_ciphertext_tampering() {
    let (mut envelope, object_key_material) = encrypted_sample();
    envelope.encrypted_payload[0] ^= 0x01;

    let error = envelope
        .decrypt_payload(&object_key_material)
        .expect_err("ciphertext mutation fails before plaintext is returned");
    assert_eq!(error, CryptoError::CiphertextHashMismatch);
}

#[test]
fn decrypt_rejects_aad_tampering() {
    let (mut envelope, object_key_material) = encrypted_sample();
    envelope.version += 1;

    let error = envelope
        .decrypt_payload(&object_key_material)
        .expect_err("AAD mutation fails before plaintext is returned");
    assert_eq!(error, CryptoError::CiphertextHashMismatch);
}

#[test]
fn decrypt_rejects_wrong_key_material() {
    let (envelope, _) = encrypted_sample();
    let wrong_key = ObjectKeyMaterial::new([9u8; OBJECT_KEY_LEN]).expect("wrong key");

    let error = envelope
        .decrypt_payload(&wrong_key)
        .expect_err("wrong key must not decrypt");
    assert_eq!(error, CryptoError::DecryptionFailed);
}

fn sample_envelope() -> EncryptedObjectEnvelope {
    envelope_with_key_epoch(3, nonce(1))
}

fn envelope_with_key_epoch(key_epoch: u64, nonce: Nonce) -> EncryptedObjectEnvelope {
    let object_key = object_key(key_epoch);
    EncryptedObjectEnvelope::new(
        "dictionary-user-terms-device-a",
        CryptoObjectType::DictionaryUserTerms,
        "device-a",
        &object_key,
        algorithm(),
        nonce,
        2,
        b"ciphertext".to_vec(),
        ciphertext_hash("ciphertext-hash"),
        10,
    )
    .expect("valid envelope")
    .with_base_version(1)
}

fn object_key(key_epoch: u64) -> KeyDescriptor {
    KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, key_epoch).expect("key")
}

fn sync_master_key() -> SyncMasterKeyMaterial {
    SyncMasterKeyMaterial::new([42u8; OBJECT_KEY_LEN]).expect("master key")
}

fn object_key_material(object_key: &KeyDescriptor) -> ObjectKeyMaterial {
    sync_master_key()
        .derive_object_key(
            object_key,
            CryptoObjectType::DictionaryUserTerms,
            "dictionary-user-terms-device-a",
        )
        .expect("object key material")
}

fn algorithm() -> AlgorithmId {
    AlgorithmId::xchacha20poly1305_hkdf_sha256()
}

fn nonce(seed: u8) -> Nonce {
    Nonce::new(vec![seed; 24]).expect("nonce")
}

fn ciphertext_hash(value: &str) -> CiphertextHash {
    CiphertextHash::new(value).expect("ciphertext hash")
}

fn encrypted_sample() -> (EncryptedObjectEnvelope, ObjectKeyMaterial) {
    let object_key = object_key(3);
    let object_key_material = object_key_material(&object_key);
    let payload = PlaintextPayload::new(
        CryptoObjectType::DictionaryUserTerms,
        b"synthetic payload".to_vec(),
    )
    .expect("payload");
    let envelope = EncryptedObjectEnvelope::encrypt_payload_with_nonce(
        "dictionary-user-terms-device-a",
        "device-a",
        &object_key,
        &object_key_material,
        2,
        Some(1),
        payload,
        10,
        nonce(7),
    )
    .expect("encrypts");
    (envelope, object_key_material)
}
