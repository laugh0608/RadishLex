use std::fmt;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::{Nonce, SignedSyncObjectManifest, WrappedEpochMaterial};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::assemble::AssembledSyncObject;
use crate::device::{SyncDeviceStatus, SyncDomain};
use crate::model::{SyncObjectType, SyncPayloadError};
use crate::product_provider::{SyncCryptoLoadError, SyncWrappedEpochMaterialSource};

mod epoch_distribution_api;
mod lifecycle_api;
mod lifecycle_dto;
mod recovery_activation_api;
mod recovery_api;
mod recovery_revocation_api;
mod wrapped_epoch_source;

pub use epoch_distribution_api::RemoteEpochDistributionResult;
pub use lifecycle_api::{
    RemoteDeviceAuthorization, RemoteDeviceRevocation, RemoteLifecycleDevice, RemoteLifecycleEvent,
    RemoteLifecycleEventKind, RemoteLifecyclePage, RemoteLifecycleSnapshot,
    RemoteRecoveredDeviceActivation, RemoteRecoveryRecordRevocation, RemoteRecoveryRecordRotation,
};
pub use recovery_activation_api::RemoteRecoveredDeviceResult;
pub use recovery_api::RemoteVerifiedRecoveryRecord;
pub use recovery_revocation_api::RemoteRecoveryRecordRevocationResult;
pub use wrapped_epoch_source::{RemoteWrappedEpochLocator, RemoteWrappedEpochMaterialSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRemoteMethod {
    Get,
    Post,
}

impl SyncRemoteMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SyncRemoteRequest {
    method: SyncRemoteMethod,
    path: String,
    query: Vec<(String, String)>,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl SyncRemoteRequest {
    pub fn new(
        method: SyncRemoteMethod,
        path: impl Into<String>,
        content_type: Option<String>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            content_type,
            body: body.into(),
        }
    }

    pub fn with_query_param(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SyncRemoteError> {
        let name = name.into();
        let value = value.into();
        validate_query_component("query name", &name, false)?;
        validate_query_component("query value", &value, true)?;
        self.query.push((name, value));
        Ok(self)
    }

    pub fn method(&self) -> SyncRemoteMethod {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn query(&self) -> &[(String, String)] {
        &self.query
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for SyncRemoteRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncRemoteRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field(
                "query",
                &format_args!("[redacted values; {} parameters]", self.query.len()),
            )
            .field("content_type", &self.content_type)
            .field(
                "body",
                &format_args!("[redacted; {} bytes]", self.body.len()),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRemoteResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl SyncRemoteResponse {
    pub fn new(status: u16, content_type: Option<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type,
            body: body.into(),
        }
    }

    pub fn json<T: Serialize>(status: u16, value: &T) -> Result<Self, SyncRemoteError> {
        Ok(Self::new(
            status,
            Some("application/json".to_owned()),
            serde_json::to_vec(value).map_err(SyncRemoteError::from_json_error)?,
        ))
    }
}

pub trait SyncRemoteTransport {
    fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncServerErrorCode {
    InvalidRequest,
    Unauthenticated,
    InvalidCiphertextMetadata,
    InvalidSignature,
    ForbiddenDevice,
    NotFound,
    ConflictStaleBaseVersion,
    ConflictObjectVersion,
    ConflictEpochDistribution,
    ConflictRecoveryRecord,
    PayloadTooLarge,
    RecoveryRateLimited,
    StorageUnavailable,
    Unknown,
}

impl SyncServerErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthenticated => "unauthenticated",
            Self::InvalidCiphertextMetadata => "invalid_ciphertext_metadata",
            Self::InvalidSignature => "invalid_signature",
            Self::ForbiddenDevice => "forbidden_device",
            Self::NotFound => "not_found",
            Self::ConflictStaleBaseVersion => "conflict_stale_base_version",
            Self::ConflictObjectVersion => "conflict_object_version",
            Self::ConflictEpochDistribution => "conflict_epoch_distribution",
            Self::ConflictRecoveryRecord => "conflict_recovery_record",
            Self::PayloadTooLarge => "payload_too_large",
            Self::RecoveryRateLimited => "recovery_rate_limited",
            Self::StorageUnavailable => "storage_unavailable",
            Self::Unknown => "unknown",
        }
    }

    fn from_server_code(value: &str) -> Self {
        match value {
            "invalid_request" => Self::InvalidRequest,
            "unauthenticated" => Self::Unauthenticated,
            "invalid_ciphertext_metadata" => Self::InvalidCiphertextMetadata,
            "invalid_signature" => Self::InvalidSignature,
            "forbidden_device" => Self::ForbiddenDevice,
            "not_found" => Self::NotFound,
            "conflict_stale_base_version" => Self::ConflictStaleBaseVersion,
            "conflict_object_version" => Self::ConflictObjectVersion,
            "conflict_epoch_distribution" => Self::ConflictEpochDistribution,
            "conflict_recovery_record" => Self::ConflictRecoveryRecord,
            "payload_too_large" => Self::PayloadTooLarge,
            "recovery_rate_limited" => Self::RecoveryRateLimited,
            "storage_unavailable" => Self::StorageUnavailable,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for SyncServerErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestObjectConflictMetadata {
    pub version: u64,
    pub ciphertext_hash: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct OpaqueSyncCursor(String);

impl OpaqueSyncCursor {
    pub fn new(value: impl Into<String>) -> Result<Self, SyncRemoteError> {
        let value = value.into();
        validate_query_component("sync cursor", &value, true)?;
        if value.len() > 256 {
            return invalid_request("sync cursor must be at most 256 bytes");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for OpaqueSyncCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[opaque sync cursor]")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum SyncRemoteError {
    InvalidRequest {
        message: String,
    },
    Transport {
        message: String,
    },
    InvalidResponse {
        message: String,
    },
    Server {
        status: u16,
        code: SyncServerErrorCode,
        message: String,
        retryable: bool,
        server_time_ms: Option<i64>,
        latest: Option<LatestObjectConflictMetadata>,
    },
}

impl SyncRemoteError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
        }
    }

    fn from_payload_error(error: SyncPayloadError) -> Self {
        Self::InvalidRequest {
            message: error.to_string(),
        }
    }

    fn from_crypto_error(error: radishlex_ime_crypto::CryptoError) -> Self {
        Self::InvalidRequest {
            message: error.to_string(),
        }
    }

    fn from_json_error(error: serde_json::Error) -> Self {
        Self::InvalidResponse {
            message: error.to_string(),
        }
    }
}

impl fmt::Debug for SyncRemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { message } => f
                .debug_struct("InvalidRequest")
                .field("message", message)
                .finish(),
            Self::Transport { message } => f
                .debug_struct("Transport")
                .field("message", message)
                .finish(),
            Self::InvalidResponse { message } => f
                .debug_struct("InvalidResponse")
                .field("message", message)
                .finish(),
            Self::Server {
                status,
                code,
                message,
                retryable,
                server_time_ms,
                latest,
            } => f
                .debug_struct("Server")
                .field("status", status)
                .field("code", code)
                .field("message", message)
                .field("retryable", retryable)
                .field("server_time_ms", server_time_ms)
                .field("latest", latest)
                .finish(),
        }
    }
}

impl fmt::Display for SyncRemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { message } => write!(f, "invalid remote sync request: {message}"),
            Self::Transport { message } => write!(f, "remote sync transport failed: {message}"),
            Self::InvalidResponse { message } => {
                write!(f, "invalid remote sync response: {message}")
            }
            Self::Server {
                status,
                code,
                message,
                ..
            } => write!(f, "sync server returned {status} {code}: {message}"),
        }
    }
}

