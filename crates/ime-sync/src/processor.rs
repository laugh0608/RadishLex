use std::collections::BTreeMap;
use std::fmt;

use radishlex_ime_crypto::{
    AlgorithmId, CiphertextHash, CryptoError, DevicePrivateKeyStoreStatus, DeviceSignature,
    DeviceSigningKeyHandle, DeviceSigningPublicKey, EncryptedObjectEnvelope, KeyDescriptor,
    KeyRole, Nonce, SignatureAlgorithmId, SignedSyncObjectManifest, SyncMasterKeyMaterial,
    ED25519_SIGNATURE_LEN, ENVELOPE_SCHEMA_VERSION, P256_SIGNATURE_LEN,
    SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1, SIGNATURE_ALGORITHM_ED25519_V1,
    SIGNATURE_SCHEMA_VERSION,
};

use crate::{
    DecryptedSyncObject, LocalSyncSnapshot, PreparedSyncOutbox, RemoteObjectPayload,
    RemoteObjectVersion, SyncCyclePhase, SyncEnvelopeAssembler, SyncObjectAssemblySpec,
    SyncObjectProcessor, SyncOrchestrationError, SyncOrchestrationErrorCode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncCryptoSnapshotKind {
    Production,
    SyntheticTest,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SyncEpochKeyMaterial {
    object_key: KeyDescriptor,
    sync_master_key: SyncMasterKeyMaterial,
}

impl SyncEpochKeyMaterial {
    pub fn new(
        object_key: KeyDescriptor,
        sync_master_key: SyncMasterKeyMaterial,
    ) -> Result<Self, SyncOrchestrationError> {
        object_key.validate().map_err(|_| invalid_preflight())?;
        if object_key.role != KeyRole::ObjectKey {
            return Err(invalid_preflight());
        }
        Ok(Self {
            object_key,
            sync_master_key,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.object_key.key_id
    }

    pub fn key_epoch(&self) -> u64 {
        self.object_key.key_epoch
    }
}

impl fmt::Debug for SyncEpochKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncEpochKeyMaterial")
            .field("key_id", &self.object_key.key_id)
            .field("key_epoch", &self.object_key.key_epoch)
            .field("sync_master_key", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SyncRemoteSigningProfile {
    public_key: DeviceSigningPublicKey,
    reject_from_change_sequence: Option<u64>,
}

impl SyncRemoteSigningProfile {
    pub fn active(public_key: DeviceSigningPublicKey) -> Result<Self, SyncOrchestrationError> {
        Self::new(public_key, None)
    }

    pub fn revoked_from_change_sequence(
        public_key: DeviceSigningPublicKey,
        reject_from_change_sequence: u64,
    ) -> Result<Self, SyncOrchestrationError> {
        Self::new(public_key, Some(reject_from_change_sequence))
    }

    fn new(
        public_key: DeviceSigningPublicKey,
        reject_from_change_sequence: Option<u64>,
    ) -> Result<Self, SyncOrchestrationError> {
        public_key.validate().map_err(|_| invalid_preflight())?;
        if reject_from_change_sequence == Some(0) {
            return Err(invalid_preflight());
        }
        Ok(Self {
            public_key,
            reject_from_change_sequence,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.public_key.device_id
    }

    pub fn signing_key_id(&self) -> &str {
        &self.public_key.signing_key_id
    }

    fn rejects(&self, change_sequence: u64) -> bool {
        self.reject_from_change_sequence
            .map(|first_rejected| change_sequence >= first_rejected)
            .unwrap_or(false)
    }
}

impl fmt::Debug for SyncRemoteSigningProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncRemoteSigningProfile")
            .field("device_id", &self.public_key.device_id)
            .field("signing_key_id", &self.public_key.signing_key_id)
            .field("signature_algorithm", &self.public_key.signature_algorithm)
            .field(
                "reject_from_change_sequence",
                &self.reject_from_change_sequence,
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct SyncCryptoCycleSnapshot {
    kind: SyncCryptoSnapshotKind,
    domain_id: String,
    signing_handle: DeviceSigningKeyHandle,
    local_signing_public_key: DeviceSigningPublicKey,
    backend_status: DevicePrivateKeyStoreStatus,
    current_write_epoch: u64,
    epoch_materials: BTreeMap<u64, SyncEpochKeyMaterial>,
    remote_signers: BTreeMap<(String, String), SyncRemoteSigningProfile>,
}

impl SyncCryptoCycleSnapshot {
    pub fn production(
        domain_id: impl Into<String>,
        signing_handle: DeviceSigningKeyHandle,
        local_signing_public_key: DeviceSigningPublicKey,
        backend_status: DevicePrivateKeyStoreStatus,
        current_write_epoch: u64,
        epoch_materials: impl IntoIterator<Item = SyncEpochKeyMaterial>,
        remote_signers: impl IntoIterator<Item = SyncRemoteSigningProfile>,
    ) -> Result<Self, SyncOrchestrationError> {
        let snapshot = Self::new(
            SyncCryptoSnapshotKind::Production,
            domain_id,
            signing_handle,
            local_signing_public_key,
            backend_status,
            current_write_epoch,
            epoch_materials,
            remote_signers,
        )?;
        snapshot
            .backend_status
            .ensure_production_signing_allowed()
            .map_err(|_| backend_preflight())?;
        Ok(snapshot)
    }

    #[doc(hidden)]
    pub fn synthetic_for_tests(
        domain_id: impl Into<String>,
        signing_handle: DeviceSigningKeyHandle,
        local_signing_public_key: DeviceSigningPublicKey,
        backend_status: DevicePrivateKeyStoreStatus,
        current_write_epoch: u64,
        epoch_materials: impl IntoIterator<Item = SyncEpochKeyMaterial>,
        remote_signers: impl IntoIterator<Item = SyncRemoteSigningProfile>,
    ) -> Result<Self, SyncOrchestrationError> {
        let snapshot = Self::new(
            SyncCryptoSnapshotKind::SyntheticTest,
            domain_id,
            signing_handle,
            local_signing_public_key,
            backend_status,
            current_write_epoch,
            epoch_materials,
            remote_signers,
        )?;
        if !snapshot.backend_status.storage_backend.is_test_only()
            || snapshot.backend_status.product_qualified
        {
            return Err(invalid_preflight());
        }
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        kind: SyncCryptoSnapshotKind,
        domain_id: impl Into<String>,
        signing_handle: DeviceSigningKeyHandle,
        local_signing_public_key: DeviceSigningPublicKey,
        backend_status: DevicePrivateKeyStoreStatus,
        current_write_epoch: u64,
        epoch_materials: impl IntoIterator<Item = SyncEpochKeyMaterial>,
        remote_signers: impl IntoIterator<Item = SyncRemoteSigningProfile>,
    ) -> Result<Self, SyncOrchestrationError> {
        let domain_id = domain_id.into();
        if domain_id.trim().is_empty() || current_write_epoch == 0 {
            return Err(invalid_preflight());
        }
        signing_handle.validate().map_err(|_| invalid_preflight())?;
        local_signing_public_key
            .validate()
            .map_err(|_| invalid_preflight())?;
        backend_status.validate().map_err(|_| backend_preflight())?;
        if signing_handle.device_id != local_signing_public_key.device_id
            || signing_handle.signing_key_id != local_signing_public_key.signing_key_id
            || signing_handle.signature_algorithm != local_signing_public_key.signature_algorithm
            || backend_status.storage_backend != signing_handle.storage_backend
            || backend_status.signature_algorithm.as_ref()
                != Some(&signing_handle.signature_algorithm)
        {
            return Err(invalid_preflight());
        }

        let mut epoch_map = BTreeMap::new();
        for material in epoch_materials {
            let epoch = material.key_epoch();
            if epoch_map.insert(epoch, material).is_some() {
                return Err(invalid_preflight());
            }
        }
        if !epoch_map.contains_key(&current_write_epoch) {
            return Err(invalid_preflight());
        }

        let mut signer_map = BTreeMap::new();
        for profile in remote_signers {
            let identity = (
                profile.device_id().to_owned(),
                profile.signing_key_id().to_owned(),
            );
            if signer_map.insert(identity, profile).is_some() {
                return Err(invalid_preflight());
            }
        }
        if signer_map.is_empty() {
            return Err(invalid_preflight());
        }

        Ok(Self {
            kind,
            domain_id,
            signing_handle,
            local_signing_public_key,
            backend_status,
            current_write_epoch,
            epoch_materials: epoch_map,
            remote_signers: signer_map,
        })
    }

    pub fn current_write_epoch(&self) -> u64 {
        self.current_write_epoch
    }

    pub fn accepted_key_epochs(&self) -> impl Iterator<Item = u64> + '_ {
        self.epoch_materials.keys().copied()
    }
}

impl fmt::Debug for SyncCryptoCycleSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncCryptoCycleSnapshot")
            .field("kind", &self.kind)
            .field("domain_id", &self.domain_id)
            .field("local_device_id", &self.signing_handle.device_id)
            .field("signing_key_id", &self.signing_handle.signing_key_id)
            .field("storage_backend", &self.backend_status.storage_backend)
            .field("current_write_epoch", &self.current_write_epoch)
            .field("accepted_epoch_count", &self.epoch_materials.len())
            .field("remote_signer_count", &self.remote_signers.len())
            .finish()
    }
}

pub trait SyncCryptoProvider {
    fn freeze_cycle(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError>;

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableSyncCryptoProvider;

impl SyncCryptoProvider for UnavailableSyncCryptoProvider {
    fn freeze_cycle(
        &mut self,
        _domain_id: &str,
    ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError> {
        Err(backend_preflight())
    }

    fn sign(
        &self,
        _handle: &DeviceSigningKeyHandle,
        _canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        Err(CryptoError::StorageBackendUnavailable {
            backend: "unavailable".to_owned(),
        })
    }
}

#[derive(Debug)]
pub struct DefaultSyncObjectProcessor<P> {
    provider: P,
    allow_synthetic_snapshot: bool,
    snapshot: Option<SyncCryptoCycleSnapshot>,
    assembler: SyncEnvelopeAssembler,
}

impl Default for DefaultSyncObjectProcessor<UnavailableSyncCryptoProvider> {
    fn default() -> Self {
        Self::production(UnavailableSyncCryptoProvider)
    }
}

impl<P> DefaultSyncObjectProcessor<P> {
    pub fn production(provider: P) -> Self {
        Self {
            provider,
            allow_synthetic_snapshot: false,
            snapshot: None,
            assembler: SyncEnvelopeAssembler::new(),
        }
    }

    #[doc(hidden)]
    pub fn synthetic_for_tests(provider: P) -> Self {
        Self {
            provider,
            allow_synthetic_snapshot: true,
            snapshot: None,
            assembler: SyncEnvelopeAssembler::new(),
        }
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }

    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    pub fn cycle_snapshot(&self) -> Option<&SyncCryptoCycleSnapshot> {
        self.snapshot.as_ref()
    }

    fn snapshot_for(
        &self,
        domain_id: &str,
        phase: SyncCyclePhase,
    ) -> Result<&SyncCryptoCycleSnapshot, SyncOrchestrationError> {
        self.snapshot
            .as_ref()
            .filter(|snapshot| snapshot.domain_id == domain_id)
            .ok_or_else(|| {
                SyncOrchestrationError::new(
                    SyncOrchestrationErrorCode::BackendUnavailable,
                    phase,
                    false,
                )
            })
    }
}

impl<P: SyncCryptoProvider> SyncObjectProcessor for DefaultSyncObjectProcessor<P> {
    fn preflight(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError> {
        self.snapshot = None;
        let snapshot = self.provider.freeze_cycle(domain_id)?;
        if snapshot.domain_id != domain_id
            || (snapshot.kind == SyncCryptoSnapshotKind::SyntheticTest
                && !self.allow_synthetic_snapshot)
            || (snapshot.kind == SyncCryptoSnapshotKind::Production
                && self.allow_synthetic_snapshot)
        {
            return Err(invalid_preflight());
        }
        self.snapshot = Some(snapshot);
        Ok(())
    }

    fn verify_and_decrypt(
        &mut self,
        expected: &RemoteObjectVersion,
        downloaded: RemoteObjectPayload,
    ) -> Result<DecryptedSyncObject, SyncOrchestrationError> {
        if expected != &downloaded.object {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::Verify,
            ));
        }
        let remote = downloaded.object.clone();
        let snapshot = self.snapshot_for(&remote.domain_id, SyncCyclePhase::Verify)?;
        if remote.signature_schema_version != SIGNATURE_SCHEMA_VERSION {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::UnsupportedSchema,
                SyncCyclePhase::Verify,
            ));
        }
        let signer = snapshot
            .remote_signers
            .get(&(
                remote.owner_device_id.clone(),
                remote.signature_key_id.clone(),
            ))
            .ok_or_else(|| {
                orchestration_error(
                    SyncOrchestrationErrorCode::SignatureMismatch,
                    SyncCyclePhase::Verify,
                )
            })?;
        if signer.rejects(remote.change_sequence) {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::RevokedDevice,
                SyncCyclePhase::Verify,
            ));
        }
        let epoch_material = snapshot
            .epoch_materials
            .get(&remote.key_epoch)
            .ok_or_else(|| {
                orchestration_error(
                    SyncOrchestrationErrorCode::KeyEpochRejected,
                    SyncCyclePhase::Verify,
                )
            })?;
        if remote.key_id != epoch_material.object_key.key_id {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::Verify,
            ));
        }
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
        let envelope = envelope_from_remote(downloaded)?;
        let manifest = SignedSyncObjectManifest::new(&remote.domain_id, &envelope, signature)
            .map_err(|_| {
                orchestration_error(
                    SyncOrchestrationErrorCode::InvalidMetadata,
                    SyncCyclePhase::Verify,
                )
            })?;
        manifest
            .verify(&signer.public_key)
            .map_err(map_verify_error)?;
        let object_key_material = epoch_material
            .sync_master_key
            .derive_object_key(
                &epoch_material.object_key,
                envelope.object_type,
                &envelope.object_id,
            )
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
        if snapshot.object_type != snapshot.payload.object_type {
            return Err(orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::PrepareSignedOutbox,
            ));
        }
        let cycle = self.snapshot_for(&snapshot.domain_id, SyncCyclePhase::PrepareSignedOutbox)?;
        let write_material = cycle
            .epoch_materials
            .get(&cycle.current_write_epoch)
            .ok_or_else(invalid_prepare)?
            .clone();
        let signing_handle = cycle.signing_handle.clone();
        let local_public_key = cycle.local_signing_public_key.clone();
        let domain_id = cycle.domain_id.clone();
        let spec = SyncObjectAssemblySpec::new(
            snapshot.object_id,
            &signing_handle.device_id,
            write_material.object_key,
            version,
            snapshot.remote_base_version,
            prepared_at_ms,
        )
        .map_err(|_| invalid_prepare())?;
        let local_revision = snapshot.local_revision;
        let object = self
            .assembler
            .assemble_payload(snapshot.payload, spec, &write_material.sync_master_key)
            .map_err(|_| invalid_prepare())?;
        let placeholder = DeviceSignature::new_for_algorithm(
            signing_handle.signature_algorithm.clone(),
            &signing_handle.signing_key_id,
            &signing_handle.device_id,
            placeholder_signature(&signing_handle.signature_algorithm)?,
        )
        .map_err(|_| invalid_prepare())?;
        let unsigned = SignedSyncObjectManifest::new(&domain_id, &object.envelope, placeholder)
            .map_err(|_| invalid_prepare())?;
        let signature = self
            .provider
            .sign(&signing_handle, &unsigned.canonical_bytes())
            .map_err(map_sign_error)?;
        let manifest = SignedSyncObjectManifest::new(&domain_id, &object.envelope, signature)
            .map_err(|_| invalid_prepare())?;
        manifest.verify(&local_public_key).map_err(map_sign_error)?;
        Ok(PreparedSyncOutbox {
            domain_id,
            local_revision,
            object,
            manifest,
            attempt_count: 0,
        })
    }
}

