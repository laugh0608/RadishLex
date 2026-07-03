use std::fmt;

use radishlex_ime_crypto::DeviceWrappingRecord;

use crate::model::{SyncObjectType, SyncPayloadError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDomain {
    pub domain_id: String,
    pub current_key_epoch: u64,
    pub active_key_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl SyncDomain {
    pub fn new(
        domain_id: impl Into<String>,
        current_key_epoch: u64,
        active_key_id: impl Into<String>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let domain = Self {
            domain_id: domain_id.into(),
            current_key_epoch,
            active_key_id: active_key_id.into(),
            created_at_ms,
            updated_at_ms,
        };
        domain.validate()?;
        Ok(domain)
    }

    pub fn advance_key_epoch(
        &self,
        active_key_id: impl Into<String>,
        updated_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        Self::new(
            self.domain_id.clone(),
            self.current_key_epoch + 1,
            active_key_id,
            self.created_at_ms,
            updated_at_ms,
        )
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("domain_id", &self.domain_id)?;
        validate_required("active_key_id", &self.active_key_id)?;
        if self.current_key_epoch == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "current_key_epoch",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(SyncPayloadError::InvalidField {
                field: "updated_at_ms",
                message: "value must be greater than or equal to created_at_ms".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDeviceStatus {
    Pending,
    Active,
    Revoked,
    Lost,
}

impl SyncDeviceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Lost => "lost",
        }
    }

    pub fn can_receive_key_epoch(self) -> bool {
        self == Self::Active
    }
}

impl fmt::Display for SyncDeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDevice {
    pub device_id: String,
    pub public_key_id: String,
    pub status: SyncDeviceStatus,
    pub authorized_at_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
    pub last_seen_at_ms: Option<i64>,
}

impl SyncDevice {
    pub fn new(
        device_id: impl Into<String>,
        public_key_id: impl Into<String>,
        status: SyncDeviceStatus,
        authorized_at_ms: Option<i64>,
        revoked_at_ms: Option<i64>,
        last_seen_at_ms: Option<i64>,
    ) -> Result<Self, SyncPayloadError> {
        let device = Self {
            device_id: device_id.into(),
            public_key_id: public_key_id.into(),
            status,
            authorized_at_ms,
            revoked_at_ms,
            last_seen_at_ms,
        };
        device.validate()?;
        Ok(device)
    }

    pub fn pending(
        device_id: impl Into<String>,
        public_key_id: impl Into<String>,
        last_seen_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        Self::new(
            device_id,
            public_key_id,
            SyncDeviceStatus::Pending,
            None,
            None,
            Some(last_seen_at_ms),
        )
    }

    pub fn activate(&self, authorized_at_ms: i64) -> Result<Self, SyncPayloadError> {
        if self.status != SyncDeviceStatus::Pending {
            return Err(SyncPayloadError::InvalidField {
                field: "device_status",
                message: "only pending devices can be activated".to_owned(),
            });
        }
        Self::new(
            self.device_id.clone(),
            self.public_key_id.clone(),
            SyncDeviceStatus::Active,
            Some(authorized_at_ms),
            None,
            Some(authorized_at_ms),
        )
    }

    pub fn revoke(&self, revoked_at_ms: i64, lost: bool) -> Result<Self, SyncPayloadError> {
        if self.status != SyncDeviceStatus::Active {
            return Err(SyncPayloadError::InvalidField {
                field: "device_status",
                message: "only active devices can be revoked".to_owned(),
            });
        }
        let status = if lost {
            SyncDeviceStatus::Lost
        } else {
            SyncDeviceStatus::Revoked
        };
        Self::new(
            self.device_id.clone(),
            self.public_key_id.clone(),
            status,
            self.authorized_at_ms,
            Some(revoked_at_ms),
            self.last_seen_at_ms,
        )
    }

    pub fn can_receive_key_epoch(&self) -> bool {
        self.status.can_receive_key_epoch()
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("public_key_id", &self.public_key_id)?;
        match self.status {
            SyncDeviceStatus::Pending => {
                if self.authorized_at_ms.is_some() || self.revoked_at_ms.is_some() {
                    return Err(SyncPayloadError::InvalidField {
                        field: "device_status",
                        message: "pending devices cannot have authorization or revocation time"
                            .to_owned(),
                    });
                }
            }
            SyncDeviceStatus::Active => {
                if self.authorized_at_ms.is_none() || self.revoked_at_ms.is_some() {
                    return Err(SyncPayloadError::InvalidField {
                        field: "device_status",
                        message: "active devices require authorization time and no revocation time"
                            .to_owned(),
                    });
                }
            }
            SyncDeviceStatus::Revoked | SyncDeviceStatus::Lost => {
                if self.revoked_at_ms.is_none() {
                    return Err(SyncPayloadError::InvalidField {
                        field: "device_status",
                        message: "revoked or lost devices require revocation time".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceJoinRequest {
    pub device_id: String,
    pub public_key_id: String,
    pub challenge: String,
    pub short_code: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}

impl DeviceJoinRequest {
    pub fn new(
        device_id: impl Into<String>,
        public_key_id: impl Into<String>,
        challenge: impl Into<String>,
        short_code: impl Into<String>,
        created_at_ms: i64,
        expires_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let request = Self {
            device_id: device_id.into(),
            public_key_id: public_key_id.into(),
            challenge: challenge.into(),
            short_code: short_code.into(),
            created_at_ms,
            expires_at_ms,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("public_key_id", &self.public_key_id)?;
        validate_required("challenge", &self.challenge)?;
        validate_required("short_code", &self.short_code)?;
        if self.expires_at_ms <= self.created_at_ms {
            return Err(SyncPayloadError::InvalidField {
                field: "expires_at_ms",
                message: "value must be greater than created_at_ms".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorizationPackage {
    pub recipient_device_id: String,
    pub authorized_by_device_id: String,
    pub key_epoch: u64,
    pub wrapping_key_id: String,
    pub encrypted_key_len: usize,
    pub created_at_ms: i64,
}

impl DeviceAuthorizationPackage {
    pub fn new(
        recipient: &SyncDevice,
        authorized_by: &SyncDevice,
        wrapping_record: &DeviceWrappingRecord,
        created_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        if !recipient.can_receive_key_epoch() {
            return Err(SyncPayloadError::InvalidField {
                field: "device_status",
                message: "only active devices can receive key epoch material".to_owned(),
            });
        }
        if !authorized_by.can_receive_key_epoch() {
            return Err(SyncPayloadError::InvalidField {
                field: "authorized_by_device_id",
                message: "only active devices can authorize key epoch material".to_owned(),
            });
        }
        if recipient.device_id != wrapping_record.recipient_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "wrapping record recipient must match device".to_owned(),
            });
        }

        let package = Self {
            recipient_device_id: recipient.device_id.clone(),
            authorized_by_device_id: authorized_by.device_id.clone(),
            key_epoch: wrapping_record.key_epoch,
            wrapping_key_id: wrapping_record.wrapping_key_id.clone(),
            encrypted_key_len: wrapping_record.encrypted_key.len(),
            created_at_ms,
        };
        package.validate()?;
        Ok(package)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("recipient_device_id", &self.recipient_device_id)?;
        validate_required("authorized_by_device_id", &self.authorized_by_device_id)?;
        validate_required("wrapping_key_id", &self.wrapping_key_id)?;
        if self.key_epoch == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "key_epoch",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.encrypted_key_len == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "encrypted_key_len",
                message: "value must be greater than 0".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRevocationReason {
    UserRequested,
    DeviceLost,
}

impl DeviceRevocationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserRequested => "user_requested",
            Self::DeviceLost => "device_lost",
        }
    }
}

impl fmt::Display for DeviceRevocationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRevocationRecord {
    pub device_id: String,
    pub revoked_by_device_id: String,
    pub previous_key_epoch: u64,
    pub new_key_epoch: u64,
    pub reason: DeviceRevocationReason,
    pub revoked_at_ms: i64,
}

impl DeviceRevocationRecord {
    pub fn new(
        device_id: impl Into<String>,
        revoked_by_device_id: impl Into<String>,
        previous_key_epoch: u64,
        new_key_epoch: u64,
        reason: DeviceRevocationReason,
        revoked_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let record = Self {
            device_id: device_id.into(),
            revoked_by_device_id: revoked_by_device_id.into(),
            previous_key_epoch,
            new_key_epoch,
            reason,
            revoked_at_ms,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("device_id", &self.device_id)?;
        validate_required("revoked_by_device_id", &self.revoked_by_device_id)?;
        if self.previous_key_epoch == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "previous_key_epoch",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.new_key_epoch <= self.previous_key_epoch {
            return Err(SyncPayloadError::InvalidField {
                field: "new_key_epoch",
                message: "value must be greater than previous_key_epoch".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncObjectVersion {
    pub object_id: String,
    pub object_type: SyncObjectType,
    pub version: u64,
    pub base_version: Option<u64>,
    pub key_epoch: u64,
    pub updated_at_ms: i64,
}

impl SyncObjectVersion {
    pub fn new(
        object_id: impl Into<String>,
        object_type: SyncObjectType,
        version: u64,
        base_version: Option<u64>,
        key_epoch: u64,
        updated_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let object = Self {
            object_id: object_id.into(),
            object_type,
            version,
            base_version,
            key_epoch,
            updated_at_ms,
        };
        object.validate()?;
        Ok(object)
    }

    pub fn needs_client_merge_against(&self, current: &Self) -> bool {
        self.object_id == current.object_id
            && self.object_type == current.object_type
            && self.base_version != Some(current.version)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("object_id", &self.object_id)?;
        if self.version == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "version",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if let Some(base_version) = self.base_version {
            if base_version >= self.version {
                return Err(SyncPayloadError::InvalidField {
                    field: "base_version",
                    message: "value must be lower than version".to_owned(),
                });
            }
        }
        if self.key_epoch == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "key_epoch",
                message: "value must be greater than 0".to_owned(),
            });
        }
        Ok(())
    }
}

fn validate_required(field: &'static str, value: &str) -> Result<(), SyncPayloadError> {
    if value.trim().is_empty() {
        return Err(SyncPayloadError::InvalidField {
            field,
            message: "value cannot be empty".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radishlex_ime_crypto::{KeyDescriptor, KeyRole};

    #[test]
    fn sync_domain_advances_key_epoch_with_new_active_key() {
        let domain = SyncDomain::new("domain-a", 3, "sync-master-epoch-3", 10, 20).expect("domain");
        let next = domain
            .advance_key_epoch("sync-master-epoch-4", 30)
            .expect("next epoch");

        assert_eq!(next.domain_id, "domain-a");
        assert_eq!(next.current_key_epoch, 4);
        assert_eq!(next.active_key_id, "sync-master-epoch-4");
        assert_eq!(next.created_at_ms, 10);
        assert_eq!(next.updated_at_ms, 30);

        let error = SyncDomain::new("domain-a", 0, "sync-master-epoch-0", 10, 20)
            .expect_err("zero epoch fails");
        assert!(error.to_string().contains("current_key_epoch"));
    }

    #[test]
    fn sync_device_state_transitions_gate_key_epoch_delivery() {
        let pending = SyncDevice::pending("device-b", "public-key-b", 10).expect("pending");
        assert_eq!(pending.status, SyncDeviceStatus::Pending);
        assert!(!pending.can_receive_key_epoch());

        let active = pending.activate(20).expect("active");
        assert_eq!(active.status, SyncDeviceStatus::Active);
        assert!(active.can_receive_key_epoch());

        let revoked = active.revoke(30, false).expect("revoked");
        assert_eq!(revoked.status, SyncDeviceStatus::Revoked);
        assert!(!revoked.can_receive_key_epoch());

        let error = revoked
            .revoke(40, false)
            .expect_err("revoked device cannot be revoked again");
        assert!(error.to_string().contains("device_status"));
    }

    #[test]
    fn device_join_request_requires_expiry_and_challenge() {
        let request = DeviceJoinRequest::new(
            "device-b",
            "public-key-b",
            "challenge-token",
            "123456",
            10,
            20,
        )
        .expect("join request");

        assert_eq!(request.device_id, "device-b");
        assert_eq!(request.short_code, "123456");

        let error = DeviceJoinRequest::new(
            "device-b",
            "public-key-b",
            "challenge-token",
            "123456",
            20,
            20,
        )
        .expect_err("expired request fails");
        assert!(error.to_string().contains("expires_at_ms"));
    }

    #[test]
    fn authorization_package_requires_active_devices() {
        let authorizer = SyncDevice::pending("device-a", "public-key-a", 10)
            .expect("pending authorizer")
            .activate(20)
            .expect("active authorizer");
        let recipient = SyncDevice::pending("device-b", "public-key-b", 10)
            .expect("pending")
            .activate(20)
            .expect("active");
        let wrapping_record = device_wrapping_record("device-b", 4);
        let package =
            DeviceAuthorizationPackage::new(&recipient, &authorizer, &wrapping_record, 30)
                .expect("authorization package");

        assert_eq!(package.recipient_device_id, "device-b");
        assert_eq!(package.authorized_by_device_id, "device-a");
        assert_eq!(package.key_epoch, 4);
        assert_eq!(package.wrapping_key_id, "wrapping-key-a");
        assert_eq!(package.encrypted_key_len, "wrapped-sync-key".len());

        let revoked = recipient.revoke(40, true).expect("lost device");
        let error = DeviceAuthorizationPackage::new(&revoked, &authorizer, &wrapping_record, 50)
            .expect_err("revoked device cannot receive key material");
        assert!(error.to_string().contains("device_status"));

        let revoked_authorizer = authorizer.revoke(40, false).expect("revoked authorizer");
        let error =
            DeviceAuthorizationPackage::new(&recipient, &revoked_authorizer, &wrapping_record, 50)
                .expect_err("revoked device cannot authorize key material");
        assert!(error.to_string().contains("authorized_by_device_id"));

        let mismatched_record = device_wrapping_record("device-c", 4);
        let error =
            DeviceAuthorizationPackage::new(&recipient, &authorizer, &mismatched_record, 30)
                .expect_err("recipient mismatch fails");
        assert!(error.to_string().contains("recipient_device_id"));
    }

    #[test]
    fn device_revocation_record_requires_new_key_epoch() {
        let record = DeviceRevocationRecord::new(
            "device-b",
            "device-a",
            3,
            4,
            DeviceRevocationReason::DeviceLost,
            40,
        )
        .expect("revocation");

        assert_eq!(record.reason.as_str(), "device_lost");
        assert_eq!(record.new_key_epoch, 4);

        let error = DeviceRevocationRecord::new(
            "device-b",
            "device-a",
            3,
            3,
            DeviceRevocationReason::UserRequested,
            40,
        )
        .expect_err("new epoch must advance");
        assert!(error.to_string().contains("new_key_epoch"));
    }

    #[test]
    fn sync_object_version_detects_stale_base_version() {
        let current = SyncObjectVersion::new(
            "dictionary-user-terms",
            SyncObjectType::DictionaryUserTerms,
            5,
            Some(4),
            3,
            50,
        )
        .expect("current");
        let clean_update = SyncObjectVersion::new(
            "dictionary-user-terms",
            SyncObjectType::DictionaryUserTerms,
            6,
            Some(5),
            3,
            60,
        )
        .expect("clean update");
        let stale_update = SyncObjectVersion::new(
            "dictionary-user-terms",
            SyncObjectType::DictionaryUserTerms,
            6,
            Some(4),
            3,
            60,
        )
        .expect("stale update");

        assert!(!clean_update.needs_client_merge_against(&current));
        assert!(stale_update.needs_client_merge_against(&current));

        let error = SyncObjectVersion::new(
            "dictionary-user-terms",
            SyncObjectType::DictionaryUserTerms,
            5,
            Some(5),
            3,
            50,
        )
        .expect_err("base version cannot equal version");
        assert!(error.to_string().contains("base_version"));
    }

    fn device_wrapping_record(device_id: &str, key_epoch: u64) -> DeviceWrappingRecord {
        let wrapping_key = KeyDescriptor::new("wrapping-key-a", KeyRole::DeviceWrapping, key_epoch)
            .expect("wrapping key");
        DeviceWrappingRecord::new(device_id, &wrapping_key, b"wrapped-sync-key", 30)
            .expect("wrapping record")
    }
}