impl std::error::Error for SyncRemoteError {}

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteObjectVersion {
    pub domain_id: String,
    pub object_id: String,
    pub object_type: SyncObjectType,
    pub version: u64,
    pub base_version: Option<u64>,
    pub change_sequence: u64,
    pub owner_device_id: String,
    pub key_id: String,
    pub key_epoch: u64,
    pub algorithm: String,
    pub nonce: Vec<u8>,
    pub encrypted_payload_len: usize,
    pub ciphertext_hash: String,
    pub signature_schema_version: u16,
    pub signature_algorithm: String,
    pub signature_key_id: String,
    pub signature: Vec<u8>,
    pub server_received_at_ms: i64,
    pub client_created_at_ms: i64,
    pub client_updated_at_ms: i64,
}

impl fmt::Debug for RemoteObjectVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteObjectVersion")
            .field("domain_id", &self.domain_id)
            .field("object_id", &self.object_id)
            .field("object_type", &self.object_type)
            .field("version", &self.version)
            .field("base_version", &self.base_version)
            .field("change_sequence", &self.change_sequence)
            .field("owner_device_id", &self.owner_device_id)
            .field("key_id", &self.key_id)
            .field("key_epoch", &self.key_epoch)
            .field("algorithm", &self.algorithm)
            .field(
                "nonce",
                &format_args!("[redacted; {} bytes]", self.nonce.len()),
            )
            .field("encrypted_payload_len", &self.encrypted_payload_len)
            .field("ciphertext_hash", &self.ciphertext_hash)
            .field("signature_schema_version", &self.signature_schema_version)
            .field("signature_algorithm", &self.signature_algorithm)
            .field("signature_key_id", &self.signature_key_id)
            .field(
                "signature",
                &format_args!("[redacted; {} bytes]", self.signature.len()),
            )
            .field("server_received_at_ms", &self.server_received_at_ms)
            .field("client_created_at_ms", &self.client_created_at_ms)
            .field("client_updated_at_ms", &self.client_updated_at_ms)
            .finish()
    }
}

