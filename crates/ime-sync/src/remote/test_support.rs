use super::*;
use radishlex_ime_crypto::{
    DeviceSignature, KeyDescriptor, KeyRole, PlaintextPayload, SignedSyncObjectManifest,
    SyncMasterKeyMaterial, TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
};
use serde_json::Value;

pub(crate) fn signed_object() -> (AssembledSyncObject, SignedSyncObjectManifest) {
    let object_key = KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 1).expect("key");
    let sync_master_key = SyncMasterKeyMaterial::new([3u8; 32]).expect("master key");
    let object_key_material = sync_master_key
        .derive_object_key(
            &object_key,
            SyncObjectType::DictionaryUserTerms.to_crypto_object_type(),
            "object-a",
        )
        .expect("object key");
    let payload = PlaintextPayload::new(
        SyncObjectType::DictionaryUserTerms.to_crypto_object_type(),
        br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"term_id":"synthetic-term"}]}"#.to_vec(),
    )
    .expect("payload");
    let envelope = radishlex_ime_crypto::EncryptedObjectEnvelope::encrypt_payload(
        "object-a",
        "device-a",
        &object_key,
        &object_key_material,
        1,
        None,
        payload,
        100,
    )
    .expect("envelope");
    let draft = crate::EncryptedSyncObjectDraft::from_crypto_envelope(&envelope).expect("draft");
    let mut store = TestMemoryDeviceKeyStore::new();
    store
        .insert_signing_key("device-a", "signing-key-a", [8u8; 32], 1)
        .expect("signing handle");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    let empty_signature = DeviceSignature::new(
        "signing-key-a",
        "device-a",
        vec![1u8; ED25519_SIGNATURE_LEN],
    )
    .expect("empty signature");
    let unsigned =
        SignedSyncObjectManifest::new("domain-a", &envelope, empty_signature).expect("manifest");
    let signature = store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("signature");
    let manifest =
        SignedSyncObjectManifest::new("domain-a", &envelope, signature).expect("manifest");

    (
        AssembledSyncObject {
            envelope,
            draft,
            record_count: 1,
        },
        manifest,
    )
}

pub(crate) fn response_for(object: &AssembledSyncObject) -> Value {
    serde_json::json!({
        "domain_id": "domain-a",
        "object_id": object.draft.object_id,
        "object_type": object.draft.object_type.as_str(),
        "version": object.draft.version,
        "base_version": object.draft.base_version.unwrap_or(0),
        "change_sequence": object.draft.version,
        "owner_device_id": object.draft.owner_device_id,
        "key_id": object.draft.key_id,
        "key_epoch": object.draft.key_epoch,
        "algorithm": object.draft.algorithm,
        "nonce": Base64::encode_string(&object.draft.nonce),
        "encrypted_payload_len": object.draft.encrypted_payload_len,
        "ciphertext_hash": object.draft.ciphertext_hash,
        "signature_schema_version": 1,
        "signature_algorithm": "ed25519-v1",
        "signature_key_id": "signing-key-a",
        "signature": Base64::encode_string(&[1u8; ED25519_SIGNATURE_LEN]),
        "server_received_at_ms": 110,
        "client_created_at_ms": object.draft.created_at_ms,
        "client_updated_at_ms": object.draft.updated_at_ms
    })
}
