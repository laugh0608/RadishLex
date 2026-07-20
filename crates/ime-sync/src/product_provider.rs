use std::collections::BTreeMap;
use std::fmt;

use radishlex_ime_crypto::{
    CryptoError, DeviceKeyAgreementKeyHandle, DeviceKeyAgreementPublicKey,
    DevicePrivateKeyStoreStatus, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    EcdhSharedSecret, WrappedEpochMaterial,
};

use crate::processor::validate_local_signing_boundary;
use crate::{
    SyncCryptoCycleSnapshot, SyncCryptoProvider, SyncCyclePhase, SyncDevice, SyncDeviceStatus,
    SyncDomain, SyncEpochKeyMaterial, SyncOrchestrationError, SyncOrchestrationErrorCode,
    SyncRemoteSigningProfile,
};

/// Stable, redacted failure returned by product crypto state sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncCryptoLoadError {
    Unavailable,
    Locked,
    AccessDenied,
    AuthenticationFailed,
    Revoked,
    InvalidState,
}

/// One locally verified device lifecycle profile used to build a cycle snapshot.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncTrustedDeviceProfile {
    device: SyncDevice,
    signing_public_key: DeviceSigningPublicKey,
    key_agreement_public_key: Option<DeviceKeyAgreementPublicKey>,
    reject_from_change_sequence: Option<u64>,
}

impl SyncTrustedDeviceProfile {
    pub fn active(
        device: SyncDevice,
        signing_public_key: DeviceSigningPublicKey,
    ) -> Result<Self, SyncCryptoLoadError> {
        Self::new(device, signing_public_key, None, None)
    }

    pub fn active_with_key_agreement(
        device: SyncDevice,
        signing_public_key: DeviceSigningPublicKey,
        key_agreement_public_key: DeviceKeyAgreementPublicKey,
    ) -> Result<Self, SyncCryptoLoadError> {
        Self::new(
            device,
            signing_public_key,
            Some(key_agreement_public_key),
            None,
        )
    }

    pub fn revoked_from_change_sequence(
        device: SyncDevice,
        signing_public_key: DeviceSigningPublicKey,
        reject_from_change_sequence: u64,
    ) -> Result<Self, SyncCryptoLoadError> {
        Self::new(
            device,
            signing_public_key,
            None,
            Some(reject_from_change_sequence),
        )
    }

    pub fn revoked_with_key_agreement_from_change_sequence(
        device: SyncDevice,
        signing_public_key: DeviceSigningPublicKey,
        key_agreement_public_key: DeviceKeyAgreementPublicKey,
        reject_from_change_sequence: u64,
    ) -> Result<Self, SyncCryptoLoadError> {
        Self::new(
            device,
            signing_public_key,
            Some(key_agreement_public_key),
            Some(reject_from_change_sequence),
        )
    }

    fn new(
        device: SyncDevice,
        signing_public_key: DeviceSigningPublicKey,
        key_agreement_public_key: Option<DeviceKeyAgreementPublicKey>,
        reject_from_change_sequence: Option<u64>,
    ) -> Result<Self, SyncCryptoLoadError> {
        device
            .validate()
            .map_err(|_| SyncCryptoLoadError::InvalidState)?;
        signing_public_key
            .validate()
            .map_err(|_| SyncCryptoLoadError::InvalidState)?;
        if device.device_id != signing_public_key.device_id
            || device.public_key_id != signing_public_key.signing_key_id
        {
            return Err(SyncCryptoLoadError::InvalidState);
        }
        let authorized_at_ms = device
            .authorized_at_ms
            .ok_or(SyncCryptoLoadError::InvalidState)?;
        if let Some(key_agreement_public_key) = &key_agreement_public_key {
            key_agreement_public_key
                .validate()
                .map_err(|_| SyncCryptoLoadError::InvalidState)?;
            if key_agreement_public_key.device_id != device.device_id
                || key_agreement_public_key.created_at_ms > authorized_at_ms
            {
                return Err(SyncCryptoLoadError::InvalidState);
            }
        }
        if signing_public_key.created_at_ms > authorized_at_ms {
            return Err(SyncCryptoLoadError::InvalidState);
        }

        match device.status {
            SyncDeviceStatus::Active => {
                if reject_from_change_sequence.is_some()
                    || signing_public_key.revoked_at_ms.is_some()
                {
                    return Err(SyncCryptoLoadError::InvalidState);
                }
            }
            SyncDeviceStatus::Revoked | SyncDeviceStatus::Lost => {
                if !matches!(reject_from_change_sequence, Some(sequence) if sequence > 0) {
                    return Err(SyncCryptoLoadError::InvalidState);
                }
                let revoked_at_ms = device
                    .revoked_at_ms
                    .ok_or(SyncCryptoLoadError::InvalidState)?;
                if signing_public_key.revoked_at_ms != Some(revoked_at_ms)
                    || revoked_at_ms < authorized_at_ms
                {
                    return Err(SyncCryptoLoadError::InvalidState);
                }
            }
            SyncDeviceStatus::Pending => return Err(SyncCryptoLoadError::InvalidState),
        }

        Ok(Self {
            device,
            signing_public_key,
            key_agreement_public_key,
            reject_from_change_sequence,
        })
    }

