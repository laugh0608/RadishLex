use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    canonical_signature_bytes, AlgorithmId, CiphertextHash, DeviceSignature,
    DeviceSigningPublicKey, EncryptedObjectEnvelope, KeyDescriptor, KeyRole, Nonce, SignatureField,
    SignedSyncObjectManifest, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
    ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256, ED25519_SIGNATURE_LEN, ENVELOPE_SCHEMA_VERSION,
    SIGNATURE_ALGORITHM_ED25519_V1, SIGNATURE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    AssembledSyncObject, HttpSyncRemoteTransport, LatestObjectConflictMetadata,
    PlaintextSyncPayload, RemoteObjectPayload, SyncEnvelopeAssembler, SyncObjectAssemblySpec,
    SyncObjectType, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest,
    SyncRemoteTransport, SyncServerErrorCode,
};
use radishlex_ime_userdb::{
    decode_userdb_sync_objects, NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel,
    SelectionEventDraft, TermSource, TermStatus, UserDb, UserDbDecryptedSyncObject,
    UserDbSyncPayloadObjectType,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DOMAIN_ID: &str = "domain-two-client-go-http";
const DEVICE_A: &str = "device-a";
const DEVICE_B: &str = "device-b";
const SIGNING_KEY_A: &str = "signing-key-a";
const SIGNING_KEY_B: &str = "signing-key-b";
const AGREEMENT_KEY_A: &str = "agreement-key-a";
const AGREEMENT_KEY_B: &str = "agreement-key-b";
const OBJECT_KEY_ID: &str = "object-key-v1";
const BASE_TIMESTAMP_MS: i64 = 1_790_001_000_000;

#[test]
fn two_clients_sync_userdb_payloads_through_go_http_server() {
    let Some(server) = GoSyncServer::try_spawn() else {
        return;
    };
    let transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("http transport");
    let client = SyncRemoteClient::new(transport);

    let sync_master_key = SyncMasterKeyMaterial::new([11u8; 32]).expect("sync master key");
    let object_key = KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, 1).expect("object key");
    let mut signing_store = TestMemoryDeviceKeyStore::new();
    let public_key_a = signing_store
        .insert_signing_key(DEVICE_A, SIGNING_KEY_A, [8u8; 32], BASE_TIMESTAMP_MS)
        .expect("device a signing key");
    let public_key_b = signing_store
        .insert_signing_key(DEVICE_B, SIGNING_KEY_B, [9u8; 32], BASE_TIMESTAMP_MS)
        .expect("device b signing key");

    create_domain(client.transport(), &public_key_a);
    authorize_device_b(client.transport(), &signing_store, &public_key_b);

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
    assert_eq!(device_a_objects.len(), 3);
    for object in &device_a_objects {
        upload_object(&client, object, &signing_store, DEVICE_A, SIGNING_KEY_A);
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
        &signing_store,
        DEVICE_B,
        SIGNING_KEY_B,
    );
    let stale_error = client
        .upload_object_version(DOMAIN_ID, &stale_device_b_object, &stale_manifest)
        .expect_err("stale upload must conflict");
    assert_stale_conflict(stale_error, &device_a_user_terms_hash);

    let downloaded_v1 = download_decrypt_userdb_payloads(&client, &sync_master_key, &object_key, 1);
    let decoded_v1 = decode_userdb_sync_objects(downloaded_v1).expect("decode v1 payloads");
    let summary_b = device_b_db
        .apply_decoded_sync_payload_batch(&decoded_v1)
        .expect("apply v1 remote payloads to device b");
    assert_eq!(summary_b.user_terms_written, 2);
    assert_eq!(summary_b.deleted_terms_written, 1);
    assert_eq!(summary_b.ranker_weights_written, 1);
    assert!(summary_b.blocked_by_tombstone >= 1);

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

    let device_b_merged_objects = assemble_userdb_objects(
        &device_b_db,
        DEVICE_B,
        2,
        Some(1),
        &sync_master_key,
        &object_key,
        None,
    );
    assert_eq!(device_b_merged_objects.len(), 3);
    for object in &device_b_merged_objects {
        let uploaded = upload_object(&client, object, &signing_store, DEVICE_B, SIGNING_KEY_B);
        assert_eq!(uploaded.version, 2);
        assert_eq!(uploaded.base_version, Some(1));
        assert_eq!(uploaded.owner_device_id, DEVICE_B);
    }

    let downloaded_v2 = download_decrypt_userdb_payloads(&client, &sync_master_key, &object_key, 2);
    let decoded_v2 = decode_userdb_sync_objects(downloaded_v2).expect("decode v2 payloads");
    let summary_a = device_a_db
        .apply_decoded_sync_payload_batch(&decoded_v2)
        .expect("apply v2 remote payloads to device a");
    assert!(summary_a.user_terms_written >= 1);
    assert!(summary_a.deleted_terms_written >= 1);
    assert_term_status(
        &device_a_db,
        "clientb",
        "client-b-alpha",
        "client b reading",
        TermStatus::Active,
    );
    assert_term_status(
        &device_a_db,
        "blocked",
        "blocked-alpha",
        "blocked reading",
        TermStatus::Deleted,
    );

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

    let log_text = server.stop();
    assert_runtime_logs_redacted(&log_text);
}