impl RemoteObjectVersion {
    pub fn validate(&self) -> Result<(), SyncRemoteError> {
        validate_path_segment("domain_id", &self.domain_id)?;
        validate_path_segment("object_id", &self.object_id)?;
        validate_path_segment("owner_device_id", &self.owner_device_id)?;
        validate_required("key_id", &self.key_id)?;
        validate_required("algorithm", &self.algorithm)?;
        validate_required("ciphertext_hash", &self.ciphertext_hash)?;
        validate_required("signature_algorithm", &self.signature_algorithm)?;
        validate_required("signature_key_id", &self.signature_key_id)?;
        if self.version == 0 {
            return invalid_request("version must be greater than 0");
        }
        if self.change_sequence == 0 {
            return invalid_request("change_sequence must be greater than 0");
        }
        if let Some(base_version) = self.base_version {
            if base_version >= self.version {
                return invalid_request("base_version must be lower than version");
            }
        }
        if self.key_epoch == 0 {
            return invalid_request("key_epoch must be greater than 0");
        }
        if self.nonce.is_empty() {
            return invalid_request("nonce cannot be empty");
        }
        if self.encrypted_payload_len == 0 {
            return invalid_request("encrypted_payload_len must be greater than 0");
        }
        if self.signature.is_empty() {
            return invalid_request("signature cannot be empty");
        }
        if self.client_updated_at_ms < self.client_created_at_ms {
            return invalid_request("client_updated_at_ms must be >= client_created_at_ms");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteObjectDiscoveryPage {
    pub entries: Vec<RemoteObjectVersion>,
    pub next_cursor: OpaqueSyncCursor,
    pub has_more: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteObjectPayload {
    pub object: RemoteObjectVersion,
    pub payload: Vec<u8>,
}

impl fmt::Debug for RemoteObjectPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteObjectPayload")
            .field("object", &self.object)
            .field(
                "payload",
                &format_args!("[redacted; {} bytes]", self.payload.len()),
            )
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct SyncRemoteClient<T> {
    transport: T,
    api_prefix: String,
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            api_prefix: "/api/v1".to_owned(),
        }
    }

    pub fn with_api_prefix(
        transport: T,
        api_prefix: impl Into<String>,
    ) -> Result<Self, SyncRemoteError> {
        let api_prefix = api_prefix.into();
        if !api_prefix.starts_with('/') || api_prefix.ends_with('/') {
            return invalid_request("api_prefix must start with '/' and must not end with '/'");
        }
        Ok(Self {
            transport,
            api_prefix,
        })
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn upload_object_version(
        &self,
        domain_id: &str,
        object: &AssembledSyncObject,
        manifest: &SignedSyncObjectManifest,
    ) -> Result<RemoteObjectVersion, SyncRemoteError> {
        validate_manifest_matches_object(domain_id, object, manifest)?;
        let request = ObjectVersionUploadDto::from_object(object, manifest);
        let path = self.object_versions_path(domain_id, &object.draft.object_id)?;
        self.send_json(SyncRemoteMethod::Post, path, &request)
    }

    pub fn discover_object_versions(
        &self,
        domain_id: &str,
        after_cursor: Option<&OpaqueSyncCursor>,
        limit: u16,
    ) -> Result<RemoteObjectDiscoveryPage, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        if limit == 0 || limit > 200 {
            return invalid_request("discovery limit must be between 1 and 200");
        }
        let path = format!("{}/domains/{domain_id}/objects", self.api_prefix);
        let mut request = SyncRemoteRequest::new(SyncRemoteMethod::Get, path, None, Vec::new())
            .with_query_param("limit", limit.to_string())?;
        if let Some(cursor) = after_cursor {
            request = request.with_query_param("after_cursor", cursor.as_str())?;
        }
        let response = self.transport.send(request)?;
        let dto: ObjectDiscoveryResponseDto = decode_json_response(response)?;
        dto.try_into()
    }

    pub fn object_version(
        &self,
        domain_id: &str,
        object_id: &str,
        version: u64,
    ) -> Result<RemoteObjectVersion, SyncRemoteError> {
        let path = self.object_version_path(domain_id, object_id, version)?;
        self.send_empty(SyncRemoteMethod::Get, path)
    }

    pub fn object_payload(
        &self,
        domain_id: &str,
        object_id: &str,
        version: u64,
    ) -> Result<RemoteObjectPayload, SyncRemoteError> {
        let object = self.object_version(domain_id, object_id, version)?;
        let path = self.object_payload_path(domain_id, object_id, version)?;
        let response = self.transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            path,
            None,
            Vec::new(),
        ))?;
        if response.status == 200 {
            if response.body.len() != object.encrypted_payload_len {
                return Err(SyncRemoteError::InvalidResponse {
                    message: "payload length does not match object metadata".to_owned(),
                });
            }
            return Ok(RemoteObjectPayload {
                object,
                payload: response.body,
            });
        }
        Err(decode_error_response(response))
    }

    pub fn device_wrapped_epoch_material(
        &self,
        domain_id: &str,
        recipient_device_id: &str,
        key_epoch: u64,
        wrapping_key_id: &str,
    ) -> Result<WrappedEpochMaterial, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        validate_path_segment("recipient_device_id", recipient_device_id)?;
        if key_epoch == 0 {
            return invalid_request("key_epoch must be greater than zero");
        }
        validate_query_component("wrapping_key_id", wrapping_key_id, true)?;
        let path = format!(
            "{}/domains/{domain_id}/devices/{recipient_device_id}/wrapped-epochs/{key_epoch}",
            self.api_prefix
        );
        let request = SyncRemoteRequest::new(SyncRemoteMethod::Get, path, None, Vec::new())
            .with_query_param("wrapping_key_id", wrapping_key_id)?;
        let response = self.transport.send(request)?;
        let dto = decode_json_response::<DeviceWrappedEpochResponseDto>(response)?;
        if dto.signature_record_type != "device_authorization" {
            return invalid_response(
                "wrapped epoch source requires a lifecycle authorization record",
            );
        }
        let record = WrappedEpochMaterial::try_from(dto)?;
        if record.domain_id != domain_id
            || record.recipient_device_id != recipient_device_id
            || record.key_epoch != key_epoch
            || record.wrapping_key_id != wrapping_key_id
        {
            return invalid_response("wrapped epoch response does not match request locator");
        }
        Ok(record)
    }

    fn send_empty<R>(&self, method: SyncRemoteMethod, path: String) -> Result<R, SyncRemoteError>
    where
        R: for<'de> Deserialize<'de>,
    {
        let response =
            self.transport
                .send(SyncRemoteRequest::new(method, path, None, Vec::new()))?;
        decode_json_response(response)
    }

    fn send_json<R, B>(
        &self,
        method: SyncRemoteMethod,
        path: String,
        body: &B,
    ) -> Result<R, SyncRemoteError>
    where
        R: for<'de> Deserialize<'de>,
        B: Serialize,
    {
        let body = serde_json::to_vec(body).map_err(SyncRemoteError::from_json_error)?;
        let response = self.transport.send(SyncRemoteRequest::new(
            method,
            path,
            Some("application/json".to_owned()),
            body,
        ))?;
        decode_json_response(response)
    }

    fn object_versions_path(
        &self,
        domain_id: &str,
        object_id: &str,
    ) -> Result<String, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        validate_path_segment("object_id", object_id)?;
        Ok(format!(
            "{}/domains/{domain_id}/objects/{object_id}/versions",
            self.api_prefix
        ))
    }

    fn object_version_path(
        &self,
        domain_id: &str,
        object_id: &str,
        version: u64,
    ) -> Result<String, SyncRemoteError> {
        if version == 0 {
            return invalid_request("version must be greater than 0");
        }
        Ok(format!(
            "{}/{}",
            self.object_versions_path(domain_id, object_id)?,
            version
        ))
    }

    fn object_payload_path(
        &self,
        domain_id: &str,
        object_id: &str,
        version: u64,
    ) -> Result<String, SyncRemoteError> {
        Ok(format!(
            "{}/payload",
            self.object_version_path(domain_id, object_id, version)?
        ))
    }
}

