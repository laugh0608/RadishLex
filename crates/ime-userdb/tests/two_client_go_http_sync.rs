use std::time::Duration;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    canonical_signature_bytes, AlgorithmId, CiphertextHash, DeviceKeyAgreementPublicKey,
    DeviceSignature, DeviceSigningPublicKey, EncryptedObjectEnvelope, KeyDescriptor, KeyRole,
    Nonce, SignatureField, SignedSyncObjectManifest, SyncMasterKeyMaterial,
    TestMemoryDeviceKeyStore, WrappedEpochMaterial, ED25519_SIGNATURE_LEN, ENVELOPE_SCHEMA_VERSION,
    SIGNATURE_ALGORITHM_ED25519_V1, SIGNATURE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    device_join_profile_challenge, verify_lifecycle_snapshot, AssembledSyncObject,
    HttpSyncRemoteTransport, LatestObjectConflictMetadata, LocalSyncSnapshot, PlaintextSyncPayload,
    ProductWrappedEpochMaterialStore, RemoteLifecycleDevice, RemoteObjectPayload,
    RemoteObjectVersion, RemoteWrappedEpochLocator, RemoteWrappedEpochMaterialSource,
    SignedEpochDistribution, SyncCryptoLoadError, SyncCycleOutcome, SyncCyclePhase,
    SyncDeviceStatus, SyncEnvelopeAssembler, SyncEpochMaterialStore, SyncObjectAssemblySpec,
    SyncObjectProcessor, SyncObjectType, SyncOnceConfig, SyncOrchestrationErrorCode,
    SyncOrchestrationService, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod,
    SyncRemoteRequest, SyncRemoteTransport, SyncServerErrorCode, SyncWrappedEpochMaterialSource,
};
use radishlex_ime_userdb::{
    decode_userdb_sync_objects, NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel,
    SelectionEventDraft, TermSource, TermStatus, UserDb, UserDbDecryptedSyncObject,
    UserDbSyncPayloadObjectType,
};
use serde_json::{json, Value};

#[path = "support/go_sync_server.rs"]
mod go_sync_server;
#[path = "support/p256_agreement_backend.rs"]
mod p256_agreement_backend;
#[path = "support/sync_crypto_processor.rs"]
mod sync_crypto_processor;
#[path = "support/sync_log_assertions.rs"]
mod sync_log_assertions;

use go_sync_server::GoSyncServer;
use p256_agreement_backend::{agreement_public_key, TestP256AgreementBackend};
use sync_crypto_processor::{test_signing_material, TestCryptoProcessor};
use sync_log_assertions::{
    assert_runtime_logs_redacted, assert_runtime_logs_redacted_without_conflict,
};