fn create_domain(transport: &HttpSyncRemoteTransport, public_key: &DeviceSigningPublicKey) {
    let body = json!({
        "domain_id": DOMAIN_ID,
        "current_key_epoch": 1,
        "active_key_id": "sync-key-v1",
        "first_device": {
            "device_id": DEVICE_A,
            "signing_public_key_id": SIGNING_KEY_A,
            "signing_public_key": b64(&public_key.public_key),
            "key_agreement_public_key_id": AGREEMENT_KEY_A,
            "key_agreement_public_key": b64(&[0x41u8; 32]),
            "status": "active"
        },
        "created_at_ms": BASE_TIMESTAMP_MS,
        "updated_at_ms": BASE_TIMESTAMP_MS
    });
    let response = send_json(transport, SyncRemoteMethod::Post, "/api/v1/domains", body);
    assert_eq!(
        response.status,
        201,
        "create domain failed: {}",
        String::from_utf8_lossy(&response.body)
    );
    assert_no_plaintext_leak(&response.body);
}

fn authorize_device_b(
    transport: &HttpSyncRemoteTransport,
    signing_store: &TestMemoryDeviceKeyStore,
    public_key_b: &DeviceSigningPublicKey,
) {
    let challenge = b"join-challenge-device-b";
    let join_body = json!({
        "join_request_id": "join-device-b",
        "device_id": DEVICE_B,
        "signing_public_key_id": SIGNING_KEY_B,
        "signing_public_key": b64(&public_key_b.public_key),
        "key_agreement_public_key_id": AGREEMENT_KEY_B,
        "key_agreement_public_key": b64(&[0x42u8; 32]),
        "challenge": b64(challenge),
        "created_at_ms": BASE_TIMESTAMP_MS + 10,
        "expires_at_ms": BASE_TIMESTAMP_MS + 600
    });
    let join_response = send_json(
        transport,
        SyncRemoteMethod::Post,
        "/api/v1/domains/domain-two-client-go-http/join-requests",
        join_body,
    );
    assert_eq!(
        join_response.status,
        201,
        "create join request failed: {}",
        String::from_utf8_lossy(&join_response.body)
    );
    assert_no_plaintext_leak(&join_response.body);

    let wrapped_key = b"encrypted-sync-key-for-device-b";
    let wrapped_key_len = wrapped_key.len();
    let wrapping_key_id = "wrapping-key-device-b";
    let created_at_ms = BASE_TIMESTAMP_MS + 20;
    let authorization_signature = sign_authorization(
        signing_store,
        challenge,
        "123456",
        wrapping_key_id,
        wrapped_key_len,
        created_at_ms,
    );
    let authorization_body = json!({
        "authorization": {
            "authorizer_device_id": DEVICE_A,
            "recipient_device_id": DEVICE_B,
            "recipient_signing_public_key_id": SIGNING_KEY_B,
            "recipient_key_agreement_key_id": AGREEMENT_KEY_B,
            "join_short_code": "123456",
            "key_epoch": 1,
            "created_at_ms": created_at_ms,
            "signature_schema_version": SIGNATURE_SCHEMA_VERSION,
            "signature_algorithm": SIGNATURE_ALGORITHM_ED25519_V1,
            "signature_key_id": SIGNING_KEY_A,
            "signature": b64(&authorization_signature.signature)
        },
        "wrapping": {
            "authorizer_device_id": DEVICE_A,
            "recipient_device_id": DEVICE_B,
            "key_epoch": 1,
            "wrapping_key_id": wrapping_key_id,
            "algorithm": ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256,
            "nonce": b64(b"wrapping-nonce-device-b"),
            "wrapped_key_len": wrapped_key_len,
            "ciphertext_hash": ciphertext_hash(wrapped_key),
            "created_at_ms": created_at_ms,
            "signature": b64(b"synthetic-wrapping-signature")
        },
        "wrapped_key": b64(wrapped_key)
    });
    let authorization_response = send_json(
        transport,
        SyncRemoteMethod::Post,
        "/api/v1/domains/domain-two-client-go-http/join-requests/join-device-b/authorization",
        authorization_body,
    );
    assert_eq!(
        authorization_response.status,
        204,
        "authorize join request failed: {}",
        String::from_utf8_lossy(&authorization_response.body)
    );
}

