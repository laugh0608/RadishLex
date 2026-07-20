use std::collections::{BTreeMap, BTreeSet};

use radishlex_ime_crypto::{
    CryptoError, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey, KeyDescriptor,
    KeyRole, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
};
use radishlex_ime_sync::{
    DecryptedSyncObject, DefaultSyncObjectProcessor, LocalSyncSnapshot, PreparedSyncOutbox,
    RemoteObjectPayload, RemoteObjectVersion, SyncCryptoCycleSnapshot, SyncCryptoProvider,
    SyncEpochKeyMaterial, SyncObjectProcessor, SyncOrchestrationError, SyncRemoteSigningProfile,
};

use super::{BASE_TIMESTAMP_MS, DEVICE_A, DEVICE_B, OBJECT_KEY_ID, SIGNING_KEY_A, SIGNING_KEY_B};

pub(super) struct TestCryptoProcessor {
    inner: DefaultSyncObjectProcessor<TestCryptoProvider>,
}

impl TestCryptoProcessor {
    pub(super) fn new(device_id: &'static str, signing_key_id: &'static str) -> Self {
        Self {
            inner: DefaultSyncObjectProcessor::synthetic_for_tests(TestCryptoProvider::new(
                device_id,
                signing_key_id,
            )),
        }
    }

    pub(super) fn signing_store(&self) -> &TestMemoryDeviceKeyStore {
        &self.inner.provider().signing_store
    }

    pub(super) fn replace_accepted_key_epochs(&mut self, epochs: impl IntoIterator<Item = u64>) {
        let accepted_key_epochs: BTreeSet<_> = epochs.into_iter().collect();
        self.inner.provider_mut().current_write_epoch = accepted_key_epochs
            .iter()
            .next_back()
            .copied()
            .expect("test provider requires at least one accepted epoch");
        self.inner.provider_mut().accepted_key_epochs = accepted_key_epochs;
    }

    pub(super) fn revoke_device(&mut self, device_id: impl Into<String>) {
        self.inner
            .provider_mut()
            .revoked_devices
            .insert(device_id.into(), 1);
    }
}

impl SyncObjectProcessor for TestCryptoProcessor {
    fn preflight(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError> {
        self.inner.preflight(domain_id)
    }

    fn verify_and_decrypt(
        &mut self,
        expected: &RemoteObjectVersion,
        downloaded: RemoteObjectPayload,
    ) -> Result<DecryptedSyncObject, SyncOrchestrationError> {
        self.inner.verify_and_decrypt(expected, downloaded)
    }

    fn prepare_outbox(
        &mut self,
        snapshot: LocalSyncSnapshot,
        version: u64,
        prepared_at_ms: i64,
    ) -> Result<PreparedSyncOutbox, SyncOrchestrationError> {
        self.inner.prepare_outbox(snapshot, version, prepared_at_ms)
    }
}

struct TestCryptoProvider {
    device_id: &'static str,
    signing_key_id: &'static str,
    signing_store: TestMemoryDeviceKeyStore,
    public_keys: BTreeMap<String, DeviceSigningPublicKey>,
    current_write_epoch: u64,
    accepted_key_epochs: BTreeSet<u64>,
    revoked_devices: BTreeMap<String, u64>,
}

impl TestCryptoProvider {
    fn new(device_id: &'static str, signing_key_id: &'static str) -> Self {
        let (signing_store, public_key_a, public_key_b) = test_signing_material();
        Self {
            device_id,
            signing_key_id,
            signing_store,
            public_keys: BTreeMap::from([
                (DEVICE_A.to_owned(), public_key_a),
                (DEVICE_B.to_owned(), public_key_b),
            ]),
            current_write_epoch: 1,
            accepted_key_epochs: BTreeSet::from([1]),
            revoked_devices: BTreeMap::new(),
        }
    }
}

impl SyncCryptoProvider for TestCryptoProvider {
    fn freeze_cycle(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError> {
        let handle = self
            .signing_store
            .handle(self.device_id, self.signing_key_id)
            .map_err(|_| backend_unavailable())?;
        let local_public_key = self
            .public_keys
            .get(self.device_id)
            .filter(|public_key| public_key.signing_key_id == self.signing_key_id)
            .cloned()
            .ok_or_else(backend_unavailable)?;
        let epoch_materials = self
            .accepted_key_epochs
            .iter()
            .copied()
            .map(test_epoch_material)
            .collect::<Result<Vec<_>, _>>()?;
        let remote_signers = self
            .public_keys
            .values()
            .cloned()
            .map(
                |public_key| match self.revoked_devices.get(&public_key.device_id).copied() {
                    Some(first_rejected_sequence) => {
                        SyncRemoteSigningProfile::revoked_from_change_sequence(
                            public_key,
                            first_rejected_sequence,
                        )
                    }
                    None => SyncRemoteSigningProfile::active(public_key),
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        SyncCryptoCycleSnapshot::synthetic_for_tests(
            domain_id,
            handle,
            local_public_key,
            self.signing_store.backend_status(),
            self.current_write_epoch,
            epoch_materials,
            remote_signers,
        )
    }

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        self.signing_store.sign(handle, canonical_bytes)
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

fn test_epoch_material(epoch: u64) -> Result<SyncEpochKeyMaterial, SyncOrchestrationError> {
    let object_key = KeyDescriptor::new(OBJECT_KEY_ID, KeyRole::ObjectKey, epoch)
        .map_err(|_| invalid_metadata())?;
    let sync_master_key = SyncMasterKeyMaterial::new([11u8; 32]).map_err(|_| invalid_metadata())?;
    SyncEpochKeyMaterial::new(object_key, sync_master_key)
}

fn backend_unavailable() -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        radishlex_ime_sync::SyncOrchestrationErrorCode::BackendUnavailable,
        radishlex_ime_sync::SyncCyclePhase::Preflight,
        false,
    )
}

fn invalid_metadata() -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        radishlex_ime_sync::SyncOrchestrationErrorCode::InvalidMetadata,
        radishlex_ime_sync::SyncCyclePhase::Preflight,
        false,
    )
}
