use std::cell::Cell;
use std::rc::Rc;

use p256::{ecdh::diffie_hellman, elliptic_curve::sec1::ToEncodedPoint, PublicKey, SecretKey};
use radishlex_ime_crypto::{
    DeviceKeyAgreementKeyHandle, DeviceKeyAgreementPublicKey, DeviceSigningBackendCapabilities,
    DeviceSigningStorageBackend, DeviceWrappingRecord, EcdhSharedSecret, KeyDescriptor, KeyRole,
    Nonce, SignatureAlgorithmId, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
    WrappedEpochMaterial,
};

use super::*;
use crate::{
    DefaultSyncObjectProcessor, DeviceAuthorizationPackage, DeviceRevocationReason,
    DeviceRevocationRecord, LocalSyncSnapshot, PlaintextSyncPayload, PreparedSyncOutbox,
    RemoteObjectPayload, RemoteObjectVersion, SyncObjectProcessor, SyncObjectType,
};

const DOMAIN_ID: &str = "domain-product-provider-test";
const DEVICE_A: &str = "device-product-a";
const DEVICE_B: &str = "device-product-b";
const SIGNING_KEY_A: &str = "signing-product-a";
const SIGNING_KEY_B: &str = "signing-product-b";
const OBJECT_KEY_1: &str = "object-key-product-1";
const OBJECT_KEY_2: &str = "object-key-product-2";
const CREATED_AT_MS: i64 = 1_735_689_600_000;
const REVOKED_AT_MS: i64 = CREATED_AT_MS + 100;
const REJECT_FROM_SEQUENCE: u64 = 5;

#[derive(Clone)]
struct StaticTrustedDeviceSource {
    state: SyncTrustedDomainState,
    load_count: Rc<Cell<usize>>,
}

impl StaticTrustedDeviceSource {
    fn new(state: SyncTrustedDomainState) -> (Self, Rc<Cell<usize>>) {
        let load_count = Rc::new(Cell::new(0));
        (
            Self {
                state,
                load_count: Rc::clone(&load_count),
            },
            load_count,
        )
    }
}

impl SyncTrustedDeviceSource for StaticTrustedDeviceSource {
    fn load_trusted_domain(
        &mut self,
        _domain_id: &str,
    ) -> Result<SyncTrustedDomainState, SyncCryptoLoadError> {
        self.load_count.set(self.load_count.get() + 1);
        Ok(self.state.clone())
    }
}

#[derive(Clone)]
struct StaticEpochMaterialStore {
    materials: Vec<SyncEpochKeyMaterial>,
    load_count: Rc<Cell<usize>>,
}

#[derive(Clone)]
struct MemoryWrappedEpochSource {
    records: Vec<WrappedEpochMaterial>,
    loads: Rc<Cell<usize>>,
}

impl MemoryWrappedEpochSource {
    fn new(records: Vec<WrappedEpochMaterial>) -> (Self, Rc<Cell<usize>>) {
        let loads = Rc::new(Cell::new(0));
        (
            Self {
                records,
                loads: Rc::clone(&loads),
            },
            loads,
        )
    }
}

impl SyncWrappedEpochMaterialSource for MemoryWrappedEpochSource {
    fn load_wrapped_epoch_materials(
        &mut self,
        _domain_id: &str,
        _local_device_id: &str,
    ) -> Result<Vec<WrappedEpochMaterial>, SyncCryptoLoadError> {
        self.loads.set(self.loads.get() + 1);
        Ok(self.records.clone())
    }
}

#[derive(Clone, Copy)]
enum AgreementFailure {
    None,
    Locked,
    Unavailable,
}

struct MemoryP256AgreementBackend {
    device_id: String,
    key_id: String,
    secret: SecretKey,
    public_key: DeviceKeyAgreementPublicKey,
    failure: AgreementFailure,
}