    pub fn device(&self) -> &SyncDevice {
        &self.device
    }

    pub fn signing_public_key(&self) -> &DeviceSigningPublicKey {
        &self.signing_public_key
    }

    pub fn key_agreement_public_key(&self) -> Option<&DeviceKeyAgreementPublicKey> {
        self.key_agreement_public_key.as_ref()
    }

    pub fn reject_from_change_sequence(&self) -> Option<u64> {
        self.reject_from_change_sequence
    }

    fn into_remote_profile(self) -> Result<SyncRemoteSigningProfile, SyncOrchestrationError> {
        match self.reject_from_change_sequence {
            Some(sequence) => SyncRemoteSigningProfile::revoked_from_change_sequence(
                self.signing_public_key,
                sequence,
            ),
            None => SyncRemoteSigningProfile::active(self.signing_public_key),
        }
    }
}

impl fmt::Debug for SyncTrustedDeviceProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncTrustedDeviceProfile")
            .field("device_id", &self.device.device_id)
            .field("public_key_id", &self.device.public_key_id)
            .field("status", &self.device.status)
            .field(
                "signature_algorithm",
                &self.signing_public_key.signature_algorithm,
            )
            .field(
                "key_agreement_key_id",
                &self
                    .key_agreement_public_key
                    .as_ref()
                    .map(|public_key| public_key.key_id.as_str()),
            )
            .field(
                "reject_from_change_sequence",
                &self.reject_from_change_sequence,
            )
            .finish()
    }
}

/// Public domain and device lifecycle state already verified by the Rust client.
#[derive(Clone, PartialEq, Eq)]
pub struct SyncTrustedDomainState {
    domain: SyncDomain,
    device_profiles: BTreeMap<String, SyncTrustedDeviceProfile>,
}

impl SyncTrustedDomainState {
    pub fn new(
        domain: SyncDomain,
        device_profiles: impl IntoIterator<Item = SyncTrustedDeviceProfile>,
    ) -> Result<Self, SyncCryptoLoadError> {
        domain
            .validate()
            .map_err(|_| SyncCryptoLoadError::InvalidState)?;
        let mut profiles = BTreeMap::new();
        for profile in device_profiles {
            let device_id = profile.device.device_id.clone();
            if profiles.insert(device_id, profile).is_some() {
                return Err(SyncCryptoLoadError::InvalidState);
            }
        }
        if profiles.is_empty() {
            return Err(SyncCryptoLoadError::InvalidState);
        }
        Ok(Self {
            domain,
            device_profiles: profiles,
        })
    }

    pub fn domain(&self) -> &SyncDomain {
        &self.domain
    }

    pub fn device_profile(&self, device_id: &str) -> Option<&SyncTrustedDeviceProfile> {
        self.device_profiles.get(device_id)
    }

    pub fn device_profiles(&self) -> impl ExactSizeIterator<Item = &SyncTrustedDeviceProfile> + '_ {
        self.device_profiles.values()
    }
}

impl fmt::Debug for SyncTrustedDomainState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncTrustedDomainState")
            .field("domain", &self.domain)
            .field("device_profile_count", &self.device_profiles.len())
            .finish()
    }
}