fn envelope_from_remote(
    downloaded: RemoteObjectPayload,
) -> Result<EncryptedObjectEnvelope, SyncOrchestrationError> {
    let remote = downloaded.object;
    Ok(EncryptedObjectEnvelope {
        schema_version: ENVELOPE_SCHEMA_VERSION,
        object_id: remote.object_id,
        object_type: remote.object_type.to_crypto_object_type(),
        owner_device_id: remote.owner_device_id,
        key_id: remote.key_id,
        key_epoch: remote.key_epoch,
        algorithm: AlgorithmId::new(remote.algorithm).map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::UnsupportedAlgorithm,
                SyncCyclePhase::Verify,
            )
        })?,
        nonce: Nonce::new(remote.nonce).map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::Verify,
            )
        })?,
        version: remote.version,
        base_version: remote.base_version,
        encrypted_payload: downloaded.payload,
        ciphertext_hash: CiphertextHash::new(remote.ciphertext_hash).map_err(|_| {
            orchestration_error(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::Verify,
            )
        })?,
        created_at_ms: remote.client_created_at_ms,
        updated_at_ms: remote.client_updated_at_ms,
    })
}

fn placeholder_signature(
    algorithm: &SignatureAlgorithmId,
) -> Result<Vec<u8>, SyncOrchestrationError> {
    match algorithm.as_str() {
        SIGNATURE_ALGORITHM_ED25519_V1 => Ok(vec![1u8; ED25519_SIGNATURE_LEN]),
        SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1 => Ok(vec![1u8; P256_SIGNATURE_LEN]),
        _ => Err(orchestration_error(
            SyncOrchestrationErrorCode::UnsupportedAlgorithm,
            SyncCyclePhase::PrepareSignedOutbox,
        )),
    }
}