const DOMAIN_ID: &str = "domain-two-client-go-http";
const DEVICE_A: &str = "device-a";
const DEVICE_B: &str = "device-b";
const DEVICE_C: &str = "device-c";
const SIGNING_KEY_A: &str = "signing-key-a";
const SIGNING_KEY_B: &str = "signing-key-b";
const SIGNING_KEY_C: &str = "signing-key-c";
const AGREEMENT_KEY_A: &str = "agreement-key-a";
const AGREEMENT_KEY_B: &str = "agreement-key-b";
const AGREEMENT_KEY_C: &str = "agreement-key-c";
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
    let public_key_c = signing_store
        .insert_signing_key(DEVICE_C, SIGNING_KEY_C, [10u8; 32], BASE_TIMESTAMP_MS)
        .expect("device c signing key");

    create_domain(client.transport(), &public_key_a);
    let wrapped_epoch_b = authorize_device(
        client.transport(),
        &signing_store,
        &public_key_b,
        DEVICE_B,
        SIGNING_KEY_B,
        AGREEMENT_KEY_B,
        2,
        "join-device-b",
        "wrapping-key-device-b",
        BASE_TIMESTAMP_MS + 20,
    );
    let _wrapped_epoch_c = authorize_device(
        client.transport(),
        &signing_store,
        &public_key_c,
        DEVICE_C,
        SIGNING_KEY_C,
        AGREEMENT_KEY_C,
        3,
        "join-device-c",
        "wrapping-key-device-c",
        BASE_TIMESTAMP_MS + 30,
    );
    let verified_lifecycle = verify_lifecycle_snapshot(
        client
            .lifecycle_snapshot(DOMAIN_ID)
            .expect("load two-device lifecycle"),
        &public_key_a,
    )
    .expect("verify two-device lifecycle");
    assert!(verified_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_B)
        .is_some());
    assert!(verified_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_C)
        .is_some());

    let device_a_db_path = server.data_path("device-a-userdb.sqlite");
    let device_b_db_path = server.data_path("device-b-userdb.sqlite");
    let device_c_db_path = server.data_path("device-c-userdb.sqlite");
    let mut device_a_db = UserDb::open(&device_a_db_path).expect("device a db");
    device_a_db
        .store_verified_lifecycle(&verified_lifecycle)
        .expect("cache lifecycle on device a");
    let wrapped_epoch_a1 = wrapped_epoch_for_device(
        DEVICE_A,
        AGREEMENT_KEY_A,
        1,
        OBJECT_KEY_ID,
        1,
        [11u8; 32],
        "wrapping-key-device-a-1",
        BASE_TIMESTAMP_MS + 21,
    );
    device_a_db
        .cache_wrapped_epoch_materials(std::slice::from_ref(&wrapped_epoch_a1))
        .expect("cache device a epoch 1");
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

    let mut device_b_db = UserDb::open(&device_b_db_path).expect("device b db");
    device_b_db
        .store_verified_lifecycle(&verified_lifecycle)
        .expect("cache lifecycle on device b");
    let material_transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("material transport")
            .with_device_identity(DEVICE_B)
            .expect("device b transport identity");
    let material_client = SyncRemoteClient::new(material_transport);
    let verified_authorization = verified_lifecycle
        .events()
        .iter()
        .filter_map(|event| event.authorization.as_ref())
        .find(|authorization| authorization.recipient_device_id == DEVICE_B)
        .expect("verified device b authorization");
    let locator = RemoteWrappedEpochLocator::new(
        verified_authorization.key_epoch,
        verified_authorization.wrapping_key_id.clone(),
    )
    .expect("wrapped epoch locator");
    let mut remote_materials =
        RemoteWrappedEpochMaterialSource::new(&material_client, vec![locator.clone()])
            .expect("remote wrapped source");
    let fetched = remote_materials
        .load_wrapped_epoch_materials(DOMAIN_ID, DEVICE_B)
        .expect("download wrapped epoch for device b");
    assert_eq!(fetched, vec![wrapped_epoch_b.clone()]);
    assert_eq!(
        device_b_db
            .cache_wrapped_epoch_materials(&fetched)
            .expect("cache remote wrapped epoch"),
        1
    );
    assert_eq!(
        device_b_db
            .cache_wrapped_epoch_materials(&fetched)
            .expect("repeat remote wrapped epoch"),
        0
    );
    let trusted_b = verified_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_B)
        .expect("trusted device b")
        .clone();
    let mut product_materials = ProductWrappedEpochMaterialStore::new(
        UserDb::open(&device_b_db_path).expect("open second device b cache"),
        TestP256AgreementBackend::new(DEVICE_B, AGREEMENT_KEY_B, 2),
    );
    let loaded_materials = product_materials
        .load_epoch_materials(verified_lifecycle.trusted_domain().domain(), &trusted_b)
        .expect("unwrap cached remote epoch");
    assert_eq!(loaded_materials.len(), 1);
    assert_eq!(loaded_materials[0].key_id(), OBJECT_KEY_ID);
    assert_eq!(loaded_materials[0].key_epoch(), 1);
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

    revoke_device_b(client.transport(), &signing_store);
    let revoked_lifecycle = verify_lifecycle_snapshot(
        client
            .lifecycle_snapshot(DOMAIN_ID)
            .expect("load revoked lifecycle"),
        &public_key_a,
    )
    .expect("verify revoked lifecycle");
    let revoked_profile = revoked_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_B)
        .expect("revoked device profile");
    assert_eq!(revoked_profile.device().status, SyncDeviceStatus::Lost);
    assert_eq!(revoked_profile.reject_from_change_sequence(), Some(7));
    device_a_db
        .store_verified_lifecycle(&revoked_lifecycle)
        .expect("cache revoked lifecycle on device a");
    let wrapped_epoch_a2 = wrapped_epoch_for_device(
        DEVICE_A,
        AGREEMENT_KEY_A,
        1,
        "object-key-v2",
        2,
        [12u8; 32],
        "wrapping-key-device-a-2",
        BASE_TIMESTAMP_MS + 2_001,
    );
    let wrapped_epoch_c2 = wrapped_epoch_for_device(
        DEVICE_C,
        AGREEMENT_KEY_C,
        3,
        "object-key-v2",
        2,
        [12u8; 32],
        "wrapping-key-device-c-2",
        BASE_TIMESTAMP_MS + 2_001,
    );
    let signed_epoch_a2 = sign_epoch_distribution(&signing_store, wrapped_epoch_a2.clone());
    let signed_epoch_c2 = sign_epoch_distribution(&signing_store, wrapped_epoch_c2.clone());
    let distribution_transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("distribution transport")
            .with_device_identity(DEVICE_A)
            .expect("device a distribution identity");
    let distribution_client = SyncRemoteClient::new(distribution_transport);
    let mut invalid_epoch_c2 = signed_epoch_c2.clone();
    invalid_epoch_c2.signature.signature[0] ^= 0x80;
    let partial_response = send_json(
        distribution_client.transport(),
        SyncRemoteMethod::Post,
        "/api/v1/domains/domain-two-client-go-http/epoch-distributions",
        epoch_distribution_request_body(&[signed_epoch_a2.clone(), invalid_epoch_c2]),
    );
    assert_eq!(partial_response.status, 400);
    let partial_body: Value =
        serde_json::from_slice(&partial_response.body).expect("partial failure response");
    assert_eq!(partial_body["error_code"], "invalid_signature");
    let missing_after_partial = distribution_client
        .device_verified_epoch_distribution(
            revoked_lifecycle.trusted_domain(),
            DOMAIN_ID,
            DEVICE_A,
            2,
            "wrapping-key-device-a-2",
        )
        .expect_err("partial failure must not expose device a metadata");
    assert!(matches!(
        missing_after_partial,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::NotFound,
            ..
        }
    ));
    let incomplete_response = send_json(
        distribution_client.transport(),
        SyncRemoteMethod::Post,
        "/api/v1/domains/domain-two-client-go-http/epoch-distributions",
        epoch_distribution_request_body(std::slice::from_ref(&signed_epoch_a2)),
    );
    assert_eq!(incomplete_response.status, 400);
    let incomplete_body: Value =
        serde_json::from_slice(&incomplete_response.body).expect("incomplete response");
    assert_eq!(incomplete_body["error_code"], "invalid_request");
    let distributed = distribution_client
        .upload_epoch_distribution(
            revoked_lifecycle.trusted_domain(),
            &[signed_epoch_a2.clone(), signed_epoch_c2.clone()],
        )
        .expect("upload complete active epoch distribution");
    assert_eq!(distributed.accepted_records, 2);
    assert_eq!(distributed.inserted_records, 2);
    assert_eq!(
        distribution_client
            .upload_epoch_distribution(
                revoked_lifecycle.trusted_domain(),
                &[signed_epoch_a2.clone(), signed_epoch_c2.clone()],
            )
            .expect("retry complete active epoch distribution")
            .inserted_records,
        0
    );
    let conflicting_epoch_c2 = sign_epoch_distribution(
        &signing_store,
        wrapped_epoch_for_device(
            DEVICE_C,
            AGREEMENT_KEY_C,
            3,
            "object-key-v2",
            2,
            [12u8; 32],
            "wrapping-key-device-c-2",
            BASE_TIMESTAMP_MS + 2_002,
        ),
    );
    let conflict_error = distribution_client
        .upload_epoch_distribution(
            revoked_lifecycle.trusted_domain(),
            &[signed_epoch_a2.clone(), conflicting_epoch_c2],
        )
        .expect_err("same locator with different sealed material must conflict");
    assert!(matches!(
        conflict_error,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ConflictEpochDistribution,
            ..
        }
    ));
    let downloaded_epoch_a2 = distribution_client
        .device_verified_epoch_distribution(
            revoked_lifecycle.trusted_domain(),
            DOMAIN_ID,
            DEVICE_A,
            2,
            "wrapping-key-device-a-2",
        )
        .expect("download signed device a epoch 2");
    device_a_db
        .cache_wrapped_epoch_materials(std::slice::from_ref(&downloaded_epoch_a2.material))
        .expect("cache device a rotated epoch 2");
    let device_c_transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("device c material transport")
            .with_device_identity(DEVICE_C)
            .expect("device c material identity");
    let device_c_client = SyncRemoteClient::new(device_c_transport);
    let downloaded_epoch_c2 = device_c_client
        .device_verified_epoch_distribution(
            revoked_lifecycle.trusted_domain(),
            DOMAIN_ID,
            DEVICE_C,
            2,
            "wrapping-key-device-c-2",
        )
        .expect("download signed device c epoch 2");
    let mut device_c_db = UserDb::open(&device_c_db_path).expect("device c db");
    device_c_db
        .store_verified_lifecycle(&revoked_lifecycle)
        .expect("cache lifecycle on device c");
    device_c_db
        .cache_wrapped_epoch_materials(std::slice::from_ref(&downloaded_epoch_c2.material))
        .expect("cache device c epoch 2");
    device_b_db
        .store_verified_lifecycle(&revoked_lifecycle)
        .expect("cache revoked lifecycle on device b");

    let remote_error = material_client
        .device_wrapped_epoch_material(DOMAIN_ID, DEVICE_B, 2, "wrapping-key-device-b-2")
        .expect_err("revoked device cannot request new wrapped epoch");
    assert!(matches!(
        remote_error,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ForbiddenDevice,
            ..
        }
    ));

    drop(device_a_db);
    drop(device_b_db);
    drop(device_c_db);
    let restarted_a = UserDb::open(&device_a_db_path).expect("restart device a userdb");
    let restarted_b = UserDb::open(&device_b_db_path).expect("restart device b userdb");
    let restarted_c = UserDb::open(&device_c_db_path).expect("restart device c userdb");
    for restarted in [&restarted_a, &restarted_b] {
        let trusted = restarted
            .trusted_domain_state(DOMAIN_ID)
            .expect("restore trusted lifecycle after restart");
        assert_eq!(trusted.domain().current_key_epoch, 2);
        assert_eq!(
            trusted
                .device_profile(DEVICE_B)
                .expect("restored revoked device")
                .reject_from_change_sequence(),
            Some(7)
        );
    }
    assert_eq!(
        restarted_b
            .wrapped_epoch_materials(DOMAIN_ID, DEVICE_B)
            .expect("restore wrapped cache after restart"),
        vec![wrapped_epoch_b]
    );
    let trusted_c = revoked_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_C)
        .expect("active device c profile");
    let mut restarted_c_product_materials = ProductWrappedEpochMaterialStore::new(
        restarted_c,
        TestP256AgreementBackend::new(DEVICE_C, AGREEMENT_KEY_C, 3),
    );
    let restarted_c_materials = restarted_c_product_materials
        .load_epoch_materials(revoked_lifecycle.trusted_domain().domain(), trusted_c)
        .expect("restore device c distributed epoch after restart");
    assert_eq!(
        restarted_c_materials
            .iter()
            .map(|material| (material.key_epoch(), material.key_id()))
            .collect::<Vec<_>>(),
        vec![(2, "object-key-v2")]
    );
    let trusted_a = revoked_lifecycle
        .trusted_domain()
        .device_profile(DEVICE_A)
        .expect("active device a profile")
        .clone();
    let mut restarted_a_product_materials = ProductWrappedEpochMaterialStore::new(
        restarted_a,
        TestP256AgreementBackend::new(DEVICE_A, AGREEMENT_KEY_A, 1),
    );
    let restarted_a_materials = restarted_a_product_materials
        .load_epoch_materials(revoked_lifecycle.trusted_domain().domain(), &trusted_a)
        .expect("restore device a historical and rotated epochs");
    assert_eq!(
        restarted_a_materials
            .iter()
            .map(|material| (material.key_epoch(), material.key_id()))
            .collect::<Vec<_>>(),
        vec![(1, OBJECT_KEY_ID), (2, "object-key-v2")]
    );
    let mut revoked_product_materials = ProductWrappedEpochMaterialStore::new(
        restarted_b,
        TestP256AgreementBackend::new(DEVICE_B, AGREEMENT_KEY_B, 2),
    );
    let error = revoked_product_materials
        .load_epoch_materials(revoked_lifecycle.trusted_domain().domain(), revoked_profile)
        .expect_err("revoked cached material must be blocked before read");
    assert_eq!(error, SyncCryptoLoadError::Revoked);

    let log_text = server.stop();
    assert_runtime_logs_redacted(&log_text);
}