impl MemoryP256AgreementBackend {
    fn new(device_id: &str, key_id: &str, scalar: u8) -> Self {
        let mut secret_bytes = [0u8; 32];
        secret_bytes[31] = scalar;
        let secret = SecretKey::from_slice(&secret_bytes).expect("agreement secret");
        let public_bytes = secret.public_key().to_encoded_point(false);
        let public_key = DeviceKeyAgreementPublicKey::p256(
            device_id,
            key_id,
            public_bytes.as_bytes(),
            CREATED_AT_MS,
            None,
        )
        .expect("agreement public key");
        Self {
            device_id: device_id.to_owned(),
            key_id: key_id.to_owned(),
            secret,
            public_key,
            failure: AgreementFailure::None,
        }
    }

    fn with_failure(mut self, failure: AgreementFailure) -> Self {
        self.failure = failure;
        self
    }
}

impl SyncDeviceKeyAgreementBackend for MemoryP256AgreementBackend {
    fn key_handle(
        &self,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        match self.failure {
            AgreementFailure::Locked => {
                return Err(CryptoError::PrivateKeyLocked {
                    key_id: key_id.to_owned(),
                })
            }
            AgreementFailure::Unavailable => {
                return Err(CryptoError::StorageBackendUnavailable {
                    backend: "synthetic-agreement".to_owned(),
                })
            }
            AgreementFailure::None => {}
        }
        if device_id != self.device_id || key_id != self.key_id {
            return Err(CryptoError::PrivateKeyUnavailable {
                key_id: key_id.to_owned(),
            });
        }
        DeviceKeyAgreementKeyHandle::p256(device_id, key_id, "synthetic-agreement")
    }

    fn public_key(
        &self,
        _handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        Ok(self.public_key.clone())
    }

    fn derive_shared_secret(
        &self,
        _handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        let peer = PublicKey::from_sec1_bytes(peer_public_key).map_err(|_| {
            CryptoError::PrivateKeyCorrupted {
                key_id: self.key_id.clone(),
            }
        })?;
        let shared = diffie_hellman(self.secret.to_nonzero_scalar(), peer.as_affine());
        EcdhSharedSecret::new((*shared.raw_secret_bytes()).into())
    }
}

impl StaticEpochMaterialStore {
    fn new(materials: Vec<SyncEpochKeyMaterial>) -> (Self, Rc<Cell<usize>>) {
        let load_count = Rc::new(Cell::new(0));
        (
            Self {
                materials,
                load_count: Rc::clone(&load_count),
            },
            load_count,
        )
    }
}

impl SyncEpochMaterialStore for StaticEpochMaterialStore {
    fn load_epoch_materials(
        &mut self,
        _domain: &SyncDomain,
        _local_device: &SyncTrustedDeviceProfile,
    ) -> Result<Vec<SyncEpochKeyMaterial>, SyncCryptoLoadError> {
        self.load_count.set(self.load_count.get() + 1);
        Ok(self.materials.clone())
    }
}

struct SyntheticQualifiedSigningBackend {
    signing_store: TestMemoryDeviceKeyStore,
    public_key: DeviceSigningPublicKey,
    handle_backend: DeviceSigningStorageBackend,
}

impl SyntheticQualifiedSigningBackend {
    fn new(device_id: &str, signing_key_id: &str, seed: [u8; 32]) -> Self {
        let mut signing_store = TestMemoryDeviceKeyStore::new();
        let public_key = signing_store
            .insert_signing_key(device_id, signing_key_id, seed, CREATED_AT_MS)
            .expect("synthetic platform signing key");
        Self {
            signing_store,
            public_key,
            handle_backend: DeviceSigningStorageBackend::LinuxSecretServiceV1,
        }
    }

    fn with_handle_backend(mut self, handle_backend: DeviceSigningStorageBackend) -> Self {
        self.handle_backend = handle_backend;
        self
    }
}

