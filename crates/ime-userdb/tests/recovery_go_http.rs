use std::time::Duration;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    DeviceKeyAgreementPublicKey, DeviceSignature, KeyDescriptor, KeyRole, Nonce,
    RecoveredDeviceActivationManifest, RecoveryCode, RecoveryKdfProfile, RecoveryMaterial,
    SignedRecoveredDeviceActivation, SignedRecoveryRecordManifest, SignedRecoveryRecordRevocation,
    SyncMasterKeyMaterial, TestMemoryDeviceKeyStore, WrappedEpochMaterial, ED25519_SIGNATURE_LEN,
    RECOVERY_CODE_SECRET_LEN,
};
use radishlex_ime_sync::{
    verify_lifecycle_snapshot, HttpSyncRemoteTransport, SignedEpochDistribution, SyncDevice,
    SyncDomain, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest,
    SyncRemoteTransport, SyncServerErrorCode, SyncTrustedDeviceProfile, SyncTrustedDomainState,
};
use radishlex_ime_userdb::UserDb;
use serde_json::json;
use sha2::{Digest, Sha256};

#[path = "support/go_sync_server.rs"]
#[allow(dead_code)]
mod go_sync_server;

#[path = "support/p256_agreement_backend.rs"]
#[allow(dead_code)]
mod p256_agreement_backend;

use go_sync_server::GoSyncServer;
use p256_agreement_backend::agreement_public_key;