fn map_verify_error(error: CryptoError) -> SyncOrchestrationError {
    let code = match error {
        CryptoError::SignatureKeyNotActive { .. } => SyncOrchestrationErrorCode::RevokedDevice,
        CryptoError::UnsupportedSignatureAlgorithm { .. } => {
            SyncOrchestrationErrorCode::UnsupportedAlgorithm
        }
        _ => SyncOrchestrationErrorCode::SignatureMismatch,
    };
    orchestration_error(code, SyncCyclePhase::Verify)
}

fn map_decrypt_error(error: CryptoError) -> SyncOrchestrationError {
    let code = match error {
        CryptoError::CiphertextHashMismatch => SyncOrchestrationErrorCode::CiphertextHashMismatch,
        CryptoError::AssociatedDataMismatch { .. } => SyncOrchestrationErrorCode::AadMismatch,
        _ => SyncOrchestrationErrorCode::DecryptFailed,
    };
    orchestration_error(code, SyncCyclePhase::DecryptAndDecode)
}

fn map_sign_error(error: CryptoError) -> SyncOrchestrationError {
    let code = match error {
        CryptoError::PrivateKeyRevoked { .. } => SyncOrchestrationErrorCode::RevokedDevice,
        CryptoError::UnsupportedSignatureAlgorithm { .. } => {
            SyncOrchestrationErrorCode::UnsupportedAlgorithm
        }
        _ => SyncOrchestrationErrorCode::BackendUnavailable,
    };
    orchestration_error(code, SyncCyclePhase::PrepareSignedOutbox)
}