impl SyncDeviceSigningBackend for SyntheticQualifiedSigningBackend {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::platform(
            DeviceSigningStorageBackend::LinuxSecretServiceV1,
            false,
            false,
            false,
        )
        .expect("synthetic qualified backend status")
    }

    fn signing_handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        DeviceSigningKeyHandle::new(
            device_id,
            signing_key_id,
            SignatureAlgorithmId::ed25519_v1(),
            self.handle_backend,
            DeviceSigningBackendCapabilities::platform(self.handle_backend, false, false, false)?,
            CREATED_AT_MS,
        )
    }

    fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        if handle.device_id != self.public_key.device_id
            || handle.signing_key_id != self.public_key.signing_key_id
        {
            return Err(CryptoError::PrivateKeyUnavailable {
                key_id: handle.signing_key_id.clone(),
            });
        }
        Ok(self.public_key.clone())
    }

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        let test_handle = self
            .signing_store
            .handle(&handle.device_id, &handle.signing_key_id)?;
        self.signing_store.sign(&test_handle, canonical_bytes)
    }
}

struct TestMemorySigningBackend {
    signing_store: TestMemoryDeviceKeyStore,
}

impl TestMemorySigningBackend {
    fn new(device_id: &str, signing_key_id: &str, seed: [u8; 32]) -> Self {
        let mut signing_store = TestMemoryDeviceKeyStore::new();
        signing_store
            .insert_signing_key(device_id, signing_key_id, seed, CREATED_AT_MS)
            .expect("test memory signing key");
        Self { signing_store }
    }
}

impl SyncDeviceSigningBackend for TestMemorySigningBackend {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        self.signing_store.backend_status()
    }

    fn signing_handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        self.signing_store.handle(device_id, signing_key_id)
    }

    fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        self.signing_store.public_key(handle)
    }

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        self.signing_store.sign(handle, canonical_bytes)
    }
}

#[test]
fn product_provider_rejects_test_backend_before_loading_epoch_material() {
    let state = active_domain_state(1);
    let (trusted_devices, lifecycle_loads) = StaticTrustedDeviceSource::new(state);
    let (epoch_materials, material_loads) = StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let backend = TestMemorySigningBackend::new(DEVICE_A, SIGNING_KEY_A, [31u8; 32]);
    let mut provider =
        ProductSyncCryptoProvider::new(DEVICE_A, trusted_devices, epoch_materials, backend)
            .expect("product provider");

    let error = provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("test backend must not enter the product path");

    assert_eq!(error.code, SyncOrchestrationErrorCode::BackendUnavailable);
    assert_eq!(error.phase, SyncCyclePhase::Preflight);
    assert_eq!(lifecycle_loads.get(), 1);
    assert_eq!(material_loads.get(), 0);
}

#[test]
fn revoked_local_device_is_blocked_before_backend_or_epoch_material_access() {
    let state = rotated_domain_state();
    let (trusted_devices, lifecycle_loads) = StaticTrustedDeviceSource::new(state);
    let (epoch_materials, material_loads) = StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let backend = SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [31u8; 32]);
    let mut provider =
        ProductSyncCryptoProvider::new(DEVICE_A, trusted_devices, epoch_materials, backend)
            .expect("product provider");

    let error = provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("revoked local device must be blocked");

    assert_eq!(error.code, SyncOrchestrationErrorCode::RevokedDevice);
    assert_eq!(error.phase, SyncCyclePhase::Preflight);
    assert_eq!(lifecycle_loads.get(), 1);
    assert_eq!(material_loads.get(), 0);
}

