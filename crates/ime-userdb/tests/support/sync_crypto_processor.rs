use std::collections::BTreeMap;

use radishlex_ime_crypto::{
    CryptoError, DeviceSignature, DeviceSigningPublicKey, KeyDescriptor, KeyRole,
    SignatureAlgorithmId, SignedSyncObjectManifest, SyncMasterKeyMaterial,
    TestMemoryDeviceKeyStore, SIGNATURE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    DecryptedSyncObject, LocalSyncSnapshot, PreparedSyncOutbox, RemoteObjectPayload,
    RemoteObjectVersion, SyncCyclePhase, SyncEnvelopeAssembler, SyncObjectAssemblySpec,
    SyncObjectProcessor, SyncOrchestrationError, SyncOrchestrationErrorCode,
};

use super::{
    envelope_from_remote, sign_object, BASE_TIMESTAMP_MS, DEVICE_A, DEVICE_B, DOMAIN_ID,
    OBJECT_KEY_ID, SIGNING_KEY_A, SIGNING_KEY_B,
};

pub(super) struct TestCryptoProcessor {
    device_id: &'static str,
    signing_key_id: &'static str,
    signing_store: TestMemoryDeviceKeyStore,
    public_keys: BTreeMap<String, DeviceSigningPublicKey>,
    sync_master_key: SyncMasterKeyMaterial,
    object_key: KeyDescriptor,
    assembler: SyncEnvelopeAssembler,
}

impl TestCryptoProcessor {
    pub(super) fn new(device_id: &'static str, signing_key_id: &'static str) -> Self {
        let (signing_store, public_key_a, public_key_b) = test_signing_material();
        Self {
            device_id,
            signing_key_id,
            signing_store,
            public_keys: BTreeMap::from([
                (DEVICE_A.to_owned(), public_key_a),
                (DEVICE_B.to_owned(), public_key_b),
            ]),
            sync_master_key: SyncMasterKeyMaterial::new([11u8; 32]).expect("sync master key"),
            object_key: KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, 1)
                .expect("object key"),
            assembler: SyncEnvelopeAssembler::new(),
        }
    }

    pub(super) fn signing_store(&self) -> &TestMemoryDeviceKeyStore {
        &self.signing_store
    }
}

