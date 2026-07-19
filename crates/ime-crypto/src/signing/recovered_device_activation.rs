use std::fmt;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use super::{canonical_signature_bytes, SignatureField, SIGNATURE_SCHEMA_VERSION};
use crate::device::RecoveryMaterial;
use crate::epoch_material::{
    KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1, P256_KEY_AGREEMENT_PUBLIC_KEY_LEN,
};
use crate::model::{validate_non_empty_bytes, validate_required, CryptoError};
use crate::recovery::{RecoveryCode, RECOVERY_ACTIVATION_ALGORITHM_ED25519_V1};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredDeviceActivationManifest {
    pub signature_schema_version: u16,
    pub activation_algorithm: String,
    pub activation_public_key_id: String,
    pub recovery_record_id: String,
    pub domain_id: String,
    pub device_id: String,
    pub signing_algorithm: String,
    pub signing_public_key_id: String,
    pub signing_public_key: Vec<u8>,
    pub key_agreement_algorithm: String,
    pub key_agreement_public_key_id: String,
    pub key_agreement_public_key: Vec<u8>,
    pub key_epoch: u64,
    pub created_at_ms: i64,
}

impl RecoveredDeviceActivationManifest {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("recovered_device_activation", &self.signature_fields())
    }

    pub fn signature_fields(&self) -> Vec<SignatureField> {
        vec![
            SignatureField::u16("signature_schema_version", self.signature_schema_version),
            SignatureField::text("activation_algorithm", &self.activation_algorithm),
            SignatureField::text("activation_public_key_id", &self.activation_public_key_id),
            SignatureField::text("recovery_record_id", &self.recovery_record_id),
            SignatureField::text("domain_id", &self.domain_id),
            SignatureField::text("device_id", &self.device_id),
            SignatureField::text("signing_algorithm", &self.signing_algorithm),
            SignatureField::text("signing_public_key_id", &self.signing_public_key_id),
            SignatureField::bytes("signing_public_key", &self.signing_public_key),
            SignatureField::text("key_agreement_algorithm", &self.key_agreement_algorithm),
            SignatureField::text(
                "key_agreement_public_key_id",
                &self.key_agreement_public_key_id,
            ),
            SignatureField::bytes("key_agreement_public_key", &self.key_agreement_public_key),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::i64("created_at_ms", self.created_at_ms),
        ]
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.signature_schema_version != SIGNATURE_SCHEMA_VERSION {
            return Err(CryptoError::invalid_field(
                "signature_schema_version",
                "recovered device activation requires signature schema version 1",
            ));
        }
        if self.activation_algorithm != RECOVERY_ACTIVATION_ALGORITHM_ED25519_V1 {
            return Err(CryptoError::invalid_field(
                "activation_algorithm",
                "recovery activation requires ed25519-v1",
            ));
        }
        for (field, value) in [
            (
                "activation_public_key_id",
                self.activation_public_key_id.as_str(),
            ),
            ("recovery_record_id", self.recovery_record_id.as_str()),
            ("domain_id", self.domain_id.as_str()),
            ("device_id", self.device_id.as_str()),
            ("signing_algorithm", self.signing_algorithm.as_str()),
            ("signing_public_key_id", self.signing_public_key_id.as_str()),
            (
                "key_agreement_public_key_id",
                self.key_agreement_public_key_id.as_str(),
            ),
        ] {
            validate_required(field, value)?;
        }
        validate_non_empty_bytes("signing_public_key", &self.signing_public_key)?;
        if self.key_agreement_algorithm != KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1
            || self.key_agreement_public_key.len() != P256_KEY_AGREEMENT_PUBLIC_KEY_LEN
        {
            return Err(CryptoError::invalid_field(
                "key_agreement_public_key",
                "recovered device key agreement profile must be p256-ecdh-v1",
            ));
        }
        if self.key_epoch == 0 || self.created_at_ms <= 0 {
            return Err(CryptoError::invalid_field(
                "activation_counters",
                "key epoch and creation time must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SignedRecoveredDeviceActivation {
    pub manifest: RecoveredDeviceActivationManifest,
    pub signature: Vec<u8>,
}

impl SignedRecoveredDeviceActivation {
    pub fn sign(
        material: &RecoveryMaterial,
        code: &RecoveryCode,
        manifest: RecoveredDeviceActivationManifest,
    ) -> Result<Self, CryptoError> {
        manifest.validate()?;
        material.validate()?;
        if manifest.recovery_record_id != material.recovery_id
            || manifest.domain_id != material.domain_id
            || manifest.key_epoch != material.key_epoch
            || manifest.activation_algorithm != material.activation_algorithm
            || manifest.activation_public_key_id != material.activation_public_key_id
        {
            return Err(CryptoError::invalid_field(
                "recovered_device_activation",
                "activation manifest does not match recovery material",
            ));
        }
        let signature =
            material.sign_recovered_device_activation(code, &manifest.canonical_bytes())?;
        let signed = Self {
            manifest,
            signature,
        };
        signed.verify(&material.activation_public_key)?;
        Ok(signed)
    }

    pub fn verify(&self, activation_public_key: &[u8]) -> Result<(), CryptoError> {
        self.manifest.validate()?;
        let key_bytes: [u8; 32] = activation_public_key.try_into().map_err(|_| {
            CryptoError::invalid_field(
                "activation_public_key",
                "Ed25519 activation public key must contain 32 bytes",
            )
        })?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| {
            CryptoError::invalid_field(
                "activation_public_key",
                "Ed25519 activation public key encoding is invalid",
            )
        })?;
        let signature = Signature::from_slice(&self.signature).map_err(|_| {
            CryptoError::invalid_field(
                "activation_signature",
                "Ed25519 activation signature must contain 64 bytes",
            )
        })?;
        verifying_key
            .verify(&self.manifest.canonical_bytes(), &signature)
            .map_err(|_| CryptoError::invalid_field("activation_signature", "signature is invalid"))
    }
}

impl fmt::Debug for SignedRecoveredDeviceActivation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedRecoveredDeviceActivation")
            .field("manifest", &self.manifest)
            .field("signature_len", &self.signature.len())
            .finish()
    }
}
