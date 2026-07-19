use std::time::Duration;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    DeviceSignature, Nonce, RecoveryCode, RecoveryKdfProfile, RecoveryMaterial,
    SignedRecoveryRecordManifest, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
    ED25519_SIGNATURE_LEN, RECOVERY_CODE_SECRET_LEN,
};
use radishlex_ime_sync::{
    HttpSyncRemoteTransport, SyncDevice, SyncDomain, SyncRemoteClient, SyncRemoteError,
    SyncRemoteMethod, SyncRemoteRequest, SyncRemoteTransport, SyncServerErrorCode,
    SyncTrustedDeviceProfile, SyncTrustedDomainState,
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[path = "support/go_sync_server.rs"]
#[allow(dead_code)]
mod go_sync_server;

use go_sync_server::GoSyncServer;

const DOMAIN_ID: &str = "domain-recovery-go-http";
const DEVICE_ID: &str = "device-recovery-a";
const SIGNING_KEY_ID: &str = "signing-key-recovery-a";
const BASE_TIMESTAMP_MS: i64 = 1_790_010_000_000;
const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[test]
fn recovery_rotates_and_decrypts_through_go_http_server() {
    let Some(server) = GoSyncServer::try_spawn() else {
        return;
    };
    let transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("transport")
            .with_device_identity(DEVICE_ID)
            .expect("device identity");
    let client = SyncRemoteClient::new(transport);
    let mut signing_store = TestMemoryDeviceKeyStore::new();
    let public_key = signing_store
        .insert_signing_key(DEVICE_ID, SIGNING_KEY_ID, [8; 32], BASE_TIMESTAMP_MS)
        .expect("signing key");
    create_domain(client.transport(), &public_key.public_key);
    let trusted = SyncTrustedDomainState::new(
        SyncDomain::new(
            DOMAIN_ID,
            1,
            "sync-key-v1",
            BASE_TIMESTAMP_MS,
            BASE_TIMESTAMP_MS,
        )
        .expect("domain"),
        [SyncTrustedDeviceProfile::active(
            SyncDevice::pending(DEVICE_ID, SIGNING_KEY_ID, BASE_TIMESTAMP_MS - 1)
                .expect("pending")
                .activate(BASE_TIMESTAMP_MS)
                .expect("active"),
            public_key,
        )
        .expect("trusted profile")],
    )
    .expect("trusted domain");
    let master_key = SyncMasterKeyMaterial::new([11; 32]).expect("master key");
    let first_code = recovery_code([3; RECOVERY_CODE_SECRET_LEN]);
    let first = RecoveryMaterial::encrypt_sync_master_key(
        "recovery-a",
        "",
        DOMAIN_ID,
        1,
        &RecoveryKdfProfile::argon2id_v1(),
        &first_code,
        vec![1; 16],
        &master_key,
        BASE_TIMESTAMP_MS + 10,
        Nonce::new(vec![2; 24]).expect("nonce"),
    )
    .expect("first recovery");
    let first_manifest = sign_recovery(&signing_store, &first);
    client
        .upload_recovery_record(&trusted, &first, &first_manifest)
        .expect("upload first recovery");
    let downloaded_first = client
        .latest_verified_recovery_record(&trusted, DOMAIN_ID)
        .expect("download first recovery");
    assert_eq!(
        downloaded_first
            .material
            .decrypt_sync_master_key(&first_code)
            .expect("decrypt first")
            .as_bytes(),
        master_key.as_bytes()
    );

    let second_code = recovery_code([4; RECOVERY_CODE_SECRET_LEN]);
    let second = RecoveryMaterial::encrypt_sync_master_key(
        "recovery-b",
        "recovery-a",
        DOMAIN_ID,
        1,
        &RecoveryKdfProfile::argon2id_v1(),
        &second_code,
        vec![5; 16],
        &master_key,
        BASE_TIMESTAMP_MS + 20,
        Nonce::new(vec![6; 24]).expect("nonce"),
    )
    .expect("second recovery");
    let second_manifest = sign_recovery(&signing_store, &second);
    client
        .upload_recovery_record(&trusted, &second, &second_manifest)
        .expect("rotate recovery");
    client
        .upload_recovery_record(&trusted, &second, &second_manifest)
        .expect("idempotent retry");
    let downloaded_second = client
        .latest_verified_recovery_record(&trusted, DOMAIN_ID)
        .expect("download rotated recovery");
    assert_eq!(downloaded_second.material.recovery_id, "recovery-b");
    assert_eq!(
        downloaded_second.material.previous_recovery_id,
        "recovery-a"
    );
    assert_eq!(
        downloaded_second
            .material
            .decrypt_sync_master_key(&second_code)
            .expect("decrypt rotated")
            .as_bytes(),
        master_key.as_bytes()
    );
    assert!(downloaded_second
        .material
        .decrypt_sync_master_key(&first_code)
        .is_err());

    let stale = RecoveryMaterial::encrypt_sync_master_key(
        "recovery-stale",
        "recovery-a",
        DOMAIN_ID,
        1,
        &RecoveryKdfProfile::argon2id_v1(),
        &second_code,
        vec![7; 16],
        &master_key,
        BASE_TIMESTAMP_MS + 30,
        Nonce::new(vec![8; 24]).expect("nonce"),
    )
    .expect("stale recovery");
    let stale_manifest = sign_recovery(&signing_store, &stale);
    let error = client
        .upload_recovery_record(&trusted, &stale, &stale_manifest)
        .expect_err("stale predecessor fails");
    assert!(matches!(
        error,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ConflictRecoveryRecord,
            ..
        }
    ));

    let logs = server.stop();
    for forbidden in [
        format_recovery_code([3; RECOVERY_CODE_SECRET_LEN]),
        format_recovery_code([4; RECOVERY_CODE_SECRET_LEN]),
        Base64::encode_string(master_key.as_bytes()),
    ] {
        assert!(
            !logs.contains(&forbidden),
            "runtime log leaked recovery secret"
        );
    }
}

