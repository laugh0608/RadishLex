use radishlex_ime_crypto::{
    canonical_signature_bytes, DeviceKeyAgreementPublicKey, DeviceSignature,
    DeviceSigningPublicKey, SignatureField, WrappedEpochMaterial,
};

use crate::{SyncDevice, SyncDeviceStatus, SyncPayloadError, SyncTrustedDomainState};

pub const MAX_EPOCH_DISTRIBUTION_RECORDS: usize = 64;

#[derive(Clone, PartialEq, Eq)]
pub struct SignedEpochDistribution {
    pub distributor_device_id: String,
    pub material: WrappedEpochMaterial,
    pub signature: DeviceSignature,
}

impl SignedEpochDistribution {
    pub fn new(
        distributor_device_id: impl Into<String>,
        material: WrappedEpochMaterial,
        signature: DeviceSignature,
    ) -> Result<Self, SyncPayloadError> {
        let value = Self {
            distributor_device_id: distributor_device_id.into(),
            material,
            signature,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes("epoch_distribution", &self.signature_fields())
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
            SignatureField::text("distributor_device_id", &self.distributor_device_id),
            SignatureField::text("domain_id", &self.material.domain_id),
            SignatureField::text("recipient_device_id", &self.material.recipient_device_id),
            SignatureField::text(
                "recipient_key_agreement_key_id",
                &self.material.recipient_key_agreement_key_id,
            ),
            SignatureField::u64("key_epoch", self.material.key_epoch),
            SignatureField::text("wrapping_key_id", &self.material.wrapping_key_id),
            SignatureField::text("envelope_algorithm", &self.material.algorithm),
            SignatureField::bytes("envelope_nonce", self.material.nonce.as_bytes()),
            SignatureField::usize("wrapped_key_len", self.material.wrapped_key.len()),
            SignatureField::text("ciphertext_hash", &self.material.ciphertext_hash),
            SignatureField::i64("created_at_ms", self.material.created_at_ms),
        ]
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.material
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        self.signature
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        if self.distributor_device_id.trim().is_empty() {
            return Err(SyncPayloadError::InvalidField {
                field: "distributor_device_id",
                message: "value is required".to_owned(),
            });
        }
        if self.signature.signer_device_id != self.distributor_device_id {
            return Err(SyncPayloadError::InvalidField {
                field: "signer_device_id",
                message: "signature signer must match distributor device".to_owned(),
            });
        }
        Ok(())
    }

    pub fn verify(
        &self,
        distributor: &SyncDevice,
        distributor_public_key: &DeviceSigningPublicKey,
        recipient: &DeviceKeyAgreementPublicKey,
    ) -> Result<(), SyncPayloadError> {
        self.validate()?;
        distributor.validate()?;
        recipient
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        if !distributor.can_receive_key_epoch()
            || distributor.device_id != self.distributor_device_id
            || distributor.public_key_id != self.signature.signature_key_id
        {
            return Err(SyncPayloadError::InvalidField {
                field: "distributor_device_id",
                message: "distribution signer must match an active trusted device".to_owned(),
            });
        }
        if recipient.device_id != self.material.recipient_device_id
            || recipient.key_id != self.material.recipient_key_agreement_key_id
            || self.material.created_at_ms < recipient.created_at_ms
            || recipient
                .revoked_at_ms
                .is_some_and(|revoked_at_ms| self.material.created_at_ms >= revoked_at_ms)
        {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_key_agreement_key_id",
                message: "distribution recipient must match an active trusted key".to_owned(),
            });
        }
        self.signature
            .verify_at(
                distributor_public_key,
                &self.canonical_bytes(),
                self.material.created_at_ms,
            )
            .map_err(SyncPayloadError::from_crypto_error)
    }
}

impl std::fmt::Debug for SignedEpochDistribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedEpochDistribution")
            .field("distributor_device_id", &self.distributor_device_id)
            .field("recipient_device_id", &self.material.recipient_device_id)
            .field("key_epoch", &self.material.key_epoch)
            .field("wrapping_key_id", &"[redacted]")
            .field("wrapped_key_len", &self.material.wrapped_key.len())
            .field("signature", &self.signature)
            .finish()
    }
}