impl SyncObjectProcessor for TestCryptoProcessor {
    fn preflight(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError> {
        if domain_id != DOMAIN_ID
            || self
                .signing_store
                .handle(self.device_id, self.signing_key_id)
                .is_err()
        {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::BackendUnavailable,
                SyncCyclePhase::Preflight,
            ));
        }
        Ok(())
    }

    fn verify_and_decrypt(
        &mut self,
        expected: &RemoteObjectVersion,
        downloaded: RemoteObjectPayload,
    ) -> Result<DecryptedSyncObject, SyncOrchestrationError> {
        if expected.domain_id != DOMAIN_ID
            || expected.signature_schema_version != SIGNATURE_SCHEMA_VERSION
        {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::UnsupportedSchema,
                SyncCyclePhase::Verify,
            ));
        }
        let remote = downloaded.object.clone();
        let signature_algorithm = SignatureAlgorithmId::new(remote.signature_algorithm.clone())
            .map_err(|_| {
                orchestration_error(
                    SyncOrchestrationErrorCode::UnsupportedAlgorithm,
                    SyncCyclePhase::Verify,
                )
            })?;
        let signature = DeviceSignature::new_for_algorithm(
            signature_algorithm,
            remote.signature_key_id.clone(),
            remote.owner_device_id.clone(),
            remote.signature.clone(),
        )
        .map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::SignatureMismatch,
                SyncCyclePhase::Verify,
            )
        })?;
        let public_key = self
            .public_keys
            .get(&remote.owner_device_id)
            .filter(|key| key.signing_key_id == remote.signature_key_id)
            .ok_or_else(|| {
                orchestration_error(
                    SyncOrchestrationErrorCode::SignatureMismatch,
                    SyncCyclePhase::Verify,
                )
            })?;
        let envelope = envelope_from_remote(downloaded);
        let manifest =
            SignedSyncObjectManifest::new(DOMAIN_ID, &envelope, signature).map_err(|_| {
                orchestration_error(
                    SyncOrchestrationErrorCode::InvalidMetadata,
                    SyncCyclePhase::Verify,
                )
            })?;
        manifest.verify(public_key).map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::SignatureMismatch,
                SyncCyclePhase::Verify,
            )
        })?;
        let object_key_material = self
            .sync_master_key
            .derive_object_key(&self.object_key, envelope.object_type, &envelope.object_id)
            .map_err(|_| {
                orchestration_error(
                    SyncOrchestrationErrorCode::DecryptFailed,
                    SyncCyclePhase::DecryptAndDecode,
                )
            })?;
        let plaintext = envelope
            .decrypt_payload(&object_key_material)
            .map_err(map_decrypt_error)?;
        Ok(DecryptedSyncObject {
            remote,
            plaintext_payload: plaintext.bytes,
        })
    }

    fn prepare_outbox(
        &mut self,
        snapshot: LocalSyncSnapshot,
        version: u64,
        prepared_at_ms: i64,
    ) -> Result<PreparedSyncOutbox, SyncOrchestrationError> {
        if snapshot.domain_id != DOMAIN_ID || snapshot.object_type != snapshot.payload.object_type {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::PrepareSignedOutbox,
            ));
        }
        let spec = SyncObjectAssemblySpec::new(
            snapshot.object_id,
            self.device_id,
            self.object_key.clone(),
            version,
            snapshot.remote_base_version,
            prepared_at_ms,
        )
        .map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::PrepareSignedOutbox,
            )
        })?;
        let object = self
            .assembler
            .assemble_payload(snapshot.payload, spec, &self.sync_master_key)
            .map_err(|_| {
                orchestration_error(
                    SyncOrchestrationErrorCode::DecryptFailed,
                    SyncCyclePhase::PrepareSignedOutbox,
                )
            })?;
        let manifest = sign_object(
            &object,
            &self.signing_store,
            self.device_id,
            self.signing_key_id,
        );
        Ok(PreparedSyncOutbox {
            domain_id: DOMAIN_ID.to_owned(),
            local_revision: snapshot.local_revision,
            object,
            manifest,
            attempt_count: 0,
        })
    }
}

pub(super) fn test_signing_material() -> (
    TestMemoryDeviceKeyStore,
    DeviceSigningPublicKey,
    DeviceSigningPublicKey,
) {
    let mut signing_store = TestMemoryDeviceKeyStore::new();
    let public_key_a = signing_store
        .insert_signing_key(DEVICE_A, SIGNING_KEY_A, [8u8; 32], BASE_TIMESTAMP_MS)
        .expect("device a signing key");
    let public_key_b = signing_store
        .insert_signing_key(DEVICE_B, SIGNING_KEY_B, [9u8; 32], BASE_TIMESTAMP_MS)
        .expect("device b signing key");
    (signing_store, public_key_a, public_key_b)
}

fn map_decrypt_error(error: CryptoError) -> SyncOrchestrationError {
    let code = match error {
        CryptoError::CiphertextHashMismatch => SyncOrchestrationErrorCode::CiphertextHashMismatch,
        CryptoError::AssociatedDataMismatch { .. } => SyncOrchestrationErrorCode::AadMismatch,
        _ => SyncOrchestrationErrorCode::DecryptFailed,
    };
    orchestration_error(code, SyncCyclePhase::DecryptAndDecode)
}

fn orchestration_error(
    code: SyncOrchestrationErrorCode,
    phase: SyncCyclePhase,
) -> SyncOrchestrationError {
    SyncOrchestrationError::new(code, phase, false)
}