#[test]
fn two_sync_once_services_converge_isolated_userdbs_through_go_http_server() {
    let Some(server) = GoSyncServer::try_spawn() else {
        return;
    };
    let setup_transport =
        HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(5))
            .expect("setup transport");
    let (setup_signing_store, public_key_a, public_key_b) = test_signing_material();
    create_domain(&setup_transport, &public_key_a);
    authorize_device(
        &setup_transport,
        &setup_signing_store,
        &public_key_b,
        DEVICE_B,
        SIGNING_KEY_B,
        AGREEMENT_KEY_B,
        2,
        "join-device-b",
        "wrapping-key-device-b",
        BASE_TIMESTAMP_MS + 20,
    );

    let mut device_a_db = UserDb::open_in_memory().expect("device a db");
    seed_device_a_userdb(&mut device_a_db);
    let mut device_a_service = sync_service(server.base_url(), DEVICE_A, SIGNING_KEY_A);
    let first_a =
        device_a_service.sync_once(&mut device_a_db, DOMAIN_ID, BASE_TIMESTAMP_MS + 1_000);
    assert_eq!(first_a.outcome, SyncCycleOutcome::Completed, "{first_a:?}");
    assert_eq!(first_a.discovered, 0);
    assert_eq!(first_a.downloaded, 0);
    assert_eq!(first_a.uploaded, 3);

    let mut device_b_db = UserDb::open_in_memory().expect("device b db");
    seed_device_b_local_state(&mut device_b_db);
    let mut device_b_service = sync_service(server.base_url(), DEVICE_B, SIGNING_KEY_B);
    let first_b =
        device_b_service.sync_once(&mut device_b_db, DOMAIN_ID, BASE_TIMESTAMP_MS + 2_000);
    assert_eq!(first_b.outcome, SyncCycleOutcome::Completed, "{first_b:?}");
    assert_eq!(first_b.discovered, 3);
    assert_eq!(first_b.downloaded, 3);
    assert_eq!(first_b.uploaded, 3);
    assert_converged_userdb_state(&device_b_db);

    let second_a =
        device_a_service.sync_once(&mut device_a_db, DOMAIN_ID, BASE_TIMESTAMP_MS + 3_000);
    assert_eq!(
        second_a.outcome,
        SyncCycleOutcome::Completed,
        "{second_a:?}"
    );
    assert_eq!(second_a.discovered, 6);
    assert_eq!(second_a.downloaded, 6);
    assert_eq!(second_a.uploaded, 3);
    assert_converged_userdb_state(&device_a_db);

    // Rebuild the orchestration service to prove that the persisted userdb cursor and
    // remote observations, rather than process memory, drive the next cycle.
    drop(device_b_service);
    let mut restarted_device_b_service = sync_service(server.base_url(), DEVICE_B, SIGNING_KEY_B);
    let second_b = restarted_device_b_service.sync_once(
        &mut device_b_db,
        DOMAIN_ID,
        BASE_TIMESTAMP_MS + 4_000,
    );
    assert_eq!(
        second_b.outcome,
        SyncCycleOutcome::Completed,
        "{second_b:?}"
    );
    assert_eq!(second_b.discovered, 6);
    assert_eq!(second_b.downloaded, 6);
    assert_eq!(second_b.uploaded, 0);
    assert_converged_userdb_state(&device_b_db);

    let log_text = server.stop();
    assert_runtime_logs_redacted_without_conflict(&log_text);
    assert!(
        log_text.contains(r#"route="objects.discover""#),
        "runtime log missing discovery route: {log_text}"
    );
}

