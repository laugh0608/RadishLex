pub const RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION: u32 = 1;

pub const RADISHLEX_MANAGER_SIGNING_BACKEND_UNAVAILABLE: u32 = 0;
pub const RADISHLEX_MANAGER_SIGNING_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1: u32 = 1;

pub const RADISHLEX_MANAGER_SIGNING_ALGORITHM_UNAVAILABLE: u32 = 0;
pub const RADISHLEX_MANAGER_SIGNING_ALGORITHM_ECDSA_P256_SHA256_V1: u32 = 1;

pub const RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_UNAVAILABLE: u32 = 0;
pub const RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1: u32 = 1;

pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE: u32 = 0;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_NOT_COMPILED: u32 = 1;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_UNAVAILABLE: u32 = 2;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_QUALIFICATION_REQUIRED: u32 = 3;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_NOT_COMPILED: u32 = 4;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_RUNTIME_QUALIFICATION_REQUIRED: u32 =
    5;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_QUALIFICATION_REQUIRED: u32 = 6;
pub const RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_USER_SYNC_CLOSED: u32 = 7;

/// Fixed, redacted product status consumed by the Manager snapshot.
///
/// This structure contains only schema values, enum values, and boolean flags.
/// It never contains device/key identifiers, public keys, platform errors, or
/// secret material.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexManagerSyncProductStatus {
    pub version: u32,
    pub signing_backend: u32,
    pub signing_algorithm: u32,
    pub signing_compiled: u32,
    pub signing_runtime_available: u32,
    pub signing_can_create: u32,
    pub signing_can_sign: u32,
    pub signing_exportable: u32,
    pub signing_hardware_backed: u32,
    pub signing_user_presence_required: u32,
    pub signing_backup_migratable: u32,
    pub signing_product_qualified: u32,
    pub key_agreement_backend: u32,
    pub key_agreement_compiled: u32,
    pub key_agreement_runtime_qualified: u32,
    pub key_agreement_product_qualified: u32,
    pub product_qualified: u32,
    pub user_sync_enabled: u32,
    pub blocker: u32,
}

impl RadishLexManagerSyncProductStatus {
    pub const fn empty() -> Self {
        Self {
            version: RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION,
            signing_backend: RADISHLEX_MANAGER_SIGNING_BACKEND_UNAVAILABLE,
            signing_algorithm: RADISHLEX_MANAGER_SIGNING_ALGORITHM_UNAVAILABLE,
            signing_compiled: 0,
            signing_runtime_available: 0,
            signing_can_create: 0,
            signing_can_sign: 0,
            signing_exportable: 0,
            signing_hardware_backed: 0,
            signing_user_presence_required: 0,
            signing_backup_migratable: 0,
            signing_product_qualified: 0,
            key_agreement_backend: RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_UNAVAILABLE,
            key_agreement_compiled: 0,
            key_agreement_runtime_qualified: 0,
            key_agreement_product_qualified: 0,
            product_qualified: 0,
            user_sync_enabled: 0,
            blocker: RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_NOT_COMPILED,
        }
    }

