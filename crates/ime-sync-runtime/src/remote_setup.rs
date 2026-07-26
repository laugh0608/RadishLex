use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{
    canonical_signature_bytes, DeviceKeyAgreementPublicKey, DeviceSignature,
    DeviceSigningPublicKey, KeyDescriptor, KeyRole, Nonce, SignatureField, SyncMasterKeyMaterial,
    TestMemoryDeviceKeyStore, WrappedEpochMaterial, SIGNATURE_ALGORITHM_ED25519_V1,
    SIGNATURE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    device_join_profile_challenge, HttpSyncRemoteTransport, RemoteLifecycleDevice,
    SyncDeviceStatus, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteTransport,
};
use serde_json::{json, Value};

use crate::{QualificationError, QualificationErrorCode, QualificationPhase, QualificationRequest};

const P256_GENERATOR_PUBLIC_KEY: [u8; 65] = [
    0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
    0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2,
    0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e,
    0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51,
    0xf5,
];

#[derive(Debug, Clone)]
pub(crate) struct QualificationIds {
    pub(crate) domain_id: String,
    pub(crate) device_a_id: String,
    pub(crate) device_b_id: String,
    pub(crate) signing_key_a_id: String,
    pub(crate) signing_key_b_id: String,
    pub(crate) agreement_key_a_id: String,
    pub(crate) agreement_key_b_id: String,
    pub(crate) object_key_id: String,
    pub(crate) join_request_id: String,
    pub(crate) wrapping_key_id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) master_key_byte: u8,
    nonce_byte: u8,
}

impl QualificationIds {
    pub(crate) fn new(sequence: u64, created_at_ms: i64) -> Self {
        let suffix = format!("{}-{sequence}", std::process::id());
        Self {
            domain_id: format!("qualification-domain-{suffix}"),
            device_a_id: format!("qualification-device-a-{suffix}"),
            device_b_id: format!("qualification-device-b-{suffix}"),
            signing_key_a_id: format!("qualification-signing-a-{suffix}"),
            signing_key_b_id: format!("qualification-signing-b-{suffix}"),
            agreement_key_a_id: format!("qualification-agreement-a-{suffix}"),
            agreement_key_b_id: format!("qualification-agreement-b-{suffix}"),
            object_key_id: format!("qualification-object-key-{suffix}"),
            join_request_id: format!("qualification-join-{suffix}"),
            wrapping_key_id: format!("qualification-wrapping-{suffix}"),
            created_at_ms,
            master_key_byte: ((sequence % 127) as u8).saturating_add(64),
            nonce_byte: ((sequence % 251) as u8).saturating_add(1),
        }
    }
}

pub(crate) fn build_transport(
    request: &QualificationRequest,
) -> Result<HttpSyncRemoteTransport, QualificationError> {
    let mut transport =
        HttpSyncRemoteTransport::with_timeout(request.endpoint().to_owned(), request.timeout())
            .map_err(|error| map_remote_error(error, QualificationPhase::ValidateRequest))?
            .with_bearer_access_token(request.access_token_copy())
            .map_err(|error| map_remote_error(error, QualificationPhase::ValidateRequest))?;
    if let Some(certificate) = request.local_ca_der_copy() {
        transport = transport
            .with_additional_root_certificate_der(certificate)
            .map_err(|error| map_remote_error(error, QualificationPhase::ValidateRequest))?;
    }
    Ok(transport)
}

pub(crate) fn create_domain(
    transport: &HttpSyncRemoteTransport,
    ids: &QualificationIds,
    public_key_a: &DeviceSigningPublicKey,
) -> Result<(), QualificationError> {
    let body = json!({
        "domain_id": ids.domain_id,
        "current_key_epoch": 1,
        "active_key_id": "qualification-sync-key-v1",
        "first_device": {
            "device_id": ids.device_a_id,
            "signing_algorithm": public_key_a.signature_algorithm.as_str(),
            "signing_public_key_id": ids.signing_key_a_id,
            "signing_public_key": b64(&public_key_a.public_key),
            "key_agreement_public_key_id": ids.agreement_key_a_id,
            "key_agreement_public_key": b64(&P256_GENERATOR_PUBLIC_KEY),
            "status": "active"
        },
        "created_at_ms": ids.created_at_ms,
        "updated_at_ms": ids.created_at_ms
    });
    expect_status(
        send_json(transport, SyncRemoteMethod::Post, "/api/v1/domains", body)?,
        201,
        QualificationPhase::CreateDomain,
    )
}