fn invalid_preflight() -> SyncOrchestrationError {
    orchestration_error(
        SyncOrchestrationErrorCode::InvalidMetadata,
        SyncCyclePhase::Preflight,
    )
}

fn backend_preflight() -> SyncOrchestrationError {
    orchestration_error(
        SyncOrchestrationErrorCode::BackendUnavailable,
        SyncCyclePhase::Preflight,
    )
}

fn invalid_prepare() -> SyncOrchestrationError {
    orchestration_error(
        SyncOrchestrationErrorCode::InvalidMetadata,
        SyncCyclePhase::PrepareSignedOutbox,
    )
}

fn orchestration_error(
    code: SyncOrchestrationErrorCode,
    phase: SyncCyclePhase,
) -> SyncOrchestrationError {
    SyncOrchestrationError::new(code, phase, false)
}

#[cfg(test)]
mod tests {
    use radishlex_ime_crypto::TestMemoryDeviceKeyStore;

    use super::*;
    use crate::{PlaintextSyncPayload, SyncObjectType};

    const DOMAIN_ID: &str = "domain-processor-test";
    const DEVICE_ID: &str = "device-processor-test";
    const SIGNING_KEY_ID: &str = "signing-key-processor-test";
    const CREATED_AT_MS: i64 = 1_735_689_600_000;

