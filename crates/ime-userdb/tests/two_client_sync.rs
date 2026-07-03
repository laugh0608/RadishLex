use std::cell::RefCell;
use std::collections::BTreeMap;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    AlgorithmId, CiphertextHash, DeviceSignature, EncryptedObjectEnvelope, KeyDescriptor, KeyRole,
    Nonce, SignedSyncObjectManifest, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
    ED25519_SIGNATURE_LEN, ENVELOPE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    AssembledSyncObject, LatestObjectConflictMetadata, PlaintextSyncPayload, RemoteObjectPayload,
    SyncEnvelopeAssembler, SyncObjectAssemblySpec, SyncObjectType, SyncRemoteClient,
    SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteResponse, SyncRemoteTransport,
    SyncServerErrorCode,
};
use radishlex_ime_userdb::{
    decode_userdb_sync_objects, NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel,
    SelectionEventDraft, TermSource, TermStatus, UserDb, UserDbDecryptedSyncObject,
    UserDbSyncPayloadObjectType,
};
use serde_json::{json, Map, Value};

const DOMAIN_ID: &str = "domain-a";
const DEVICE_A: &str = "device-a";
const DEVICE_B: &str = "device-b";
const SIGNING_KEY_A: &str = "signing-key-a";
const SIGNING_KEY_B: &str = "signing-key-b";
const OBJECT_KEY_ID: &str = "object-key-v1";
const BASE_TIMESTAMP_MS: i64 = 1_790_000_100_000;

#[test]
fn two_clients_sync_encrypted_userdb_payloads_through_remote_boundary() {
    let sync_master_key = SyncMasterKeyMaterial::new([11u8; 32]).expect("sync master key");
    let object_key = KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, 1).expect("object key");
    let mut signing_store = signing_store();
    let remote = InMemoryObjectRemote::default();
    let client = SyncRemoteClient::new(&remote);

    let mut device_a_db = UserDb::open_in_memory().expect("device a db");
    seed_device_a_userdb(&mut device_a_db);
    let device_a_objects = assemble_userdb_objects(
        &device_a_db,
        DEVICE_A,
        1,
        None,
        &sync_master_key,
        &object_key,
        None,
    );
    for object in &device_a_objects {
        upload_object(&client, object, &mut signing_store, DEVICE_A, SIGNING_KEY_A);
    }
    let device_a_user_terms_hash = device_a_objects
        .iter()
        .find(|object| {
            object.envelope.object_id == object_id(UserDbSyncPayloadObjectType::DictionaryUserTerms)
        })
        .expect("device a user terms object")
        .envelope
        .ciphertext_hash
        .as_str()
        .to_owned();

    let mut device_b_db = UserDb::open_in_memory().expect("device b db");
    seed_device_b_local_state(&mut device_b_db);
    let stale_device_b_object = assemble_userdb_objects(
        &device_b_db,
        DEVICE_B,
        2,
        None,
        &sync_master_key,
        &object_key,
        Some(UserDbSyncPayloadObjectType::DictionaryUserTerms),
    )
    .pop()
    .expect("stale device b object");
    let stale_manifest = sign_object(
        &stale_device_b_object,
        &mut signing_store,
        DEVICE_B,
        SIGNING_KEY_B,
    );
    let stale_error = client
        .upload_object_version(DOMAIN_ID, &stale_device_b_object, &stale_manifest)
        .expect_err("stale upload must conflict");
    assert_stale_conflict(stale_error, &device_a_user_terms_hash);

    let downloaded = download_decrypt_userdb_payloads(&client, &sync_master_key, &object_key, 1);
    let decoded = decode_userdb_sync_objects(downloaded).expect("decode payloads");
    let summary = device_b_db
        .apply_decoded_sync_payload_batch(&decoded)
        .expect("apply remote payloads");
    assert_eq!(summary.user_terms_written, 2);
    assert_eq!(summary.deleted_terms_written, 1);
    assert_eq!(summary.ranker_weights_written, 1);
    assert!(summary.blocked_by_tombstone >= 1);

    assert_term_status(
        &device_b_db,
        "radish",
        "radish-alpha",
        "ra dish",
        TermStatus::Active,
    );
    assert_term_status(
        &device_b_db,
        "blocked",
        "blocked-alpha",
        "blocked reading",
        TermStatus::Deleted,
    );
    assert_term_status(
        &device_b_db,
        "deleted",
        "deleted-alpha",
        "deleted reading",
        TermStatus::Deleted,
    );
    assert_term_status(
        &device_b_db,
        "rank",
        "ranker-alpha",
        "ranker reading",
        TermStatus::Suppressed,
    );
    assert!(device_b_db
        .ranker_weight("rank", "ranker-alpha", Some("ranker reading"), "chat")
        .expect("ranker weight")
        .is_some());

    let device_b_merged_object = assemble_userdb_objects(
        &device_b_db,
        DEVICE_B,
        2,
        Some(1),
        &sync_master_key,
        &object_key,
        Some(UserDbSyncPayloadObjectType::DictionaryUserTerms),
    )
    .pop()
    .expect("merged device b object");
    let uploaded = upload_object(
        &client,
        &device_b_merged_object,
        &mut signing_store,
        DEVICE_B,
        SIGNING_KEY_B,
    );
    assert_eq!(uploaded.version, 2);
    assert_eq!(uploaded.base_version, Some(1));
    assert_eq!(uploaded.owner_device_id, DEVICE_B);

    let latest_user_terms = download_decrypt_userdb_payload(
        &client,
        &sync_master_key,
        &object_key,
        UserDbSyncPayloadObjectType::DictionaryUserTerms,
        2,
    );
    let latest_text = String::from_utf8(latest_user_terms.bytes).expect("utf8 payload");
    assert!(latest_text.contains("client-b-alpha"));
    assert!(latest_text.contains("radish-alpha"));
    assert!(!latest_text.contains("blocked-alpha"));
}

