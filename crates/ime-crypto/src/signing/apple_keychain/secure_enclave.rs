use super::*;

pub struct AppleSecureEnclaveP256DeviceKeyStore {
    service: String,
    label: String,
    revoked_keys: Mutex<BTreeSet<(String, String)>>,
}

impl AppleSecureEnclaveP256DeviceKeyStore {
    pub fn new() -> Self {
        Self {
            service: DEFAULT_SECURE_ENCLAVE_P256_KEYCHAIN_SERVICE.to_owned(),
            label: DEFAULT_SECURE_ENCLAVE_P256_KEYCHAIN_LABEL.to_owned(),
            revoked_keys: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn with_service(
        service: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Self, CryptoError> {
        let service = service.into();
        let label = label.into();
        validate_required("keychain_service", &service)?;
        validate_required("keychain_label", &label)?;
        Ok(Self {
            service,
            label,
            revoked_keys: Mutex::new(BTreeSet::new()),
        })
    }

    pub fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        platform::backend_status(AppleKeychainProfile::SecureEnclaveP256V1)
    }

    pub fn create_signing_key(
        &self,
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        platform::create_signing_key(self, device_id, signing_key_id, created_at_ms)
    }

    pub fn handle(
        &self,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        platform::handle(self, device_id, signing_key_id)
    }

    pub fn public_key(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        platform::public_key(self, handle)
    }

    pub fn sign(
        &self,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        platform::sign(self, handle, canonical_bytes)
    }

    pub fn verify_private_key_non_exportable(
        &self,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<(), CryptoError> {
        platform::verify_private_key_non_exportable(self, handle)
    }

    pub fn delete_or_revoke(
        &self,
        handle: &DeviceSigningKeyHandle,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        platform::delete_or_revoke(self, handle, revoked_at_ms)
    }

    fn key_tag(&self, signing_key_id: &str) -> Vec<u8> {
        format!("{}:{signing_key_id}", self.service).into_bytes()
    }

    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError> {
        let revoked_keys =
            self.revoked_keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
        if revoked_keys.contains(&(device_id.to_owned(), signing_key_id.to_owned())) {
            return Err(CryptoError::PrivateKeyRevoked {
                key_id: signing_key_id.to_owned(),
            });
        }
        Ok(())
    }

    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        _revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        let mut revoked_keys =
            self.revoked_keys
                .lock()
                .map_err(|_| CryptoError::PrivateKeyCorrupted {
                    key_id: signing_key_id.to_owned(),
                })?;
        revoked_keys.insert((device_id.to_owned(), signing_key_id.to_owned()));
        Ok(())
    }
}

impl AppleKeychainStore for AppleSecureEnclaveP256DeviceKeyStore {
    fn profile(&self) -> AppleKeychainProfile {
        AppleKeychainProfile::SecureEnclaveP256V1
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn key_tag(&self, signing_key_id: &str) -> Vec<u8> {
        AppleSecureEnclaveP256DeviceKeyStore::key_tag(self, signing_key_id)
    }

    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError> {
        AppleSecureEnclaveP256DeviceKeyStore::ensure_not_revoked(self, device_id, signing_key_id)
    }

    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        AppleSecureEnclaveP256DeviceKeyStore::mark_revoked(
            self,
            device_id,
            signing_key_id,
            revoked_at_ms,
        )
    }
}

impl Default for AppleSecureEnclaveP256DeviceKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AppleSecureEnclaveP256DeviceKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let revoked_key_count = self
            .revoked_keys
            .lock()
            .map(|revoked_keys| revoked_keys.len())
            .unwrap_or_default();
        f.debug_struct("AppleSecureEnclaveP256DeviceKeyStore")
            .field(
                "storage_backend",
                &DEVICE_KEY_STORE_APPLE_SECURE_ENCLAVE_P256_V1,
            )
            .field(
                "signature_algorithm",
                &SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1,
            )
            .field("revoked_key_count", &revoked_key_count)
            .finish()
    }
}