#[derive(Debug, Deserialize)]
struct DeviceWrappedEpochResponseDto {
    schema_version: u16,
    algorithm: String,
    domain_id: String,
    recipient_device_id: String,
    recipient_key_agreement_key_id: String,
    wrapping_key_id: String,
    key_epoch: u64,
    #[serde(with = "base64_bytes")]
    nonce: Vec<u8>,
    #[serde(with = "base64_bytes")]
    wrapped_key: Vec<u8>,
    ciphertext_hash: String,
    created_at_ms: i64,
    #[serde(default)]
    distributor_device_id: String,
    #[serde(default)]
    signature_record_type: String,
    #[serde(default)]
    signature_schema_version: u16,
    #[serde(default)]
    signature_algorithm: String,
    #[serde(default)]
    signature_key_id: String,
    #[serde(default, with = "base64_bytes")]
    signature: Vec<u8>,
}

impl TryFrom<DeviceWrappedEpochResponseDto> for WrappedEpochMaterial {
    type Error = SyncRemoteError;

    fn try_from(value: DeviceWrappedEpochResponseDto) -> Result<Self, Self::Error> {
        let record = Self {
            schema_version: value.schema_version,
            algorithm: value.algorithm,
            domain_id: value.domain_id,
            recipient_device_id: value.recipient_device_id,
            recipient_key_agreement_key_id: value.recipient_key_agreement_key_id,
            wrapping_key_id: value.wrapping_key_id,
            key_epoch: value.key_epoch,
            nonce: Nonce::new(value.nonce).map_err(|error| SyncRemoteError::InvalidResponse {
                message: error.to_string(),
            })?,
            wrapped_key: value.wrapped_key,
            ciphertext_hash: value.ciphertext_hash,
            created_at_ms: value.created_at_ms,
        };
        record
            .validate()
            .map_err(|error| SyncRemoteError::InvalidResponse {
                message: error.to_string(),
            })?;
        Ok(record)
    }
}

fn map_wrapped_epoch_remote_error(error: SyncRemoteError) -> SyncCryptoLoadError {
    match error {
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ForbiddenDevice,
            ..
        } => SyncCryptoLoadError::Revoked,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::Unauthenticated,
            ..
        } => SyncCryptoLoadError::AccessDenied,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::StorageUnavailable,
            ..
        }
        | SyncRemoteError::Transport { .. } => SyncCryptoLoadError::Unavailable,
        SyncRemoteError::InvalidRequest { .. }
        | SyncRemoteError::InvalidResponse { .. }
        | SyncRemoteError::Server { .. } => SyncCryptoLoadError::InvalidState,
    }
}

#[derive(Debug, Serialize)]
struct ObjectVersionUploadDto<'a> {
    object_type: &'a str,
    version: u64,
    base_version: u64,
    owner_device_id: &'a str,
    key_id: &'a str,
    key_epoch: u64,
    algorithm: &'a str,
    #[serde(with = "base64_bytes")]
    nonce: &'a [u8],
    encrypted_payload_len: i64,
    ciphertext_hash: &'a str,
    signature_schema_version: u16,
    signature_algorithm: &'a str,
    signature_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    signature: &'a [u8],
    client_created_at_ms: i64,
    client_updated_at_ms: i64,
    #[serde(with = "base64_bytes")]
    payload: &'a [u8],
}