fn seed_device_a_userdb(db: &mut UserDb) {
    db.add_term(
        "radish",
        "radish-alpha",
        Some("ra dish"),
        TermSource::ManualAdd,
    )
    .expect("manual term");
    db.record_selection(
        SelectionEventDraft::new("session-blocked", "blocked", "blocked-alpha", 0, 2)
            .with_reading("blocked reading")
            .with_context_kind("chat")
            .with_privacy(PrivacyLevel::P1LocalOnly),
    )
    .expect("blocked selection");
    db.record_selection(
        SelectionEventDraft::new("session-ranker", "rank", "ranker-alpha", 0, 2)
            .with_reading("ranker reading")
            .with_context_kind("chat")
            .with_privacy(PrivacyLevel::P1LocalOnly),
    )
    .expect("ranker selection");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new(
            "rank",
            "ranker-alpha",
            NegativeFeedbackReason::ManualSuppress,
        )
        .with_reading("ranker reading")
        .with_context_kind("chat")
        .with_privacy(PrivacyLevel::P1LocalOnly),
    )
    .expect("ranker feedback");
    db.add_term(
        "deleted",
        "deleted-alpha",
        Some("deleted reading"),
        TermSource::ManualAdd,
    )
    .expect("term before delete");
    db.delete_term("deleted", "deleted-alpha", Some("deleted reading"))
        .expect("delete term");
}

fn seed_device_b_local_state(db: &mut UserDb) {
    db.add_term(
        "blocked",
        "blocked-alpha",
        Some("blocked reading"),
        TermSource::ManualAdd,
    )
    .expect("local blocked term");
    db.delete_term("blocked", "blocked-alpha", Some("blocked reading"))
        .expect("local tombstone");
    db.add_term(
        "clientb",
        "client-b-alpha",
        Some("client b reading"),
        TermSource::ManualAdd,
    )
    .expect("local client b term");
}

fn assemble_userdb_objects(
    db: &UserDb,
    owner_device_id: &str,
    version: u64,
    base_version: Option<u64>,
    sync_master_key: &SyncMasterKeyMaterial,
    object_key: &KeyDescriptor,
    only_type: Option<UserDbSyncPayloadObjectType>,
) -> Vec<AssembledSyncObject> {
    let mut assembler = SyncEnvelopeAssembler::new();
    db.p2_plaintext_payloads()
        .expect("p2 payloads")
        .filter(|payload| match only_type {
            Some(object_type) => object_type == payload.object_type,
            None => true,
        })
        .map(|payload| {
            let sync_payload = PlaintextSyncPayload::new(
                sync_object_type(payload.object_type),
                payload.record_count,
                payload.bytes,
            )
            .expect("sync payload");
            let spec = SyncObjectAssemblySpec::new(
                object_id(payload.object_type),
                owner_device_id,
                object_key.clone(),
                version,
                base_version,
                BASE_TIMESTAMP_MS + version as i64,
            )
            .expect("assembly spec");
            assembler
                .assemble_payload(sync_payload, spec, sync_master_key)
                .expect("assembled payload")
        })
        .collect()
}

