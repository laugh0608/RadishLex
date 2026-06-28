use std::fmt;

use radishlex_ime_crypto::{
    EncryptedObjectEnvelope, KeyDescriptor, KeyRole, NonceTracker, PlaintextPayload,
    SyncMasterKeyMaterial,
};

use crate::model::{EncryptedSyncObjectDraft, SyncObjectType, SyncPayloadError};

#[derive(Clone, PartialEq, Eq)]
pub struct PlaintextSyncPayload {
    pub object_type: SyncObjectType,
    pub record_count: usize,
    pub bytes: Vec<u8>,
}

impl PlaintextSyncPayload {
    pub fn new(
        object_type: SyncObjectType,
        record_count: usize,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, SyncPayloadError> {
        let payload = Self {
            object_type,
            record_count,
            bytes: bytes.into(),
        };
        payload.validate()?;
        Ok(payload)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        if self.record_count == 0 {
            return Err(SyncPayloadError::InvalidField {
                field: "record_count",
                message: "value must be greater than 0".to_owned(),
            });
        }
        if self.bytes.is_empty() {
            return Err(SyncPayloadError::InvalidField {
                field: "plaintext_payload",
                message: "value cannot be empty".to_owned(),
            });
        }
        Ok(())
    }
}

impl fmt::Debug for PlaintextSyncPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaintextSyncPayload")
            .field("object_type", &self.object_type)
            .field("record_count", &self.record_count)
            .field("bytes", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncObjectAssemblySpec {
    pub object_id: String,
    pub owner_device_id: String,
    pub object_key: KeyDescriptor,
    pub version: u64,
    pub base_version: Option<u64>,
    pub timestamp_ms: i64,
}

impl SyncObjectAssemblySpec {
    pub fn new(
        object_id: impl Into<String>,
        owner_device_id: impl Into<String>,
        object_key: KeyDescriptor,
        version: u64,
        base_version: Option<u64>,
        timestamp_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let spec = Self {
            object_id: object_id.into(),
            owner_device_id: owner_device_id.into(),
            object_key,
            version,
            base_version,
            timestamp_ms,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("object_id", &self.object_id)?;
        validate_required("owner_device_id", &self.owner_device_id)?;
        self.object_key
            .validate()
            .map_err(SyncPayloadError::from_crypto_error)?;
        if self.object_key.role != KeyRole::ObjectKey {
            return Err(SyncPayloadError::InvalidField {
                field: "key_role",
                message: "object assembly requires an object key descriptor".to_owned(),
            });
        }
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
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AssembledSyncObject {
    pub envelope: EncryptedObjectEnvelope,
    pub draft: EncryptedSyncObjectDraft,
    pub record_count: usize,
}

impl fmt::Debug for AssembledSyncObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssembledSyncObject")
            .field("draft", &self.draft)
            .field("record_count", &self.record_count)
            .field(
                "encrypted_payload_len",
                &self.envelope.encrypted_payload.len(),
            )
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct SyncEnvelopeAssembler {
    nonce_tracker: NonceTracker,
}

impl SyncEnvelopeAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assemble_payload(
        &mut self,
        payload: PlaintextSyncPayload,
        spec: SyncObjectAssemblySpec,
        sync_master_key: &SyncMasterKeyMaterial,
    ) -> Result<AssembledSyncObject, SyncPayloadError> {
        let object_key_material = sync_master_key
            .derive_object_key(
                &spec.object_key,
                payload.object_type.to_crypto_object_type(),
                &spec.object_id,
            )
            .map_err(SyncPayloadError::from_crypto_error)?;
        let crypto_payload = PlaintextPayload::new(
            payload.object_type.to_crypto_object_type(),
            payload.bytes.clone(),
        )
        .map_err(SyncPayloadError::from_crypto_error)?;

        let envelope = EncryptedObjectEnvelope::encrypt_payload(
            spec.object_id,
            spec.owner_device_id,
            &spec.object_key,
            &object_key_material,
            spec.version,
            spec.base_version,
            crypto_payload,
            spec.timestamp_ms,
        )
        .map_err(SyncPayloadError::from_crypto_error)?;

        self.finish_payload(payload.object_type, payload.record_count, envelope)
    }

    #[cfg(test)]
    fn assemble_payload_with_nonce(
        &mut self,
        payload: PlaintextSyncPayload,
        spec: SyncObjectAssemblySpec,
        sync_master_key: &SyncMasterKeyMaterial,
        nonce: radishlex_ime_crypto::Nonce,
    ) -> Result<AssembledSyncObject, SyncPayloadError> {
        let object_key_material = sync_master_key
            .derive_object_key(
                &spec.object_key,
                payload.object_type.to_crypto_object_type(),
                &spec.object_id,
            )
            .map_err(SyncPayloadError::from_crypto_error)?;
        let crypto_payload = PlaintextPayload::new(
            payload.object_type.to_crypto_object_type(),
            payload.bytes.clone(),
        )
        .map_err(SyncPayloadError::from_crypto_error)?;

        let envelope = EncryptedObjectEnvelope::encrypt_payload_with_nonce(
            spec.object_id,
            spec.owner_device_id,
            &spec.object_key,
            &object_key_material,
            spec.version,
            spec.base_version,
            crypto_payload,
            spec.timestamp_ms,
            nonce,
        )
        .map_err(SyncPayloadError::from_crypto_error)?;

        self.finish_payload(payload.object_type, payload.record_count, envelope)
    }

    fn finish_payload(
        &mut self,
        object_type: SyncObjectType,
        record_count: usize,
        envelope: EncryptedObjectEnvelope,
    ) -> Result<AssembledSyncObject, SyncPayloadError> {
        self.nonce_tracker
            .observe(&envelope)
            .map_err(SyncPayloadError::from_crypto_error)?;
        let draft = EncryptedSyncObjectDraft::from_crypto_envelope(&envelope)?;
        if draft.object_type != object_type {
            return Err(SyncPayloadError::InvalidField {
                field: "object_type",
                message: "draft object type must match plaintext payload".to_owned(),
            });
        }
        Ok(AssembledSyncObject {
            envelope,
            draft,
            record_count,
        })
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
    use radishlex_ime_crypto::{
        KeyRole, Nonce, SyncMasterKeyMaterial, XCHACHA20POLY1305_NONCE_LEN,
    };

    #[test]
    fn assembler_encrypts_payload_and_returns_sync_draft_without_plaintext() {
        let payload = sample_payload("radish-alpha");
        let spec = sample_spec(1, None);
        let sync_master_key = sync_master_key();
        let mut assembler = SyncEnvelopeAssembler::new();

        let object = assembler
            .assemble_payload(payload.clone(), spec, &sync_master_key)
            .expect("assembled object");

        assert_eq!(object.record_count, payload.record_count);
        assert_eq!(object.draft.object_type, payload.object_type);
        assert_eq!(object.draft.object_id, "dictionary-user-terms");
        assert_eq!(object.draft.owner_device_id, "device-a");
        assert_eq!(object.draft.version, 1);
        assert_eq!(object.draft.key_id, "object-key-a");
        assert_eq!(object.draft.key_epoch, 3);
        assert_eq!(
            object.draft.encrypted_payload_len,
            object.envelope.encrypted_payload.len()
        );
        assert_ne!(object.envelope.encrypted_payload, payload.bytes);

        let debug = format!("{object:?}");
        assert!(!debug.contains("radish-alpha"));
        assert!(!debug.contains("payload_schema_version"));
    }

    #[test]
    fn assembler_derives_object_keys_from_object_identity() {
        let payload = sample_payload("radish-alpha");
        let sync_master_key = sync_master_key();
        let mut first_assembler = SyncEnvelopeAssembler::new();
        let mut second_assembler = SyncEnvelopeAssembler::new();

        let first = first_assembler
            .assemble_payload_with_nonce(
                payload.clone(),
                sample_spec(1, None),
                &sync_master_key,
                nonce(9),
            )
            .expect("first object");
        let second = second_assembler
            .assemble_payload_with_nonce(
                payload,
                SyncObjectAssemblySpec::new(
                    "dictionary-user-terms-b",
                    "device-a",
                    KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 3).expect("key"),
                    1,
                    None,
                    100,
                )
                .expect("spec"),
                &sync_master_key,
                nonce(9),
            )
            .expect("second object");

        assert_ne!(
            first.envelope.encrypted_payload,
            second.envelope.encrypted_payload
        );
        assert_ne!(
            first.envelope.ciphertext_hash,
            second.envelope.ciphertext_hash
        );
    }

    #[test]
    fn assembler_rejects_duplicate_nonce_for_same_key_epoch() {
        let sync_master_key = sync_master_key();
        let mut assembler = SyncEnvelopeAssembler::new();

        assembler
            .assemble_payload_with_nonce(
                sample_payload("first"),
                sample_spec(1, None),
                &sync_master_key,
                nonce(9),
            )
            .expect("first object");

        let error = assembler
            .assemble_payload_with_nonce(
                sample_payload("second"),
                sample_spec(2, Some(1)),
                &sync_master_key,
                nonce(9),
            )
            .expect_err("duplicate nonce fails");
        assert!(error.to_string().contains("duplicate nonce"));
    }

    #[test]
    fn payload_and_spec_validate_sync_metadata() {
        let error = PlaintextSyncPayload::new(SyncObjectType::DictionaryUserTerms, 0, b"payload")
            .expect_err("record count required");
        assert!(error.to_string().contains("record_count"));

        let error = SyncObjectAssemblySpec::new(
            "dictionary-user-terms",
            "device-a",
            KeyDescriptor::new("sync-master", KeyRole::SyncMaster, 3).expect("key"),
            1,
            None,
            100,
        )
        .expect_err("object key required");
        assert!(error.to_string().contains("key_role"));
    }

    fn sample_payload(text: &str) -> PlaintextSyncPayload {
        PlaintextSyncPayload::new(
            SyncObjectType::DictionaryUserTerms,
            1,
            format!(
                r#"{{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{{"input_code":"radish","text":"{text}"}}]}}"#
            )
            .into_bytes(),
        )
        .expect("payload")
    }

    fn sample_spec(version: u64, base_version: Option<u64>) -> SyncObjectAssemblySpec {
        SyncObjectAssemblySpec::new(
            "dictionary-user-terms",
            "device-a",
            KeyDescriptor::new("object-key-a", KeyRole::ObjectKey, 3).expect("key"),
            version,
            base_version,
            100,
        )
        .expect("spec")
    }

    fn sync_master_key() -> SyncMasterKeyMaterial {
        SyncMasterKeyMaterial::new([7u8; 32]).expect("sync master key")
    }

    fn nonce(seed: u8) -> Nonce {
        Nonce::new(vec![seed; XCHACHA20POLY1305_NONCE_LEN]).expect("nonce")
    }
}