fn sign_authorization(
    signing_store: &TestMemoryDeviceKeyStore,
    challenge: &[u8],
    join_short_code: &str,
    wrapping_key_id: &str,
    wrapped_key_len: usize,
    created_at_ms: i64,
) -> DeviceSignature {
    let fields = [
        SignatureField::u16("signature_schema_version", SIGNATURE_SCHEMA_VERSION),
        SignatureField::text("signature_algorithm", SIGNATURE_ALGORITHM_ED25519_V1),
        SignatureField::text("signature_key_id", SIGNING_KEY_A),
        SignatureField::text("authorizer_device_id", DEVICE_A),
        SignatureField::text("recipient_device_id", DEVICE_B),
        SignatureField::text("recipient_public_key_id", SIGNING_KEY_B),
        SignatureField::bytes("join_challenge", challenge),
        SignatureField::text("join_short_code", join_short_code),
        SignatureField::u64("key_epoch", 1),
        SignatureField::text("wrapping_key_id", wrapping_key_id),
        SignatureField::usize("encrypted_key_len", wrapped_key_len),
        SignatureField::i64("created_at_ms", created_at_ms),
    ];
    let canonical = canonical_signature_bytes("device_authorization", &fields);
    let handle = signing_store
        .handle(DEVICE_A, SIGNING_KEY_A)
        .expect("device a signing handle");
    signing_store
        .sign(&handle, &canonical)
        .expect("authorization signature")
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
                BASE_TIMESTAMP_MS + 1_000 + version as i64,
            )
            .expect("assembly spec");
            assembler
                .assemble_payload(sync_payload, spec, sync_master_key)
                .expect("assembled payload")
        })
        .collect()
}

fn upload_object(
    client: &SyncRemoteClient<HttpSyncRemoteTransport>,
    object: &AssembledSyncObject,
    signing_store: &TestMemoryDeviceKeyStore,
    signer_device_id: &str,
    signing_key_id: &str,
) -> radishlex_ime_sync::RemoteObjectVersion {
    let manifest = sign_object(object, signing_store, signer_device_id, signing_key_id);
    let response = client
        .upload_object_version(DOMAIN_ID, object, &manifest)
        .expect("upload object");
    assert_no_plaintext_leak(response.ciphertext_hash.as_bytes());
    response
}

fn sign_object(
    object: &AssembledSyncObject,
    signing_store: &TestMemoryDeviceKeyStore,
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
    client: &SyncRemoteClient<HttpSyncRemoteTransport>,
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
    client: &SyncRemoteClient<HttpSyncRemoteTransport>,
    sync_master_key: &SyncMasterKeyMaterial,
    object_key: &KeyDescriptor,
    object_type: UserDbSyncPayloadObjectType,
    version: u64,
) -> UserDbDecryptedSyncObject {
    let remote_payload = client
        .object_payload(DOMAIN_ID, object_id(object_type), version)
        .expect("remote payload");
    assert_ne!(remote_payload.payload, Vec::<u8>::new());
    assert!(
        !String::from_utf8_lossy(&remote_payload.payload).contains("radish-alpha"),
        "remote payload must remain encrypted"
    );
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

fn send_json(
    transport: &HttpSyncRemoteTransport,
    method: SyncRemoteMethod,
    path: &str,
    value: Value,
) -> radishlex_ime_sync::SyncRemoteResponse {
    transport
        .send(SyncRemoteRequest::new(
            method,
            path,
            Some("application/json".to_owned()),
            serde_json::to_vec(&value).expect("json request"),
        ))
        .expect("http response")
}

fn b64(bytes: &[u8]) -> String {
    Base64::encode_string(bytes)
}

fn ciphertext_hash(ciphertext: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ciphertext);
    let digest = hasher.finalize();
    let mut out = String::from("sha256:");
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("write hex");
    }
    out
}

