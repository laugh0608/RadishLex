use std::collections::BTreeMap;

use radishlex_ime_crypto::{
    DeviceKeyAgreementPublicKey, DeviceSigningPublicKey, SignatureAlgorithmId,
    SignedRecoveredDeviceActivation,
};
use serde::{Deserialize, Serialize};

use super::epoch_distribution_api::EpochDistributionUploadDto;
use super::*;
use crate::epoch_distribution::{SignedEpochDistribution, MAX_EPOCH_DISTRIBUTION_RECORDS};
use crate::product_provider::SyncTrustedDomainState;
use crate::{SyncDevice, SyncDeviceStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecoveredDeviceResult {
    pub device_id: String,
    pub lifecycle_sequence: u64,
    pub distributed_records: usize,
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn activate_recovered_device(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        recovery: &RemoteVerifiedRecoveryRecord,
        activation: &SignedRecoveredDeviceActivation,
        records: &[SignedEpochDistribution],
    ) -> Result<RemoteRecoveredDeviceResult, SyncRemoteError> {
        validate_recovered_device_request(trusted_domain, recovery, activation, records)?;
        let domain_id = &activation.manifest.domain_id;
        let recovery_id = &activation.manifest.recovery_record_id;
        validate_path_segment("domain_id", domain_id)?;
        validate_path_segment("recovery_record_id", recovery_id)?;
        let request = RecoveredDeviceUploadDto {
            activation: RecoveredDeviceActivationUploadDto::from_signed(activation),
            distribution: EpochDistributionUploadDto::from_records(records),
        };
        let response: RecoveredDeviceResponseDto = self.send_json(
            SyncRemoteMethod::Post,
            format!(
                "{}/domains/{domain_id}/recovery-records/{recovery_id}/activation",
                self.api_prefix
            ),
            &request,
        )?;
        if response.device.device_id != activation.manifest.device_id
            || response.device.status != "active"
            || response.lifecycle_sequence == 0
            || response.distributed_records != records.len()
        {
            return invalid_response("recovered device response does not match request");
        }
        Ok(RemoteRecoveredDeviceResult {
            device_id: response.device.device_id,
            lifecycle_sequence: response.lifecycle_sequence,
            distributed_records: response.distributed_records,
        })
    }
}

fn validate_recovered_device_request(
    trusted_domain: &SyncTrustedDomainState,
    recovery: &RemoteVerifiedRecoveryRecord,
    activation: &SignedRecoveredDeviceActivation,
    records: &[SignedEpochDistribution],
) -> Result<(), SyncRemoteError> {
    let manifest = &activation.manifest;
    manifest
        .validate()
        .map_err(|error| SyncRemoteError::InvalidRequest {
            message: error.to_string(),
        })?;
    if recovery.material.domain_id != trusted_domain.domain().domain_id
        || recovery.material.key_epoch != trusted_domain.domain().current_key_epoch
        || recovery.material.recovery_id != manifest.recovery_record_id
        || recovery.material.domain_id != manifest.domain_id
        || recovery.material.key_epoch != manifest.key_epoch
        || recovery.material.activation_algorithm != manifest.activation_algorithm
        || recovery.material.activation_public_key_id != manifest.activation_public_key_id
    {
        return invalid_request("recovered device activation does not match trusted recovery");
    }
    activation
        .verify(&recovery.material.activation_public_key)
        .map_err(|error| SyncRemoteError::InvalidRequest {
            message: error.to_string(),
        })?;
    if trusted_domain.device_profile(&manifest.device_id).is_some() {
        return invalid_request("recovered device id is already trusted");
    }
    if records.is_empty() || records.len() > MAX_EPOCH_DISTRIBUTION_RECORDS {
        return invalid_request("recovery epoch distribution size is invalid");
    }

    let signing_algorithm =
        SignatureAlgorithmId::new(manifest.signing_algorithm.clone()).map_err(|error| {
            SyncRemoteError::InvalidRequest {
                message: error.to_string(),
            }
        })?;
    let recovered_device = SyncDevice::new(
        manifest.device_id.clone(),
        manifest.signing_public_key_id.clone(),
        SyncDeviceStatus::Active,
        Some(manifest.created_at_ms),
        None,
        None,
    )
    .map_err(|error| SyncRemoteError::InvalidRequest {
        message: error.to_string(),
    })?;
    let recovered_signing_key = DeviceSigningPublicKey::new(
        manifest.device_id.clone(),
        manifest.signing_public_key_id.clone(),
        signing_algorithm,
        manifest.signing_public_key.clone(),
        manifest.created_at_ms,
        None,
    )
    .map_err(|error| SyncRemoteError::InvalidRequest {
        message: error.to_string(),
    })?;
    let recovered_agreement_key = DeviceKeyAgreementPublicKey::p256(
        manifest.device_id.clone(),
        manifest.key_agreement_public_key_id.clone(),
        manifest.key_agreement_public_key.clone(),
        manifest.created_at_ms,
        None,
    )
    .map_err(|error| SyncRemoteError::InvalidRequest {
        message: error.to_string(),
    })?;

    let mut recipients = BTreeMap::<String, DeviceKeyAgreementPublicKey>::new();
    for profile in trusted_domain
        .device_profiles()
        .filter(|profile| profile.device().status == SyncDeviceStatus::Active)
    {
        let key =
            profile
                .key_agreement_public_key()
                .ok_or_else(|| SyncRemoteError::InvalidRequest {
                    message: "active recovery cohort is missing key-agreement profile".to_owned(),
                })?;
        recipients.insert(profile.device().device_id.clone(), key.clone());
    }
    recipients.insert(manifest.device_id.clone(), recovered_agreement_key);
    if recipients.len() != records.len() {
        return invalid_request("recovery epoch distribution must cover active cohort");
    }
    for record in records {
        if record.distributor_device_id != manifest.device_id
            || record.material.domain_id != manifest.domain_id
            || record.material.key_epoch != manifest.key_epoch
        {
            return invalid_request("recovery epoch distribution does not match activation");
        }
        let recipient = recipients
            .remove(&record.material.recipient_device_id)
            .ok_or_else(|| SyncRemoteError::InvalidRequest {
                message: "recovery epoch distribution recipient is not active".to_owned(),
            })?;
        record
            .verify(&recovered_device, &recovered_signing_key, &recipient)
            .map_err(|error| SyncRemoteError::InvalidRequest {
                message: error.to_string(),
            })?;
    }
    if !recipients.is_empty() {
        return invalid_request("recovery epoch distribution is incomplete");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct RecoveredDeviceUploadDto<'a> {
    activation: RecoveredDeviceActivationUploadDto<'a>,
    distribution: EpochDistributionUploadDto<'a>,
}

#[derive(Debug, Serialize)]
struct RecoveredDeviceActivationUploadDto<'a> {
    device_id: &'a str,
    signing_algorithm: &'a str,
    signing_public_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    signing_public_key: &'a [u8],
    key_agreement_algorithm: &'a str,
    key_agreement_public_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    key_agreement_public_key: &'a [u8],
    key_epoch: u64,
    created_at_ms: i64,
    signature_schema_version: u16,
    activation_algorithm: &'a str,
    activation_public_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    activation_signature: &'a [u8],
}

impl<'a> RecoveredDeviceActivationUploadDto<'a> {
    fn from_signed(value: &'a SignedRecoveredDeviceActivation) -> Self {
        let manifest = &value.manifest;
        Self {
            device_id: &manifest.device_id,
            signing_algorithm: &manifest.signing_algorithm,
            signing_public_key_id: &manifest.signing_public_key_id,
            signing_public_key: &manifest.signing_public_key,
            key_agreement_algorithm: &manifest.key_agreement_algorithm,
            key_agreement_public_key_id: &manifest.key_agreement_public_key_id,
            key_agreement_public_key: &manifest.key_agreement_public_key,
            key_epoch: manifest.key_epoch,
            created_at_ms: manifest.created_at_ms,
            signature_schema_version: manifest.signature_schema_version,
            activation_algorithm: &manifest.activation_algorithm,
            activation_public_key_id: &manifest.activation_public_key_id,
            activation_signature: &value.signature,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RecoveredDeviceResponseDto {
    device: RecoveredDeviceResponseProfileDto,
    lifecycle_sequence: u64,
    distributed_records: usize,
}

#[derive(Debug, Deserialize)]
struct RecoveredDeviceResponseProfileDto {
    device_id: String,
    status: String,
}