impl<'a> ObjectVersionUploadDto<'a> {
    fn from_object(
        object: &'a AssembledSyncObject,
        manifest: &'a SignedSyncObjectManifest,
    ) -> Self {
        let draft = &object.draft;
        Self {
            object_type: draft.object_type.as_str(),
            version: draft.version,
            base_version: draft.base_version.unwrap_or(0),
            owner_device_id: &draft.owner_device_id,
            key_id: &draft.key_id,
            key_epoch: draft.key_epoch,
            algorithm: &draft.algorithm,
            nonce: &draft.nonce,
            encrypted_payload_len: draft.encrypted_payload_len as i64,
            ciphertext_hash: &draft.ciphertext_hash,
            signature_schema_version: manifest.signature.signature_schema_version,
            signature_algorithm: manifest.signature.signature_algorithm.as_str(),
            signature_key_id: &manifest.signature.signature_key_id,
            signature: &manifest.signature.signature,
            client_created_at_ms: draft.created_at_ms,
            client_updated_at_ms: draft.updated_at_ms,
            payload: &object.envelope.encrypted_payload,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ObjectVersionResponseDto {
    domain_id: String,
    object_id: String,
    object_type: String,
    version: u64,
    base_version: u64,
    change_sequence: u64,
    owner_device_id: String,
    key_id: String,
    key_epoch: u64,
    algorithm: String,
    #[serde(with = "base64_bytes")]
    nonce: Vec<u8>,
    encrypted_payload_len: i64,
    ciphertext_hash: String,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
    server_received_at_ms: i64,
    client_created_at_ms: i64,
    client_updated_at_ms: i64,
}

impl TryFrom<ObjectVersionResponseDto> for RemoteObjectVersion {
    type Error = SyncRemoteError;

    fn try_from(value: ObjectVersionResponseDto) -> Result<Self, Self::Error> {
        if value.encrypted_payload_len <= 0 {
            return invalid_response("encrypted_payload_len must be positive");
        }
        let object_type = parse_object_type(&value.object_type)?;
        let object = Self {
            domain_id: value.domain_id,
            object_id: value.object_id,
            object_type,
            version: value.version,
            base_version: if value.base_version == 0 {
                None
            } else {
                Some(value.base_version)
            },
            change_sequence: value.change_sequence,
            owner_device_id: value.owner_device_id,
            key_id: value.key_id,
            key_epoch: value.key_epoch,
            algorithm: value.algorithm,
            nonce: value.nonce,
            encrypted_payload_len: value.encrypted_payload_len as usize,
            ciphertext_hash: value.ciphertext_hash,
            signature_schema_version: value.signature_schema_version,
            signature_algorithm: value.signature_algorithm,
            signature_key_id: value.signature_key_id,
            signature: value.signature,
            server_received_at_ms: value.server_received_at_ms,
            client_created_at_ms: value.client_created_at_ms,
            client_updated_at_ms: value.client_updated_at_ms,
        };
        object.validate()?;
        Ok(object)
    }
}

#[derive(Debug, Deserialize)]
struct ObjectDiscoveryResponseDto {
    entries: Vec<ObjectVersionResponseDto>,
    next_cursor: String,
    has_more: bool,
}

impl TryFrom<ObjectDiscoveryResponseDto> for RemoteObjectDiscoveryPage {
    type Error = SyncRemoteError;

    fn try_from(value: ObjectDiscoveryResponseDto) -> Result<Self, Self::Error> {
        let entries = value
            .entries
            .into_iter()
            .map(RemoteObjectVersion::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        for pair in entries.windows(2) {
            if pair[0].change_sequence >= pair[1].change_sequence {
                return invalid_response("discovery entries must be ordered by change_sequence");
            }
        }
        if value.has_more && entries.is_empty() {
            return invalid_response("discovery page with has_more must contain entries");
        }
        Ok(Self {
            entries,
            next_cursor: OpaqueSyncCursor::new(value.next_cursor).map_err(|_| {
                SyncRemoteError::InvalidResponse {
                    message: "discovery next_cursor is invalid".to_owned(),
                }
            })?,
            has_more: value.has_more,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LifecycleDeviceDto {
    domain_id: String,
    device_id: String,
    signing_algorithm: String,
    signing_public_key_id: String,
    #[serde(with = "base64_bytes")]
    signing_public_key: Vec<u8>,
    key_agreement_public_key_id: String,
    #[serde(with = "base64_bytes")]
    key_agreement_public_key: Vec<u8>,
    status: String,
    authorized_at_ms: Option<i64>,
    revoked_at_ms: Option<i64>,
    last_seen_at_ms: Option<i64>,
}

impl TryFrom<LifecycleDeviceDto> for RemoteLifecycleDevice {
    type Error = SyncRemoteError;

    fn try_from(value: LifecycleDeviceDto) -> Result<Self, Self::Error> {
        let status = match value.status.as_str() {
            "pending" => SyncDeviceStatus::Pending,
            "active" => SyncDeviceStatus::Active,
            "revoked" => SyncDeviceStatus::Revoked,
            "lost" => SyncDeviceStatus::Lost,
            _ => return invalid_response("lifecycle device status is invalid"),
        };
        validate_path_segment("domain_id", &value.domain_id).map_err(request_error_as_response)?;
        validate_path_segment("device_id", &value.device_id).map_err(request_error_as_response)?;
        validate_required("signing_algorithm", &value.signing_algorithm)
            .map_err(request_error_as_response)?;
        validate_required("signing_public_key_id", &value.signing_public_key_id)
            .map_err(request_error_as_response)?;
        validate_required(
            "key_agreement_public_key_id",
            &value.key_agreement_public_key_id,
        )
        .map_err(request_error_as_response)?;
        if value.signing_public_key.is_empty() || value.key_agreement_public_key.is_empty() {
            return invalid_response("lifecycle device public keys cannot be empty");
        }
        Ok(Self {
            domain_id: value.domain_id,
            device_id: value.device_id,
            signing_algorithm: value.signing_algorithm,
            signing_public_key_id: value.signing_public_key_id,
            signing_public_key: value.signing_public_key,
            key_agreement_public_key_id: value.key_agreement_public_key_id,
            key_agreement_public_key: value.key_agreement_public_key,
            status,
            authorized_at_ms: value.authorized_at_ms.filter(|value| *value != 0),
            revoked_at_ms: value.revoked_at_ms.filter(|value| *value != 0),
            last_seen_at_ms: value.last_seen_at_ms.filter(|value| *value != 0),
        })
    }
}

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationDto {
    domain_id: String,
    join_request_id: String,
    authorizer_device_id: String,
    recipient_device_id: String,
    recipient_signing_public_key_id: String,
    recipient_key_agreement_key_id: String,
    join_short_code: String,
    #[serde(with = "base64_bytes")]
    join_challenge: Vec<u8>,
    join_created_at_ms: i64,
    join_expires_at_ms: i64,
    key_epoch: u64,
    wrapping_key_id: String,
    encrypted_key_len: i64,
    created_at_ms: i64,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
}

impl TryFrom<DeviceAuthorizationDto> for RemoteDeviceAuthorization {
    type Error = SyncRemoteError;

    fn try_from(value: DeviceAuthorizationDto) -> Result<Self, Self::Error> {
        if value.key_epoch == 0
            || value.encrypted_key_len <= 0
            || value.created_at_ms <= 0
            || value.join_created_at_ms <= 0
            || value.join_expires_at_ms <= value.join_created_at_ms
            || value.created_at_ms > value.join_expires_at_ms
        {
            return invalid_response("authorization lifecycle counters are invalid");
        }
        let join_challenge = String::from_utf8(value.join_challenge).map_err(|_| {
            SyncRemoteError::InvalidResponse {
                message: "authorization join challenge must be UTF-8".to_owned(),
            }
        })?;
        for (field, text) in [
            ("domain_id", value.domain_id.as_str()),
            ("join_request_id", value.join_request_id.as_str()),
            ("authorizer_device_id", value.authorizer_device_id.as_str()),
            ("recipient_device_id", value.recipient_device_id.as_str()),
            ("join_short_code", value.join_short_code.as_str()),
            ("wrapping_key_id", value.wrapping_key_id.as_str()),
            ("signature_algorithm", value.signature_algorithm.as_str()),
            ("signature_key_id", value.signature_key_id.as_str()),
        ] {
            validate_required(field, text).map_err(request_error_as_response)?;
        }
        if join_challenge.is_empty() || value.signature.is_empty() {
            return invalid_response("authorization lifecycle signature fields are empty");
        }
        Ok(Self {
            domain_id: value.domain_id,
            join_request_id: value.join_request_id,
            authorizer_device_id: value.authorizer_device_id,
            recipient_device_id: value.recipient_device_id,
            recipient_signing_public_key_id: value.recipient_signing_public_key_id,
            recipient_key_agreement_key_id: value.recipient_key_agreement_key_id,
            join_short_code: value.join_short_code,
            join_challenge,
            join_created_at_ms: value.join_created_at_ms,
            join_expires_at_ms: value.join_expires_at_ms,
            key_epoch: value.key_epoch,
            wrapping_key_id: value.wrapping_key_id,
            encrypted_key_len: value.encrypted_key_len as usize,
            created_at_ms: value.created_at_ms,
            signature_schema_version: value.signature_schema_version,
            signature_algorithm: value.signature_algorithm,
            signature_key_id: value.signature_key_id,
            signature: value.signature,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LifecycleEventDto {
    domain_id: String,
    lifecycle_sequence: u64,
    event_type: String,
    record_id: String,
    key_epoch: u64,
    reject_from_object_change_sequence: Option<u64>,
    created_at_ms: i64,
    device: LifecycleDeviceDto,
    authorization: Option<DeviceAuthorizationDto>,
    revocation: Option<lifecycle_dto::DeviceRevocationDto>,
    recovery_record: Option<lifecycle_dto::LifecycleRecoveryRecordDto>,
    recovered_activation: Option<lifecycle_dto::RecoveredDeviceActivationDto>,
    recovery_revocation: Option<lifecycle_dto::RecoveryRecordRevocationDto>,
}

impl TryFrom<LifecycleEventDto> for RemoteLifecycleEvent {
    type Error = SyncRemoteError;

    fn try_from(value: LifecycleEventDto) -> Result<Self, Self::Error> {
        let event_type = match value.event_type.as_str() {
            "initial_device" => RemoteLifecycleEventKind::InitialDevice,
            "device_authorized" => RemoteLifecycleEventKind::DeviceAuthorized,
            "device_revoked" => RemoteLifecycleEventKind::DeviceRevoked,
            "recovery_record_rotated" => RemoteLifecycleEventKind::RecoveryRecordRotated,
            "device_recovered" => RemoteLifecycleEventKind::DeviceRecovered,
            "recovery_record_revoked" => RemoteLifecycleEventKind::RecoveryRecordRevoked,
            _ => return invalid_response("lifecycle event type is invalid"),
        };
        if value.lifecycle_sequence == 0
            || value.key_epoch == 0
            || value.created_at_ms <= 0
            || value.record_id.is_empty()
        {
            return invalid_response("lifecycle event metadata is invalid");
        }
        let device = RemoteLifecycleDevice::try_from(value.device)?;
        let authorization = value
            .authorization
            .map(RemoteDeviceAuthorization::try_from)
            .transpose()?;
        let revocation = value
            .revocation
            .map(RemoteDeviceRevocation::try_from)
            .transpose()?;
        let recovery_record = value
            .recovery_record
            .map(RemoteRecoveryRecordRotation::try_from)
            .transpose()?;
        let recovered_activation = value
            .recovered_activation
            .map(RemoteRecoveredDeviceActivation::try_from)
            .transpose()?;
        let recovery_revocation = value
            .recovery_revocation
            .map(RemoteRecoveryRecordRevocation::try_from)
            .transpose()?;
        match event_type {
            RemoteLifecycleEventKind::InitialDevice
                if authorization.is_none()
                    && revocation.is_none()
                    && recovery_record.is_none()
                    && recovered_activation.is_none()
                    && recovery_revocation.is_none()
                    && value.reject_from_object_change_sequence.is_none() => {}
            RemoteLifecycleEventKind::DeviceAuthorized
                if authorization.is_some()
                    && revocation.is_none()
                    && recovery_record.is_none()
                    && recovered_activation.is_none()
                    && recovery_revocation.is_none()
                    && value.reject_from_object_change_sequence.is_none() => {}
            RemoteLifecycleEventKind::DeviceRevoked
                if authorization.is_none()
                    && revocation.is_some()
                    && recovery_record.is_none()
                    && recovered_activation.is_none()
                    && recovery_revocation.is_none()
                    && matches!(value.reject_from_object_change_sequence, Some(sequence) if sequence > 0) =>
                {}
            RemoteLifecycleEventKind::RecoveryRecordRotated
                if authorization.is_none()
                    && revocation.is_none()
                    && recovery_record.is_some()
                    && recovered_activation.is_none()
                    && recovery_revocation.is_none()
                    && value.reject_from_object_change_sequence.is_none() => {}
            RemoteLifecycleEventKind::DeviceRecovered
                if authorization.is_none()
                    && revocation.is_none()
                    && recovery_record.is_none()
                    && recovered_activation.is_some()
                    && recovery_revocation.is_none()
                    && value.reject_from_object_change_sequence.is_none() => {}
            RemoteLifecycleEventKind::RecoveryRecordRevoked
                if authorization.is_none()
                    && revocation.is_none()
                    && recovery_record.is_none()
                    && recovered_activation.is_none()
                    && recovery_revocation.is_some()
                    && value.reject_from_object_change_sequence.is_none() => {}
            _ => return invalid_response("lifecycle event payload does not match event type"),
        }
        if device.domain_id != value.domain_id {
            return invalid_response("lifecycle device domain does not match event domain");
        }
        Ok(Self {
            domain_id: value.domain_id,
            lifecycle_sequence: value.lifecycle_sequence,
            event_type,
            record_id: value.record_id,
            key_epoch: value.key_epoch,
            reject_from_object_change_sequence: value.reject_from_object_change_sequence,
            created_at_ms: value.created_at_ms,
            device,
            authorization,
            revocation,
            recovery_record,
            recovered_activation,
            recovery_revocation,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LifecycleSnapshotResponseDto {
    domain: lifecycle_dto::DomainResponseDto,
    entries: Vec<LifecycleEventDto>,
    next_cursor: String,
}

impl TryFrom<LifecycleSnapshotResponseDto> for RemoteLifecycleSnapshot {
    type Error = SyncRemoteError;

    fn try_from(value: LifecycleSnapshotResponseDto) -> Result<Self, Self::Error> {
        let domain = SyncDomain::try_from(value.domain)?;
        let entries = decode_lifecycle_entries(value.entries, &domain.domain_id)?;
        if entries.first().map(|event| event.lifecycle_sequence) != Some(1) {
            return invalid_response("lifecycle snapshot must start at sequence 1");
        }
        Ok(Self {
            domain,
            entries,
            next_cursor: lifecycle_dto::response_cursor(value.next_cursor)?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LifecycleDiscoveryResponseDto {
    entries: Vec<LifecycleEventDto>,
    next_cursor: String,
    has_more: bool,
}

impl TryFrom<LifecycleDiscoveryResponseDto> for RemoteLifecyclePage {
    type Error = SyncRemoteError;

    fn try_from(value: LifecycleDiscoveryResponseDto) -> Result<Self, Self::Error> {
        let entries = value
            .entries
            .into_iter()
            .map(RemoteLifecycleEvent::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        validate_lifecycle_order(&entries)?;
        if value.has_more && entries.is_empty() {
            return invalid_response("lifecycle page with has_more must contain entries");
        }
        Ok(Self {
            entries,
            next_cursor: lifecycle_dto::response_cursor(value.next_cursor)?,
            has_more: value.has_more,
        })
    }
}

fn decode_lifecycle_entries(
    entries: Vec<LifecycleEventDto>,
    domain_id: &str,
) -> Result<Vec<RemoteLifecycleEvent>, SyncRemoteError> {
    let entries = entries
        .into_iter()
        .map(RemoteLifecycleEvent::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    if entries.iter().any(|event| event.domain_id != domain_id) {
        return invalid_response("lifecycle entry domain does not match snapshot domain");
    }
    validate_lifecycle_order(&entries)?;
    Ok(entries)
}

fn validate_lifecycle_order(entries: &[RemoteLifecycleEvent]) -> Result<(), SyncRemoteError> {
    for pair in entries.windows(2) {
        if pair[0].lifecycle_sequence + 1 != pair[1].lifecycle_sequence {
            return invalid_response("lifecycle entries must be contiguous and ordered");
        }
    }
    Ok(())
}

fn request_error_as_response(error: SyncRemoteError) -> SyncRemoteError {
    SyncRemoteError::InvalidResponse {
        message: error.to_string(),
    }
}

impl<'de> Deserialize<'de> for RemoteObjectVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ObjectVersionResponseDto::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
struct ErrorResponseDto {
    error_code: String,
    message: String,
    retryable: bool,
    server_time_ms: Option<i64>,
    latest_version: Option<u64>,
    latest_ciphertext_hash: Option<String>,
}

fn decode_json_response<R>(response: SyncRemoteResponse) -> Result<R, SyncRemoteError>
where
    R: for<'de> Deserialize<'de>,
{
    if (200..300).contains(&response.status) {
        return serde_json::from_slice(&response.body).map_err(SyncRemoteError::from_json_error);
    }
    Err(decode_error_response(response))
}

fn decode_error_response(response: SyncRemoteResponse) -> SyncRemoteError {
    let fallback = || SyncRemoteError::Server {
        status: response.status,
        code: SyncServerErrorCode::Unknown,
        message: "sync server returned an invalid error response".to_owned(),
        retryable: false,
        server_time_ms: None,
        latest: None,
    };

    let Ok(error) = serde_json::from_slice::<ErrorResponseDto>(&response.body) else {
        return fallback();
    };
    let latest = error.latest_version.and_then(|version| {
        if version == 0 {
            None
        } else {
            Some(LatestObjectConflictMetadata {
                version,
                ciphertext_hash: error.latest_ciphertext_hash,
            })
        }
    });

    SyncRemoteError::Server {
        status: response.status,
        code: SyncServerErrorCode::from_server_code(&error.error_code),
        message: error.message,
        retryable: error.retryable,
        server_time_ms: error.server_time_ms,
        latest,
    }
}

fn validate_manifest_matches_object(
    domain_id: &str,
    object: &AssembledSyncObject,
    manifest: &SignedSyncObjectManifest,
) -> Result<(), SyncRemoteError> {
    validate_path_segment("domain_id", domain_id)?;
    object
        .draft
        .validate()
        .map_err(SyncRemoteError::from_payload_error)?;
    object
        .envelope
        .validate()
        .map_err(SyncRemoteError::from_crypto_error)?;
    manifest
        .validate()
        .map_err(SyncRemoteError::from_crypto_error)?;
    if manifest.domain_id != domain_id {
        return invalid_request("signed manifest domain_id must match upload domain");
    }
    if manifest.object_id != object.draft.object_id {
        return invalid_request("signed manifest object_id must match encrypted object");
    }
    if manifest.object_type != object.draft.object_type.as_str() {
        return invalid_request("signed manifest object_type must match encrypted object");
    }
    if manifest.version != object.draft.version {
        return invalid_request("signed manifest version must match encrypted object");
    }
    if manifest.base_version != object.draft.base_version {
        return invalid_request("signed manifest base_version must match encrypted object");
    }
    if manifest.key_id != object.draft.key_id {
        return invalid_request("signed manifest key_id must match encrypted object");
    }
    if manifest.key_epoch != object.draft.key_epoch {
        return invalid_request("signed manifest key_epoch must match encrypted object");
    }
    if manifest.envelope_algorithm != object.draft.algorithm {
        return invalid_request("signed manifest algorithm must match encrypted object");
    }
    if manifest.nonce != object.draft.nonce {
        return invalid_request("signed manifest nonce must match encrypted object");
    }
    if manifest.encrypted_payload_len != object.draft.encrypted_payload_len {
        return invalid_request("signed manifest payload length must match encrypted object");
    }
    if manifest.ciphertext_hash != object.draft.ciphertext_hash {
        return invalid_request("signed manifest ciphertext_hash must match encrypted object");
    }
    if manifest.created_at_ms != object.draft.created_at_ms
        || manifest.updated_at_ms != object.draft.updated_at_ms
    {
        return invalid_request("signed manifest timestamps must match encrypted object");
    }
    if manifest.signature.signer_device_id != object.draft.owner_device_id {
        return invalid_request("signed manifest signer must match object owner device");
    }
    Ok(())
}

fn parse_object_type(value: &str) -> Result<SyncObjectType, SyncRemoteError> {
    match value {
        "dictionary.user_terms" => Ok(SyncObjectType::DictionaryUserTerms),
        "dictionary.deleted_terms" => Ok(SyncObjectType::DictionaryDeletedTerms),
        "ranker.weights" => Ok(SyncObjectType::RankerWeights),
        "settings.profile" => Ok(SyncObjectType::SettingsProfile),
        "settings.schema" => Ok(SyncObjectType::SettingsSchema),
        "backup.snapshot" => Ok(SyncObjectType::BackupSnapshot),
        _ => invalid_response("object_type is not supported"),
    }
}

fn validate_path_segment(field: &'static str, value: &str) -> Result<(), SyncRemoteError> {
    validate_required(field, value)?;
    if value.bytes().any(|byte| matches!(byte, b'/' | b'?' | b'#')) {
        return Err(SyncRemoteError::InvalidRequest {
            message: format!("{field} cannot contain path separators or query fragments"),
        });
    }
    Ok(())
}

fn validate_query_component(
    field: &'static str,
    value: &str,
    allow_dot: bool,
) -> Result<(), SyncRemoteError> {
    if value.is_empty() {
        return invalid_request(format!("{field} cannot be empty"));
    }
    let valid = value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || (allow_dot && byte == b'.')
    });
    if !valid {
        return invalid_request(format!("{field} contains unsupported characters"));
    }
    Ok(())
}

fn validate_required(field: &'static str, value: &str) -> Result<(), SyncRemoteError> {
    if value.trim().is_empty() {
        return Err(SyncRemoteError::InvalidRequest {
            message: format!("{field} cannot be empty"),
        });
    }
    Ok(())
}

fn invalid_request<T>(message: impl Into<String>) -> Result<T, SyncRemoteError> {
    Err(SyncRemoteError::InvalidRequest {
        message: message.into(),
    })
}

fn invalid_response<T>(message: impl Into<String>) -> Result<T, SyncRemoteError> {
    Err(invalid_response_value(message))
}

fn invalid_response_value(message: impl Into<String>) -> SyncRemoteError {
    SyncRemoteError::InvalidResponse {
        message: message.into(),
    }
}

fn invalid_crypto_response(error: radishlex_ime_crypto::CryptoError) -> SyncRemoteError {
    invalid_response_value(error.to_string())
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S, T>(bytes: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]> + ?Sized,
    {
        serializer.serialize_str(&Base64::encode_string(bytes.as_ref()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Base64::decode_vec(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
