use super::test_support::{response_for, signed_object};
use super::*;
use crate::{SignedEpochDistribution, SyncDevice, SyncTrustedDomainState};
use p256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
use radishlex_ime_crypto::{
    DeviceKeyAgreementPublicKey, DeviceSignature, KeyDescriptor, KeyRole, Nonce,
    SyncMasterKeyMaterial, TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
};
use serde_json::Value;
use std::cell::RefCell;

#[derive(Default)]
struct RecordingTransport {
    requests: RefCell<Vec<SyncRemoteRequest>>,
    responses: RefCell<Vec<Result<SyncRemoteResponse, SyncRemoteError>>>,
}

impl RecordingTransport {
    fn push_json<T: Serialize>(&self, status: u16, value: &T) {
        self.responses
            .borrow_mut()
            .push(SyncRemoteResponse::json(status, value));
    }

    fn push_bytes(&self, status: u16, bytes: &[u8]) {
        self.responses.borrow_mut().push(Ok(SyncRemoteResponse::new(
            status,
            Some("application/octet-stream".to_owned()),
            bytes.to_vec(),
        )));
    }

    fn requests(&self) -> Vec<SyncRemoteRequest> {
        self.requests.borrow().clone()
    }
}

impl SyncRemoteTransport for RecordingTransport {
    fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError> {
        self.requests.borrow_mut().push(request);
        self.responses.borrow_mut().remove(0)
    }
}

#[test]
fn upload_object_version_sends_only_metadata_and_encrypted_payload() {
    let (object, manifest) = signed_object();
    let transport = RecordingTransport::default();
    transport.push_json(201, &response_for(&object));
    let client = SyncRemoteClient::new(transport);

    let uploaded = client
        .upload_object_version("domain-a", &object, &manifest)
        .expect("upload");

    assert_eq!(uploaded.object_id, object.draft.object_id);
    assert_eq!(uploaded.version, object.draft.version);
    let requests = client.transport().requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method(), SyncRemoteMethod::Post);
    assert_eq!(
        request.path(),
        "/api/v1/domains/domain-a/objects/object-a/versions"
    );
    assert_eq!(request.content_type(), Some("application/json"));

    let body: Value = serde_json::from_slice(request.body()).expect("json");
    assert_eq!(body["object_type"], "dictionary.user_terms");
    assert_eq!(body["base_version"], 0);
    assert_eq!(
        body["payload"],
        Base64::encode_string(&object.envelope.encrypted_payload)
    );
    assert_eq!(body["nonce"], Base64::encode_string(&object.draft.nonce));
    assert_eq!(
        body["signature"],
        Base64::encode_string(&manifest.signature.signature)
    );
    let body_text = String::from_utf8(request.body().to_vec()).expect("utf8");
    assert!(!body_text.contains("plaintext"));
    assert!(!body_text.contains("input_code"));
    assert!(!body_text.contains("reading"));
    assert!(!body_text.contains("ranker_detail"));
}

#[test]
fn upload_rejects_manifest_that_does_not_match_encrypted_object() {
    let (object, mut manifest) = signed_object();
    manifest.object_id = "object-b".to_owned();
    let client = SyncRemoteClient::new(RecordingTransport::default());

    let error = client
        .upload_object_version("domain-a", &object, &manifest)
        .expect_err("mismatch fails");

    assert!(matches!(error, SyncRemoteError::InvalidRequest { .. }));
    assert!(error.to_string().contains("object_id"));
}

