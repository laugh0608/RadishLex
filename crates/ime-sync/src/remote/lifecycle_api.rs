use crate::device::{SyncDeviceStatus, SyncDomain};
use radishlex_ime_crypto::{
    SignedRecoveredDeviceActivation, SignedRecoveryRecordManifest, SignedRecoveryRecordRevocation,
};

use super::{
    decode_json_response, invalid_request, validate_path_segment, OpaqueSyncCursor,
    SyncRemoteClient, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteTransport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLifecycleDevice {
    pub domain_id: String,
    pub device_id: String,
    pub signing_algorithm: String,
    pub signing_public_key_id: String,
    pub signing_public_key: Vec<u8>,
    pub key_agreement_public_key_id: String,
    pub key_agreement_public_key: Vec<u8>,
    pub status: SyncDeviceStatus,
    pub authorized_at_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
    pub last_seen_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDeviceAuthorization {
    pub domain_id: String,
    pub join_request_id: String,
    pub authorizer_device_id: String,
    pub recipient_device_id: String,
    pub recipient_signing_public_key_id: String,
    pub recipient_key_agreement_key_id: String,
    pub join_short_code: String,
    pub join_challenge: String,
    pub join_created_at_ms: i64,
    pub join_expires_at_ms: i64,
    pub key_epoch: u64,
    pub wrapping_key_id: String,
    pub encrypted_key_len: usize,
    pub created_at_ms: i64,
    pub signature_schema_version: u16,
    pub signature_algorithm: String,
    pub signature_key_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDeviceRevocation {
    pub domain_id: String,
    pub revoked_device_id: String,
    pub revoker_device_id: String,
    pub previous_key_epoch: u64,
    pub new_key_epoch: u64,
    pub reason: String,
    pub created_at_ms: i64,
    pub signature_schema_version: u16,
    pub signature_algorithm: String,
    pub signature_key_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteLifecycleEventKind {
    InitialDevice,
    DeviceAuthorized,
    DeviceRevoked,
    RecoveryRecordRotated,
    DeviceRecovered,
    RecoveryRecordRevoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecoveryRecordRotation {
    pub manifest: SignedRecoveryRecordManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecoveredDeviceActivation {
    pub signed: SignedRecoveredDeviceActivation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecoveryRecordRevocation {
    pub signed: SignedRecoveryRecordRevocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLifecycleEvent {
    pub domain_id: String,
    pub lifecycle_sequence: u64,
    pub event_type: RemoteLifecycleEventKind,
    pub record_id: String,
    pub key_epoch: u64,
    pub reject_from_object_change_sequence: Option<u64>,
    pub created_at_ms: i64,
    pub device: RemoteLifecycleDevice,
    pub authorization: Option<RemoteDeviceAuthorization>,
    pub revocation: Option<RemoteDeviceRevocation>,
    pub recovery_record: Option<RemoteRecoveryRecordRotation>,
    pub recovered_activation: Option<RemoteRecoveredDeviceActivation>,
    pub recovery_revocation: Option<RemoteRecoveryRecordRevocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLifecycleSnapshot {
    pub domain: SyncDomain,
    pub entries: Vec<RemoteLifecycleEvent>,
    pub next_cursor: OpaqueSyncCursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLifecyclePage {
    pub entries: Vec<RemoteLifecycleEvent>,
    pub next_cursor: OpaqueSyncCursor,
    pub has_more: bool,
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn lifecycle_snapshot(
        &self,
        domain_id: &str,
    ) -> Result<RemoteLifecycleSnapshot, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        let path = format!("{}/domains/{domain_id}/lifecycle", self.api_prefix);
        let response = self.transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            path,
            None,
            Vec::new(),
        ))?;
        let dto: super::LifecycleSnapshotResponseDto = decode_json_response(response)?;
        dto.try_into()
    }

    pub fn lifecycle_events(
        &self,
        domain_id: &str,
        after_cursor: Option<&OpaqueSyncCursor>,
        limit: u16,
    ) -> Result<RemoteLifecyclePage, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        if limit == 0 || limit > 200 {
            return invalid_request("lifecycle limit must be between 1 and 200");
        }
        let path = format!("{}/domains/{domain_id}/lifecycle/events", self.api_prefix);
        let mut request = SyncRemoteRequest::new(SyncRemoteMethod::Get, path, None, Vec::new())
            .with_query_param("limit", limit.to_string())?;
        if let Some(cursor) = after_cursor {
            request = request.with_query_param("after_cursor", cursor.as_str())?;
        }
        let response = self.transport.send(request)?;
        let dto: super::LifecycleDiscoveryResponseDto = decode_json_response(response)?;
        dto.try_into()
    }
}