#[test]
fn crypto_processor_rejects_tampered_signature_ciphertext_and_authenticated_metadata() {
    let (mut processor, mut signature_tampered) = prepared_remote_fixture();
    signature_tampered.object.signature[0] ^= 0x01;
    assert_processor_error(
        &mut processor,
        signature_tampered,
        SyncOrchestrationErrorCode::SignatureMismatch,
        SyncCyclePhase::Verify,
    );

    let (mut processor, mut ciphertext_tampered) = prepared_remote_fixture();
    ciphertext_tampered.payload[0] ^= 0x01;
    assert_processor_error(
        &mut processor,
        ciphertext_tampered,
        SyncOrchestrationErrorCode::CiphertextHashMismatch,
        SyncCyclePhase::DecryptAndDecode,
    );

    let (mut processor, mut metadata_tampered) = prepared_remote_fixture();
    metadata_tampered.object.client_updated_at_ms += 1;
    let envelope = envelope_from_remote(metadata_tampered.clone());
    let manifest = sign_envelope(
        &envelope,
        processor.signing_store(),
        DEVICE_A,
        SIGNING_KEY_A,
    );
    metadata_tampered.object.signature = manifest.signature.signature;
    assert_processor_error(
        &mut processor,
        metadata_tampered,
        SyncOrchestrationErrorCode::CiphertextHashMismatch,
        SyncCyclePhase::DecryptAndDecode,
    );
}