    struct SyntheticProvider {
        store: TestMemoryDeviceKeyStore,
        local_public_key: DeviceSigningPublicKey,
        current_write_epoch: u64,
        epoch_materials: BTreeMap<u64, SyncEpochKeyMaterial>,
        signer_reject_from_sequence: Option<u64>,
        freeze_count: usize,
    }

    impl SyntheticProvider {
        fn new(current_write_epoch: u64, accepted_epochs: impl IntoIterator<Item = u64>) -> Self {
            let mut store = TestMemoryDeviceKeyStore::new();
            let local_public_key = store
                .insert_signing_key(DEVICE_ID, SIGNING_KEY_ID, [17u8; 32], CREATED_AT_MS)
                .expect("synthetic signing key");
            let epoch_materials = accepted_epochs
                .into_iter()
                .map(|epoch| (epoch, epoch_material(epoch)))
                .collect();
            Self {
                store,
                local_public_key,
                current_write_epoch,
                epoch_materials,
                signer_reject_from_sequence: None,
                freeze_count: 0,
            }
        }

        fn rotate_to(&mut self, epoch: u64) {
            self.epoch_materials.insert(epoch, epoch_material(epoch));
            self.current_write_epoch = epoch;
        }

        fn reject_signer_from(&mut self, change_sequence: u64) {
            self.signer_reject_from_sequence = Some(change_sequence);
        }
    }

