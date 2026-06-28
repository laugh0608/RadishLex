use std::fmt;

use crate::model::{
    push_aad_field, validate_key_material, validate_non_empty_bytes, validate_required,
    AlgorithmId, CryptoError, KeyDescriptor, KeyRole, Nonce, OBJECT_KEY_LEN,
    XCHACHA20POLY1305_NONCE_LEN,
};

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceWrappingKeyMaterial([u8; OBJECT_KEY_LEN]);

impl DeviceWrappingKeyMaterial {
    pub fn new(bytes: [u8; OBJECT_KEY_LEN]) -> Result<Self, CryptoError> {
        validate_key_material("device_wrapping_key", &bytes)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; OBJECT_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for DeviceWrappingKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceWrappingKeyMaterial([redacted])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyDescriptor {
    pub device_id: String,
    pub public_key_id: String,
    pub key_id: String,
    pub key_epoch: u64,
}

impl DeviceKeyDescriptor {
    pub fn new(
        device_id: impl Into<String>,
        public_key_id: impl Into<String>,
        device_key: &KeyDescriptor,
    ) -> Result<Self, CryptoError> {
        if device_key.role != KeyRole::DeviceKeyPair {
            return Err(CryptoError::invalid_field(
                "key_role",
                "device key descriptor requires a device key pair",
            ));
        }

        let descriptor = Self {
            device_id: device_id.into(),
            public_key_id: public_key_id.into(),
            key_id: device_key.key_id.clone(),
            key_epoch: device_key.key_epoch,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("public_key_id", &self.public_key_id)?;
        validate_required("key_id", &self.key_id)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceWrappingRecord {
    pub recipient_device_id: String,
    pub wrapping_key_id: String,
    pub key_epoch: u64,
    pub encrypted_key: Vec<u8>,
    pub created_at_ms: i64,
}

impl DeviceWrappingRecord {
    pub fn new(
        recipient_device_id: impl Into<String>,
        wrapping_key: &KeyDescriptor,
        encrypted_key: impl Into<Vec<u8>>,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        if wrapping_key.role != KeyRole::DeviceWrapping {
            return Err(CryptoError::invalid_field(
                "key_role",
                "device wrapping record requires a device wrapping key",
            ));
        }

        let record = Self {
            recipient_device_id: recipient_device_id.into(),
            wrapping_key_id: wrapping_key.key_id.clone(),
            key_epoch: wrapping_key.key_epoch,
            encrypted_key: encrypted_key.into(),
            created_at_ms,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("recipient_device_id", &self.recipient_device_id)?;
        validate_required("wrapping_key_id", &self.wrapping_key_id)?;
        validate_non_empty_bytes("encrypted_key", &self.encrypted_key)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for DeviceWrappingRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceWrappingRecord")
            .field("recipient_device_id", &self.recipient_device_id)
            .field("wrapping_key_id", &self.wrapping_key_id)
            .field("key_epoch", &self.key_epoch)
            .field("encrypted_key", &"[redacted]")
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryMaterial {
    pub recovery_id: String,
    pub domain_id: String,
    pub key_epoch: u64,
    pub kdf_id: String,
    pub kdf_version: u16,
    pub salt: Vec<u8>,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
    pub envelope_algorithm: AlgorithmId,
    pub envelope_nonce: Nonce,
    pub encrypted_recovery_key: Vec<u8>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl RecoveryMaterial {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        recovery_id: impl Into<String>,
        domain_id: impl Into<String>,
        key_epoch: u64,
        kdf_id: impl Into<String>,
        kdf_version: u16,
        salt: impl Into<Vec<u8>>,
        memory_kib: u32,
        iterations: u32,
        parallelism: u32,
        output_len: usize,
        envelope_algorithm: AlgorithmId,
        envelope_nonce: Nonce,
        encrypted_recovery_key: impl Into<Vec<u8>>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        let material = Self {
            recovery_id: recovery_id.into(),
            domain_id: domain_id.into(),
            key_epoch,
            kdf_id: kdf_id.into(),
            kdf_version,
            salt: salt.into(),
            memory_kib,
            iterations,
            parallelism,
            output_len,
            envelope_algorithm,
            envelope_nonce,
            encrypted_recovery_key: encrypted_recovery_key.into(),
            created_at_ms,
            updated_at_ms,
        };
        material.validate()?;
        Ok(material)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("recovery_id", &self.recovery_id)?;
        validate_required("domain_id", &self.domain_id)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        validate_required("kdf_id", &self.kdf_id)?;
        if self.kdf_version == 0 {
            return Err(CryptoError::invalid_field(
                "kdf_version",
                "value must be greater than 0",
            ));
        }
        validate_non_empty_bytes("salt", &self.salt)?;
        if self.memory_kib == 0 {
            return Err(CryptoError::invalid_field(
                "memory_kib",
                "value must be greater than 0",
            ));
        }
        if self.iterations == 0 {
            return Err(CryptoError::invalid_field(
                "iterations",
                "value must be greater than 0",
            ));
        }
        if self.parallelism == 0 {
            return Err(CryptoError::invalid_field(
                "parallelism",
                "value must be greater than 0",
            ));
        }
        if self.output_len == 0 {
            return Err(CryptoError::invalid_field(
                "output_len",
                "value must be greater than 0",
            ));
        }
        if !self.envelope_algorithm.is_supported() {
            return Err(CryptoError::invalid_field(
                "envelope_algorithm",
                format!("unsupported algorithm {}", self.envelope_algorithm.as_str()),
            ));
        }
        if self.envelope_nonce.as_bytes().len() != XCHACHA20POLY1305_NONCE_LEN {
            return Err(CryptoError::invalid_field(
                "envelope_nonce",
                format!("value must be {XCHACHA20POLY1305_NONCE_LEN} bytes"),
            ));
        }
        validate_non_empty_bytes("encrypted_recovery_key", &self.encrypted_recovery_key)?;
        if self.updated_at_ms < self.created_at_ms {
            return Err(CryptoError::invalid_field(
                "updated_at_ms",
                "value must be greater than or equal to created_at_ms",
            ));
        }
        Ok(())
    }

    pub fn associated_data(&self) -> RecoveryAssociatedData {
        RecoveryAssociatedData {
            recovery_id: self.recovery_id.clone(),
            domain_id: self.domain_id.clone(),
            key_epoch: self.key_epoch,
            kdf_id: self.kdf_id.clone(),
            kdf_version: self.kdf_version,
            salt: self.salt.clone(),
            memory_kib: self.memory_kib,
            iterations: self.iterations,
            parallelism: self.parallelism,
            output_len: self.output_len,
            envelope_algorithm: self.envelope_algorithm.clone(),
            envelope_nonce: self.envelope_nonce.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        }
    }
}

impl fmt::Debug for RecoveryMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveryMaterial")
            .field("recovery_id", &self.recovery_id)
            .field("domain_id", &self.domain_id)
            .field("key_epoch", &self.key_epoch)
            .field("kdf_id", &self.kdf_id)
            .field("kdf_version", &self.kdf_version)
            .field("salt_len", &self.salt.len())
            .field("memory_kib", &self.memory_kib)
            .field("iterations", &self.iterations)
            .field("parallelism", &self.parallelism)
            .field("output_len", &self.output_len)
            .field("envelope_algorithm", &self.envelope_algorithm)
            .field("envelope_nonce_len", &self.envelope_nonce.as_bytes().len())
            .field("encrypted_recovery_key", &"[redacted]")
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryAssociatedData {
    pub recovery_id: String,
    pub domain_id: String,
    pub key_epoch: u64,
    pub kdf_id: String,
    pub kdf_version: u16,
    pub salt: Vec<u8>,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
    pub envelope_algorithm: AlgorithmId,
    pub envelope_nonce: Nonce,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl RecoveryAssociatedData {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_aad_field(&mut bytes, "recovery_id", self.recovery_id.as_bytes());
        push_aad_field(&mut bytes, "domain_id", self.domain_id.as_bytes());
        push_aad_field(
            &mut bytes,
            "key_epoch",
            self.key_epoch.to_string().as_bytes(),
        );
        push_aad_field(&mut bytes, "kdf_id", self.kdf_id.as_bytes());
        push_aad_field(
            &mut bytes,
            "kdf_version",
            self.kdf_version.to_string().as_bytes(),
        );
        push_aad_field(&mut bytes, "salt", &self.salt);
        push_aad_field(
            &mut bytes,
            "memory_kib",
            self.memory_kib.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "iterations",
            self.iterations.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "parallelism",
            self.parallelism.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "output_len",
            self.output_len.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "envelope_algorithm",
            self.envelope_algorithm.as_str().as_bytes(),
        );
        push_aad_field(&mut bytes, "envelope_nonce", self.envelope_nonce.as_bytes());
        push_aad_field(
            &mut bytes,
            "created_at_ms",
            self.created_at_ms.to_string().as_bytes(),
        );
        push_aad_field(
            &mut bytes,
            "updated_at_ms",
            self.updated_at_ms.to_string().as_bytes(),
        );
        bytes
    }
}
