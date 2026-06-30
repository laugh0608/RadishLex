use std::fs;
use std::io::{self, Read};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    DeviceSignature, DeviceSigningPublicKey, KeyDescriptor, KeyRole, SignedSyncObjectManifest,
    SyncMasterKeyMaterial, TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
};
use radishlex_ime_sync::{
    AssembledSyncObject, HttpSyncRemoteTransport, PlaintextSyncPayload, SyncEnvelopeAssembler,
    SyncObjectAssemblySpec, SyncObjectType, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod,
    SyncRemoteRequest, SyncRemoteTransport, SyncServerErrorCode,
};

const DOMAIN_ID: &str = "domain-rust-go-http";
const DEVICE_ID: &str = "device-a";
const SIGNING_KEY_ID: &str = "signing-key-a";
const OBJECT_ID: &str = "dictionary-user-terms";
const OBJECT_KEY_ID: &str = "object-key-v1";

#[test]
fn http_transport_round_trips_encrypted_object_through_go_sync_server() {
    let Some(server) = GoSyncServer::try_spawn() else {
        return;
    };
    let transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("http transport");
    let mut signing_store = TestMemoryDeviceKeyStore::new();
    let public_key = signing_store
        .insert_signing_key(DEVICE_ID, SIGNING_KEY_ID, [8u8; 32], 100)
        .expect("test signing public key");
    create_domain(&transport, &public_key);

    let client = SyncRemoteClient::new(transport);
    let version_1 = assemble_object(
        1,
        None,
        200,
        br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"term_id":"synthetic-term-a","text":"radish-alpha"}]}"#,
    );
    let manifest_1 = sign_object(&version_1, &signing_store);

    let uploaded = client
        .upload_object_version(DOMAIN_ID, &version_1, &manifest_1)
        .expect("upload object v1");
    assert_eq!(uploaded.domain_id, DOMAIN_ID);
    assert_eq!(uploaded.object_id, OBJECT_ID);
    assert_eq!(uploaded.object_type, SyncObjectType::DictionaryUserTerms);
    assert_eq!(uploaded.version, 1);
    assert_eq!(uploaded.base_version, None);
    assert_eq!(uploaded.owner_device_id, DEVICE_ID);
    assert_eq!(uploaded.key_id, OBJECT_KEY_ID);
    assert_eq!(uploaded.ciphertext_hash, version_1.draft.ciphertext_hash);

    let remote_payload = client
        .object_payload(DOMAIN_ID, OBJECT_ID, 1)
        .expect("download encrypted payload");
    assert_eq!(remote_payload.object.version, 1);
    assert_eq!(
        remote_payload.object.encrypted_payload_len,
        version_1.envelope.encrypted_payload.len()
    );
    assert_eq!(
        remote_payload.object.ciphertext_hash,
        version_1.draft.ciphertext_hash
    );
    assert_eq!(
        remote_payload.payload, version_1.envelope.encrypted_payload,
        "server must return the encrypted bytes unchanged"
    );
    assert!(
        !String::from_utf8_lossy(&remote_payload.payload).contains("radish-alpha"),
        "downloaded payload must stay encrypted"
    );

    let stale_version_2 = assemble_object(
        2,
        None,
        300,
        br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"term_id":"synthetic-term-b","text":"radish-beta"}]}"#,
    );
    let stale_manifest = sign_object(&stale_version_2, &signing_store);
    let error = client
        .upload_object_version(DOMAIN_ID, &stale_version_2, &stale_manifest)
        .expect_err("stale base version should conflict");
    match error {
        SyncRemoteError::Server {
            status,
            code,
            latest,
            ..
        } => {
            assert_eq!(status, 409);
            assert_eq!(code, SyncServerErrorCode::ConflictStaleBaseVersion);
            let latest = latest.expect("latest conflict metadata");
            assert_eq!(latest.version, 1);
            assert_eq!(
                latest.ciphertext_hash.as_deref(),
                Some(version_1.draft.ciphertext_hash.as_str())
            );
        }
        other => panic!("expected stale conflict from Go server, got {other:?}"),
    }
}

fn create_domain(transport: &HttpSyncRemoteTransport, public_key: &DeviceSigningPublicKey) {
    let body = serde_json::json!({
        "domain_id": DOMAIN_ID,
        "current_key_epoch": 1,
        "active_key_id": "sync-key-a",
        "first_device": {
            "device_id": DEVICE_ID,
            "signing_public_key_id": SIGNING_KEY_ID,
            "signing_public_key": b64(&public_key.public_key),
            "key_agreement_public_key_id": "agreement-key-a",
            "key_agreement_public_key": b64(&[0x42u8; 32]),
            "status": "active"
        },
        "created_at_ms": 100,
        "updated_at_ms": 100
    });
    let response = transport
        .send(SyncRemoteRequest::new(
            SyncRemoteMethod::Post,
            "/api/v1/domains",
            Some("application/json".to_owned()),
            serde_json::to_vec(&body).expect("domain request JSON"),
        ))
        .expect("create domain response");
    assert_eq!(
        response.status,
        201,
        "create domain failed: {}",
        String::from_utf8_lossy(&response.body)
    );
    let response_text = String::from_utf8_lossy(&response.body);
    assert!(!response_text.contains("radish-alpha"));
    assert!(!response_text.contains("input_code"));
    assert!(!response_text.contains("reading"));
}

fn assemble_object(
    version: u64,
    base_version: Option<u64>,
    timestamp_ms: i64,
    plaintext: &[u8],
) -> AssembledSyncObject {
    let mut assembler = SyncEnvelopeAssembler::new();
    let payload =
        PlaintextSyncPayload::new(SyncObjectType::DictionaryUserTerms, 1, plaintext.to_vec())
            .expect("plaintext sync payload");
    let spec = SyncObjectAssemblySpec::new(
        OBJECT_ID,
        DEVICE_ID,
        object_key_descriptor(),
        version,
        base_version,
        timestamp_ms,
    )
    .expect("object assembly spec");
    let sync_master_key = SyncMasterKeyMaterial::new([11u8; 32]).expect("sync master key");
    assembler
        .assemble_payload(payload, spec, &sync_master_key)
        .expect("assembled sync object")
}

fn object_key_descriptor() -> KeyDescriptor {
    KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, 1).expect("object key descriptor")
}

fn sign_object(
    object: &AssembledSyncObject,
    signing_store: &TestMemoryDeviceKeyStore,
) -> SignedSyncObjectManifest {
    let handle = signing_store
        .handle(DEVICE_ID, SIGNING_KEY_ID)
        .expect("signing handle");
    let placeholder =
        DeviceSignature::new(SIGNING_KEY_ID, DEVICE_ID, vec![1u8; ED25519_SIGNATURE_LEN])
            .expect("placeholder signature");
    let unsigned =
        SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, placeholder).expect("manifest");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("signature");
    SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, signature).expect("signed manifest")
}

fn b64(bytes: &[u8]) -> String {
    Base64::encode_string(bytes)
}

struct GoSyncServer {
    child: Child,
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
        let mut child = match Command::new("go")
            .args(["run", "./cmd/radishlex-sync-server"])
            .current_dir(&server_dir)
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
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                eprintln!("skipping Go sync server integration: go command not found");
                let _ = fs::remove_dir_all(&root);
                return None;
            }
            Err(error) => panic!("spawn Go sync server: {error}"),
        };
        let base_url = format!("http://127.0.0.1:{port}");
        wait_until_ready(&base_url, &mut child);
        Some(Self {
            child,
            root,
            base_url,
        })
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }
}

impl Drop for GoSyncServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
        "radishlex-go-sync-http-{}-{nanos}",
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
