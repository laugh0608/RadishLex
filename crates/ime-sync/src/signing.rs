use radishlex_ime_crypto::{
    canonical_signature_bytes, DeviceSignature, DeviceSigningPublicKey, SignatureField,
};

use crate::device::{
    DeviceAuthorizationPackage, DeviceJoinRequest, DeviceRevocationRecord, SyncDevice,
    SyncDeviceStatus,
};
use crate::model::SyncPayloadError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDeviceAuthorization {
    pub signature: DeviceSignature,
    pub authorizer_device_id: String,
    pub recipient_device_id: String,
    pub recipient_public_key_id: String,
    pub join_challenge: String,
    pub join_short_code: String,
    pub key_epoch: u64,
    pub wrapping_key_id: String,
    pub encrypted_key_len: usize,
    pub created_at_ms: i64,
}

impl SignedDeviceAuthorization {
    pub fn new(
        join_request: &DeviceJoinRequest,
        package: &DeviceAuthorizationPackage,
        authorizer: &SyncDevice,
        recipient: &SyncDevice,
        signature: DeviceSignature,
    ) -> Result<Self, SyncPayloadError> {
        join_request.validate()?;
        package.validate()?;
        validate_authorization_devices(package, authorizer, recipient)?;

        let authorization = Self {
            signature,
            authorizer_device_id: package.authorized_by_device_id.clone(),
            recipient_device_id: package.recipient_device_id.clone(),
            recipient_public_key_id: join_request.public_key_id.clone(),
            join_challenge: join_request.challenge.clone(),
            join_short_code: join_request.short_code.clone(),
            key_epoch: package.key_epoch,
            wrapping_key_id: package.wrapping_key_id.clone(),
            encrypted_key_len: package.encrypted_key_len,
            created_at_ms: package.created_at_ms,
        };
        authorization.validate()?;
        authorization.validate_join_request(join_request)?;
        authorization.validate_against_devices(authorizer, recipient)?;
        Ok(authorization)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("device_authorization", &self.signature_fields())
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
            SignatureField::text("authorizer_device_id", &self.authorizer_device_id),
            SignatureField::text("recipient_device_id", &self.recipient_device_id),
            SignatureField::text("recipient_public_key_id", &self.recipient_public_key_id),
            SignatureField::text("join_challenge", &self.join_challenge),
            SignatureField::text("join_short_code", &self.join_short_code),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::text("wrapping_key_id", &self.wrapping_key_id),
            SignatureField::usize("encrypted_key_len", self.encrypted_key_len),
            SignatureField::i64("created_at_ms", self.created_at_ms),
        ]
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.signature
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        validate_required("authorizer_device_id", &self.authorizer_device_id)?;
        validate_required("recipient_device_id", &self.recipient_device_id)?;
        validate_required("recipient_public_key_id", &self.recipient_public_key_id)?;
        validate_required("join_challenge", &self.join_challenge)?;
        validate_required("join_short_code", &self.join_short_code)?;
        validate_required("wrapping_key_id", &self.wrapping_key_id)?;
        validate_key_epoch(self.key_epoch)?;
        if self.encrypted_key_len == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "encrypted_key_len",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.signature.signer_device_id != self.authorizer_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "signer_device_id",
                message: "signature signer must match authorizer device".to_owned(),
            });
        }
        Ok(())
    }

    pub fn validate_join_request(
        &self,
        join_request: &DeviceJoinRequest,
    ) -> Result<(), SyncPayloadError> {
        join_request.validate()?;
        if self.recipient_device_id != join_request.device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "join request device must match authorization recipient".to_owned(),
            });
        }
        if self.recipient_public_key_id != join_request.public_key_id {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_public_key_id",
                message: "join request public key must match authorization recipient".to_owned(),
            });
        }
        if self.join_challenge != join_request.challenge {
            return Err(SyncPayloadError::InvalidField {
                field: "join_challenge",
                message: "join request challenge must match authorization".to_owned(),
            });
        }
        if self.join_short_code != join_request.short_code {
            return Err(SyncPayloadError::InvalidField {
                field: "join_short_code",
                message: "join request short code must match authorization".to_owned(),
            });
        }
        Ok(())
    }

    pub fn validate_against_devices(
        &self,
        authorizer: &SyncDevice,
        recipient: &SyncDevice,
    ) -> Result<(), SyncPayloadError> {
        self.validate()?;
        authorizer.validate()?;
        recipient.validate()?;
        if !authorizer.can_receive_key_epoch() {
            return Err(SyncPayloadError::InvalidField {
                field: "authorizer_device_id",
                message: "authorizer device must be active".to_owned(),
            });
        }
        if authorizer.device_id != self.authorizer_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "authorizer_device_id",
                message: "authorizer device must match signed authorization".to_owned(),
            });
        }
        if !matches!(
            recipient.status,
            SyncDeviceStatus::Pending | SyncDeviceStatus::Active
        ) {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "recipient device must be pending or active".to_owned(),
            });
        }
        if recipient.device_id != self.recipient_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "recipient device must match signed authorization".to_owned(),
            });
        }
        if recipient.public_key_id != self.recipient_public_key_id {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_public_key_id",
                message: "recipient public key must match signed authorization".to_owned(),
            });
        }
        Ok(())
    }

    pub fn verify_with_devices(
        &self,
        public_key: &DeviceSigningPublicKey,
        authorizer: &SyncDevice,
        recipient: &SyncDevice,
    ) -> Result<(), SyncPayloadError> {
        self.validate_against_devices(authorizer, recipient)?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.created_at_ms)
            .map_err(SyncPayloadError::from_crypto_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDeviceRevocation {
    pub signature: DeviceSignature,
    pub revoked_by_device_id: String,
    pub revoked_device_id: String,
    pub previous_key_epoch: u64,
    pub new_key_epoch: u64,
    pub reason: String,
    pub revoked_at_ms: i64,
}

impl SignedDeviceRevocation {
    pub fn new(
        record: &DeviceRevocationRecord,
        revoked_by: &SyncDevice,
        signature: DeviceSignature,
    ) -> Result<Self, SyncPayloadError> {
        record.validate()?;
        validate_revoker(record, revoked_by)?;

        let revocation = Self {
            signature,
            revoked_by_device_id: record.revoked_by_device_id.clone(),
            revoked_device_id: record.device_id.clone(),
            previous_key_epoch: record.previous_key_epoch,
            new_key_epoch: record.new_key_epoch,
            reason: record.reason.as_str().to_owned(),
            revoked_at_ms: record.revoked_at_ms,
        };
        revocation.validate()?;
        Ok(revocation)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("device_revocation", &self.signature_fields())
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
            SignatureField::text("revoked_by_device_id", &self.revoked_by_device_id),
            SignatureField::text("revoked_device_id", &self.revoked_device_id),
            SignatureField::u64("previous_key_epoch", self.previous_key_epoch),
            SignatureField::u64("new_key_epoch", self.new_key_epoch),
            SignatureField::text("reason", &self.reason),
            SignatureField::i64("revoked_at_ms", self.revoked_at_ms),
        ]
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.signature
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        validate_required("revoked_by_device_id", &self.revoked_by_device_id)?;
        validate_required("revoked_device_id", &self.revoked_device_id)?;
        validate_required("reason", &self.reason)?;
        validate_key_epoch(self.previous_key_epoch)?;
        if self.new_key_epoch <= self.previous_key_epoch {
            return Err(SyncPayloadError::InvalidField {
                field: "new_key_epoch",
                message: "value must be greater than previous_key_epoch".to_owned(),
            });
        }
        if self.signature.signer_device_id != self.revoked_by_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "signer_device_id",
                message: "signature signer must match revocation device".to_owned(),
            });
        }
        Ok(())
    }

    pub fn validate_against_revoker(
        &self,
        revoked_by: &SyncDevice,
    ) -> Result<(), SyncPayloadError> {
        self.validate()?;
        revoked_by.validate()?;
        if !revoked_by.can_receive_key_epoch() {
            return Err(SyncPayloadError::InvalidField {
                field: "revoked_by_device_id",
                message: "revocation signer device must be active".to_owned(),
            });
        }
        if revoked_by.device_id != self.revoked_by_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "revoked_by_device_id",
                message: "revocation signer device must match signed record".to_owned(),
            });
        }
        Ok(())
    }

    pub fn verify_with_revoker(
        &self,
        public_key: &DeviceSigningPublicKey,
        revoked_by: &SyncDevice,
    ) -> Result<(), SyncPayloadError> {
        self.validate_against_revoker(revoked_by)?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.revoked_at_ms)
            .map_err(SyncPayloadError::from_crypto_error)
    }
}