#[test]
fn discovery_uses_structured_cursor_query_and_redacts_its_value() {
    let (object, _) = signed_object();
    let mut entry = response_for(&object);
    entry["change_sequence"] = serde_json::json!(7);
    let transport = RecordingTransport::default();
    transport.push_json(
        200,
        &serde_json::json!({
            "entries": [entry],
            "next_cursor": "v1.cursor_7",
            "has_more": false
        }),
    );
    let client = SyncRemoteClient::new(transport);
    let cursor = OpaqueSyncCursor::new("v1.cursor_6").expect("cursor");

    let page = client
        .discover_object_versions("domain-a", Some(&cursor), 50)
        .expect("discovery page");

    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].change_sequence, 7);
    assert_eq!(page.next_cursor.as_str(), "v1.cursor_7");
    assert!(!page.has_more);
    let requests = client.transport().requests();
    assert_eq!(requests[0].path(), "/api/v1/domains/domain-a/objects");
    assert_eq!(
        requests[0].query(),
        &[
            ("limit".to_owned(), "50".to_owned()),
            ("after_cursor".to_owned(), "v1.cursor_6".to_owned())
        ]
    );
    let debug = format!("{:?}", requests[0]);
    assert!(!debug.contains("v1.cursor_6"));
}

#[test]
fn discovery_rejects_non_monotonic_change_sequences() {
    let (object, _) = signed_object();
    let mut later = response_for(&object);
    later["object_id"] = serde_json::json!("object-b");
    later["change_sequence"] = serde_json::json!(9);
    let mut earlier = response_for(&object);
    earlier["change_sequence"] = serde_json::json!(8);
    let transport = RecordingTransport::default();
    transport.push_json(
        200,
        &serde_json::json!({
            "entries": [later, earlier],
            "next_cursor": "v1.cursor_9",
            "has_more": false
        }),
    );
    let client = SyncRemoteClient::new(transport);

    let error = client
        .discover_object_versions("domain-a", None, 100)
        .expect_err("out-of-order discovery page must fail");

    assert!(matches!(error, SyncRemoteError::InvalidResponse { .. }));
    assert!(error.to_string().contains("change_sequence"));
}

#[test]
fn stale_base_version_maps_latest_conflict_metadata_without_payload() {
    let (object, manifest) = signed_object();
    let transport = RecordingTransport::default();
    transport.push_json(
        409,
        &serde_json::json!({
            "error_code": "conflict_stale_base_version",
            "message": "base version is stale",
            "retryable": false,
            "server_time_ms": 123,
            "latest_version": 3,
            "latest_ciphertext_hash": "latest-hash"
        }),
    );
    let client = SyncRemoteClient::new(transport);

    let error = client
        .upload_object_version("domain-a", &object, &manifest)
        .expect_err("stale conflict");

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
                    version: 3,
                    ciphertext_hash: Some("latest-hash".to_owned()),
                })
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let debug = format!("{:?}", client.transport().requests()[0]);
    let payload_text = String::from_utf8_lossy(&object.envelope.encrypted_payload);
    assert!(!debug.contains(payload_text.as_ref()));
}