#[test]
fn missing_local_profile_and_backend_key_mismatch_fail_before_epoch_material_access() {
    let domain = SyncDomain::new(
        DOMAIN_ID,
        1,
        OBJECT_KEY_1,
        CREATED_AT_MS,
        CREATED_AT_MS + 10,
    )
    .expect("domain");
    let missing_local_state = SyncTrustedDomainState::new(
        domain,
        [SyncTrustedDeviceProfile::active(
            active_device(DEVICE_B, SIGNING_KEY_B),
            public_key(DEVICE_B, SIGNING_KEY_B, [32u8; 32]),
        )
        .expect("device b profile")],
    )
    .expect("trusted state without local device");
    let (trusted_devices, _) = StaticTrustedDeviceSource::new(missing_local_state);
    let (epoch_materials, missing_profile_material_loads) =
        StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let backend = SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [31u8; 32]);
    let mut missing_profile_provider =
        ProductSyncCryptoProvider::new(DEVICE_A, trusted_devices, epoch_materials, backend)
            .expect("provider without local profile");
    let error = missing_profile_provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("missing local profile must block the cycle");
    assert_eq!(error.code, SyncOrchestrationErrorCode::InvalidMetadata);
    assert_eq!(missing_profile_material_loads.get(), 0);

    let (trusted_devices, _) = StaticTrustedDeviceSource::new(active_domain_state(1));
    let (epoch_materials, mismatched_key_material_loads) =
        StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let mismatched_backend =
        SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [33u8; 32]);
    let mut mismatched_key_provider = ProductSyncCryptoProvider::new(
        DEVICE_A,
        trusted_devices,
        epoch_materials,
        mismatched_backend,
    )
    .expect("provider with mismatched backend key");
    let error = mismatched_key_provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("backend key mismatch must block the cycle");
    assert_eq!(error.code, SyncOrchestrationErrorCode::InvalidMetadata);
    assert_eq!(mismatched_key_material_loads.get(), 0);

    let (trusted_devices, _) = StaticTrustedDeviceSource::new(active_domain_state(1));
    let (epoch_materials, mismatched_handle_material_loads) =
        StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let mismatched_handle_backend =
        SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [31u8; 32])
            .with_handle_backend(DeviceSigningStorageBackend::WindowsCngV1);
    let mut mismatched_handle_provider = ProductSyncCryptoProvider::new(
        DEVICE_A,
        trusted_devices,
        epoch_materials,
        mismatched_handle_backend,
    )
    .expect("provider with mismatched handle backend");
    let error = mismatched_handle_provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("handle capability mismatch must block the cycle");
    assert_eq!(error.code, SyncOrchestrationErrorCode::InvalidMetadata);
    assert_eq!(mismatched_handle_material_loads.get(), 0);
}

#[test]
fn missing_current_epoch_material_blocks_the_cycle_without_fallback() {
    let state = rotated_domain_state();
    let (trusted_devices, _) = StaticTrustedDeviceSource::new(state);
    let (epoch_materials, material_loads) = StaticEpochMaterialStore::new(vec![epoch_material(1)]);
    let backend = SyntheticQualifiedSigningBackend::new(DEVICE_B, SIGNING_KEY_B, [32u8; 32]);
    let mut provider =
        ProductSyncCryptoProvider::new(DEVICE_B, trusted_devices, epoch_materials, backend)
            .expect("product provider");

    let error = provider
        .freeze_cycle(DOMAIN_ID)
        .expect_err("missing current epoch must block the cycle");

    assert_eq!(error.code, SyncOrchestrationErrorCode::KeyEpochRejected);
    assert_eq!(error.phase, SyncCyclePhase::Preflight);
    assert_eq!(material_loads.get(), 1);
}