    impl SyncCryptoProvider for SyntheticProvider {
        fn freeze_cycle(
            &mut self,
            domain_id: &str,
        ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError> {
            self.freeze_count += 1;
            let handle = self
                .store
                .handle(DEVICE_ID, SIGNING_KEY_ID)
                .map_err(|_| backend_preflight())?;
            let signer = match self.signer_reject_from_sequence {
                Some(change_sequence) => SyncRemoteSigningProfile::revoked_from_change_sequence(
                    self.local_public_key.clone(),
                    change_sequence,
                )?,
                None => SyncRemoteSigningProfile::active(self.local_public_key.clone())?,
            };
            SyncCryptoCycleSnapshot::synthetic_for_tests(
                domain_id,
                handle,
                self.local_public_key.clone(),
                self.store.backend_status(),
                self.current_write_epoch,
                self.epoch_materials.values().cloned(),
                [signer],
            )
        }

        fn sign(
            &self,
            handle: &DeviceSigningKeyHandle,
            canonical_bytes: &[u8],
        ) -> Result<DeviceSignature, CryptoError> {
            self.store.sign(handle, canonical_bytes)
        }
    }

    #[test]
    fn default_processor_is_closed_before_any_network_work() {
        let mut processor = DefaultSyncObjectProcessor::default();

        let error = processor
            .preflight(DOMAIN_ID)
            .expect_err("missing product provider must block the cycle");

        assert_eq!(error.code, SyncOrchestrationErrorCode::BackendUnavailable);
        assert_eq!(error.phase, SyncCyclePhase::Preflight);
        assert!(processor.cycle_snapshot().is_none());
    }

    #[test]
    fn synthetic_snapshot_requires_the_explicit_test_constructor() {
        let mut product_processor =
            DefaultSyncObjectProcessor::production(SyntheticProvider::new(1, [1]));
        let error = product_processor
            .preflight(DOMAIN_ID)
            .expect_err("test memory backend must not enter the product route");
        assert_eq!(error.code, SyncOrchestrationErrorCode::InvalidMetadata);
        assert_eq!(error.phase, SyncCyclePhase::Preflight);

        let mut test_processor =
            DefaultSyncObjectProcessor::synthetic_for_tests(SyntheticProvider::new(1, [1]));
        test_processor
            .preflight(DOMAIN_ID)
            .expect("explicit synthetic test route");
    }