pub fn validate_epoch_distribution_batch(
    trusted_domain: &SyncTrustedDomainState,
    records: &[SignedEpochDistribution],
) -> Result<(), SyncPayloadError> {
    if records.is_empty() || records.len() > MAX_EPOCH_DISTRIBUTION_RECORDS {
        return Err(SyncPayloadError::InvalidField {
            field: "epoch_distribution_records",
            message: "record count must cover 1 to 64 active devices".to_owned(),
        });
    }
    let first = &records[0];
    let distributor = trusted_domain
        .device_profile(&first.distributor_device_id)
        .ok_or_else(|| SyncPayloadError::InvalidField {
            field: "distributor_device_id",
            message: "distributor is not present in trusted lifecycle".to_owned(),
        })?;
    if distributor.device().status != SyncDeviceStatus::Active {
        return Err(SyncPayloadError::InvalidField {
            field: "distributor_device_id",
            message: "distributor must be active".to_owned(),
        });
    }
    let active_count = trusted_domain
        .device_profiles()
        .filter(|profile| profile.device().status == SyncDeviceStatus::Active)
        .count();
    if active_count != records.len() {
        return Err(SyncPayloadError::InvalidField {
            field: "epoch_distribution_records",
            message: "records must cover the complete trusted active device cohort".to_owned(),
        });
    }

    let mut recipients = std::collections::BTreeSet::new();
    for record in records {
        if record.distributor_device_id != first.distributor_device_id
            || record.material.domain_id != trusted_domain.domain().domain_id
            || record.material.key_epoch != trusted_domain.domain().current_key_epoch
            || !recipients.insert(record.material.recipient_device_id.as_str())
        {
            return Err(SyncPayloadError::InvalidField {
                field: "epoch_distribution_records",
                message: "records must share trusted domain/distributor/current epoch and unique recipients"
                    .to_owned(),
            });
        }
        let recipient = trusted_domain
            .device_profile(&record.material.recipient_device_id)
            .ok_or_else(|| SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "recipient is not present in trusted lifecycle".to_owned(),
            })?;
        if recipient.device().status != SyncDeviceStatus::Active {
            return Err(SyncPayloadError::InvalidField {
                field: "recipient_device_id",
                message: "distribution recipient must be active".to_owned(),
            });
        }
        record.verify(
            distributor.device(),
            distributor.signing_public_key(),
            recipient
                .key_agreement_public_key()
                .ok_or_else(|| SyncPayloadError::InvalidField {
                    field: "recipient_key_agreement_key_id",
                    message: "trusted recipient key-agreement profile is required".to_owned(),
                })?,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
    use radishlex_ime_crypto::{
        KeyDescriptor, KeyRole, Nonce, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
        ED25519_SIGNATURE_LEN,
    };

    #[test]
    fn signed_distribution_binds_recipient_epoch_and_ciphertext_metadata() {
        let (store, handle, public_key) = signing_fixture();
        let distributor = active_device();
        let recipient = recipient_key(None);
        let material = wrapped_material(&recipient);
        let unsigned = SignedEpochDistribution::new(
            "device-a",
            material,
            DeviceSignature::new("signing-key-a", "device-a", vec![1; ED25519_SIGNATURE_LEN])
                .expect("placeholder signature"),
        )
        .expect("unsigned-shaped distribution");
        let signature = store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("distribution signature");
        let signed = SignedEpochDistribution::new("device-a", unsigned.material.clone(), signature)
            .expect("signed distribution");
        signed
            .verify(&distributor, &public_key, &recipient)
            .expect("verify signed distribution");

        let mut tampered = signed.clone();
        tampered.material.wrapping_key_id = "different-locator".to_owned();
        assert!(tampered
            .verify(&distributor, &public_key, &recipient)
            .is_err());
    }

    #[test]
    fn signed_distribution_rejects_revoked_recipient_key() {
        let (store, handle, public_key) = signing_fixture();
        let distributor = active_device();
        let recipient = recipient_key(None);
        let material = wrapped_material(&recipient);
        let unsigned = SignedEpochDistribution::new(
            "device-a",
            material,
            DeviceSignature::new("signing-key-a", "device-a", vec![1; ED25519_SIGNATURE_LEN])
                .expect("placeholder signature"),
        )
        .expect("unsigned-shaped distribution");
        let signature = store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("distribution signature");
        let signed = SignedEpochDistribution::new("device-a", unsigned.material, signature)
            .expect("signed distribution");
        assert!(signed
            .verify(&distributor, &public_key, &recipient_key(Some(19)))
            .is_err());
    }

    fn signing_fixture() -> (
        TestMemoryDeviceKeyStore,
        radishlex_ime_crypto::DeviceSigningKeyHandle,
        DeviceSigningPublicKey,
    ) {
        let mut store = TestMemoryDeviceKeyStore::new();
        store
            .insert_signing_key("device-a", "signing-key-a", [9; 32], 10)
            .expect("insert signing key");
        let handle = store
            .handle("device-a", "signing-key-a")
            .expect("signing handle");
        let public_key = store.public_key(&handle).expect("signing public key");
        (store, handle, public_key)
    }

    fn active_device() -> SyncDevice {
        SyncDevice::pending("device-a", "signing-key-a", 9)
            .expect("pending distributor")
            .activate(10)
            .expect("active distributor")
    }

    fn recipient_key(revoked_at_ms: Option<i64>) -> DeviceKeyAgreementPublicKey {
        let secret = SecretKey::from_slice(&[7; 32]).expect("recipient secret");
        DeviceKeyAgreementPublicKey::p256(
            "device-b",
            "agreement-key-b",
            secret
                .public_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
            10,
            revoked_at_ms,
        )
        .expect("recipient key")
    }

    fn wrapped_material(recipient: &DeviceKeyAgreementPublicKey) -> WrappedEpochMaterial {
        WrappedEpochMaterial::seal_for_recipient(
            "domain-a",
            recipient,
            "epoch-2-device-b",
            &KeyDescriptor::new("object-key-v2", KeyRole::ObjectKey, 2).expect("object key"),
            &SyncMasterKeyMaterial::new([5; 32]).expect("master key"),
            Nonce::new(vec![2; 24]).expect("nonce"),
            20,
        )
        .expect("wrapped material")
    }
}