#[test]
fn lifecycle_rotation_preserves_history_rejects_new_revoked_objects_and_rebuilds_after_restart() {
    let writer_provider = product_provider(
        DEVICE_A,
        active_domain_state(1),
        vec![epoch_material(1)],
        SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [31u8; 32]),
    );
    let mut writer = DefaultSyncObjectProcessor::production(writer_provider);
    writer.preflight(DOMAIN_ID).expect("writer preflight");
    let historical_payload = remote_payload(
        writer
            .prepare_outbox(local_snapshot("history-object"), 1, CREATED_AT_MS + 20)
            .expect("historical outbox"),
        REJECT_FROM_SEQUENCE - 1,
    );

    let reader_provider = product_provider(
        DEVICE_B,
        rotated_domain_state(),
        vec![epoch_material(1), epoch_material(2)],
        SyntheticQualifiedSigningBackend::new(DEVICE_B, SIGNING_KEY_B, [32u8; 32]),
    );
    let mut reader = DefaultSyncObjectProcessor::production(reader_provider);
    reader.preflight(DOMAIN_ID).expect("reader preflight");
    let expected = historical_payload.object.clone();
    let decrypted = reader
        .verify_and_decrypt(&expected, historical_payload.clone())
        .expect("pre-revocation history remains readable");
    assert_eq!(decrypted.plaintext_payload, br#"{"synthetic":true}"#);

    let mut revoked_payload = historical_payload;
    revoked_payload.object.change_sequence = REJECT_FROM_SEQUENCE;
    let expected = revoked_payload.object.clone();
    let error = reader
        .verify_and_decrypt(&expected, revoked_payload)
        .expect_err("post-revocation sequence must be rejected");
    assert_eq!(error.code, SyncOrchestrationErrorCode::RevokedDevice);

    let rotated_outbox = reader
        .prepare_outbox(local_snapshot("rotated-object"), 1, REVOKED_AT_MS + 20)
        .expect("rotated epoch outbox");
    assert_eq!(rotated_outbox.object.draft.key_epoch, 2);
    assert_eq!(rotated_outbox.object.draft.key_id, OBJECT_KEY_2);

    drop(reader);
    let restarted_provider = product_provider(
        DEVICE_B,
        rotated_domain_state(),
        vec![epoch_material(1), epoch_material(2)],
        SyntheticQualifiedSigningBackend::new(DEVICE_B, SIGNING_KEY_B, [32u8; 32]),
    );
    let mut restarted = DefaultSyncObjectProcessor::production(restarted_provider);
    restarted
        .preflight(DOMAIN_ID)
        .expect("restarted provider rebuilds the trusted snapshot");
    let snapshot = restarted
        .cycle_snapshot()
        .expect("restarted cycle snapshot");
    assert_eq!(snapshot.current_write_epoch(), 2);
    assert_eq!(snapshot.accepted_key_epochs().collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn wrapped_epoch_product_store_loads_history_rotation_and_restart_snapshot() {
    let agreement = MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7);
    let trusted_agreement = agreement.public_key.clone();
    let records = vec![
        wrapped_epoch(&trusted_agreement, 1, OBJECT_KEY_1, 11),
        wrapped_epoch(&trusted_agreement, 2, OBJECT_KEY_2, 12),
    ];
    let (source, source_loads) = MemoryWrappedEpochSource::new(records.clone());
    let material_store = ProductWrappedEpochMaterialStore::new(source, agreement);
    let state = active_domain_state_with_agreement(2, trusted_agreement.clone());
    let (trusted_devices, _) = StaticTrustedDeviceSource::new(state);
    let backend = SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [41u8; 32]);
    let mut provider =
        ProductSyncCryptoProvider::new(DEVICE_A, trusted_devices, material_store, backend)
            .expect("provider");
    let snapshot = provider.freeze_cycle(DOMAIN_ID).expect("wrapped snapshot");
    assert_eq!(snapshot.current_write_epoch(), 2);
    assert_eq!(snapshot.accepted_key_epochs().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(source_loads.get(), 1);

    let (restart_source, _) = MemoryWrappedEpochSource::new(records);
    let restart_material_store = ProductWrappedEpochMaterialStore::new(
        restart_source,
        MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7),
    );
    let (restart_devices, _) =
        StaticTrustedDeviceSource::new(active_domain_state_with_agreement(2, trusted_agreement));
    let restart_backend =
        SyntheticQualifiedSigningBackend::new(DEVICE_A, SIGNING_KEY_A, [41u8; 32]);
    let mut restart_provider = ProductSyncCryptoProvider::new(
        DEVICE_A,
        restart_devices,
        restart_material_store,
        restart_backend,
    )
    .expect("restart provider");
    let restart_snapshot = restart_provider
        .freeze_cycle(DOMAIN_ID)
        .expect("restart unwraps protected records again");
    assert_eq!(
        restart_snapshot.accepted_key_epochs().collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn wrapped_epoch_store_rejects_identity_tamper_and_ciphertext_tamper() {
    let agreement = MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7);
    let trusted = agreement.public_key.clone();
    let domain =
        SyncDomain::new(DOMAIN_ID, 1, OBJECT_KEY_1, CREATED_AT_MS, CREATED_AT_MS).expect("domain");
    let profile = active_profile_with_agreement(trusted.clone());
    let original = wrapped_epoch(&trusted, 1, OBJECT_KEY_1, 13);

    for mutate in [
        |record: &mut WrappedEpochMaterial| record.domain_id.push('x'),
        |record: &mut WrappedEpochMaterial| record.recipient_device_id.push('x'),
        |record: &mut WrappedEpochMaterial| record.recipient_key_agreement_key_id.push('x'),
    ] {
        let mut record = original.clone();
        mutate(&mut record);
        let (source, _) = MemoryWrappedEpochSource::new(vec![record]);
        let mut store = ProductWrappedEpochMaterialStore::new(
            source,
            MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7),
        );
        assert_eq!(
            store.load_epoch_materials(&domain, &profile),
            Err(SyncCryptoLoadError::InvalidState)
        );
    }

    let mut tampered = original;
    let last = tampered.wrapped_key.len() - 1;
    tampered.wrapped_key[last] ^= 1;
    let (source, _) = MemoryWrappedEpochSource::new(vec![tampered]);
    let mut store = ProductWrappedEpochMaterialStore::new(
        source,
        MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7),
    );
    assert_eq!(
        store.load_epoch_materials(&domain, &profile),
        Err(SyncCryptoLoadError::AuthenticationFailed)
    );
}