fn validate_authorization_devices(
    package: &DeviceAuthorizationPackage,
    authorizer: &SyncDevice,
    recipient: &SyncDevice,
) -> Result<(), SyncPayloadError> {
    authorizer.validate()?;
    recipient.validate()?;
    if !authorizer.can_receive_key_epoch() {
        return Err(SyncPayloadError::InvalidField {
            field: "authorizer_device_id",
            message: "authorizer device must be active".to_owned(),
        });
    }
    if package.authorized_by_device_id != authorizer.device_id {
        return Err(SyncPayloadError::InvalidField {
            field: "authorizer_device_id",
            message: "authorization package must match authorizer device".to_owned(),
        });
    }
    if !matches!(
        recipient.status,
        SyncDeviceStatus::Pending | SyncDeviceStatus::Active
    ) {
        return Err(SyncPayloadError::InvalidField {
            field: "recipient_device_id",
            message: "recipient device must be pending or active".to_owned(),
        });
    }
    if package.recipient_device_id != recipient.device_id {
        return Err(SyncPayloadError::InvalidField {
            field: "recipient_device_id",
            message: "authorization package must match recipient device".to_owned(),
        });
    }
    Ok(())
}

fn validate_revoker(
    record: &DeviceRevocationRecord,
    revoked_by: &SyncDevice,
) -> Result<(), SyncPayloadError> {
    revoked_by.validate()?;
    if !revoked_by.can_receive_key_epoch() {
        return Err(SyncPayloadError::InvalidField {
            field: "revoked_by_device_id",
            message: "revocation signer device must be active".to_owned(),
        });
    }
    if record.revoked_by_device_id != revoked_by.device_id {
        return Err(SyncPayloadError::InvalidField {
            field: "revoked_by_device_id",
            message: "revocation record must match signer device".to_owned(),
        });
    }
    Ok(())
}

