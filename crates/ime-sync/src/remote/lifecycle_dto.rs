use radishlex_ime_crypto::{
    DeviceSignature, RecoveredDeviceActivationManifest, SignatureAlgorithmId,
    SignedRecoveredDeviceActivation, SignedRecoveryRecordManifest, SignedRecoveryRecordRevocation,
};
use serde::Deserialize;

use crate::device::SyncDomain;

use super::{
    base64_bytes, invalid_crypto_response, invalid_response, invalid_response_value,
    OpaqueSyncCursor, RemoteDeviceRevocation, RemoteRecoveredDeviceActivation,
    RemoteRecoveryRecordRevocation, RemoteRecoveryRecordRotation, SyncRemoteError,
};

#[derive(Debug, Deserialize)]
pub(super) struct DomainResponseDto {
    domain_id: String,
    current_key_epoch: u64,
    active_key_id: String,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl TryFrom<DomainResponseDto> for SyncDomain {
    type Error = SyncRemoteError;

    fn try_from(value: DomainResponseDto) -> Result<Self, Self::Error> {
        SyncDomain::new(
            value.domain_id,
            value.current_key_epoch,
            value.active_key_id,
            value.created_at_ms,
            value.updated_at_ms,
        )
        .map_err(|error| SyncRemoteError::InvalidResponse {
            message: error.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct LifecycleRecoveryRecordDto {
    record_schema_version: u16,
    domain_id: String,
    recovery_record_id: String,
    previous_recovery_id: String,
    key_epoch: u64,
    kdf_profile: String,
    kdf_version: u16,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_len: i64,
    #[serde(with = "base64_bytes")]
    salt: Vec<u8>,
    algorithm: String,
    #[serde(with = "base64_bytes")]
    nonce: Vec<u8>,
    wrapped_material_len: i64,
    ciphertext_hash: String,
    activation_algorithm: String,
    activation_public_key_id: String,
    #[serde(with = "base64_bytes")]
    activation_public_key: Vec<u8>,
    created_at_ms: i64,
    updated_at_ms: i64,
    signer_device_id: String,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
}

impl TryFrom<LifecycleRecoveryRecordDto> for RemoteRecoveryRecordRotation {
    type Error = SyncRemoteError;

    fn try_from(value: LifecycleRecoveryRecordDto) -> Result<Self, Self::Error> {
        if value.signature_schema_version != 1 {
            return invalid_response("recovery lifecycle signature schema is unsupported");
        }
        let output_len = usize::try_from(value.output_len)
            .map_err(|_| invalid_response_value("recovery output_len is invalid"))?;
        let wrapped_len = usize::try_from(value.wrapped_material_len)
            .map_err(|_| invalid_response_value("recovery wrapped length is invalid"))?;
        let signature = DeviceSignature::new_for_algorithm(
            SignatureAlgorithmId::new(value.signature_algorithm)
                .map_err(invalid_crypto_response)?,
            value.signature_key_id,
            value.signer_device_id,
            value.signature,
        )
        .map_err(invalid_crypto_response)?;
        let manifest = SignedRecoveryRecordManifest {
            signature,
            record_schema_version: value.record_schema_version,
            recovery_id: value.recovery_record_id,
            previous_recovery_id: value.previous_recovery_id,
            domain_id: value.domain_id,
            key_epoch: value.key_epoch,
            kdf_id: value.kdf_profile,
            kdf_version: value.kdf_version,
            salt: value.salt,
            memory_kib: value.memory_kib,
            iterations: value.iterations,
            parallelism: value.parallelism,
            output_len,
            envelope_algorithm: value.algorithm,
            envelope_nonce: value.nonce,
            encrypted_recovery_key_len: wrapped_len,
            ciphertext_hash: value.ciphertext_hash,
            activation_algorithm: value.activation_algorithm,
            activation_public_key_id: value.activation_public_key_id,
            activation_public_key: value.activation_public_key,
            created_at_ms: value.created_at_ms,
            updated_at_ms: value.updated_at_ms,
        };
        manifest.validate().map_err(invalid_crypto_response)?;
        Ok(Self { manifest })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RecoveredDeviceActivationDto {
    recovery_record_id: String,
    domain_id: String,
    device_id: String,
    signing_algorithm: String,
    signing_public_key_id: String,
    #[serde(with = "base64_bytes")]
    signing_public_key: Vec<u8>,
    key_agreement_algorithm: String,
    key_agreement_public_key_id: String,
    #[serde(with = "base64_bytes")]
    key_agreement_public_key: Vec<u8>,
    key_epoch: u64,
    created_at_ms: i64,
    signature_schema_version: u16,
    activation_algorithm: String,
    activation_public_key_id: String,
    #[serde(with = "base64_bytes")]
    activation_signature: Vec<u8>,
}

impl TryFrom<RecoveredDeviceActivationDto> for RemoteRecoveredDeviceActivation {
    type Error = SyncRemoteError;

    fn try_from(value: RecoveredDeviceActivationDto) -> Result<Self, Self::Error> {
        let manifest = RecoveredDeviceActivationManifest {
            signature_schema_version: value.signature_schema_version,
            activation_algorithm: value.activation_algorithm,
            activation_public_key_id: value.activation_public_key_id,
            recovery_record_id: value.recovery_record_id,
            domain_id: value.domain_id,
            device_id: value.device_id,
            signing_algorithm: value.signing_algorithm,
            signing_public_key_id: value.signing_public_key_id,
            signing_public_key: value.signing_public_key,
            key_agreement_algorithm: value.key_agreement_algorithm,
            key_agreement_public_key_id: value.key_agreement_public_key_id,
            key_agreement_public_key: value.key_agreement_public_key,
            key_epoch: value.key_epoch,
            created_at_ms: value.created_at_ms,
        };
        manifest.validate().map_err(invalid_crypto_response)?;
        Ok(Self {
            signed: SignedRecoveredDeviceActivation {
                manifest,
                signature: value.activation_signature,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct DeviceRevocationDto {
    domain_id: String,
    revoked_device_id: String,
    revoker_device_id: String,
    previous_key_epoch: u64,
    new_key_epoch: u64,
    reason: String,
    created_at_ms: i64,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
}

impl TryFrom<DeviceRevocationDto> for RemoteDeviceRevocation {
    type Error = SyncRemoteError;

    fn try_from(value: DeviceRevocationDto) -> Result<Self, Self::Error> {
        if value.previous_key_epoch == 0
            || value.new_key_epoch <= value.previous_key_epoch
            || value.created_at_ms <= 0
            || value.signature.is_empty()
        {
            return invalid_response("revocation lifecycle fields are invalid");
        }
        Ok(Self {
            domain_id: value.domain_id,
            revoked_device_id: value.revoked_device_id,
            revoker_device_id: value.revoker_device_id,
            previous_key_epoch: value.previous_key_epoch,
            new_key_epoch: value.new_key_epoch,
            reason: value.reason,
            created_at_ms: value.created_at_ms,
            signature_schema_version: value.signature_schema_version,
            signature_algorithm: value.signature_algorithm,
            signature_key_id: value.signature_key_id,
            signature: value.signature,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RecoveryRecordRevocationDto {
    recovery_record_id: String,
    domain_id: String,
    revoker_device_id: String,
    key_epoch: u64,
    reason: String,
    created_at_ms: i64,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
}

impl TryFrom<RecoveryRecordRevocationDto> for RemoteRecoveryRecordRevocation {
    type Error = SyncRemoteError;

    fn try_from(value: RecoveryRecordRevocationDto) -> Result<Self, Self::Error> {
        if value.signature_schema_version != 1 {
            return invalid_response("recovery revocation signature schema is unsupported");
        }
        let signature = DeviceSignature::new_for_algorithm(
            SignatureAlgorithmId::new(value.signature_algorithm)
                .map_err(invalid_crypto_response)?,
            value.signature_key_id,
            value.revoker_device_id,
            value.signature,
        )
        .map_err(invalid_crypto_response)?;
        let signed = SignedRecoveryRecordRevocation::new(
            value.recovery_record_id,
            value.domain_id,
            value.key_epoch,
            value.reason,
            value.created_at_ms,
            signature,
        )
        .map_err(invalid_crypto_response)?;
        Ok(Self { signed })
    }
}

pub(super) fn response_cursor(value: String) -> Result<OpaqueSyncCursor, SyncRemoteError> {
    OpaqueSyncCursor::new(value).map_err(|_| SyncRemoteError::InvalidResponse {
        message: "lifecycle next_cursor is invalid".to_owned(),
    })
}