#[test]
fn crypto_processor_rejects_unaccepted_epoch_and_revoked_signer() {
    let (mut epoch_processor, retired_epoch_payload) = prepared_remote_fixture();
    epoch_processor.replace_accepted_key_epochs([2]);
    epoch_processor
        .preflight(DOMAIN_ID)
        .expect("refresh retired epoch snapshot");
    assert_processor_error(
        &mut epoch_processor,
        retired_epoch_payload,
        SyncOrchestrationErrorCode::KeyEpochRejected,
        SyncCyclePhase::Verify,
    );

    let (mut revoked_processor, revoked_payload) = prepared_remote_fixture();
    revoked_processor.revoke_device(DEVICE_A);
    revoked_processor
        .preflight(DOMAIN_ID)
        .expect("refresh revoked signer snapshot");
    assert_processor_error(
        &mut revoked_processor,
        revoked_payload,
        SyncOrchestrationErrorCode::RevokedDevice,
        SyncCyclePhase::Verify,
    );
}

fn prepared_remote_fixture() -> (TestCryptoProcessor, RemoteObjectPayload) {
    let mut processor = TestCryptoProcessor::new(DEVICE_A, SIGNING_KEY_A);
    processor
        .preflight(DOMAIN_ID)
        .expect("fixture processor preflight");
    let snapshot = LocalSyncSnapshot {
        domain_id: DOMAIN_ID.to_owned(),
        object_id: "dictionary-user-terms-v1".to_owned(),
        object_type: SyncObjectType::DictionaryUserTerms,
        local_revision: 1,
        remote_base_version: None,
        payload: PlaintextSyncPayload::new(
            SyncObjectType::DictionaryUserTerms,
            1,
            br#"{"synthetic":true}"#.to_vec(),
        )
        .expect("fixture payload"),
    };
    let outbox = processor
        .prepare_outbox(snapshot, 1, BASE_TIMESTAMP_MS + 1_000)
        .expect("prepared fixture outbox");
    let draft = &outbox.object.draft;
    let signature = &outbox.manifest.signature;
    let remote = RemoteObjectVersion {
        domain_id: DOMAIN_ID.to_owned(),
        object_id: draft.object_id.clone(),
        object_type: draft.object_type,
        version: draft.version,
        base_version: draft.base_version,
        change_sequence: 1,
        owner_device_id: draft.owner_device_id.clone(),
        key_id: draft.key_id.clone(),
        key_epoch: draft.key_epoch,
        algorithm: draft.algorithm.clone(),
        nonce: draft.nonce.clone(),
        encrypted_payload_len: draft.encrypted_payload_len,
        ciphertext_hash: draft.ciphertext_hash.clone(),
        signature_schema_version: signature.signature_schema_version,
        signature_algorithm: signature.signature_algorithm.as_str().to_owned(),
        signature_key_id: signature.signature_key_id.clone(),
        signature: signature.signature.clone(),
        server_received_at_ms: BASE_TIMESTAMP_MS + 1_001,
        client_created_at_ms: draft.created_at_ms,
        client_updated_at_ms: draft.updated_at_ms,
    };
    (
        processor,
        RemoteObjectPayload {
            object: remote,
            payload: outbox.object.envelope.encrypted_payload,
        },
    )
}