/// Loads locally trusted public lifecycle state without performing network I/O.
pub trait SyncTrustedDeviceSource {
    fn load_trusted_domain(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncTrustedDomainState, SyncCryptoLoadError>;
}

/// Returns unwrapped epoch material authorized for one local device.
///
/// Implementations must not use Manager settings or plaintext SQLite columns as
/// the secret source. Returned material is consumed into one cycle snapshot.
pub trait SyncEpochMaterialStore {
    fn load_epoch_materials(
        &mut self,
        domain: &SyncDomain,
        local_device: &SyncTrustedDeviceProfile,
    ) -> Result<Vec<SyncEpochKeyMaterial>, SyncCryptoLoadError>;
}

/// Loads only opaque wrapped records and public metadata.
pub trait SyncWrappedEpochMaterialSource {
    fn load_wrapped_epoch_materials(
        &mut self,
        domain_id: &str,
        local_device_id: &str,
    ) -> Result<Vec<WrappedEpochMaterial>, SyncCryptoLoadError>;
}

/// Independent key-agreement port over an opaque platform private-key handle.
pub trait SyncDeviceKeyAgreementBackend {
    fn key_handle(
        &self,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError>;

    fn public_key(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError>;

    fn derive_shared_secret(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError>;
}

/// Product material adapter: validates the signed public profile, opens each
/// wrapped record with the separate platform key-agreement backend, and returns
/// plaintext material only to the Rust cycle snapshot.
pub struct ProductWrappedEpochMaterialStore<R, K> {
    records: R,
    key_agreement: K,
}

impl<R, K> ProductWrappedEpochMaterialStore<R, K> {
    pub fn new(records: R, key_agreement: K) -> Self {
        Self {
            records,
            key_agreement,
        }
    }
}

impl<R, K> fmt::Debug for ProductWrappedEpochMaterialStore<R, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductWrappedEpochMaterialStore")
            .field("records", &std::any::type_name::<R>())
            .field("key_agreement", &std::any::type_name::<K>())
            .finish()
    }
}

impl<R, K> SyncEpochMaterialStore for ProductWrappedEpochMaterialStore<R, K>
where
    R: SyncWrappedEpochMaterialSource,
    K: SyncDeviceKeyAgreementBackend,
{
    fn load_epoch_materials(
        &mut self,
        domain: &SyncDomain,
        local_device: &SyncTrustedDeviceProfile,
    ) -> Result<Vec<SyncEpochKeyMaterial>, SyncCryptoLoadError> {
        if local_device.device.status != SyncDeviceStatus::Active {
            return Err(SyncCryptoLoadError::Revoked);
        }
        let trusted_key = local_device
            .key_agreement_public_key()
            .ok_or(SyncCryptoLoadError::InvalidState)?;
        let handle = self
            .key_agreement
            .key_handle(&local_device.device.device_id, &trusted_key.key_id)
            .map_err(map_key_agreement_error)?;
        let backend_public_key = self
            .key_agreement
            .public_key(&handle)
            .map_err(map_key_agreement_error)?;
        if backend_public_key.device_id != trusted_key.device_id
            || backend_public_key.key_id != trusted_key.key_id
            || backend_public_key.algorithm != trusted_key.algorithm
            || backend_public_key.public_key != trusted_key.public_key
            || backend_public_key.revoked_at_ms != trusted_key.revoked_at_ms
        {
            return Err(SyncCryptoLoadError::InvalidState);
        }
        let records = self
            .records
            .load_wrapped_epoch_materials(&domain.domain_id, &local_device.device.device_id)?;
        let mut materials = Vec::with_capacity(records.len());
        for record in records {
            if record.domain_id != domain.domain_id
                || record.recipient_device_id != local_device.device.device_id
                || record.recipient_key_agreement_key_id != trusted_key.key_id
                || record.key_epoch > domain.current_key_epoch
            {
                return Err(SyncCryptoLoadError::InvalidState);
            }
            let ephemeral = record
                .ephemeral_public_key()
                .map_err(|_| SyncCryptoLoadError::InvalidState)?;
            let shared_secret = self
                .key_agreement
                .derive_shared_secret(&handle, ephemeral)
                .map_err(map_key_agreement_error)?;
            let (descriptor, master_key) = record
                .unwrap(&shared_secret)
                .map_err(|_| SyncCryptoLoadError::AuthenticationFailed)?;
            materials.push(
                SyncEpochKeyMaterial::new(descriptor, master_key)
                    .map_err(|_| SyncCryptoLoadError::InvalidState)?,
            );
        }
        Ok(materials)
    }
}

/// Narrow signing port over an opaque platform private-key handle.
pub trait SyncDeviceSigningBackend {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus;

    fn signing_handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceSigningKeyHandle, CryptoError>;

    fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError>;

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError>;
}

/// Builds product cycle snapshots from trusted lifecycle, secret material, and
/// platform signing ports. Every cycle still passes the production backend gate.
pub struct ProductSyncCryptoProvider<D, M, B> {
    local_device_id: String,
    trusted_devices: D,
    epoch_materials: M,
    signing_backend: B,
}

impl<D, M, B> fmt::Debug for ProductSyncCryptoProvider<D, M, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductSyncCryptoProvider")
            .field("local_device_id", &self.local_device_id)
            .field("trusted_devices", &std::any::type_name::<D>())
            .field("epoch_materials", &std::any::type_name::<M>())
            .field("signing_backend", &std::any::type_name::<B>())
            .finish()
    }
}

impl<D, M, B> ProductSyncCryptoProvider<D, M, B> {
    pub fn new(
        local_device_id: impl Into<String>,
        trusted_devices: D,
        epoch_materials: M,
        signing_backend: B,
    ) -> Result<Self, SyncCryptoLoadError> {
        let local_device_id = local_device_id.into();
        if local_device_id.trim().is_empty() {
            return Err(SyncCryptoLoadError::InvalidState);
        }
        Ok(Self {
            local_device_id,
            trusted_devices,
            epoch_materials,
            signing_backend,
        })
    }
}

impl<D, M, B> SyncCryptoProvider for ProductSyncCryptoProvider<D, M, B>
where
    D: SyncTrustedDeviceSource,
    M: SyncEpochMaterialStore,
    B: SyncDeviceSigningBackend,
{
    fn freeze_cycle(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncCryptoCycleSnapshot, SyncOrchestrationError> {
        let trusted_state = self
            .trusted_devices
            .load_trusted_domain(domain_id)
            .map_err(map_load_error)?;
        if trusted_state.domain.domain_id != domain_id {
            return Err(invalid_preflight());
        }

        let local_profile = trusted_state
            .device_profiles
            .get(&self.local_device_id)
            .ok_or_else(invalid_preflight)?;
        if local_profile.device.status != SyncDeviceStatus::Active {
            return Err(preflight_error(SyncOrchestrationErrorCode::RevokedDevice));
        }

        let backend_status = self.signing_backend.backend_status();
        backend_status
            .ensure_production_signing_allowed()
            .map_err(|_| backend_preflight())?;
        let signing_handle = self
            .signing_backend
            .signing_handle(
                &local_profile.device.device_id,
                &local_profile.device.public_key_id,
                local_profile.signing_public_key.created_at_ms,
            )
            .map_err(|_| backend_preflight())?;
        let backend_public_key = self
            .signing_backend
            .public_key(&signing_handle)
            .map_err(|_| backend_preflight())?;
        if backend_public_key != local_profile.signing_public_key {
            return Err(invalid_preflight());
        }
        validate_local_signing_boundary(&signing_handle, &backend_public_key, &backend_status)?;

        let epoch_materials = self
            .epoch_materials
            .load_epoch_materials(&trusted_state.domain, local_profile)
            .map_err(map_load_error)?;
        validate_epoch_materials(&trusted_state.domain, &epoch_materials)?;

        let remote_signers = trusted_state
            .device_profiles
            .into_values()
            .map(SyncTrustedDeviceProfile::into_remote_profile)
            .collect::<Result<Vec<_>, _>>()?;

        SyncCryptoCycleSnapshot::production(
            domain_id,
            signing_handle,
            backend_public_key,
            backend_status,
            trusted_state.domain.current_key_epoch,
            epoch_materials,
            remote_signers,
        )
    }

    fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        self.signing_backend.sign(handle, canonical_bytes)
    }
}

#[cfg(feature = "apple-keychain")]
macro_rules! impl_apple_signing_backend {
    ($store:ty) => {
        impl SyncDeviceSigningBackend for $store {
            fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
                <$store>::backend_status(self)
            }

            fn signing_handle(
                &self,
                device_id: &str,
                signing_key_id: &str,
                created_at_ms: i64,
            ) -> Result<DeviceSigningKeyHandle, CryptoError> {
                let mut handle = <$store>::handle(self, device_id, signing_key_id)?;
                handle.created_at_ms = created_at_ms;
                Ok(handle)
            }

            fn public_key(
                &self,
                handle: &DeviceSigningKeyHandle,
            ) -> Result<DeviceSigningPublicKey, CryptoError> {
                <$store>::public_key(self, handle)
            }

            fn sign(
                &self,
                handle: &DeviceSigningKeyHandle,
                canonical_bytes: &[u8],
            ) -> Result<DeviceSignature, CryptoError> {
                <$store>::sign(self, handle, canonical_bytes)
            }
        }
    };
}