fn upload_object(
    client: &SyncRemoteClient<&InMemoryObjectRemote>,
    object: &AssembledSyncObject,
    signing_store: &mut TestMemoryDeviceKeyStore,
    signer_device_id: &str,
    signing_key_id: &str,
) -> radishlex_ime_sync::RemoteObjectVersion {
    let manifest = sign_object(object, signing_store, signer_device_id, signing_key_id);
    client
        .upload_object_version(DOMAIN_ID, object, &manifest)
        .expect("upload object")
}

fn sign_object(
    object: &AssembledSyncObject,
    signing_store: &mut TestMemoryDeviceKeyStore,
    signer_device_id: &str,
    signing_key_id: &str,
) -> SignedSyncObjectManifest {
    let handle = signing_store
        .handle(signer_device_id, signing_key_id)
        .expect("signing handle");
    let placeholder = DeviceSignature::new(
        signing_key_id,
        signer_device_id,
        vec![1u8; ED25519_SIGNATURE_LEN],
    )
    .expect("placeholder signature");
    let unsigned = SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, placeholder)
        .expect("unsigned manifest");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("signature");
    SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, signature).expect("manifest")
}

fn download_decrypt_userdb_payloads(
    client: &SyncRemoteClient<&InMemoryObjectRemote>,
    sync_master_key: &SyncMasterKeyMaterial,
    object_key: &KeyDescriptor,
    version: u64,
) -> Vec<UserDbDecryptedSyncObject> {
    [
        UserDbSyncPayloadObjectType::DictionaryUserTerms,
        UserDbSyncPayloadObjectType::RankerWeights,
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
    ]
    .into_iter()
    .map(|object_type| {
        download_decrypt_userdb_payload(client, sync_master_key, object_key, object_type, version)
    })
    .collect()
}

fn download_decrypt_userdb_payload(
    client: &SyncRemoteClient<&InMemoryObjectRemote>,
    sync_master_key: &SyncMasterKeyMaterial,
    object_key: &KeyDescriptor,
    object_type: UserDbSyncPayloadObjectType,
    version: u64,
) -> UserDbDecryptedSyncObject {
    let remote_payload = client
        .object_payload(DOMAIN_ID, object_id(object_type), version)
        .expect("remote payload");
    let envelope = envelope_from_remote(remote_payload);
    let object_key_material = sync_master_key
        .derive_object_key(object_key, envelope.object_type, &envelope.object_id)
        .expect("object key material");
    let plaintext = envelope
        .decrypt_payload(&object_key_material)
        .expect("decrypt payload");
    assert_eq!(
        plaintext.object_type,
        sync_object_type(object_type).to_crypto_object_type()
    );
    UserDbDecryptedSyncObject::new(object_type, envelope.key_epoch, plaintext.bytes)
        .expect("decrypted userdb payload")
}

fn envelope_from_remote(remote: RemoteObjectPayload) -> EncryptedObjectEnvelope {
    EncryptedObjectEnvelope {
        schema_version: ENVELOPE_SCHEMA_VERSION,
        object_id: remote.object.object_id,
        object_type: remote.object.object_type.to_crypto_object_type(),
        owner_device_id: remote.object.owner_device_id,
        key_id: remote.object.key_id,
        key_epoch: remote.object.key_epoch,
        algorithm: AlgorithmId::new(remote.object.algorithm).expect("algorithm"),
        nonce: Nonce::new(remote.object.nonce).expect("nonce"),
        version: remote.object.version,
        base_version: remote.object.base_version,
        encrypted_payload: remote.payload,
        ciphertext_hash: CiphertextHash::new(remote.object.ciphertext_hash).expect("hash"),
        created_at_ms: remote.object.client_created_at_ms,
        updated_at_ms: remote.object.client_updated_at_ms,
    }
}