const DOMAIN_ID: &str = "domain-recovery-go-http";
const DEVICE_ID: &str = "device-recovery-a";
const SIGNING_KEY_ID: &str = "signing-key-recovery-a";
const RECOVERED_DEVICE_ID: &str = "device-recovery-b";
const RECOVERED_SIGNING_KEY_ID: &str = "signing-key-recovery-b";
const AGREEMENT_KEY_ID: &str = "agreement-key-recovery-a";
const RECOVERED_AGREEMENT_KEY_ID: &str = "agreement-key-recovery-b";
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
    let anchor_public_key = public_key.clone();
    let agreement_key = DeviceKeyAgreementPublicKey::p256(
        DEVICE_ID,
        AGREEMENT_KEY_ID,
        agreement_public_key(1),
        BASE_TIMESTAMP_MS,
        None,
    )
    .expect("agreement key");
    create_domain(
        client.transport(),
        &public_key.public_key,
        &agreement_key.public_key,
    );
    let trusted = SyncTrustedDomainState::new(
        SyncDomain::new(
            DOMAIN_ID,
            1,
            "sync-key-v1",
            BASE_TIMESTAMP_MS,
            BASE_TIMESTAMP_MS,
        )
        .expect("domain"),
        [SyncTrustedDeviceProfile::active_with_key_agreement(
            SyncDevice::pending(DEVICE_ID, SIGNING_KEY_ID, BASE_TIMESTAMP_MS - 1)
                .expect("pending")
                .activate(BASE_TIMESTAMP_MS)
                .expect("active"),
            public_key,
            agreement_key.clone(),
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

    let mut recovered_signing_store = TestMemoryDeviceKeyStore::new();
    let recovered_signing_key = recovered_signing_store
        .insert_signing_key(
            RECOVERED_DEVICE_ID,
            RECOVERED_SIGNING_KEY_ID,
            [12; 32],
            BASE_TIMESTAMP_MS + 40,
        )
        .expect("recovered signing key");
    let recovered_agreement_key = DeviceKeyAgreementPublicKey::p256(
        RECOVERED_DEVICE_ID,
        RECOVERED_AGREEMENT_KEY_ID,
        agreement_public_key(2),
        BASE_TIMESTAMP_MS + 40,
        None,
    )
    .expect("recovered agreement key");
    let activation_manifest = RecoveredDeviceActivationManifest {
        signature_schema_version: 1,
        activation_algorithm: second.activation_algorithm.clone(),
        activation_public_key_id: second.activation_public_key_id.clone(),
        recovery_record_id: second.recovery_id.clone(),
        domain_id: DOMAIN_ID.to_owned(),
        device_id: RECOVERED_DEVICE_ID.to_owned(),
        signing_algorithm: recovered_signing_key
            .signature_algorithm
            .as_str()
            .to_owned(),
        signing_public_key_id: RECOVERED_SIGNING_KEY_ID.to_owned(),
        signing_public_key: recovered_signing_key.public_key.clone(),
        key_agreement_algorithm: "p256-ecdh-v1".to_owned(),
        key_agreement_public_key_id: RECOVERED_AGREEMENT_KEY_ID.to_owned(),
        key_agreement_public_key: recovered_agreement_key.public_key.clone(),
        key_epoch: 1,
        created_at_ms: BASE_TIMESTAMP_MS + 40,
    };
    let activation = SignedRecoveredDeviceActivation::sign(
        &downloaded_second.material,
        &second_code,
        activation_manifest,
    )
    .expect("activation possession proof");
    let object_key =
        KeyDescriptor::new("sync-key-v1", KeyRole::ObjectKey, 1).expect("epoch key descriptor");
    let first_epoch = WrappedEpochMaterial::seal_for_recipient(
        DOMAIN_ID,
        &agreement_key,
        "recovered-epoch-device-a",
        &object_key,
        &master_key,
        Nonce::new(vec![9; 24]).expect("first epoch nonce"),
        BASE_TIMESTAMP_MS + 40,
    )
    .expect("wrap first active device");
    let recovered_epoch = WrappedEpochMaterial::seal_for_recipient(
        DOMAIN_ID,
        &recovered_agreement_key,
        "recovered-epoch-device-b",
        &object_key,
        &master_key,
        Nonce::new(vec![10; 24]).expect("recovered epoch nonce"),
        BASE_TIMESTAMP_MS + 40,
    )
    .expect("wrap recovered device");
    let distribution = vec![
        sign_epoch_distribution(&recovered_signing_store, first_epoch),
        sign_epoch_distribution(&recovered_signing_store, recovered_epoch),
    ];
    let activated = client
        .activate_recovered_device(&trusted, &downloaded_second, &activation, &distribution)
        .expect("activate recovered device");
    assert_eq!(activated.device_id, RECOVERED_DEVICE_ID);
    assert_eq!(activated.distributed_records, 2);
    let reuse = client
        .activate_recovered_device(&trusted, &downloaded_second, &activation, &distribution)
        .expect_err("recovery record cannot be reused");
    assert!(matches!(
        reuse,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ConflictRecoveryRecord,
            ..
        }
    ));
    let lifecycle = client
        .lifecycle_snapshot(DOMAIN_ID)
        .expect("recovery lifecycle");
    let mut verified_lifecycle = verify_lifecycle_snapshot(lifecycle, &anchor_public_key)
        .expect("verify recovery lifecycle");
    assert_eq!(
        verified_lifecycle
            .trusted_domain()
            .device_profile(RECOVERED_DEVICE_ID)
            .expect("recovered lifecycle profile")
            .device()
            .status,
        radishlex_ime_sync::SyncDeviceStatus::Active
    );
    let third_code = recovery_code([5; RECOVERY_CODE_SECRET_LEN]);
    let third = RecoveryMaterial::encrypt_sync_master_key(
        "recovery-c",
        "recovery-b",
        DOMAIN_ID,
        1,
        &RecoveryKdfProfile::argon2id_v1(),
        &third_code,
        vec![12; 16],
        &master_key,
        BASE_TIMESTAMP_MS + 50,
        Nonce::new(vec![13; 24]).expect("third nonce"),
    )
    .expect("third recovery");
    let third_manifest = sign_recovery(&signing_store, &third);
    client
        .upload_recovery_record(verified_lifecycle.trusted_domain(), &third, &third_manifest)
        .expect("rotate after recovery activation");
    let downloaded_third = client
        .latest_verified_recovery_record(verified_lifecycle.trusted_domain(), DOMAIN_ID)
        .expect("download third recovery");
    let revocation = sign_recovery_revocation(
        &signing_store,
        "recovery-c",
        "compromised",
        BASE_TIMESTAMP_MS + 60,
    );
    let mut wrong_domain = revocation.clone();
    wrong_domain.domain_id = "domain-wrong".to_owned();
    assert!(matches!(
        client
            .revoke_recovery_record(
                verified_lifecycle.trusted_domain(),
                &downloaded_third,
                &wrong_domain,
            )
            .expect_err("wrong revocation domain fails before HTTP"),
        SyncRemoteError::InvalidRequest { .. }
    ));
    let mut tampered_revocation = revocation.clone();
    tampered_revocation.signature.signature[0] ^= 1;
    assert!(matches!(
        client
            .revoke_recovery_record(
                verified_lifecycle.trusted_domain(),
                &downloaded_third,
                &tampered_revocation,
            )
            .expect_err("tampered revocation fails before HTTP"),
        SyncRemoteError::InvalidRequest { .. }
    ));
    let mut wrong_key_id = revocation.clone();
    wrong_key_id.signature.signature_key_id = "signing-key-wrong".to_owned();
    assert!(matches!(
        client
            .revoke_recovery_record(
                verified_lifecycle.trusted_domain(),
                &downloaded_third,
                &wrong_key_id,
            )
            .expect_err("wrong revocation key id fails before HTTP"),
        SyncRemoteError::InvalidRequest { .. }
    ));
    let wrong_device_client = SyncRemoteClient::new(
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("wrong-device transport")
            .with_device_identity(RECOVERED_DEVICE_ID)
            .expect("wrong-device identity"),
    );
    assert!(matches!(
        wrong_device_client
            .revoke_recovery_record(
                verified_lifecycle.trusted_domain(),
                &downloaded_third,
                &revocation,
            )
            .expect_err("transport device cannot impersonate revoker"),
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ForbiddenDevice,
            ..
        }
    ));
    let revoked = client
        .revoke_recovery_record(
            verified_lifecycle.trusted_domain(),
            &downloaded_third,
            &revocation,
        )
        .expect("revoke recovery record");
    let replayed = client
        .revoke_recovery_record(
            verified_lifecycle.trusted_domain(),
            &downloaded_third,
            &revocation,
        )
        .expect("replay recovery revocation");
    assert_eq!(replayed, revoked);
    let lifecycle = client
        .lifecycle_snapshot(DOMAIN_ID)
        .expect("recovery revocation lifecycle");
    verified_lifecycle = verify_lifecycle_snapshot(lifecycle, &anchor_public_key)
        .expect("verify recovery revocation lifecycle");
    let recovered_userdb_path = server.data_path("recovered-device-userdb.sqlite");
    {
        let mut userdb = UserDb::open(&recovered_userdb_path).expect("open recovered userdb");
        userdb
            .store_verified_lifecycle(&verified_lifecycle)
            .expect("cache recovered lifecycle");
    }
    let reopened_userdb = UserDb::open(&recovered_userdb_path).expect("reopen recovered userdb");
    assert_eq!(reopened_userdb.schema_version().expect("schema version"), 9);
    assert!(reopened_userdb
        .trusted_domain_state(DOMAIN_ID)
        .expect("recovered lifecycle after restart")
        .device_profile(RECOVERED_DEVICE_ID)
        .is_some());
    let recovered_transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("recovered transport")
            .with_device_identity(RECOVERED_DEVICE_ID)
            .expect("recovered identity");
    let recovered_client = SyncRemoteClient::new(recovered_transport);
    recovered_client
        .device_verified_epoch_distribution(
            verified_lifecycle.trusted_domain(),
            DOMAIN_ID,
            RECOVERED_DEVICE_ID,
            1,
            "recovered-epoch-device-b",
        )
        .expect("recovered device reads current epoch");
    assert!(matches!(
        client
            .latest_verified_recovery_record(&trusted, DOMAIN_ID)
            .expect_err("consumed recovery is no longer active"),
        SyncRemoteError::Server {
            code: SyncServerErrorCode::NotFound,
            ..
        }
    ));

    let logs = server.stop();
    for forbidden in [
        format_recovery_code([3; RECOVERY_CODE_SECRET_LEN]),
        format_recovery_code([4; RECOVERY_CODE_SECRET_LEN]),
        format_recovery_code([5; RECOVERY_CODE_SECRET_LEN]),
        Base64::encode_string(master_key.as_bytes()),
        Base64::encode_string(&activation.signature),
        Base64::encode_string(&revocation.signature.signature),
        "compromised".to_owned(),
    ] {
        assert!(
            !logs.contains(&forbidden),
            "runtime log leaked recovery secret"
        );
    }
}