#[test]
fn wrapped_epoch_store_distinguishes_locked_unavailable_and_revoked_before_record_read() {
    let agreement = MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7);
    let trusted = agreement.public_key.clone();
    let domain =
        SyncDomain::new(DOMAIN_ID, 2, OBJECT_KEY_2, CREATED_AT_MS, REVOKED_AT_MS).expect("domain");
    let active = active_profile_with_agreement(trusted.clone());
    let records = vec![wrapped_epoch(&trusted, 2, OBJECT_KEY_2, 14)];

    for (failure, expected) in [
        (AgreementFailure::Locked, SyncCryptoLoadError::Locked),
        (
            AgreementFailure::Unavailable,
            SyncCryptoLoadError::Unavailable,
        ),
    ] {
        let (source, source_loads) = MemoryWrappedEpochSource::new(records.clone());
        let backend =
            MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7).with_failure(failure);
        let mut store = ProductWrappedEpochMaterialStore::new(source, backend);
        assert_eq!(store.load_epoch_materials(&domain, &active), Err(expected));
        assert_eq!(source_loads.get(), 0);
    }

    let mut signing_store = TestMemoryDeviceKeyStore::new();
    let mut signing_public = signing_store
        .insert_signing_key(DEVICE_A, SIGNING_KEY_A, [41u8; 32], CREATED_AT_MS)
        .expect("signing key");
    signing_public.revoked_at_ms = Some(REVOKED_AT_MS);
    let device = SyncDevice::new(
        DEVICE_A,
        SIGNING_KEY_A,
        SyncDeviceStatus::Revoked,
        Some(CREATED_AT_MS),
        Some(REVOKED_AT_MS),
        None,
    )
    .expect("revoked device");
    let mut revoked_agreement = trusted;
    revoked_agreement.revoked_at_ms = Some(REVOKED_AT_MS);
    let revoked = SyncTrustedDeviceProfile::revoked_with_key_agreement_from_change_sequence(
        device,
        signing_public,
        revoked_agreement,
        REJECT_FROM_SEQUENCE,
    )
    .expect("revoked profile");
    let (source, source_loads) = MemoryWrappedEpochSource::new(records);
    let mut store = ProductWrappedEpochMaterialStore::new(
        source,
        MemoryP256AgreementBackend::new(DEVICE_A, "agreement-a", 7),
    );
    assert_eq!(
        store.load_epoch_materials(&domain, &revoked),
        Err(SyncCryptoLoadError::Revoked)
    );
    assert_eq!(source_loads.get(), 0);
}

