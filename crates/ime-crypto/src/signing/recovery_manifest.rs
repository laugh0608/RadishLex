use sha2::{Digest, Sha256};

use super::{canonical_signature_bytes, DeviceSignature, DeviceSigningPublicKey, SignatureField};
use crate::device::RecoveryMaterial;
use crate::model::{validate_non_empty_bytes, validate_required, CryptoError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRecoveryRecordManifest {
    pub signature: DeviceSignature,
    pub record_schema_version: u16,
    pub recovery_id: String,
    pub previous_recovery_id: String,
    pub domain_id: String,
    pub key_epoch: u64,
    pub kdf_id: String,
    pub kdf_version: u16,
    pub salt: Vec<u8>,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
    pub envelope_algorithm: String,
    pub envelope_nonce: Vec<u8>,
    pub encrypted_recovery_key_len: usize,
    pub ciphertext_hash: String,
    pub activation_algorithm: String,
    pub activation_public_key_id: String,
    pub activation_public_key: Vec<u8>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl SignedRecoveryRecordManifest {
    pub fn new(
        material: &RecoveryMaterial,
        signature: DeviceSignature,
    ) -> Result<Self, CryptoError> {
        material.validate()?;
        let manifest = Self {
            signature,
            record_schema_version: material.record_schema_version,
            recovery_id: material.recovery_id.clone(),
            previous_recovery_id: material.previous_recovery_id.clone(),
            domain_id: material.domain_id.clone(),
            key_epoch: material.key_epoch,
            kdf_id: material.kdf_id.clone(),
            kdf_version: material.kdf_version,
            salt: material.salt.clone(),
            memory_kib: material.memory_kib,
            iterations: material.iterations,
            parallelism: material.parallelism,
            output_len: material.output_len,
            envelope_algorithm: material.envelope_algorithm.as_str().to_owned(),
            envelope_nonce: material.envelope_nonce.as_bytes().to_vec(),
            encrypted_recovery_key_len: material.encrypted_recovery_key.len(),
            ciphertext_hash: recovery_ciphertext_hash(&material.encrypted_recovery_key),
            activation_algorithm: material.activation_algorithm.clone(),
            activation_public_key_id: material.activation_public_key_id.clone(),
            activation_public_key: material.activation_public_key.clone(),
            created_at_ms: material.created_at_ms,
            updated_at_ms: material.updated_at_ms,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("recovery_record_v2", &self.signature_fields())
    }

    pub fn signature_fields(&self) -> Vec<SignatureField> {
        vec![
            SignatureField::u16(
                "signature_schema_version",
                self.signature.signature_schema_version,
            ),
            SignatureField::text(
                "signature_algorithm",
                self.signature.signature_algorithm.as_str(),
            ),
            SignatureField::text("signature_key_id", &self.signature.signature_key_id),
            SignatureField::text("signer_device_id", &self.signature.signer_device_id),
            SignatureField::u16("record_schema_version", self.record_schema_version),
            SignatureField::text("recovery_id", &self.recovery_id),
            SignatureField::text("previous_recovery_id", &self.previous_recovery_id),
            SignatureField::text("domain_id", &self.domain_id),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::text("kdf_id", &self.kdf_id),
            SignatureField::u16("kdf_version", self.kdf_version),
            SignatureField::bytes("salt", &self.salt),
            SignatureField::u64("memory_kib", u64::from(self.memory_kib)),
            SignatureField::u64("iterations", u64::from(self.iterations)),
            SignatureField::u64("parallelism", u64::from(self.parallelism)),
            SignatureField::usize("output_len", self.output_len),
            SignatureField::text("envelope_algorithm", &self.envelope_algorithm),
            SignatureField::bytes("envelope_nonce", &self.envelope_nonce),
            SignatureField::usize(
                "encrypted_recovery_key_len",
                self.encrypted_recovery_key_len,
            ),
            SignatureField::text("ciphertext_hash", &self.ciphertext_hash),
            SignatureField::text("activation_algorithm", &self.activation_algorithm),
            SignatureField::text("activation_public_key_id", &self.activation_public_key_id),
            SignatureField::bytes("activation_public_key", &self.activation_public_key),
            SignatureField::i64("created_at_ms", self.created_at_ms),
            SignatureField::i64("updated_at_ms", self.updated_at_ms),
        ]
    }

    pub fn verify(&self, public_key: &DeviceSigningPublicKey) -> Result<(), CryptoError> {
        self.validate()?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.created_at_ms)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        self.signature.validate()?;
        if self.record_schema_version != 2 {
            return Err(CryptoError::invalid_field(
                "record_schema_version",
                "recovery record schema must be version 2",
            ));
        }
        validate_required("recovery_id", &self.recovery_id)?;
        if self.previous_recovery_id == self.recovery_id {
            return Err(CryptoError::invalid_field(
                "previous_recovery_id",
                "predecessor must differ from recovery id",
            ));
        }
        validate_required("domain_id", &self.domain_id)?;
        validate_required("kdf_id", &self.kdf_id)?;
        validate_non_empty_bytes("salt", &self.salt)?;
        validate_required("envelope_algorithm", &self.envelope_algorithm)?;
        validate_non_empty_bytes("envelope_nonce", &self.envelope_nonce)?;
        validate_required("ciphertext_hash", &self.ciphertext_hash)?;
        if self.activation_algorithm != "ed25519-v1"
            || self.activation_public_key_id.is_empty()
            || self.activation_public_key.len() != 32
        {
            return Err(CryptoError::invalid_field(
                "activation_public_key",
                "recovery activation Ed25519 profile is invalid",
            ));
        }
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        if self.kdf_version == 0 || self.memory_kib == 0 || self.iterations == 0 {
            return Err(CryptoError::invalid_field(
                "kdf_parameters",
                "KDF version, memory and iterations must be greater than 0",
            ));
        }
        if self.parallelism == 0 || self.output_len == 0 {
            return Err(CryptoError::invalid_field(
                "kdf_parameters",
                "parallelism and output_len must be greater than 0",
            ));
        }
        if self.encrypted_recovery_key_len == 0 {
            return Err(CryptoError::invalid_field(
                "encrypted_recovery_key_len",
                "value must be greater than 0",
            ));
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(CryptoError::invalid_field(
                "updated_at_ms",
                "value must be greater than or equal to created_at_ms",
            ));
        }
        Ok(())
    }
}

fn recovery_ciphertext_hash(ciphertext: &[u8]) -> String {
    let digest = Sha256::digest(ciphertext);
    let mut value = String::from("sha256:");
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(value, "{byte:02x}");
    }
    value
}
