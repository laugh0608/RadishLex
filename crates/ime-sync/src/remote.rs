use std::fmt;

use base64ct::{Base64, Encoding};
use radishlex_ime_crypto::SignedSyncObjectManifest;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::assemble::AssembledSyncObject;
use crate::model::{SyncObjectType, SyncPayloadError};

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
            content_type,
            body: body.into(),
        }
    }

    pub fn method(&self) -> SyncRemoteMethod {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
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
    Err(SyncRemoteError::InvalidResponse {
        message: message.into(),
    })
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
pub(crate) mod test_support {
    use super::*;
    use radishlex_ime_crypto::{
        DeviceSignature, KeyDescriptor, KeyRole, PlaintextPayload, SignedSyncObjectManifest,
        SyncMasterKeyMaterial, TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
    };
    use serde_json::Value;

    pub(crate) fn signed_object() -> (AssembledSyncObject, SignedSyncObjectManifest) {
        let object_key = KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 1).expect("key");
        let sync_master_key = SyncMasterKeyMaterial::new([3u8; 32]).expect("master key");
        let object_key_material = sync_master_key
            .derive_object_key(
                &object_key,
                SyncObjectType::DictionaryUserTerms.to_crypto_object_type(),
                "object-a",
            )
            .expect("object key");
        let payload = PlaintextPayload::new(
            SyncObjectType::DictionaryUserTerms.to_crypto_object_type(),
            br#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"term_id":"synthetic-term"}]}"#.to_vec(),
        )
        .expect("payload");
        let envelope = radishlex_ime_crypto::EncryptedObjectEnvelope::encrypt_payload(
            "object-a",
            "device-a",
            &object_key,
            &object_key_material,
            1,
            None,
            payload,
            100,
        )
        .expect("envelope");
        let draft =
            crate::EncryptedSyncObjectDraft::from_crypto_envelope(&envelope).expect("draft");
        let mut store = TestMemoryDeviceKeyStore::new();
        store
            .insert_signing_key("device-a", "signing-key-a", [8u8; 32], 1)
            .expect("signing handle");
        let handle = store
            .handle("device-a", "signing-key-a")
            .expect("signing handle");
        let empty_signature = DeviceSignature::new(
            "signing-key-a",
            "device-a",
            vec![1u8; ED25519_SIGNATURE_LEN],
        )
        .expect("empty signature");
        let unsigned = SignedSyncObjectManifest::new("domain-a", &envelope, empty_signature)
            .expect("manifest");
        let signature = store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("signature");
        let manifest =
            SignedSyncObjectManifest::new("domain-a", &envelope, signature).expect("manifest");

        (
            AssembledSyncObject {
                envelope,
                draft,
                record_count: 1,
            },
            manifest,
        )
    }

    pub(crate) fn response_for(object: &AssembledSyncObject) -> Value {
        serde_json::json!({
            "domain_id": "domain-a",
            "object_id": object.draft.object_id,
            "object_type": object.draft.object_type.as_str(),
            "version": object.draft.version,
            "base_version": object.draft.base_version.unwrap_or(0),
            "owner_device_id": object.draft.owner_device_id,
            "key_id": object.draft.key_id,
            "key_epoch": object.draft.key_epoch,
            "algorithm": object.draft.algorithm,
            "nonce": Base64::encode_string(&object.draft.nonce),
            "encrypted_payload_len": object.draft.encrypted_payload_len,
            "ciphertext_hash": object.draft.ciphertext_hash,
            "signature_schema_version": 1,
            "signature_algorithm": "ed25519-v1",
            "signature_key_id": "signing-key-a",
            "signature": Base64::encode_string(&vec![1u8; ED25519_SIGNATURE_LEN]),
            "server_received_at_ms": 110,
            "client_created_at_ms": object.draft.created_at_ms,
            "client_updated_at_ms": object.draft.updated_at_ms
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{response_for, signed_object};
    use super::*;
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
}