fn product_provider<B: SyncDeviceSigningBackend>(
    local_device_id: &str,
    state: SyncTrustedDomainState,
    materials: Vec<SyncEpochKeyMaterial>,
    backend: B,
) -> ProductSyncCryptoProvider<StaticTrustedDeviceSource, StaticEpochMaterialStore, B> {
    let (trusted_devices, _) = StaticTrustedDeviceSource::new(state);
    let (epoch_materials, _) = StaticEpochMaterialStore::new(materials);
    ProductSyncCryptoProvider::new(local_device_id, trusted_devices, epoch_materials, backend)
        .expect("product provider")
}

fn active_domain_state(current_epoch: u64) -> SyncTrustedDomainState {
    assert_eq!(
        current_epoch, 1,
        "active test state only represents epoch one"
    );
    let domain = SyncDomain::new(
        DOMAIN_ID,
        1,
        OBJECT_KEY_1,
        CREATED_AT_MS,
        CREATED_AT_MS + 10,
    )
    .expect("active domain");
    let (device_a, device_b, authorization) = authorized_devices();
    assert_eq!(authorization.key_epoch, domain.current_key_epoch);
    SyncTrustedDomainState::new(
        domain,
        [
            SyncTrustedDeviceProfile::active(
                device_a,
                public_key(DEVICE_A, SIGNING_KEY_A, [31u8; 32]),
            )
            .expect("device a profile"),
            SyncTrustedDeviceProfile::active(
                device_b,
                public_key(DEVICE_B, SIGNING_KEY_B, [32u8; 32]),
            )
            .expect("device b profile"),
        ],
    )
    .expect("active trusted state")
}

fn active_domain_state_with_agreement(
    current_epoch: u64,
    agreement_public_key: DeviceKeyAgreementPublicKey,
) -> SyncTrustedDomainState {
    let profile = active_profile_with_agreement(agreement_public_key);
    SyncTrustedDomainState::new(
        SyncDomain::new(
            DOMAIN_ID,
            current_epoch,
            if current_epoch == 1 {
                OBJECT_KEY_1
            } else {
                OBJECT_KEY_2
            },
            CREATED_AT_MS,
            CREATED_AT_MS + i64::try_from(current_epoch).expect("epoch timestamp"),
        )
        .expect("domain"),
        [profile],
    )
    .expect("trusted domain")
}

fn active_profile_with_agreement(
    agreement_public_key: DeviceKeyAgreementPublicKey,
) -> SyncTrustedDeviceProfile {
    let mut signing_store = TestMemoryDeviceKeyStore::new();
    signing_store
        .insert_signing_key(DEVICE_A, SIGNING_KEY_A, [41u8; 32], CREATED_AT_MS)
        .expect("signing key");
    let handle = signing_store
        .handle(DEVICE_A, SIGNING_KEY_A)
        .expect("signing handle");
    let signing_public_key = signing_store
        .public_key(&handle)
        .expect("signing public key");
    let device = SyncDevice::new(
        DEVICE_A,
        SIGNING_KEY_A,
        SyncDeviceStatus::Active,
        Some(CREATED_AT_MS),
        None,
        None,
    )
    .expect("active device");
    SyncTrustedDeviceProfile::active_with_key_agreement(
        device,
        signing_public_key,
        agreement_public_key,
    )
    .expect("active profile")
}

fn wrapped_epoch(
    recipient: &DeviceKeyAgreementPublicKey,
    epoch: u64,
    object_key_id: &str,
    nonce_byte: u8,
) -> WrappedEpochMaterial {
    let descriptor =
        KeyDescriptor::new(object_key_id, KeyRole::ObjectKey, epoch).expect("object key");
    let master_byte = u8::try_from(epoch).expect("epoch byte");
    let master = SyncMasterKeyMaterial::new([master_byte; 32]).expect("master key");
    WrappedEpochMaterial::seal_for_recipient(
        DOMAIN_ID,
        recipient,
        format!("wrapping-{epoch}"),
        &descriptor,
        &master,
        Nonce::new(vec![nonce_byte; 24]).expect("nonce"),
        CREATED_AT_MS + i64::try_from(epoch).expect("epoch time"),
    )
    .expect("wrapped epoch")
}