pub(crate) fn authorize_second_device(
    transport: &HttpSyncRemoteTransport,
    ids: &QualificationIds,
    signing_store: &TestMemoryDeviceKeyStore,
    public_key_b: &DeviceSigningPublicKey,
) -> Result<(), QualificationError> {
    let join_created_at_ms = ids.created_at_ms + 10;
    let authorization_created_at_ms = ids.created_at_ms + 20;
    let join_expires_at_ms = ids.created_at_ms + 120_000;
    let lifecycle_device = RemoteLifecycleDevice {
        domain_id: ids.domain_id.clone(),
        device_id: ids.device_b_id.clone(),
        signing_algorithm: public_key_b.signature_algorithm.as_str().to_owned(),
        signing_public_key_id: ids.signing_key_b_id.clone(),
        signing_public_key: public_key_b.public_key.clone(),
        key_agreement_public_key_id: ids.agreement_key_b_id.clone(),
        key_agreement_public_key: P256_GENERATOR_PUBLIC_KEY.to_vec(),
        status: SyncDeviceStatus::Active,
        authorized_at_ms: Some(authorization_created_at_ms),
        revoked_at_ms: None,
        last_seen_at_ms: None,
    };
    let challenge = device_join_profile_challenge(
        &ids.domain_id,
        &ids.join_request_id,
        &lifecycle_device,
        join_created_at_ms,
        join_expires_at_ms,
    );
    let join_body = json!({
        "join_request_id": ids.join_request_id,
        "device_id": ids.device_b_id,
        "signing_algorithm": public_key_b.signature_algorithm.as_str(),
        "signing_public_key_id": ids.signing_key_b_id,
        "signing_public_key": b64(&public_key_b.public_key),
        "key_agreement_public_key_id": ids.agreement_key_b_id,
        "key_agreement_public_key": b64(&P256_GENERATOR_PUBLIC_KEY),
        "challenge": b64(challenge.as_bytes()),
        "created_at_ms": join_created_at_ms,
        "expires_at_ms": join_expires_at_ms
    });
    let join_path = format!("/api/v1/domains/{}/join-requests", ids.domain_id);
    expect_status(
        send_json(transport, SyncRemoteMethod::Post, &join_path, join_body)?,
        201,
        QualificationPhase::AuthorizeSecondDevice,
    )?;

    let recipient = DeviceKeyAgreementPublicKey::p256(
        &ids.device_b_id,
        &ids.agreement_key_b_id,
        P256_GENERATOR_PUBLIC_KEY,
        ids.created_at_ms,
        None,
    )
    .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?;
    let object_key = KeyDescriptor::new(&ids.object_key_id, KeyRole::ObjectKey, 1)
        .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?;
    let master_key = SyncMasterKeyMaterial::new([ids.master_key_byte; 32])
        .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?;
    let wrapped_epoch = WrappedEpochMaterial::seal_for_recipient(
        &ids.domain_id,
        &recipient,
        &ids.wrapping_key_id,
        &object_key,
        &master_key,
        Nonce::new(vec![ids.nonce_byte; 24])
            .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?,
        authorization_created_at_ms,
    )
    .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?;
    let signature = sign_authorization(
        signing_store,
        ids,
        challenge.as_bytes(),
        wrapped_epoch.wrapped_key.len(),
        authorization_created_at_ms,
    )?;
    let authorization_body = json!({
        "authorization": {
            "authorizer_device_id": ids.device_a_id,
            "recipient_device_id": ids.device_b_id,
            "recipient_signing_public_key_id": ids.signing_key_b_id,
            "recipient_key_agreement_key_id": ids.agreement_key_b_id,
            "join_short_code": "314159",
            "key_epoch": 1,
            "created_at_ms": authorization_created_at_ms,
            "signature_schema_version": SIGNATURE_SCHEMA_VERSION,
            "signature_algorithm": SIGNATURE_ALGORITHM_ED25519_V1,
            "signature_key_id": ids.signing_key_a_id,
            "signature": b64(&signature.signature)
        },
        "wrapping": {
            "authorizer_device_id": ids.device_a_id,
            "recipient_device_id": ids.device_b_id,
            "recipient_key_agreement_key_id": ids.agreement_key_b_id,
            "key_epoch": 1,
            "wrapping_key_id": ids.wrapping_key_id,
            "algorithm": wrapped_epoch.algorithm,
            "nonce": b64(wrapped_epoch.nonce.as_bytes()),
            "wrapped_key_len": wrapped_epoch.wrapped_key.len(),
            "ciphertext_hash": wrapped_epoch.ciphertext_hash,
            "created_at_ms": authorization_created_at_ms,
            "signature": b64(b"qualification-wrapping-record")
        },
        "wrapped_key": b64(&wrapped_epoch.wrapped_key)
    });
    let authorization_path = format!(
        "/api/v1/domains/{}/join-requests/{}/authorization",
        ids.domain_id, ids.join_request_id
    );
    expect_status(
        send_json(
            transport,
            SyncRemoteMethod::Post,
            &authorization_path,
            authorization_body,
        )?,
        204,
        QualificationPhase::AuthorizeSecondDevice,
    )
}