fn assert_processor_error(
    processor: &mut TestCryptoProcessor,
    payload: RemoteObjectPayload,
    expected_code: SyncOrchestrationErrorCode,
    expected_phase: SyncCyclePhase,
) {
    let expected = payload.object.clone();
    let error = processor
        .verify_and_decrypt(&expected, payload)
        .expect_err("tampered payload must be rejected");
    assert_eq!(error.code, expected_code);
    assert_eq!(error.phase, expected_phase);
    assert!(!error.retryable);
}

fn assert_converged_userdb_state(db: &UserDb) {
    assert_term_status(db, "radish", "radish-alpha", "ra dish", TermStatus::Active);
    assert_term_status(
        db,
        "clientb",
        "client-b-alpha",
        "client b reading",
        TermStatus::Active,
    );
    assert_term_status(
        db,
        "blocked",
        "blocked-alpha",
        "blocked reading",
        TermStatus::Deleted,
    );
    assert_term_status(
        db,
        "deleted",
        "deleted-alpha",
        "deleted reading",
        TermStatus::Deleted,
    );
    assert!(db
        .ranker_weight("rank", "ranker-alpha", Some("ranker reading"), "chat")
        .expect("ranker weight")
        .is_some());
}

fn sync_service(
    base_url: String,
    device_id: &'static str,
    signing_key_id: &'static str,
) -> SyncOrchestrationService<HttpSyncRemoteTransport, TestCryptoProcessor> {
    let transport = HttpSyncRemoteTransport::with_timeout(base_url, Duration::from_secs(5))
        .expect("sync transport");
    SyncOrchestrationService::new(
        SyncRemoteClient::new(transport),
        TestCryptoProcessor::new(device_id, signing_key_id),
        SyncOnceConfig::default(),
    )
    .expect("sync service")
}