fn rotated_domain_state() -> SyncTrustedDomainState {
    let (device_a, device_b, authorization) = authorized_devices();
    let revocation = DeviceRevocationRecord::new(
        DEVICE_A,
        DEVICE_B,
        authorization.key_epoch,
        authorization.key_epoch + 1,
        DeviceRevocationReason::UserRequested,
        REVOKED_AT_MS,
    )
    .expect("device revocation record");
    let domain = SyncDomain::new(
        DOMAIN_ID,
        revocation.previous_key_epoch,
        OBJECT_KEY_1,
        CREATED_AT_MS,
        CREATED_AT_MS + 10,
    )
    .expect("initial domain")
    .advance_key_epoch(OBJECT_KEY_2, REVOKED_AT_MS)
    .expect("rotated domain");
    assert_eq!(domain.current_key_epoch, revocation.new_key_epoch);
    let revoked_device = device_a
        .revoke(revocation.revoked_at_ms, false)
        .expect("revoked device");
    let mut revoked_public_key = public_key(DEVICE_A, SIGNING_KEY_A, [31u8; 32]);
    revoked_public_key.revoked_at_ms = Some(revocation.revoked_at_ms);
    revoked_public_key.validate().expect("revoked public key");
    SyncTrustedDomainState::new(
        domain,
        [
            SyncTrustedDeviceProfile::revoked_from_change_sequence(
                revoked_device,
                revoked_public_key,
                REJECT_FROM_SEQUENCE,
            )
            .expect("revoked device profile"),
            SyncTrustedDeviceProfile::active(
                device_b,
                public_key(DEVICE_B, SIGNING_KEY_B, [32u8; 32]),
            )
            .expect("active device b profile"),
        ],
    )
    .expect("rotated trusted state")
}

fn authorized_devices() -> (SyncDevice, SyncDevice, DeviceAuthorizationPackage) {
    let device_a = active_device(DEVICE_A, SIGNING_KEY_A);
    let device_b = active_device(DEVICE_B, SIGNING_KEY_B);
    let wrapping_key = KeyDescriptor::new("wrapping-key-product-1", KeyRole::DeviceWrapping, 1)
        .expect("wrapping key descriptor");
    let wrapping_record = DeviceWrappingRecord::new(
        DEVICE_B,
        &wrapping_key,
        b"synthetic-wrapped-sync-key",
        CREATED_AT_MS + 10,
    )
    .expect("device wrapping record");
    let authorization =
        DeviceAuthorizationPackage::new(&device_b, &device_a, &wrapping_record, CREATED_AT_MS + 10)
            .expect("device authorization package");
    (device_a, device_b, authorization)
}

fn active_device(device_id: &str, signing_key_id: &str) -> SyncDevice {
    SyncDevice::pending(device_id, signing_key_id, CREATED_AT_MS + 1)
        .expect("pending device")
        .activate(CREATED_AT_MS + 10)
        .expect("active device")
}

fn public_key(device_id: &str, signing_key_id: &str, seed: [u8; 32]) -> DeviceSigningPublicKey {
    let mut store = TestMemoryDeviceKeyStore::new();
    store
        .insert_signing_key(device_id, signing_key_id, seed, CREATED_AT_MS)
        .expect("device public key")
}

fn epoch_material(epoch: u64) -> SyncEpochKeyMaterial {
    let key_id = match epoch {
        1 => OBJECT_KEY_1,
        2 => OBJECT_KEY_2,
        _ => panic!("unsupported test epoch"),
    };
    let descriptor =
        KeyDescriptor::new(key_id, KeyRole::ObjectKey, epoch).expect("object key descriptor");
    let key_byte = u8::try_from(epoch).expect("test epoch fits in one byte");
    let master_key = SyncMasterKeyMaterial::new([key_byte; 32]).expect("sync master key");
    SyncEpochKeyMaterial::new(descriptor, master_key).expect("epoch material")
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
            server_received_at_ms: CREATED_AT_MS + 30,
            client_created_at_ms: draft.created_at_ms,
            client_updated_at_ms: draft.updated_at_ms,
        },
        payload: outbox.object.envelope.encrypted_payload,
    }
}