    #[test]
    fn historical_epoch_is_readable_until_retired_and_revocation_threshold_is_reached() {
        let mut writer =
            DefaultSyncObjectProcessor::synthetic_for_tests(SyntheticProvider::new(1, [1, 2]));
        writer.preflight(DOMAIN_ID).expect("writer preflight");
        let payload = remote_payload(
            writer
                .prepare_outbox(local_snapshot("object-historical"), 1, CREATED_AT_MS + 10)
                .expect("epoch one outbox"),
            4,
        );

        let mut provider = SyntheticProvider::new(2, [1, 2]);
        provider.reject_signer_from(5);
        let mut reader = DefaultSyncObjectProcessor::synthetic_for_tests(provider);
        reader.preflight(DOMAIN_ID).expect("reader preflight");
        let expected = payload.object.clone();
        let decrypted = reader
            .verify_and_decrypt(&expected, payload.clone())
            .expect("historical epoch before revocation threshold remains readable");
        assert_eq!(decrypted.plaintext_payload, br#"{"synthetic":true}"#);

        let mut revoked_payload = payload.clone();
        revoked_payload.object.change_sequence = 5;
        let expected = revoked_payload.object.clone();
        let error = reader
            .verify_and_decrypt(&expected, revoked_payload)
            .expect_err("revocation threshold must reject the signer");
        assert_eq!(error.code, SyncOrchestrationErrorCode::RevokedDevice);
        assert_eq!(error.phase, SyncCyclePhase::Verify);

        let mut retired_reader =
            DefaultSyncObjectProcessor::synthetic_for_tests(SyntheticProvider::new(2, [2]));
        retired_reader
            .preflight(DOMAIN_ID)
            .expect("retired epoch reader preflight");
        let expected = payload.object.clone();
        let error = retired_reader
            .verify_and_decrypt(&expected, payload)
            .expect_err("retired historical epoch must be rejected");
        assert_eq!(error.code, SyncOrchestrationErrorCode::KeyEpochRejected);
        assert_eq!(error.phase, SyncCyclePhase::Verify);
    }

    #[test]
    fn cycle_snapshot_keeps_write_epoch_frozen_until_the_next_preflight() {
        let mut processor =
            DefaultSyncObjectProcessor::synthetic_for_tests(SyntheticProvider::new(2, [1, 2]));
        processor.preflight(DOMAIN_ID).expect("first preflight");
        assert_eq!(processor.provider().freeze_count, 1);

        processor.provider_mut().rotate_to(3);
        let first_outbox = processor
            .prepare_outbox(
                local_snapshot("object-before-refreeze"),
                1,
                CREATED_AT_MS + 20,
            )
            .expect("outbox from frozen epoch");
        assert_eq!(first_outbox.object.draft.key_epoch, 2);
        assert_eq!(first_outbox.object.draft.key_id, "object-key-epoch-2");

        processor.preflight(DOMAIN_ID).expect("second preflight");
        assert_eq!(processor.provider().freeze_count, 2);
        assert_eq!(
            processor
                .cycle_snapshot()
                .expect("second cycle snapshot")
                .current_write_epoch(),
            3
        );
        let second_outbox = processor
            .prepare_outbox(
                local_snapshot("object-after-refreeze"),
                1,
                CREATED_AT_MS + 30,
            )
            .expect("outbox from rotated epoch");
        assert_eq!(second_outbox.object.draft.key_epoch, 3);
        assert_eq!(second_outbox.object.draft.key_id, "object-key-epoch-3");
    }

    fn epoch_material(epoch: u64) -> SyncEpochKeyMaterial {
        let descriptor = KeyDescriptor::new(
            format!("object-key-epoch-{epoch}"),
            KeyRole::ObjectKey,
            epoch,
        )
        .expect("object key descriptor");
        let key_byte = u8::try_from(epoch).expect("test epoch fits in one byte");
        let master_key = SyncMasterKeyMaterial::new([key_byte; 32]).expect("sync master key");
        SyncEpochKeyMaterial::new(descriptor, master_key).expect("epoch key material")
    }

    fn local_snapshot(object_id: &str) -> LocalSyncSnapshot {
        LocalSyncSnapshot {
            domain_id: DOMAIN_ID.to_owned(),
            object_id: object_id.to_owned(),
            object_type: SyncObjectType::DictionaryUserTerms,
            local_revision: 1,
            remote_base_version: None,
            payload: PlaintextSyncPayload::new(
                SyncObjectType::DictionaryUserTerms,
                1,
                br#"{"synthetic":true}"#.to_vec(),
            )
            .expect("plaintext payload"),
        }
    }

    fn remote_payload(outbox: PreparedSyncOutbox, change_sequence: u64) -> RemoteObjectPayload {
        let draft = &outbox.object.draft;
        let signature = &outbox.manifest.signature;
        RemoteObjectPayload {
            object: RemoteObjectVersion {
                domain_id: outbox.domain_id,
                object_id: draft.object_id.clone(),
                object_type: draft.object_type,
                version: draft.version,
                base_version: draft.base_version,
                change_sequence,
                owner_device_id: draft.owner_device_id.clone(),
                key_id: draft.key_id.clone(),
                key_epoch: draft.key_epoch,
                algorithm: draft.algorithm.clone(),
                nonce: draft.nonce.clone(),
                encrypted_payload_len: draft.encrypted_payload_len,
                ciphertext_hash: draft.ciphertext_hash.clone(),
                signature_schema_version: signature.signature_schema_version,
                signature_algorithm: signature.signature_algorithm.as_str().to_owned(),
                signature_key_id: signature.signature_key_id.clone(),
                signature: signature.signature.clone(),
                server_received_at_ms: CREATED_AT_MS + 100,
                client_created_at_ms: draft.created_at_ms,
                client_updated_at_ms: draft.updated_at_ms,
            },
            payload: outbox.object.envelope.encrypted_payload,
        }
    }
}