#[test]
fn server_error_codes_map_to_public_remote_errors() {
    let forbidden_transport = RecordingTransport::default();
    forbidden_transport.push_json(
        403,
        &serde_json::json!({
            "error_code": "forbidden_device",
            "message": "device cannot write",
            "retryable": false,
            "server_time_ms": 456
        }),
    );
    let client = SyncRemoteClient::new(forbidden_transport);

    let error = client
        .object_version("domain-a", "object-a", 1)
        .expect_err("forbidden");

    match error {
        SyncRemoteError::Server {
            status,
            code,
            retryable,
            server_time_ms,
            ..
        } => {
            assert_eq!(status, 403);
            assert_eq!(code, SyncServerErrorCode::ForbiddenDevice);
            assert!(!retryable);
            assert_eq!(server_time_ms, Some(456));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let unauthenticated_transport = RecordingTransport::default();
    unauthenticated_transport.push_json(
        401,
        &serde_json::json!({
            "error_code": "unauthenticated",
            "message": "access token is missing or invalid",
            "retryable": false,
            "server_time_ms": 789
        }),
    );
    let client = SyncRemoteClient::new(unauthenticated_transport);

    let error = client
        .object_version("domain-a", "object-a", 1)
        .expect_err("unauthenticated");

    match error {
        SyncRemoteError::Server {
            status,
            code,
            retryable,
            server_time_ms,
            ..
        } => {
            assert_eq!(status, 401);
            assert_eq!(code, SyncServerErrorCode::Unauthenticated);
            assert!(!retryable);
            assert_eq!(server_time_ms, Some(789));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn object_payload_reads_metadata_then_binary_payload() {
    let (object, _) = signed_object();
    let payload = object.envelope.encrypted_payload.clone();
    let transport = RecordingTransport::default();
    transport.push_json(200, &response_for(&object));
    transport.push_bytes(200, &payload);
    let client = SyncRemoteClient::new(transport);

    let downloaded = client
        .object_payload("domain-a", "object-a", 1)
        .expect("payload");

    assert_eq!(downloaded.object.object_id, "object-a");
    assert_eq!(downloaded.payload, payload);
    let debug = format!("{downloaded:?}");
    assert!(debug.contains("[redacted;"));
    assert!(!debug.contains(&Base64::encode_string(&payload)));
    let requests = client.transport().requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path(),
        "/api/v1/domains/domain-a/objects/object-a/versions/1"
    );
    assert_eq!(
        requests[1].path(),
        "/api/v1/domains/domain-a/objects/object-a/versions/1/payload"
    );
}

#[test]
fn object_payload_rejects_length_mismatch() {
    let (object, _) = signed_object();
    let transport = RecordingTransport::default();
    transport.push_json(200, &response_for(&object));
    transport.push_bytes(200, b"short");
    let client = SyncRemoteClient::new(transport);

    let error = client
        .object_payload("domain-a", "object-a", 1)
        .expect_err("length mismatch");

    assert!(matches!(error, SyncRemoteError::InvalidResponse { .. }));
    assert!(error.to_string().contains("payload length"));
}

#[test]
fn wrapped_epoch_source_uses_exact_locator_and_redacts_ciphertext() {
    let record = wrapped_epoch_record();
    let transport = RecordingTransport::default();
    transport.push_json(
        200,
        &serde_json::json!({
            "schema_version": record.schema_version,
            "algorithm": record.algorithm,
            "domain_id": record.domain_id,
            "recipient_device_id": record.recipient_device_id,
            "recipient_key_agreement_key_id": record.recipient_key_agreement_key_id,
            "wrapping_key_id": record.wrapping_key_id,
            "key_epoch": record.key_epoch,
            "nonce": Base64::encode_string(record.nonce.as_bytes()),
            "wrapped_key": Base64::encode_string(&record.wrapped_key),
            "ciphertext_hash": record.ciphertext_hash,
            "created_at_ms": record.created_at_ms,
            "signature_record_type": "device_authorization"
        }),
    );
    let client = SyncRemoteClient::new(transport);
    let mut source = RemoteWrappedEpochMaterialSource::new(
        &client,
        vec![RemoteWrappedEpochLocator::new(1, "wrapping-key-b-1").expect("locator")],
    )
    .expect("source");

    let loaded = source
        .load_wrapped_epoch_materials("domain-a", "device-b")
        .expect("load wrapped epoch");
    assert_eq!(loaded, vec![record.clone()]);
    let requests = client.transport().requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path(),
        "/api/v1/domains/domain-a/devices/device-b/wrapped-epochs/1"
    );
    assert_eq!(
        requests[0].query(),
        &[("wrapping_key_id".to_owned(), "wrapping-key-b-1".to_owned())]
    );
    let debug = format!("{record:?}");
    assert!(!debug.contains(&Base64::encode_string(&record.wrapped_key)));
}

#[test]
fn wrapped_epoch_source_maps_revoked_before_material_use() {
    let transport = RecordingTransport::default();
    transport.push_json(
        403,
        &serde_json::json!({
            "error_code": "forbidden_device",
            "message": "device is not active",
            "retryable": false,
            "server_time_ms": 123
        }),
    );
    let client = SyncRemoteClient::new(transport);
    let mut source = RemoteWrappedEpochMaterialSource::new(
        &client,
        vec![RemoteWrappedEpochLocator::new(2, "wrapping-key-b-2").expect("locator")],
    )
    .expect("source");

    let error = source
        .load_wrapped_epoch_materials("domain-a", "device-b")
        .expect_err("revoked device must fail");
    assert_eq!(error, SyncCryptoLoadError::Revoked);
}

#[test]
fn wrapped_epoch_source_rejects_distribution_without_trusted_signature_verification() {
    let record = wrapped_epoch_record();
    let transport = RecordingTransport::default();
    transport.push_json(
        200,
        &serde_json::json!({
            "schema_version": record.schema_version,
            "algorithm": record.algorithm,
            "domain_id": record.domain_id,
            "recipient_device_id": record.recipient_device_id,
            "recipient_key_agreement_key_id": record.recipient_key_agreement_key_id,
            "wrapping_key_id": record.wrapping_key_id,
            "key_epoch": record.key_epoch,
            "nonce": Base64::encode_string(record.nonce.as_bytes()),
            "wrapped_key": Base64::encode_string(&record.wrapped_key),
            "ciphertext_hash": record.ciphertext_hash,
            "created_at_ms": record.created_at_ms,
            "signature_record_type": "epoch_distribution"
        }),
    );
    let client = SyncRemoteClient::new(transport);

    let error = client
        .device_wrapped_epoch_material("domain-a", "device-b", 1, "wrapping-key-b-1")
        .expect_err("distribution requires the verified download path");

    assert!(matches!(error, SyncRemoteError::InvalidResponse { .. }));
}

#[test]
fn signed_epoch_distribution_upload_and_download_preserve_signed_metadata() {
    let signed_a = signed_epoch_distribution(wrapped_epoch_record_for(
        "device-a",
        "agreement-key-a",
        1,
        "wrapping-key-a-1",
    ));
    let signed = signed_epoch_distribution(wrapped_epoch_record());
    let records = vec![signed_a, signed.clone()];
    let trusted_domain = trusted_distribution_domain();
    let transport = RecordingTransport::default();
    transport.push_json(
        201,
        &serde_json::json!({
            "key_epoch": signed.material.key_epoch,
            "accepted_records": 2,
            "inserted_records": 2
        }),
    );
    transport.push_json(
        200,
        &serde_json::json!({
            "schema_version": signed.material.schema_version,
            "algorithm": signed.material.algorithm,
            "domain_id": signed.material.domain_id,
            "recipient_device_id": signed.material.recipient_device_id,
            "recipient_key_agreement_key_id": signed.material.recipient_key_agreement_key_id,
            "distributor_device_id": signed.distributor_device_id,
            "wrapping_key_id": signed.material.wrapping_key_id,
            "key_epoch": signed.material.key_epoch,
            "nonce": Base64::encode_string(signed.material.nonce.as_bytes()),
            "wrapped_key": Base64::encode_string(&signed.material.wrapped_key),
            "ciphertext_hash": signed.material.ciphertext_hash,
            "created_at_ms": signed.material.created_at_ms,
            "signature_record_type": "epoch_distribution",
            "signature_schema_version": signed.signature.signature_schema_version,
            "signature_algorithm": signed.signature.signature_algorithm.as_str(),
            "signature_key_id": signed.signature.signature_key_id,
            "signature": Base64::encode_string(&signed.signature.signature)
        }),
    );
    let client = SyncRemoteClient::new(transport);

    let uploaded = client
        .upload_epoch_distribution(&trusted_domain, &records)
        .expect("upload signed distribution");
    assert_eq!(uploaded.accepted_records, 2);
    assert_eq!(uploaded.inserted_records, 2);
    let downloaded = client
        .device_verified_epoch_distribution(
            &trusted_domain,
            "domain-a",
            "device-b",
            1,
            "wrapping-key-b-1",
        )
        .expect("download signed distribution");
    assert_eq!(downloaded, signed);

    let requests = client.transport().requests();
    assert_eq!(
        requests[0].path(),
        "/api/v1/domains/domain-a/epoch-distributions"
    );
    let body: Value = serde_json::from_slice(requests[0].body()).expect("distribution json");
    assert_eq!(body["distributor_device_id"], "device-a");
    assert_eq!(
        body["records"][1]["recipient_key_agreement_key_id"],
        "agreement-key-b"
    );
    assert_eq!(body["records"][1]["signature_key_id"], "signing-key-a");
    let debug = format!("{downloaded:?}");
    assert!(!debug.contains(&Base64::encode_string(&downloaded.material.wrapped_key)));
}

fn wrapped_epoch_record() -> WrappedEpochMaterial {
    wrapped_epoch_record_for("device-b", "agreement-key-b", 7, "wrapping-key-b-1")
}

fn wrapped_epoch_record_for(
    device_id: &str,
    key_id: &str,
    scalar: u8,
    wrapping_key_id: &str,
) -> WrappedEpochMaterial {
    let secret = SecretKey::from_slice(&[scalar; 32]).expect("agreement secret");
    let public = secret.public_key().to_encoded_point(false);
    let recipient =
        DeviceKeyAgreementPublicKey::p256(device_id, key_id, public.as_bytes().to_vec(), 100, None)
            .expect("recipient");
    let object_key = KeyDescriptor::new("object-key-v1", KeyRole::ObjectKey, 1).expect("key");
    let master_key = SyncMasterKeyMaterial::new([9u8; 32]).expect("master key");
    WrappedEpochMaterial::seal_for_recipient(
        "domain-a",
        &recipient,
        wrapping_key_id,
        &object_key,
        &master_key,
        Nonce::new(vec![3u8; 24]).expect("nonce"),
        200,
    )
    .expect("wrapped epoch")
}

fn trusted_distribution_domain() -> SyncTrustedDomainState {
    let mut store = TestMemoryDeviceKeyStore::new();
    let public_a = store
        .insert_signing_key("device-a", "signing-key-a", [8; 32], 100)
        .expect("device a signing key");
    let public_b = store
        .insert_signing_key("device-b", "signing-key-b", [9; 32], 100)
        .expect("device b signing key");
    let profile_a = crate::SyncTrustedDeviceProfile::active_with_key_agreement(
        SyncDevice::pending("device-a", "signing-key-a", 99)
            .expect("pending a")
            .activate(100)
            .expect("active a"),
        public_a,
        agreement_profile("device-a", "agreement-key-a", 1),
    )
    .expect("trusted a");
    let profile_b = crate::SyncTrustedDeviceProfile::active_with_key_agreement(
        SyncDevice::pending("device-b", "signing-key-b", 99)
            .expect("pending b")
            .activate(100)
            .expect("active b"),
        public_b,
        agreement_profile("device-b", "agreement-key-b", 7),
    )
    .expect("trusted b");
    SyncTrustedDomainState::new(
        SyncDomain::new("domain-a", 1, "object-key-v1", 100, 200).expect("domain"),
        [profile_a, profile_b],
    )
    .expect("trusted domain")
}

fn agreement_profile(device_id: &str, key_id: &str, scalar: u8) -> DeviceKeyAgreementPublicKey {
    let secret = SecretKey::from_slice(&[scalar; 32]).expect("agreement secret");
    DeviceKeyAgreementPublicKey::p256(
        device_id,
        key_id,
        secret
            .public_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec(),
        100,
        None,
    )
    .expect("agreement profile")
}

fn signed_epoch_distribution(material: WrappedEpochMaterial) -> SignedEpochDistribution {
    let mut store = TestMemoryDeviceKeyStore::new();
    store
        .insert_signing_key("device-a", "signing-key-a", [8; 32], 100)
        .expect("insert signing key");
    let handle = store
        .handle("device-a", "signing-key-a")
        .expect("signing handle");
    let placeholder =
        DeviceSignature::new("signing-key-a", "device-a", vec![1; ED25519_SIGNATURE_LEN])
            .expect("placeholder signature");
    let unsigned = SignedEpochDistribution::new("device-a", material, placeholder)
        .expect("unsigned-shaped distribution");
    let signature = store
        .sign(&handle, &unsigned.canonical_bytes())
        .expect("distribution signature");
    SignedEpochDistribution::new("device-a", unsigned.material, signature)
        .expect("signed distribution")
}