fn assert_stale_conflict(error: SyncRemoteError, expected_latest_hash: &str) {
    match error {
        SyncRemoteError::Server {
            status,
            code,
            latest,
            ..
        } => {
            assert_eq!(status, 409);
            assert_eq!(code, SyncServerErrorCode::ConflictStaleBaseVersion);
            assert_eq!(
                latest,
                Some(LatestObjectConflictMetadata {
                    version: 1,
                    ciphertext_hash: Some(expected_latest_hash.to_owned()),
                })
            );
        }
        other => panic!("unexpected stale error: {other:?}"),
    }
}

fn assert_term_status(
    db: &UserDb,
    input_code: &str,
    text: &str,
    reading: &str,
    status: TermStatus,
) {
    let term = db
        .fetch_term(input_code, text, reading)
        .expect("fetch term")
        .expect("term exists");
    assert_eq!(term.status, status);
}

fn signing_store() -> TestMemoryDeviceKeyStore {
    let mut store = TestMemoryDeviceKeyStore::new();
    store
        .insert_signing_key(DEVICE_A, SIGNING_KEY_A, [8u8; 32], BASE_TIMESTAMP_MS)
        .expect("device a signing key");
    store
        .insert_signing_key(DEVICE_B, SIGNING_KEY_B, [9u8; 32], BASE_TIMESTAMP_MS)
        .expect("device b signing key");
    store
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
        UserDbSyncPayloadObjectType::DictionaryUserTerms => "dictionary-user-terms",
        UserDbSyncPayloadObjectType::RankerWeights => "ranker-weights",
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => "dictionary-deleted-terms",
    }
}

#[derive(Default)]
struct InMemoryObjectRemote {
    objects: RefCell<BTreeMap<String, Vec<StoredObject>>>,
}

#[derive(Clone)]
struct StoredObject {
    metadata: Value,
    payload: Vec<u8>,
    version: u64,
}

impl SyncRemoteTransport for &InMemoryObjectRemote {
    fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError> {
        match parse_route(request.path())? {
            ObjectRoute::Versions { object_id } if request.method() == SyncRemoteMethod::Post => {
                self.upload_object(object_id, request.body())
            }
            ObjectRoute::Version { object_id, version }
                if request.method() == SyncRemoteMethod::Get =>
            {
                self.object_metadata(&object_id, version)
            }
            ObjectRoute::Payload { object_id, version }
                if request.method() == SyncRemoteMethod::Get =>
            {
                self.object_payload(&object_id, version)
            }
            _ => Err(SyncRemoteError::InvalidRequest {
                message: "unsupported remote test route".to_owned(),
            }),
        }
    }
}

impl InMemoryObjectRemote {
    fn upload_object(
        &self,
        object_id: String,
        body: &[u8],
    ) -> Result<SyncRemoteResponse, SyncRemoteError> {
        let mut request = serde_json::from_slice::<Map<String, Value>>(body)
            .map_err(|error| invalid_response(error.to_string()))?;
        let payload = decode_required_base64(&request, "payload")?;
        request.remove("payload");
        let version = required_u64(&request, "version")?;
        let base_version = required_u64(&request, "base_version")?;
        let ciphertext_hash = required_string(&request, "ciphertext_hash")?;

        let mut objects = self.objects.borrow_mut();
        let versions = objects.entry(object_id.clone()).or_default();
        if let Some(latest) = versions.last() {
            let latest_hash = required_string(
                latest.metadata.as_object().expect("metadata object"),
                "ciphertext_hash",
            )?;
            if base_version < latest.version {
                return server_error(
                    409,
                    SyncServerErrorCode::ConflictStaleBaseVersion,
                    "base version is stale",
                    Some(latest.version),
                    Some(&latest_hash),
                );
            }
            if version == latest.version && ciphertext_hash != latest_hash {
                return server_error(
                    409,
                    SyncServerErrorCode::ConflictObjectVersion,
                    "object version already exists with a different ciphertext hash",
                    Some(latest.version),
                    Some(&latest_hash),
                );
            }
            if version != latest.version + 1 || base_version != latest.version {
                return server_error(
                    400,
                    SyncServerErrorCode::InvalidRequest,
                    "version must advance from latest remote version",
                    Some(latest.version),
                    Some(&latest_hash),
                );
            }
        } else if version != 1 || base_version != 0 {
            return server_error(
                400,
                SyncServerErrorCode::InvalidRequest,
                "first version must use version 1 and base version 0",
                None,
                None,
            );
        }

        let mut metadata = request;
        metadata.insert("domain_id".to_owned(), Value::String(DOMAIN_ID.to_owned()));
        metadata.insert("object_id".to_owned(), Value::String(object_id));
        metadata.insert("server_received_at_ms".to_owned(), json!(BASE_TIMESTAMP_MS));
        let metadata = Value::Object(metadata);
        versions.push(StoredObject {
            metadata: metadata.clone(),
            payload,
            version,
        });
        SyncRemoteResponse::json(201, &metadata)
    }