fn create_domain(
    transport: &HttpSyncRemoteTransport,
    signing_public_key: &[u8],
    agreement_public_key: &[u8],
) {
    let body = serde_json::to_vec(&json!({
        "domain_id": DOMAIN_ID,
        "current_key_epoch": 1,
        "active_key_id": "sync-key-v1",
        "first_device": {
            "device_id": DEVICE_ID,
            "signing_algorithm": "ed25519-v1",
            "signing_public_key_id": SIGNING_KEY_ID,
            "signing_public_key": Base64::encode_string(signing_public_key),
            "key_agreement_public_key_id": AGREEMENT_KEY_ID,
            "key_agreement_public_key": Base64::encode_string(agreement_public_key),
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

fn sign_epoch_distribution(
    signing_store: &TestMemoryDeviceKeyStore,
    material: WrappedEpochMaterial,
) -> SignedEpochDistribution {
    let placeholder = DeviceSignature::new(
        RECOVERED_SIGNING_KEY_ID,
        RECOVERED_DEVICE_ID,
        vec![1; ED25519_SIGNATURE_LEN],
    )
    .expect("distribution placeholder");
    let unsigned = SignedEpochDistribution::new(RECOVERED_DEVICE_ID, material, placeholder)
        .expect("unsigned distribution");
    let handle = signing_store
        .handle(RECOVERED_DEVICE_ID, RECOVERED_SIGNING_KEY_ID)
        .expect("recovered signing handle");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("distribution signature");
    SignedEpochDistribution::new(RECOVERED_DEVICE_ID, unsigned.material, signature)
        .expect("signed distribution")
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

fn sign_recovery_revocation(
    signing_store: &TestMemoryDeviceKeyStore,
    recovery_record_id: &str,
    reason: &str,
    created_at_ms: i64,
) -> SignedRecoveryRecordRevocation {
    let unsigned = SignedRecoveryRecordRevocation::new(
        recovery_record_id,
        DOMAIN_ID,
        1,
        reason,
        created_at_ms,
        DeviceSignature::new(SIGNING_KEY_ID, DEVICE_ID, vec![0; ED25519_SIGNATURE_LEN])
            .expect("placeholder signature"),
    )
    .expect("unsigned recovery revocation");
    let signature = signing_store
        .sign(
            &signing_store
                .handle(DEVICE_ID, SIGNING_KEY_ID)
                .expect("signing handle"),
            &unsigned.canonical_bytes(),
        )
        .expect("recovery revocation signature");
    SignedRecoveryRecordRevocation::new(
        recovery_record_id,
        DOMAIN_ID,
        1,
        reason,
        created_at_ms,
        signature,
    )
    .expect("signed recovery revocation")
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