fn create_domain(transport: &HttpSyncRemoteTransport, signing_public_key: &[u8]) {
    let body = serde_json::to_vec(&json!({
        "domain_id": DOMAIN_ID,
        "current_key_epoch": 1,
        "active_key_id": "sync-key-v1",
        "first_device": {
            "device_id": DEVICE_ID,
            "signing_algorithm": "ed25519-v1",
            "signing_public_key_id": SIGNING_KEY_ID,
            "signing_public_key": Base64::encode_string(signing_public_key),
            "key_agreement_public_key_id": "agreement-key-recovery-a",
            "key_agreement_public_key": Base64::encode_string(&[2; 65]),
            "status": "active"
        },
        "created_at_ms": BASE_TIMESTAMP_MS,
        "updated_at_ms": BASE_TIMESTAMP_MS
    }))
    .expect("domain request");
    let response = transport
        .send(SyncRemoteRequest::new(
            SyncRemoteMethod::Post,
            "/api/v1/domains",
            Some("application/json".to_owned()),
            body,
        ))
        .expect("create domain response");
    assert_eq!(response.status, 201, "create domain failed");
}

fn sign_recovery(
    signing_store: &TestMemoryDeviceKeyStore,
    material: &RecoveryMaterial,
) -> SignedRecoveryRecordManifest {
    let placeholder =
        DeviceSignature::new(SIGNING_KEY_ID, DEVICE_ID, vec![1; ED25519_SIGNATURE_LEN])
            .expect("placeholder");
    let unsigned =
        SignedRecoveryRecordManifest::new(material, placeholder).expect("unsigned manifest");
    let handle = signing_store
        .handle(DEVICE_ID, SIGNING_KEY_ID)
        .expect("handle");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("signature");
    SignedRecoveryRecordManifest::new(material, signature).expect("signed manifest")
}

fn recovery_code(secret: [u8; RECOVERY_CODE_SECRET_LEN]) -> RecoveryCode {
    RecoveryCode::parse(&format_recovery_code(secret)).expect("recovery code")
}

fn format_recovery_code(secret: [u8; RECOVERY_CODE_SECRET_LEN]) -> String {
    let mut encoded = String::with_capacity(32);
    let mut accumulator = 0u16;
    let mut bits = 0u8;
    for byte in secret {
        accumulator = (accumulator << 8) | u16::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            encoded.push(CROCKFORD_ALPHABET[((accumulator >> bits) & 0x1f) as usize] as char);
        }
        accumulator &= if bits == 0 { 0 } else { (1u16 << bits) - 1 };
    }
    let groups = encoded
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).expect("crockford group"));
    let checksum = Sha256::new()
        .chain_update(b"radishlex-recovery-code-checksum-v1")
        .chain_update(secret)
        .finalize()[0]
        & 0x1f;
    format!(
        "RLX1-{}-{}",
        groups.collect::<Vec<_>>().join("-"),
        CROCKFORD_ALPHABET[checksum as usize] as char
    )
}