    pub fn current() -> Self {
        #[cfg(feature = "apple-keychain")]
        {
            use radishlex_ime_crypto::AppleSecureEnclaveP256DeviceKeyStore;

            // backend_status() is metadata-only. It does not create, read,
            // sign with, or delete a platform key.
            let signing = AppleSecureEnclaveP256DeviceKeyStore::new().backend_status();
            let key_agreement = crate::apple_secure_enclave_key_agreement_product::current_status();
            let mut status = Self {
                version: RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION,
                signing_backend: RADISHLEX_MANAGER_SIGNING_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1,
                signing_algorithm: RADISHLEX_MANAGER_SIGNING_ALGORITHM_ECDSA_P256_SHA256_V1,
                signing_compiled: flag(signing.compiled),
                signing_runtime_available: flag(signing.available),
                signing_can_create: flag(signing.can_create_signing_keys),
                signing_can_sign: flag(signing.can_sign),
                signing_exportable: flag(signing.capabilities.exportable),
                signing_hardware_backed: flag(signing.capabilities.hardware_backed),
                signing_user_presence_required: flag(signing.capabilities.user_presence_required),
                signing_backup_migratable: flag(signing.capabilities.backup_migratable),
                signing_product_qualified: flag(signing.product_qualified),
                key_agreement_backend:
                    RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1,
                key_agreement_compiled: key_agreement.compiled,
                // Key-agreement qualification comes from its independently
                // reviewed real-device status. Never inherit signing flags.
                key_agreement_runtime_qualified: key_agreement.runtime_qualified,
                key_agreement_product_qualified: key_agreement.product_qualified,
                product_qualified: flag(signing.product_qualified)
                    * key_agreement.product_qualified,
                user_sync_enabled: 0,
                blocker: RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE,
            };
            status.blocker = status.first_blocker();
            status
        }

        #[cfg(not(feature = "apple-keychain"))]
        Self::empty()
    }

    #[cfg(any(feature = "apple-keychain", test))]
    fn first_blocker(self) -> u32 {
        if self.signing_compiled == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_NOT_COMPILED
        } else if self.signing_runtime_available == 0
            || self.signing_can_create == 0
            || self.signing_can_sign == 0
        {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_UNAVAILABLE
        } else if self.signing_product_qualified == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_QUALIFICATION_REQUIRED
        } else if self.key_agreement_compiled == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_NOT_COMPILED
        } else if self.key_agreement_runtime_qualified == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_RUNTIME_QUALIFICATION_REQUIRED
        } else if self.key_agreement_product_qualified == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_QUALIFICATION_REQUIRED
        } else if self.product_qualified == 0 || self.user_sync_enabled == 0 {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_USER_SYNC_CLOSED
        } else {
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE
        }
    }
}

#[cfg(feature = "apple-keychain")]
const fn flag(value: bool) -> u32 {
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_fails_closed_without_identifiers_or_secrets() {
        let status = RadishLexManagerSyncProductStatus::current();

        assert_eq!(
            status.version,
            RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION
        );
        assert_eq!(
            status.product_qualified,
            cfg!(all(feature = "apple-keychain", target_os = "macos")) as u32
        );
        assert_eq!(status.user_sync_enabled, 0);
        assert_ne!(status.blocker, RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE);

        let debug = format!("{status:?}");
        for forbidden in [
            "device_id",
            "key_id",
            "public_key",
            "signature",
            "wrapped",
            "master_key",
            "shared_secret",
            "OSStatus",
        ] {
            assert!(!debug.contains(forbidden), "debug leaked field {forbidden}");
        }
    }

    #[test]
    fn blocker_precedence_keeps_user_sync_closed() {
        let mut status = RadishLexManagerSyncProductStatus {
            version: RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION,
            signing_backend: RADISHLEX_MANAGER_SIGNING_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1,
            signing_algorithm: RADISHLEX_MANAGER_SIGNING_ALGORITHM_ECDSA_P256_SHA256_V1,
            signing_compiled: 1,
            signing_runtime_available: 1,
            signing_can_create: 1,
            signing_can_sign: 1,
            signing_exportable: 0,
            signing_hardware_backed: 1,
            signing_user_presence_required: 0,
            signing_backup_migratable: 0,
            signing_product_qualified: 1,
            key_agreement_backend:
                RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1,
            key_agreement_compiled: 1,
            key_agreement_runtime_qualified: 1,
            key_agreement_product_qualified: 1,
            product_qualified: 1,
            user_sync_enabled: 0,
            blocker: RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE,
        };

        assert_eq!(
            status.first_blocker(),
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_USER_SYNC_CLOSED
        );
        status.user_sync_enabled = 1;
        assert_eq!(
            status.first_blocker(),
            RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE
        );
    }
}