fn validate_key_epoch(value: u64) -> Result<(), SyncPayloadError> {
    if value == 0 {
        return Err(SyncPayloadError::InvalidField {
            field: "key_epoch",
            message: "value must be greater than 0".to_owned(),
        });
    }
    Ok(())
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
    use crate::{DeviceRevocationReason, SyncDeviceStatus};
    use radishlex_ime_crypto::{
        DeviceSigningKeyHandle, DeviceWrappingRecord, KeyDescriptor, KeyRole,
        TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
    };

    #[test]
    fn signed_device_authorization_covers_join_and_wrapping_metadata() {
        let (store, public_key, handle) = signing_fixture();
        let authorizer = active_device("device-a", "signing-key-a", 10);
        let recipient = active_device("device-b", "public-key-b", 10);
        let join_request = make_join_request("device-b", "public-key-b");
        let package = authorization_package(&recipient, &authorizer);

        let signature = sign_authorization(
            &store,
            &handle,
            &join_request,
            &package,
            &authorizer,
            &recipient,
        );
        let signed = SignedDeviceAuthorization::new(
            &join_request,
            &package,
            &authorizer,
            &recipient,
            signature,
        )
        .expect("signed authorization");

        signed
            .verify_with_devices(&public_key, &authorizer, &recipient)
            .expect("authorization verifies");

        let mut tampered = signed.clone();
        tampered.join_challenge = "server-replaced-challenge".to_owned();
        let error = tampered
            .verify_with_devices(&public_key, &authorizer, &recipient)
            .expect_err("tampered authorization fails");
        assert!(error.to_string().contains("signature verification failed"));
    }

    #[test]
    fn signed_device_authorization_rejects_invalid_device_state_and_join_request() {
        let (store, _public_key, handle) = signing_fixture();
        let authorizer = active_device("device-a", "signing-key-a", 10);
        let pending_authorizer =
            SyncDevice::pending("device-a", "signing-key-a", 10).expect("pending authorizer");
        let recipient = active_device("device-b", "public-key-b", 10);
        let join_request = make_join_request("device-b", "public-key-b");
        let package = authorization_package(&recipient, &authorizer);
        let signature = sign_authorization(
            &store,
            &handle,
            &join_request,
            &package,
            &authorizer,
            &recipient,
        );

        let error = SignedDeviceAuthorization::new(
            &join_request,
            &package,
            &pending_authorizer,
            &recipient,
            signature.clone(),
        )
        .expect_err("pending authorizer cannot sign authorization");
        assert!(error.to_string().contains("authorizer_device_id"));

        let mismatched_join = make_join_request("device-b", "other-public-key");
        let error = SignedDeviceAuthorization::new(
            &mismatched_join,
            &package,
            &authorizer,
            &recipient,
            signature,
        )
        .expect_err("join request public key mismatch fails");
        assert!(error.to_string().contains("recipient_public_key_id"));
    }

    #[test]
    fn signed_device_revocation_covers_epoch_and_reason() {
        let (store, public_key, handle) = signing_fixture();
        let revoked_by = active_device("device-a", "signing-key-a", 10);
        let record = DeviceRevocationRecord::new(
            "device-b",
            "device-a",
            3,
            4,
            DeviceRevocationReason::DeviceLost,
            40,
        )
        .expect("revocation record");
        let signature = sign_revocation(&store, &handle, &record, &revoked_by);
        let signed =
            SignedDeviceRevocation::new(&record, &revoked_by, signature).expect("revocation");

        signed
            .verify_with_revoker(&public_key, &revoked_by)
            .expect("revocation verifies");

        let mut tampered = signed.clone();
        tampered.reason = DeviceRevocationReason::UserRequested.as_str().to_owned();
        let error = tampered
            .verify_with_revoker(&public_key, &revoked_by)
            .expect_err("tampered revocation fails");
        assert!(error.to_string().contains("signature verification failed"));
    }

    #[test]
    fn signed_device_revocation_rejects_inactive_revoker() {
        let (store, _public_key, handle) = signing_fixture();
        let revoked_by = active_device("device-a", "signing-key-a", 10);
        let lost_revoker = SyncDevice::new(
            "device-a",
            "signing-key-a",
            SyncDeviceStatus::Lost,
            Some(10),
            Some(30),
            Some(20),
        )
        .expect("lost revoker");
        let record = DeviceRevocationRecord::new(
            "device-b",
            "device-a",
            3,
            4,
            DeviceRevocationReason::DeviceLost,
            40,
        )
        .expect("revocation record");
        let signature = sign_revocation(&store, &handle, &record, &revoked_by);

        let error = SignedDeviceRevocation::new(&record, &lost_revoker, signature)
            .expect_err("lost revoker cannot sign");
        assert!(error.to_string().contains("revoked_by_device_id"));
    }

    fn signing_fixture() -> (
        TestMemoryDeviceKeyStore,
        DeviceSigningPublicKey,
        DeviceSigningKeyHandle,
    ) {
        let mut store = TestMemoryDeviceKeyStore::new();
        let public_key = store
            .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
            .expect("public key");
        let handle = store
            .handle("device-a", "signing-key-a")
            .expect("signing handle");
        (store, public_key, handle)
    }

    fn sign_authorization(
        store: &TestMemoryDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
        join_request: &DeviceJoinRequest,
        package: &DeviceAuthorizationPackage,
        authorizer: &SyncDevice,
        recipient: &SyncDevice,
    ) -> DeviceSignature {
        let placeholder = empty_signature("signing-key-a", "device-a");
        let unsigned = SignedDeviceAuthorization::new(
            join_request,
            package,
            authorizer,
            recipient,
            placeholder,
        )
        .expect("unsigned-shaped authorization");
        store
            .sign(handle, &unsigned.canonical_bytes())
            .expect("authorization signature")
    }

    fn sign_revocation(
        store: &TestMemoryDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
        record: &DeviceRevocationRecord,
        revoked_by: &SyncDevice,
    ) -> DeviceSignature {
        let placeholder = empty_signature("signing-key-a", "device-a");
        let unsigned =
            SignedDeviceRevocation::new(record, revoked_by, placeholder).expect("unsigned-shaped");
        store
            .sign(handle, &unsigned.canonical_bytes())
            .expect("revocation signature")
    }

    fn empty_signature(key_id: &str, device_id: &str) -> DeviceSignature {
        DeviceSignature::new(key_id, device_id, vec![1u8; ED25519_SIGNATURE_LEN])
            .expect("placeholder signature")
    }

    fn active_device(device_id: &str, public_key_id: &str, authorized_at_ms: i64) -> SyncDevice {
        SyncDevice::pending(device_id, public_key_id, authorized_at_ms - 1)
            .expect("pending device")
            .activate(authorized_at_ms)
            .expect("active device")
    }

    fn make_join_request(device_id: &str, public_key_id: &str) -> DeviceJoinRequest {
        DeviceJoinRequest::new(
            device_id,
            public_key_id,
            "join-challenge-device-b",
            "123456",
            20,
            50,
        )
        .expect("join request")
    }

    fn authorization_package(
        recipient: &SyncDevice,
        authorizer: &SyncDevice,
    ) -> DeviceAuthorizationPackage {
        DeviceAuthorizationPackage::new(
            recipient,
            authorizer,
            &device_wrapping_record(&recipient.device_id, 4),
            30,
        )
        .expect("authorization package")
    }

    fn device_wrapping_record(device_id: &str, key_epoch: u64) -> DeviceWrappingRecord {
        let wrapping_key = KeyDescriptor::new("wrapping-key-a", KeyRole::DeviceWrapping, key_epoch)
            .expect("wrapping key");
        DeviceWrappingRecord::new(device_id, &wrapping_key, b"wrapped-sync-key", 30)
            .expect("wrapping record")
    }
}