fn create_domain(transport: &HttpSyncRemoteTransport, public_key: &DeviceSigningPublicKey) {
    let body = json!({
        "domain_id": DOMAIN_ID,
        "current_key_epoch": 1,
        "active_key_id": "sync-key-v1",
        "first_device": {
            "device_id": DEVICE_A,
            "signing_algorithm": public_key.signature_algorithm.as_str(),
            "signing_public_key_id": SIGNING_KEY_A,
            "signing_public_key": b64(&public_key.public_key),
            "key_agreement_public_key_id": AGREEMENT_KEY_A,
            "key_agreement_public_key": b64(&agreement_public_key(1)),
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

#[allow(clippy::too_many_arguments)]
fn authorize_device(
    transport: &HttpSyncRemoteTransport,
    signing_store: &TestMemoryDeviceKeyStore,
    recipient_signing_public_key: &DeviceSigningPublicKey,
    recipient_device_id: &str,
    recipient_signing_key_id: &str,
    recipient_agreement_key_id: &str,
    recipient_agreement_scalar: u8,
    join_request_id: &str,
    wrapping_key_id: &str,
    created_at_ms: i64,
) -> WrappedEpochMaterial {
    let join_created_at_ms = created_at_ms - 10;
    let join_expires_at_ms = BASE_TIMESTAMP_MS + 600;
    let lifecycle_device = RemoteLifecycleDevice {
        domain_id: DOMAIN_ID.to_owned(),
        device_id: recipient_device_id.to_owned(),
        signing_algorithm: recipient_signing_public_key
            .signature_algorithm
            .as_str()
            .to_owned(),
        signing_public_key_id: recipient_signing_key_id.to_owned(),
        signing_public_key: recipient_signing_public_key.public_key.clone(),
        key_agreement_public_key_id: recipient_agreement_key_id.to_owned(),
        key_agreement_public_key: agreement_public_key(recipient_agreement_scalar),
        status: SyncDeviceStatus::Active,
        authorized_at_ms: Some(created_at_ms),
        revoked_at_ms: None,
        last_seen_at_ms: None,
    };
    let challenge = device_join_profile_challenge(
        DOMAIN_ID,
        join_request_id,
        &lifecycle_device,
        join_created_at_ms,
        join_expires_at_ms,
    );
    let join_body = json!({
        "join_request_id": join_request_id,
        "device_id": recipient_device_id,
        "signing_algorithm": recipient_signing_public_key.signature_algorithm.as_str(),
        "signing_public_key_id": recipient_signing_key_id,
        "signing_public_key": b64(&recipient_signing_public_key.public_key),
        "key_agreement_public_key_id": recipient_agreement_key_id,
        "key_agreement_public_key": b64(&agreement_public_key(recipient_agreement_scalar)),
        "challenge": b64(challenge.as_bytes()),
        "created_at_ms": join_created_at_ms,
        "expires_at_ms": join_expires_at_ms
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

    let recipient = DeviceKeyAgreementPublicKey::p256(
        recipient_device_id,
        recipient_agreement_key_id,
        agreement_public_key(recipient_agreement_scalar),
        BASE_TIMESTAMP_MS,
        None,
    )
    .expect("device b agreement profile");
    let object_key =
        KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, 1).expect("wrapped epoch object key");
    let master_key = SyncMasterKeyMaterial::new([11u8; 32]).expect("wrapped epoch master key");
    let wrapped_epoch = WrappedEpochMaterial::seal_for_recipient(
        DOMAIN_ID,
        &recipient,
        wrapping_key_id,
        &object_key,
        &master_key,
        Nonce::new(vec![0x51; 24]).expect("wrapped epoch nonce"),
        created_at_ms,
    )
    .expect("wrap epoch for device b");
    let wrapped_key_len = wrapped_epoch.wrapped_key.len();
    let authorization_signature = sign_authorization(
        signing_store,
        challenge.as_bytes(),
        recipient_device_id,
        recipient_signing_key_id,
        "123456",
        wrapping_key_id,
        wrapped_key_len,
        created_at_ms,
    );
    let authorization_body = json!({
        "authorization": {
            "authorizer_device_id": DEVICE_A,
            "recipient_device_id": recipient_device_id,
            "recipient_signing_public_key_id": recipient_signing_key_id,
            "recipient_key_agreement_key_id": recipient_agreement_key_id,
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
            "recipient_device_id": recipient_device_id,
            "recipient_key_agreement_key_id": recipient_agreement_key_id,
            "key_epoch": 1,
            "wrapping_key_id": wrapping_key_id,
            "algorithm": &wrapped_epoch.algorithm,
            "nonce": b64(wrapped_epoch.nonce.as_bytes()),
            "wrapped_key_len": wrapped_key_len,
            "ciphertext_hash": &wrapped_epoch.ciphertext_hash,
            "created_at_ms": created_at_ms,
            "signature": b64(b"synthetic-wrapping-signature")
        },
        "wrapped_key": b64(&wrapped_epoch.wrapped_key)
    });
    let authorization_response = send_json(
        transport,
        SyncRemoteMethod::Post,
        &format!("/api/v1/domains/domain-two-client-go-http/join-requests/{join_request_id}/authorization"),
        authorization_body,
    );
    assert_eq!(
        authorization_response.status,
        204,
        "authorize join request failed: {}",
        String::from_utf8_lossy(&authorization_response.body)
    );
    wrapped_epoch
}

#[allow(clippy::too_many_arguments)]
fn sign_authorization(
    signing_store: &TestMemoryDeviceKeyStore,
    challenge: &[u8],
    recipient_device_id: &str,
    recipient_signing_key_id: &str,
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
        SignatureField::text("recipient_device_id", recipient_device_id),
        SignatureField::text("recipient_public_key_id", recipient_signing_key_id),
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

fn revoke_device_b(transport: &HttpSyncRemoteTransport, signing_store: &TestMemoryDeviceKeyStore) {
    let revoked_at_ms = BASE_TIMESTAMP_MS + 2_000;
    let fields = [
        SignatureField::u16("signature_schema_version", SIGNATURE_SCHEMA_VERSION),
        SignatureField::text("signature_algorithm", SIGNATURE_ALGORITHM_ED25519_V1),
        SignatureField::text("signature_key_id", SIGNING_KEY_A),
        SignatureField::text("revoked_by_device_id", DEVICE_A),
        SignatureField::text("revoked_device_id", DEVICE_B),
        SignatureField::u64("previous_key_epoch", 1),
        SignatureField::u64("new_key_epoch", 2),
        SignatureField::text("reason", "device_lost"),
        SignatureField::i64("revoked_at_ms", revoked_at_ms),
    ];
    let handle = signing_store
        .handle(DEVICE_A, SIGNING_KEY_A)
        .expect("device a signing handle");
    let signature = signing_store
        .sign(
            &handle,
            &canonical_signature_bytes("device_revocation", &fields),
        )
        .expect("revocation signature");
    let response = send_json(
        transport,
        SyncRemoteMethod::Post,
        "/api/v1/domains/domain-two-client-go-http/devices/device-b/revocations",
        json!({
            "revoker_device_id": DEVICE_A,
            "previous_key_epoch": 1,
            "new_key_epoch": 2,
            "reason": "device_lost",
            "created_at_ms": revoked_at_ms,
            "signature_schema_version": signature.signature_schema_version,
            "signature_algorithm": signature.signature_algorithm.as_str(),
            "signature_key_id": signature.signature_key_id,
            "signature": b64(&signature.signature),
        }),
    );
    assert_eq!(
        response.status,
        204,
        "revoke device failed: {}",
        String::from_utf8_lossy(&response.body)
    );
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
    sign_envelope(
        &object.envelope,
        signing_store,
        signer_device_id,
        signing_key_id,
    )
}

fn sign_envelope(
    envelope: &EncryptedObjectEnvelope,
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
    let unsigned =
        SignedSyncObjectManifest::new(DOMAIN_ID, envelope, placeholder).expect("unsigned manifest");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("signature");
    SignedSyncObjectManifest::new(DOMAIN_ID, envelope, signature).expect("manifest")
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

#[allow(clippy::too_many_arguments)]
fn wrapped_epoch_for_device(
    device_id: &str,
    key_id: &str,
    agreement_scalar: u8,
    object_key_id: &str,
    key_epoch: u64,
    master_key: [u8; 32],
    wrapping_key_id: &str,
    created_at_ms: i64,
) -> WrappedEpochMaterial {
    let recipient = DeviceKeyAgreementPublicKey::p256(
        device_id,
        key_id,
        agreement_public_key(agreement_scalar),
        BASE_TIMESTAMP_MS,
        None,
    )
    .expect("agreement profile");
    let object_key = KeyDescriptor::new(object_key_id, KeyRole::ObjectKey, key_epoch)
        .expect("wrapped object key");
    let master_key = SyncMasterKeyMaterial::new(master_key).expect("wrapped master key");
    WrappedEpochMaterial::seal_for_recipient(
        DOMAIN_ID,
        &recipient,
        wrapping_key_id,
        &object_key,
        &master_key,
        Nonce::new(vec![key_epoch as u8; 24]).expect("wrapped nonce"),
        created_at_ms,
    )
    .expect("wrapped epoch")
}

fn sign_epoch_distribution(
    signing_store: &TestMemoryDeviceKeyStore,
    material: WrappedEpochMaterial,
) -> SignedEpochDistribution {
    let placeholder = DeviceSignature::new(SIGNING_KEY_A, DEVICE_A, vec![1; ED25519_SIGNATURE_LEN])
        .expect("distribution placeholder signature");
    let unsigned = SignedEpochDistribution::new(DEVICE_A, material, placeholder)
        .expect("unsigned-shaped epoch distribution");
    let handle = signing_store
        .handle(DEVICE_A, SIGNING_KEY_A)
        .expect("device a signing handle");
    let signature = signing_store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("sign epoch distribution");
    SignedEpochDistribution::new(DEVICE_A, unsigned.material, signature)
        .expect("signed epoch distribution")
}

fn epoch_distribution_request_body(records: &[SignedEpochDistribution]) -> Value {
    json!({
        "distributor_device_id": DEVICE_A,
        "key_epoch": records[0].material.key_epoch,
        "records": records.iter().map(|record| json!({
            "recipient_device_id": record.material.recipient_device_id,
            "recipient_key_agreement_key_id": record.material.recipient_key_agreement_key_id,
            "wrapping_key_id": record.material.wrapping_key_id,
            "algorithm": record.material.algorithm,
            "nonce": b64(record.material.nonce.as_bytes()),
            "wrapped_key_len": record.material.wrapped_key.len(),
            "ciphertext_hash": record.material.ciphertext_hash,
            "created_at_ms": record.material.created_at_ms,
            "signature_schema_version": record.signature.signature_schema_version,
            "signature_algorithm": record.signature.signature_algorithm.as_str(),
            "signature_key_id": record.signature.signature_key_id,
            "signature": b64(&record.signature.signature),
            "wrapped_key": b64(&record.material.wrapped_key),
        })).collect::<Vec<_>>()
    })
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
