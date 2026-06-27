use std::fmt;

use crate::model::{
    validate_key_material, validate_non_empty_bytes, validate_required, CryptoError, KeyDescriptor,
    KeyRole, OBJECT_KEY_LEN,
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
    pub kdf_id: String,
    pub salt: Vec<u8>,
    pub encrypted_recovery_key: Vec<u8>,
    pub created_at_ms: i64,
}

impl RecoveryMaterial {
    pub fn new(
        recovery_id: impl Into<String>,
        kdf_id: impl Into<String>,
        salt: impl Into<Vec<u8>>,
        encrypted_recovery_key: impl Into<Vec<u8>>,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        let material = Self {
            recovery_id: recovery_id.into(),
            kdf_id: kdf_id.into(),
            salt: salt.into(),
            encrypted_recovery_key: encrypted_recovery_key.into(),
            created_at_ms,
        };
        material.validate()?;
        Ok(material)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("recovery_id", &self.recovery_id)?;
        validate_required("kdf_id", &self.kdf_id)?;
        validate_non_empty_bytes("salt", &self.salt)?;
        validate_non_empty_bytes("encrypted_recovery_key", &self.encrypted_recovery_key)?;
        Ok(())
    }
}

impl fmt::Debug for RecoveryMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveryMaterial")
            .field("recovery_id", &self.recovery_id)
            .field("kdf_id", &self.kdf_id)
            .field("salt_len", &self.salt.len())
            .field("encrypted_recovery_key", &"[redacted]")
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}
