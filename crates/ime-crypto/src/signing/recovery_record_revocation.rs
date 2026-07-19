use super::{canonical_signature_bytes, DeviceSignature, DeviceSigningPublicKey, SignatureField};
use crate::model::{validate_required, CryptoError};

pub const RECOVERY_RECORD_REVOCATION_RECORD_TYPE: &str = "recovery_record_revocation";
const MAX_REVOCATION_REASON_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRecoveryRecordRevocation {
    pub signature: DeviceSignature,
    pub recovery_record_id: String,
    pub domain_id: String,
    pub key_epoch: u64,
    pub reason: String,
    pub created_at_ms: i64,
}

impl SignedRecoveryRecordRevocation {
    pub fn new(
        recovery_record_id: impl Into<String>,
        domain_id: impl Into<String>,
        key_epoch: u64,
        reason: impl Into<String>,
        created_at_ms: i64,
        signature: DeviceSignature,
    ) -> Result<Self, CryptoError> {
        let revocation = Self {
            signature,
            recovery_record_id: recovery_record_id.into(),
            domain_id: domain_id.into(),
            key_epoch,
            reason: reason.into(),
            created_at_ms,
        };
        revocation.validate()?;
        Ok(revocation)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_signature_bytes(
            RECOVERY_RECORD_REVOCATION_RECORD_TYPE,
            &self.signature_fields(),
        )
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
            SignatureField::text("revoker_device_id", &self.signature.signer_device_id),
            SignatureField::text("recovery_record_id", &self.recovery_record_id),
            SignatureField::text("domain_id", &self.domain_id),
            SignatureField::u64("key_epoch", self.key_epoch),
            SignatureField::text("reason", &self.reason),
            SignatureField::i64("created_at_ms", self.created_at_ms),
        ]
    }

    pub fn verify(&self, public_key: &DeviceSigningPublicKey) -> Result<(), CryptoError> {
        self.validate()?;
        self.signature
            .verify_at(public_key, &self.canonical_bytes(), self.created_at_ms)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        self.signature.validate()?;
        validate_required("recovery_record_id", &self.recovery_record_id)?;
        validate_required("domain_id", &self.domain_id)?;
        validate_required("reason", &self.reason)?;
        if self.reason.len() > MAX_REVOCATION_REASON_BYTES
            || self.reason.chars().any(char::is_control)
        {
            return Err(CryptoError::invalid_field(
                "reason",
                "value must be at most 64 bytes and contain no control characters",
            ));
        }
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than 0",
            ));
        }
        if self.created_at_ms <= 0 {
            return Err(CryptoError::invalid_field(
                "created_at_ms",
                "value must be greater than 0",
            ));
        }
        Ok(())
    }
}
