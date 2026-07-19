use radishlex_ime_crypto::SignedRecoveryRecordRevocation;
use serde::{Deserialize, Serialize};

use crate::product_provider::SyncTrustedDomainState;

use super::{
    base64_bytes, invalid_request, invalid_response, validate_path_segment,
    RemoteVerifiedRecoveryRecord, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod,
    SyncRemoteTransport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecoveryRecordRevocationResult {
    pub recovery_record_id: String,
    pub lifecycle_sequence: u64,
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn revoke_recovery_record(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        target: &RemoteVerifiedRecoveryRecord,
        revocation: &SignedRecoveryRecordRevocation,
    ) -> Result<RemoteRecoveryRecordRevocationResult, SyncRemoteError> {
        validate_revocation_request(trusted_domain, target, revocation)?;
        let recovery_record_id = &revocation.recovery_record_id;
        validate_path_segment("recovery_record_id", recovery_record_id)?;
        let path = format!(
            "{}/domains/{}/recovery-records/{recovery_record_id}/revoke",
            self.api_prefix, revocation.domain_id
        );
        let response: RecoveryRecordRevocationResponseDto = self.send_json(
            SyncRemoteMethod::Post,
            path,
            &RecoveryRecordRevocationRequestDto::from(revocation),
        )?;
        if response.domain_id != revocation.domain_id
            || response.recovery_record_id != revocation.recovery_record_id
            || response.status != "revoked"
            || response.lifecycle_sequence == 0
        {
            return invalid_response("recovery revocation response is invalid");
        }
        Ok(RemoteRecoveryRecordRevocationResult {
            recovery_record_id: response.recovery_record_id,
            lifecycle_sequence: response.lifecycle_sequence,
        })
    }
}

fn validate_revocation_request(
    trusted_domain: &SyncTrustedDomainState,
    target: &RemoteVerifiedRecoveryRecord,
    revocation: &SignedRecoveryRecordRevocation,
) -> Result<(), SyncRemoteError> {
    revocation
        .validate()
        .map_err(SyncRemoteError::from_crypto_error)?;
    let domain = trusted_domain.domain();
    if revocation.domain_id != domain.domain_id
        || revocation.key_epoch != domain.current_key_epoch
        || target.material.domain_id != revocation.domain_id
        || target.material.recovery_id != revocation.recovery_record_id
        || target.material.key_epoch != revocation.key_epoch
        || revocation.created_at_ms <= target.material.created_at_ms
    {
        return invalid_request("recovery revocation does not match current trusted record");
    }
    let signer = trusted_domain
        .device_profile(&revocation.signature.signer_device_id)
        .ok_or_else(|| SyncRemoteError::InvalidRequest {
            message: "recovery revoker is not trusted".to_owned(),
        })?;
    if signer.device().status != crate::SyncDeviceStatus::Active {
        return invalid_request("recovery revoker is not active");
    }
    revocation
        .verify(signer.signing_public_key())
        .map_err(SyncRemoteError::from_crypto_error)
}

#[derive(Debug, Serialize)]
struct RecoveryRecordRevocationRequestDto<'a> {
    revoker_device_id: &'a str,
    key_epoch: u64,
    reason: &'a str,
    created_at_ms: i64,
    signature_schema_version: u16,
    signature_algorithm: &'a str,
    signature_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    signature: &'a [u8],
}

impl<'a> From<&'a SignedRecoveryRecordRevocation> for RecoveryRecordRevocationRequestDto<'a> {
    fn from(value: &'a SignedRecoveryRecordRevocation) -> Self {
        Self {
            revoker_device_id: &value.signature.signer_device_id,
            key_epoch: value.key_epoch,
            reason: &value.reason,
            created_at_ms: value.created_at_ms,
            signature_schema_version: value.signature.signature_schema_version,
            signature_algorithm: value.signature.signature_algorithm.as_str(),
            signature_key_id: &value.signature.signature_key_id,
            signature: &value.signature.signature,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RecoveryRecordRevocationResponseDto {
    domain_id: String,
    recovery_record_id: String,
    status: String,
    lifecycle_sequence: u64,
}
