use radishlex_ime_crypto::{KeyDescriptor, KeyRole, SyncMasterKeyMaterial};
use radishlex_ime_sync::{
    PlaintextSyncPayload, SyncEnvelopeAssembler, SyncObjectAssemblySpec, SyncObjectType,
};
use radishlex_ime_userdb::{
    NegativeFeedbackDraft, NegativeFeedbackReason, SelectionEventDraft, TermSource, UserDb,
    UserDbSyncPayloadObjectType,
};

const OWNER_DEVICE_ID: &str = "device-a";
const BASE_TIMESTAMP_MS: i64 = 1_790_000_000_000;

#[test]
fn userdb_p2_payloads_encrypt_into_sync_drafts_without_plaintext_leak() {
    let mut db = UserDb::open_in_memory().expect("userdb");
    db.add_term(
        "radish",
        "radish-alpha",
        Some("rad ish"),
        TermSource::ManualAdd,
    )
    .expect("active term");
    db.add_term("lex", "lex-beta", None, TermSource::ManualAdd)
        .expect("active term without reading");
    db.add_term(
        "deleted",
        "deleted-gamma",
        Some("del eta"),
        TermSource::ManualAdd,
    )
    .expect("term before delete");
    db.delete_term("deleted", "deleted-gamma", Some("del eta"))
        .expect("deleted term");
    db.record_selection(
        SelectionEventDraft::new("session-sensitive", "rank", "ranker-delta", 0, 2)
            .with_reading("ran ker")
            .with_context_kind("chat"),
    )
    .expect("ranker selection summary");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new(
            "rank",
            "ranker-delta",
            NegativeFeedbackReason::ManualSuppress,
        )
        .with_reading("ran ker")
        .with_context_kind("chat"),
    )
    .expect("ranker negative summary");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("p2 plaintext payloads")
        .collect();
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.object_type)
            .collect::<Vec<_>>(),
        vec![
            UserDbSyncPayloadObjectType::DictionaryUserTerms,
            UserDbSyncPayloadObjectType::RankerWeights,
            UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
        ]
    );

    let object_key = KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 3).expect("key");
    let sync_master_key = SyncMasterKeyMaterial::new([7u8; 32]).expect("sync master key");
    let mut assembler = SyncEnvelopeAssembler::new();

    for payload in payloads {
        let sync_payload = PlaintextSyncPayload::new(
            sync_object_type(payload.object_type),
            payload.record_count,
            payload.bytes.clone(),
        )
        .expect("sync plaintext payload");
        let spec = SyncObjectAssemblySpec::new(
            object_id(payload.object_type),
            OWNER_DEVICE_ID,
            object_key.clone(),
            version_for(payload.object_type),
            None,
            BASE_TIMESTAMP_MS + version_for(payload.object_type) as i64,
        )
        .expect("assembly spec");
        let assembled = assembler
            .assemble_payload(sync_payload, spec, &sync_master_key)
            .expect("assembled sync object");
        let envelope = &assembled.envelope;
        let draft = &assembled.draft;

        assert_eq!(draft.object_type.as_str(), payload.object_type.as_str());
        assert_eq!(draft.owner_device_id, OWNER_DEVICE_ID);
        assert_eq!(draft.key_id, object_key.key_id);
        assert_eq!(draft.key_epoch, object_key.key_epoch);
        assert_eq!(draft.version, version_for(payload.object_type));
        assert_eq!(
            draft.encrypted_payload_len,
            envelope.encrypted_payload.len()
        );
        assert_eq!(draft.ciphertext_hash, envelope.ciphertext_hash.as_str());
        assert_eq!(draft.nonce.as_slice(), envelope.nonce.as_bytes());
        assert_eq!(assembled.record_count, payload.record_count);

        assert_ne!(envelope.encrypted_payload, payload.bytes);
        let object_key_material = sync_master_key
            .derive_object_key(&object_key, envelope.object_type, &envelope.object_id)
            .expect("object key material");
        let decrypted = envelope
            .decrypt_payload(&object_key_material)
            .expect("decrypt envelope");
        assert_eq!(decrypted.object_type.as_str(), payload.object_type.as_str());
        assert_eq!(decrypted.bytes, payload.bytes);

        let draft_debug = format!("{draft:?}");
        assert!(!draft_debug.contains("radish-alpha"));
        assert!(!draft_debug.contains("lex-beta"));
        assert!(!draft_debug.contains("deleted-gamma"));
        assert!(!draft_debug.contains("rad ish"));
        assert!(!draft_debug.contains("del eta"));
        assert!(!draft_debug.contains("ranker-delta"));
        assert!(!draft_debug.contains("session-sensitive"));
        assert!(!draft_debug.contains("manual_suppress"));
    }
}

fn sync_object_type(object_type: UserDbSyncPayloadObjectType) -> SyncObjectType {
    match object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => SyncObjectType::DictionaryUserTerms,
        UserDbSyncPayloadObjectType::RankerWeights => SyncObjectType::RankerWeights,
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => {
            SyncObjectType::DictionaryDeletedTerms
        }
    }
}

fn object_id(object_type: UserDbSyncPayloadObjectType) -> &'static str {
    match object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => "dictionary-user-terms-device-a",
        UserDbSyncPayloadObjectType::RankerWeights => "ranker-weights-device-a",
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => "dictionary-deleted-terms-device-a",
    }
}

fn version_for(object_type: UserDbSyncPayloadObjectType) -> u64 {
    match object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => 1,
        UserDbSyncPayloadObjectType::RankerWeights => 2,
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => 3,
    }
}