    fn object_metadata(
        &self,
        object_id: &str,
        version: u64,
    ) -> Result<SyncRemoteResponse, SyncRemoteError> {
        let object = self
            .stored_object(object_id, version)
            .ok_or_else(not_found)?;
        SyncRemoteResponse::json(200, &object.metadata)
    }

    fn object_payload(
        &self,
        object_id: &str,
        version: u64,
    ) -> Result<SyncRemoteResponse, SyncRemoteError> {
        let object = self
            .stored_object(object_id, version)
            .ok_or_else(not_found)?;
        Ok(SyncRemoteResponse::new(
            200,
            Some("application/octet-stream".to_owned()),
            object.payload.clone(),
        ))
    }

    fn stored_object(&self, object_id: &str, version: u64) -> Option<StoredObject> {
        self.objects
            .borrow()
            .get(object_id)?
            .iter()
            .find(|object| object.version == version)
            .cloned()
    }
}

enum ObjectRoute {
    Versions { object_id: String },
    Version { object_id: String, version: u64 },
    Payload { object_id: String, version: u64 },
}

fn parse_route(path: &str) -> Result<ObjectRoute, SyncRemoteError> {
    let parts = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 7
        || parts[0] != "api"
        || parts[1] != "v1"
        || parts[2] != "domains"
        || parts[3] != DOMAIN_ID
        || parts[4] != "objects"
        || parts[6] != "versions"
    {
        return Err(SyncRemoteError::InvalidRequest {
            message: "unsupported remote test path".to_owned(),
        });
    }

    let object_id = parts[5].to_owned();
    match parts.as_slice() {
        [_, _, _, _, _, _, _] => Ok(ObjectRoute::Versions { object_id }),
        [_, _, _, _, _, _, _, version] => Ok(ObjectRoute::Version {
            object_id,
            version: parse_version(version)?,
        }),
        [_, _, _, _, _, _, _, version, "payload"] => Ok(ObjectRoute::Payload {
            object_id,
            version: parse_version(version)?,
        }),
        _ => Err(SyncRemoteError::InvalidRequest {
            message: "unsupported remote test path".to_owned(),
        }),
    }
}

fn parse_version(value: &str) -> Result<u64, SyncRemoteError> {
    value
        .parse::<u64>()
        .map_err(|error| invalid_response(error.to_string()))
}

fn required_u64(map: &Map<String, Value>, field: &'static str) -> Result<u64, SyncRemoteError> {
    map.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_response(format!("{field} must be u64")))
}

fn required_string(
    map: &Map<String, Value>,
    field: &'static str,
) -> Result<String, SyncRemoteError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_response(format!("{field} must be string")))
}

fn decode_required_base64(
    map: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<u8>, SyncRemoteError> {
    let value = required_string(map, field)?;
    Base64::decode_vec(&value).map_err(|error| invalid_response(error.to_string()))
}

fn server_error(
    status: u16,
    code: SyncServerErrorCode,
    message: &str,
    latest_version: Option<u64>,
    latest_ciphertext_hash: Option<&str>,
) -> Result<SyncRemoteResponse, SyncRemoteError> {
    SyncRemoteResponse::json(
        status,
        &json!({
            "error_code": code.as_str(),
            "message": message,
            "retryable": false,
            "server_time_ms": BASE_TIMESTAMP_MS,
            "latest_version": latest_version,
            "latest_ciphertext_hash": latest_ciphertext_hash
        }),
    )
}

fn not_found() -> SyncRemoteError {
    SyncRemoteError::Server {
        status: 404,
        code: SyncServerErrorCode::NotFound,
        message: "object not found".to_owned(),
        retryable: false,
        server_time_ms: Some(BASE_TIMESTAMP_MS),
        latest: None,
    }
}

fn invalid_response(message: impl Into<String>) -> SyncRemoteError {
    SyncRemoteError::InvalidResponse {
        message: message.into(),
    }
}
