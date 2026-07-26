use std::collections::BTreeMap;

use radishlex_ime_crypto::{
    CryptoError, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey, KeyDescriptor,
    KeyRole, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
};
use radishlex_ime_sync::{
    SyncCryptoCycleSnapshot, SyncCryptoProvider, SyncEpochKeyMaterial, SyncOrchestrationError,
    SyncOrchestrationErrorCode, SyncRemoteSigningProfile,
};

use crate::remote_setup::QualificationIds;

pub(crate) struct QualificationCryptoProvider {
    local_device_id: String,
    local_signing_key_id: String,
    object_key_id: String,
    signing_store: TestMemoryDeviceKeyStore,
    public_keys: BTreeMap<String, DeviceSigningPublicKey>,
    master_key_byte: u8,
}

impl QualificationCryptoProvider {
    pub(crate) fn new(
        ids: &QualificationIds,
        client_a: bool,
    ) -> Result<Self, crate::QualificationError> {
        let (local_device_id, local_signing_key_id) = if client_a {
            (&ids.device_a_id, &ids.signing_key_a_id)
        } else {
            (&ids.device_b_id, &ids.signing_key_b_id)
        };
        let mut signing_store = TestMemoryDeviceKeyStore::new();
        let public_key_a = signing_store
            .insert_signing_key(
                &ids.device_a_id,
                &ids.signing_key_a_id,
                [41u8; 32],
                ids.created_at_ms,
            )
            .map_err(|_| crypto_error())?;
        let public_key_b = signing_store
            .insert_signing_key(
                &ids.device_b_id,
                &ids.signing_key_b_id,
                [42u8; 32],
                ids.created_at_ms,
            )
            .map_err(|_| crypto_error())?;
        Ok(Self {
            local_device_id: local_device_id.to_owned(),
            local_signing_key_id: local_signing_key_id.to_owned(),
            object_key_id: ids.object_key_id.clone(),
            signing_store,
            public_keys: BTreeMap::from([
                (ids.device_a_id.clone(), public_key_a),
                (ids.device_b_id.clone(), public_key_b),
            ]),
            master_key_byte: ids.master_key_byte,
        })
    }
}

impl SyncCryptoProvider for QualificationCryptoProvider {
    fn freeze_cycle(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError> {
        let handle = self
            .signing_store
            .handle(&self.local_device_id, &self.local_signing_key_id)
            .map_err(|_| backend_unavailable())?;
        let local_public_key = self
            .public_keys
            .get(&self.local_device_id)
            .cloned()
            .ok_or_else(backend_unavailable)?;
        let object_key = KeyDescriptor::new(&self.object_key_id, KeyRole::ObjectKey, 1)
            .map_err(|_| invalid_metadata())?;
        let master_key = SyncMasterKeyMaterial::new([self.master_key_byte; 32])
            .map_err(|_| invalid_metadata())?;
        let epoch_material =
            SyncEpochKeyMaterial::new(object_key, master_key).map_err(|_| invalid_metadata())?;
        let remote_signers = self
            .public_keys
            .values()
            .cloned()
            .map(SyncRemoteSigningProfile::active)
            .collect::<Result<Vec<_>, _>>()?;
        SyncCryptoCycleSnapshot::qualification(
            domain_id,
            handle,
            local_public_key,
            self.signing_store.backend_status(),
            1,
            [epoch_material],
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

pub(crate) struct QualificationSigningMaterial {
    pub(crate) store: TestMemoryDeviceKeyStore,
    pub(crate) public_key_a: DeviceSigningPublicKey,
    pub(crate) public_key_b: DeviceSigningPublicKey,
}

impl QualificationSigningMaterial {
    pub(crate) fn new(
        device_a_id: &str,
        signing_key_a_id: &str,
        device_b_id: &str,
        signing_key_b_id: &str,
        created_at_ms: i64,
    ) -> Result<Self, crate::QualificationError> {
        let mut store = TestMemoryDeviceKeyStore::new();
        let public_key_a = store
            .insert_signing_key(device_a_id, signing_key_a_id, [41u8; 32], created_at_ms)
            .map_err(|_| crypto_error())?;
        let public_key_b = store
            .insert_signing_key(device_b_id, signing_key_b_id, [42u8; 32], created_at_ms)
            .map_err(|_| crypto_error())?;
        Ok(Self {
            store,
            public_key_a,
            public_key_b,
        })
    }
}

fn crypto_error() -> crate::QualificationError {
    crate::QualificationError::new(
        crate::QualificationErrorCode::CryptoRejected,
        crate::QualificationPhase::PrepareWorkspace,
        false,
    )
}

fn backend_unavailable() -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        SyncOrchestrationErrorCode::BackendUnavailable,
        radishlex_ime_sync::SyncCyclePhase::Preflight,
        false,
    )
}

fn invalid_metadata() -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        SyncOrchestrationErrorCode::InvalidMetadata,
        radishlex_ime_sync::SyncCyclePhase::Preflight,
        false,
    )
}
