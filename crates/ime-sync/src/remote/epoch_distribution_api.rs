use radishlex_ime_crypto::{DeviceSignature, SignatureAlgorithmId};

use super::*;
use crate::epoch_distribution::{
    validate_epoch_distribution_batch, SignedEpochDistribution, MAX_EPOCH_DISTRIBUTION_RECORDS,
};
use crate::product_provider::SyncTrustedDomainState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEpochDistributionResult {
    pub key_epoch: u64,
    pub accepted_records: usize,
    pub inserted_records: usize,
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn upload_epoch_distribution(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        records: &[SignedEpochDistribution],
    ) -> Result<RemoteEpochDistributionResult, SyncRemoteError> {
        let first = records
            .first()
            .ok_or_else(|| SyncRemoteError::InvalidRequest {
                message: "epoch distribution records are required".to_owned(),
            })?;
        if records.len() > MAX_EPOCH_DISTRIBUTION_RECORDS {
            return invalid_request("epoch distribution exceeds active device limit");
        }
        validate_epoch_distribution_batch(trusted_domain, records).map_err(|error| {
            SyncRemoteError::InvalidRequest {
                message: error.to_string(),
            }
        })?;
        validate_path_segment("domain_id", &first.material.domain_id)?;
        let request = EpochDistributionUploadDto::from_records(records);
        let response: EpochDistributionResponseDto = self.send_json(
            SyncRemoteMethod::Post,
            format!(
                "{}/domains/{}/epoch-distributions",
                self.api_prefix, first.material.domain_id
            ),
            &request,
        )?;
        if response.key_epoch != first.material.key_epoch
            || response.accepted_records != records.len()
            || response.inserted_records > response.accepted_records
        {
            return invalid_response("epoch distribution response does not match request");
        }
        Ok(RemoteEpochDistributionResult {
            key_epoch: response.key_epoch,
            accepted_records: response.accepted_records,
            inserted_records: response.inserted_records,
        })
    }

    pub fn device_verified_epoch_distribution(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        domain_id: &str,
        recipient_device_id: &str,
        key_epoch: u64,
        wrapping_key_id: &str,
    ) -> Result<SignedEpochDistribution, SyncRemoteError> {
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
        let dto =
            decode_json_response::<DeviceWrappedEpochResponseDto>(self.transport.send(request)?)?;
        if dto.signature_record_type != "epoch_distribution" {
            return invalid_response("wrapped epoch response is not a signed distribution");
        }
        let distributor_device_id = dto.distributor_device_id.clone();
        let signature_schema_version = dto.signature_schema_version;
        let signature_algorithm = dto.signature_algorithm.clone();
        let signature_key_id = dto.signature_key_id.clone();
        let signature_bytes = dto.signature.clone();
        let material = WrappedEpochMaterial::try_from(dto)?;
        if material.domain_id != domain_id
            || material.recipient_device_id != recipient_device_id
            || material.key_epoch != key_epoch
            || material.wrapping_key_id != wrapping_key_id
        {
            return invalid_response("wrapped epoch response does not match request locator");
        }
        if signature_schema_version != 1 {
            return invalid_response("epoch distribution signature schema is unsupported");
        }
        let signature = DeviceSignature::new_for_algorithm(
            SignatureAlgorithmId::new(signature_algorithm).map_err(|error| {
                SyncRemoteError::InvalidResponse {
                    message: error.to_string(),
                }
            })?,
            signature_key_id,
            &distributor_device_id,
            signature_bytes,
        )
        .map_err(|error| SyncRemoteError::InvalidResponse {
            message: error.to_string(),
        })?;
        let record = SignedEpochDistribution::new(distributor_device_id, material, signature)
            .map_err(|error| SyncRemoteError::InvalidResponse {
                message: error.to_string(),
            })?;
        if trusted_domain.domain().domain_id != domain_id
            || trusted_domain.domain().current_key_epoch != key_epoch
        {
            return invalid_response(
                "epoch distribution does not match trusted domain current epoch",
            );
        }
        let distributor = trusted_domain
            .device_profile(&record.distributor_device_id)
            .ok_or_else(|| SyncRemoteError::InvalidResponse {
                message: "epoch distributor is not present in trusted lifecycle".to_owned(),
            })?;
        let recipient = trusted_domain
            .device_profile(recipient_device_id)
            .ok_or_else(|| SyncRemoteError::InvalidResponse {
                message: "epoch recipient is not present in trusted lifecycle".to_owned(),
            })?;
        if distributor.device().status != crate::SyncDeviceStatus::Active
            || recipient.device().status != crate::SyncDeviceStatus::Active
        {
            return invalid_response("epoch distribution devices must be active");
        }
        record
            .verify(
                distributor.device(),
                distributor.signing_public_key(),
                recipient.key_agreement_public_key().ok_or_else(|| {
                    SyncRemoteError::InvalidResponse {
                        message: "epoch recipient key-agreement profile is missing".to_owned(),
                    }
                })?,
            )
            .map_err(|error| SyncRemoteError::InvalidResponse {
                message: error.to_string(),
            })?;
        Ok(record)
    }
}

#[derive(Debug, Serialize)]
struct EpochDistributionUploadDto<'a> {
    distributor_device_id: &'a str,
    key_epoch: u64,
    records: Vec<EpochDistributionRecordUploadDto<'a>>,
}

impl<'a> EpochDistributionUploadDto<'a> {
    fn from_records(records: &'a [SignedEpochDistribution]) -> Self {
        Self {
            distributor_device_id: &records[0].distributor_device_id,
            key_epoch: records[0].material.key_epoch,
            records: records
                .iter()
                .map(EpochDistributionRecordUploadDto::from_record)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EpochDistributionRecordUploadDto<'a> {
    recipient_device_id: &'a str,
    recipient_key_agreement_key_id: &'a str,
    wrapping_key_id: &'a str,
    algorithm: &'a str,
    #[serde(with = "base64_bytes")]
    nonce: &'a [u8],
    wrapped_key_len: usize,
    ciphertext_hash: &'a str,
    created_at_ms: i64,
    signature_schema_version: u16,
    signature_algorithm: &'a str,
    signature_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    signature: &'a [u8],
    #[serde(with = "base64_bytes")]
    wrapped_key: &'a [u8],
}

impl<'a> EpochDistributionRecordUploadDto<'a> {
    fn from_record(record: &'a SignedEpochDistribution) -> Self {
        Self {
            recipient_device_id: &record.material.recipient_device_id,
            recipient_key_agreement_key_id: &record.material.recipient_key_agreement_key_id,
            wrapping_key_id: &record.material.wrapping_key_id,
            algorithm: &record.material.algorithm,
            nonce: record.material.nonce.as_bytes(),
            wrapped_key_len: record.material.wrapped_key.len(),
            ciphertext_hash: &record.material.ciphertext_hash,
            created_at_ms: record.material.created_at_ms,
            signature_schema_version: record.signature.signature_schema_version,
            signature_algorithm: record.signature.signature_algorithm.as_str(),
            signature_key_id: &record.signature.signature_key_id,
            signature: &record.signature.signature,
            wrapped_key: &record.material.wrapped_key,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EpochDistributionResponseDto {
    key_epoch: u64,
    accepted_records: usize,
    inserted_records: usize,
}