#[cfg(feature = "apple-keychain")]
impl_apple_signing_backend!(radishlex_ime_crypto::AppleKeychainDeviceKeyStore);
#[cfg(feature = "apple-keychain")]
impl_apple_signing_backend!(radishlex_ime_crypto::AppleKeychainP256DeviceKeyStore);
#[cfg(feature = "apple-keychain")]
impl_apple_signing_backend!(radishlex_ime_crypto::AppleSecureEnclaveP256DeviceKeyStore);

#[cfg(feature = "apple-keychain")]
impl SyncDeviceKeyAgreementBackend
    for radishlex_ime_crypto::AppleSecureEnclaveP256KeyAgreementStore
{
    fn key_handle(
        &self,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        self.handle(device_id, key_id)
    }

    fn public_key(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        radishlex_ime_crypto::AppleSecureEnclaveP256KeyAgreementStore::public_key(self, handle)
    }

    fn derive_shared_secret(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        radishlex_ime_crypto::AppleSecureEnclaveP256KeyAgreementStore::derive_shared_secret(
            self,
            handle,
            peer_public_key,
        )
    }
}

fn validate_epoch_materials(
    domain: &SyncDomain,
    epoch_materials: &[SyncEpochKeyMaterial],
) -> Result<(), SyncOrchestrationError> {
    let mut current_material = None;
    let mut seen_epochs = BTreeMap::new();
    for material in epoch_materials {
        if material.key_epoch() > domain.current_key_epoch
            || seen_epochs.insert(material.key_epoch(), ()).is_some()
        {
            return Err(invalid_preflight());
        }
        if material.key_epoch() == domain.current_key_epoch {
            current_material = Some(material);
        }
    }
    let current_material = current_material
        .ok_or_else(|| preflight_error(SyncOrchestrationErrorCode::KeyEpochRejected))?;
    if current_material.key_id() != domain.active_key_id {
        return Err(invalid_preflight());
    }
    Ok(())
}

fn map_load_error(error: SyncCryptoLoadError) -> SyncOrchestrationError {
    match error {
        SyncCryptoLoadError::Unavailable | SyncCryptoLoadError::Locked => backend_preflight(),
        SyncCryptoLoadError::AccessDenied | SyncCryptoLoadError::AuthenticationFailed => {
            preflight_error(SyncOrchestrationErrorCode::Unauthenticated)
        }
        SyncCryptoLoadError::Revoked => preflight_error(SyncOrchestrationErrorCode::RevokedDevice),
        SyncCryptoLoadError::InvalidState => invalid_preflight(),
    }
}

fn map_key_agreement_error(error: CryptoError) -> SyncCryptoLoadError {
    match error {
        CryptoError::StorageBackendUnavailable { .. }
        | CryptoError::UnsupportedStorageBackend { .. }
        | CryptoError::PrivateKeyUnavailable { .. } => SyncCryptoLoadError::Unavailable,
        CryptoError::PrivateKeyLocked { .. }
        | CryptoError::PrivateKeyUserPresenceRequired { .. } => SyncCryptoLoadError::Locked,
        CryptoError::PrivateKeyAccessDenied { .. } => SyncCryptoLoadError::AccessDenied,
        CryptoError::PrivateKeyRevoked { .. } => SyncCryptoLoadError::Revoked,
        CryptoError::DecryptionFailed | CryptoError::CiphertextHashMismatch => {
            SyncCryptoLoadError::AuthenticationFailed
        }
        _ => SyncCryptoLoadError::InvalidState,
    }
}

fn backend_preflight() -> SyncOrchestrationError {
    preflight_error(SyncOrchestrationErrorCode::BackendUnavailable)
}

fn invalid_preflight() -> SyncOrchestrationError {
    preflight_error(SyncOrchestrationErrorCode::InvalidMetadata)
}

fn preflight_error(code: SyncOrchestrationErrorCode) -> SyncOrchestrationError {
    SyncOrchestrationError::new(code, SyncCyclePhase::Preflight, false)
}

#[cfg(test)]
mod tests;