fn sign_authorization(
    signing_store: &TestMemoryDeviceKeyStore,
    ids: &QualificationIds,
    challenge: &[u8],
    wrapped_key_len: usize,
    created_at_ms: i64,
) -> Result<DeviceSignature, QualificationError> {
    let fields = [
        SignatureField::u16("signature_schema_version", SIGNATURE_SCHEMA_VERSION),
        SignatureField::text("signature_algorithm", SIGNATURE_ALGORITHM_ED25519_V1),
        SignatureField::text("signature_key_id", &ids.signing_key_a_id),
        SignatureField::text("authorizer_device_id", &ids.device_a_id),
        SignatureField::text("recipient_device_id", &ids.device_b_id),
        SignatureField::text("recipient_public_key_id", &ids.signing_key_b_id),
        SignatureField::bytes("join_challenge", challenge),
        SignatureField::text("join_short_code", "314159"),
        SignatureField::u64("key_epoch", 1),
        SignatureField::text("wrapping_key_id", &ids.wrapping_key_id),
        SignatureField::usize("encrypted_key_len", wrapped_key_len),
        SignatureField::i64("created_at_ms", created_at_ms),
    ];
    let handle = signing_store
        .handle(&ids.device_a_id, &ids.signing_key_a_id)
        .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))?;
    signing_store
        .sign(
            &handle,
            &canonical_signature_bytes("device_authorization", &fields),
        )
        .map_err(|_| crypto_error(QualificationPhase::AuthorizeSecondDevice))
}

fn send_json(
    transport: &HttpSyncRemoteTransport,
    method: SyncRemoteMethod,
    path: &str,
    body: Value,
) -> Result<radishlex_ime_sync::SyncRemoteResponse, QualificationError> {
    let bytes = serde_json::to_vec(&body).map_err(|_| {
        QualificationError::new(
            QualificationErrorCode::Internal,
            QualificationPhase::PrepareWorkspace,
            false,
        )
    })?;
    transport
        .send(SyncRemoteRequest::new(
            method,
            path,
            Some("application/json".to_owned()),
            bytes,
        ))
        .map_err(|error| map_remote_error(error, phase_for_path(path)))
}

fn expect_status(
    response: radishlex_ime_sync::SyncRemoteResponse,
    expected: u16,
    phase: QualificationPhase,
) -> Result<(), QualificationError> {
    if response.status == expected {
        return Ok(());
    }
    let error_code = serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|value| {
            value
                .get("error_code")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let code = if response.status == 401 || error_code.as_deref() == Some("unauthenticated") {
        QualificationErrorCode::Unauthenticated
    } else {
        QualificationErrorCode::ProtocolRejected
    };
    Err(QualificationError::new(code, phase, false))
}

pub(crate) fn map_remote_error(
    error: SyncRemoteError,
    phase: QualificationPhase,
) -> QualificationError {
    let (code, retryable) = match error {
        SyncRemoteError::InvalidRequest { .. } => (QualificationErrorCode::InvalidRequest, false),
        SyncRemoteError::Transport { message } if message.contains("tls") => {
            (QualificationErrorCode::TlsRejected, false)
        }
        SyncRemoteError::Transport { message }
            if message.contains("timed out") || message.contains("timeout") =>
        {
            (QualificationErrorCode::TransportTimeout, true)
        }
        SyncRemoteError::Transport { .. } => (QualificationErrorCode::ServerUnavailable, true),
        SyncRemoteError::InvalidResponse { .. } => {
            (QualificationErrorCode::ProtocolRejected, false)
        }
        SyncRemoteError::Server { status: 401, .. } => {
            (QualificationErrorCode::Unauthenticated, false)
        }
        SyncRemoteError::Server { retryable, .. } => {
            (QualificationErrorCode::ProtocolRejected, retryable)
        }
    };
    QualificationError::new(code, phase, retryable)
}

fn crypto_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::CryptoRejected, phase, false)
}

fn b64(bytes: &[u8]) -> String {
    Base64::encode_string(bytes)
}

fn phase_for_path(path: &str) -> QualificationPhase {
    if path == "/api/v1/domains" {
        QualificationPhase::CreateDomain
    } else {
        QualificationPhase::AuthorizeSecondDevice
    }
}