fn assert_runtime_logs_redacted(log_text: &str) {
    for forbidden in [
        "radish-alpha",
        "blocked-alpha",
        "ranker-alpha",
        "client-b-alpha",
        "input_code",
        "reading",
        "plaintext",
    ] {
        assert!(
            !log_text.contains(forbidden),
            "runtime log leaked {forbidden}: {log_text}"
        );
    }
    for required in [
        r#"route="domains.create""#,
        r#"route="join_requests.collection""#,
        r#"route="join_requests.authorize""#,
        r#"route="objects.versions.create""#,
        r#"route="objects.versions.get""#,
        r#"route="objects.versions.payload""#,
        r#"result_code="conflict_stale_base_version""#,
    ] {
        assert!(
            log_text.contains(required),
            "runtime log missing {required}: {log_text}"
        );
    }
}

fn assert_no_plaintext_leak(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for forbidden in [
        "radish-alpha",
        "blocked-alpha",
        "ranker-alpha",
        "client-b-alpha",
        "input_code",
        "reading",
    ] {
        assert!(
            !text.contains(forbidden),
            "response leaked {forbidden}: {text}"
        );
    }
}

struct GoSyncServer {
    child: Option<Child>,
    root: PathBuf,
    base_url: String,
}

impl GoSyncServer {
    fn try_spawn() -> Option<Self> {
        let port = match reserve_loopback_port() {
            Ok(port) => port,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("skipping Go sync server integration: loopback bind denied by sandbox");
                return None;
            }
            Err(error) => panic!("reserve loopback port: {error}"),
        };
        let root = temp_root();
        fs::create_dir_all(root.join("objects")).expect("create temp blob dir");
        let server_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../server/sync-server");
        let binary_path = root.join("radishlex-sync-server");
        let build_status = match Command::new("go")
            .args(["build", "-o"])
            .arg(&binary_path)
            .arg("./cmd/radishlex-sync-server")
            .current_dir(&server_dir)
            .status()
        {
            Ok(status) => status,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                eprintln!("skipping Go sync server integration: go command not found");
                let _ = fs::remove_dir_all(&root);
                return None;
            }
            Err(error) => panic!("build Go sync server: {error}"),
        };
        if !build_status.success() {
            let _ = fs::remove_dir_all(&root);
            panic!("build Go sync server failed: {build_status}");
        }

        let mut child = match Command::new(&binary_path)
            .env("RADISHLEX_SYNC_LISTEN", format!("127.0.0.1:{port}"))
            .env(
                "RADISHLEX_SYNC_METADATA_PATH",
                root.join("sync-server.sqlite"),
            )
            .env("RADISHLEX_SYNC_BLOB_DIR", root.join("objects"))
            .env("RADISHLEX_SYNC_MAX_OBJECT_BYTES", "16777216")
            .env("RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR", "12")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => panic!("spawn Go sync server: {error}"),
        };
        let base_url = format!("http://127.0.0.1:{port}");
        wait_until_ready(&base_url, &mut child);
        Some(Self {
            child: Some(child),
            root,
            base_url,
        })
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    fn stop(mut self) -> String {
        let logs = self.stop_child();
        let _ = fs::remove_dir_all(&self.root);
        logs
    }

    fn stop_child(&mut self) -> String {
        let Some(mut child) = self.child.take() else {
            return String::new();
        };
        let _ = child.kill();
        let _ = child.wait();
        read_child_stderr(&mut child)
    }
}

impl Drop for GoSyncServer {
    fn drop(&mut self) {
        let _ = self.stop_child();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn reserve_loopback_port() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn temp_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "radishlex-userdb-go-http-{}-{nanos}",
        std::process::id()
    ))
}

fn wait_until_ready(base_url: &str, child: &mut Child) {
    let transport =
        HttpSyncRemoteTransport::with_timeout(base_url.to_owned(), Duration::from_millis(250))
            .expect("readiness transport");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll Go sync server") {
            let stderr = read_child_stderr(child);
            panic!("Go sync server exited before readiness: {status}\n{stderr}");
        }
        let response = transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/readiness-domain/state",
            None,
            Vec::new(),
        ));
        match response {
            Ok(_) => return,
            Err(SyncRemoteError::Transport { .. }) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => panic!("Go sync server readiness check failed: {error}"),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = read_child_stderr(child);
            panic!("Go sync server did not become ready before timeout\n{stderr}");
        }
    }
}

fn read_child_stderr(child: &mut Child) -> String {
    let Some(mut stderr) = child.stderr.take() else {
        return String::new();
    };
    let mut output = String::new();
    let _ = stderr.read_to_string(&mut output);
    output
}
