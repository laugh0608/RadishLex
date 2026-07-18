use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

use p256::ecdsa::Signature as P256Signature;

use crate::model::{
    validate_non_empty_bytes, validate_required, CryptoError, PrivateKeyAccessDeniedReason,
};

use super::{
    DevicePrivateKeyStoreStatus, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    DeviceSigningStorageBackend, SignatureAlgorithmId, DEVICE_KEY_STORE_APPLE_KEYCHAIN_P256_V1,
    DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1, DEVICE_KEY_STORE_APPLE_SECURE_ENCLAVE_P256_V1,
    ED25519_PUBLIC_KEY_LEN, ED25519_SIGNATURE_LEN, P256_PUBLIC_KEY_LEN,
    SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1, SIGNATURE_ALGORITHM_ED25519_V1,
};

const DEFAULT_KEYCHAIN_SERVICE: &str = "org.radishlex.sync.signing";
const DEFAULT_KEYCHAIN_LABEL: &str = "RadishLex Device Signing Key";
const DEFAULT_P256_KEYCHAIN_SERVICE: &str = "org.radishlex.sync.signing.p256";
const DEFAULT_P256_KEYCHAIN_LABEL: &str = "RadishLex P-256 Device Signing Key";
const DEFAULT_SECURE_ENCLAVE_P256_KEYCHAIN_SERVICE: &str =
    "org.radishlex.sync.signing.secure-enclave.p256";
const DEFAULT_SECURE_ENCLAVE_P256_KEYCHAIN_LABEL: &str =
    "RadishLex Secure Enclave P-256 Device Signing Key";

mod secure_enclave;
pub use secure_enclave::AppleSecureEnclaveP256DeviceKeyStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppleKeychainProfile {
    Ed25519V1,
    P256V1,
    SecureEnclaveP256V1,
}

impl AppleKeychainProfile {
    fn signature_algorithm(self) -> SignatureAlgorithmId {
        match self {
            Self::Ed25519V1 => SignatureAlgorithmId::ed25519_v1(),
            Self::P256V1 | Self::SecureEnclaveP256V1 => {
                SignatureAlgorithmId::ecdsa_p256_sha256_v1()
            }
        }
    }

    fn signature_algorithm_id(self) -> &'static str {
        match self {
            Self::Ed25519V1 => SIGNATURE_ALGORITHM_ED25519_V1,
            Self::P256V1 | Self::SecureEnclaveP256V1 => SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1,
        }
    }

    fn storage_backend(self) -> DeviceSigningStorageBackend {
        match self {
            Self::Ed25519V1 => DeviceSigningStorageBackend::AppleKeychainV1,
            Self::P256V1 => DeviceSigningStorageBackend::AppleKeychainP256V1,
            Self::SecureEnclaveP256V1 => DeviceSigningStorageBackend::AppleSecureEnclaveP256V1,
        }
    }

    fn storage_backend_id(self) -> &'static str {
        self.storage_backend().as_str()
    }
}

trait AppleKeychainStore {
    fn profile(&self) -> AppleKeychainProfile;
    fn label(&self) -> &str;
    fn key_tag(&self, signing_key_id: &str) -> Vec<u8>;
    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError>;
    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError>;
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;
    use std::ptr;

    use super::*;

    type Boolean = u8;
    type CFIndex = isize;
    type CFAllocatorRef = *const c_void;
    type CFTypeRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFDataRef = *const c_void;
    type CFNumberRef = *const c_void;
    type CFErrorRef = *const c_void;
    type SecKeyRef = *const c_void;
    type SecAccessControlRef = *const c_void;
    type OSStatus = i32;
    type SecKeyAlgorithm = CFStringRef;
    type SecKeyOperationType = CFIndex;
    type CFHashCode = usize;
    type SecAccessControlCreateFlags = usize;

    #[repr(C)]
    struct CFDictionaryKeyCallBacks {
        version: CFIndex,
        retain: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void) -> *const c_void>,
        release: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void)>,
        copy_description: Option<unsafe extern "C" fn(*const c_void) -> CFStringRef>,
        equal: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> Boolean>,
        hash: Option<unsafe extern "C" fn(*const c_void) -> CFHashCode>,
    }

    #[repr(C)]
    struct CFDictionaryValueCallBacks {
        version: CFIndex,
        retain: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void) -> *const c_void>,
        release: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void)>,
        copy_description: Option<unsafe extern "C" fn(*const c_void) -> CFStringRef>,
        equal: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> Boolean>,
    }

    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const CF_NUMBER_SINT32_TYPE: CFIndex = 3;
    const SEC_KEY_OPERATION_SIGN: SecKeyOperationType = 0;
    const SIGNING_KEY_SIZE_BITS: i32 = 256;
    const SEC_ACCESS_CONTROL_PRIVATE_KEY_USAGE: SecAccessControlCreateFlags = 1usize << 30;

    const ERR_SEC_SUCCESS: OSStatus = 0;
    const ERR_SEC_UNIMPLEMENTED: OSStatus = -4;
    const ERR_SEC_PARAM: OSStatus = -50;
    const ERR_SEC_WR_PERM: OSStatus = -61;
    const ERR_SEC_MISSING_ENTITLEMENT: OSStatus = -34018;
    const ERR_SEC_RESTRICTED_API: OSStatus = -34020;
    const ERR_SEC_NOT_AVAILABLE: OSStatus = -25291;
    const ERR_SEC_READ_ONLY: OSStatus = -25292;
    const ERR_SEC_AUTH_FAILED: OSStatus = -25293;
    const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
    const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25308;
    const ERR_SEC_INTERACTION_REQUIRED: OSStatus = -25315;
    const ERR_SEC_DECODE: OSStatus = -26275;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
        static kCFBooleanFalse: CFTypeRef;
        static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
        static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;

        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            num_bytes: CFIndex,
            encoding: u32,
            is_external_representation: Boolean,
        ) -> CFStringRef;
        fn CFDataCreate(allocator: CFAllocatorRef, bytes: *const u8, length: CFIndex) -> CFDataRef;
        fn CFDataGetLength(data: CFDataRef) -> CFIndex;
        fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
        fn CFNumberCreate(
            allocator: CFAllocatorRef,
            the_type: CFIndex,
            value_ptr: *const c_void,
        ) -> CFNumberRef;
        fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: CFIndex,
            key_callbacks: *const CFDictionaryKeyCallBacks,
            value_callbacks: *const CFDictionaryValueCallBacks,
        ) -> CFDictionaryRef;
        fn CFErrorGetCode(error: CFErrorRef) -> CFIndex;
    }

    #[allow(non_upper_case_globals)]
    #[link(name = "Security", kind = "framework")]
    extern "C" {
        static kSecClass: CFStringRef;
        static kSecClassKey: CFStringRef;
        static kSecAttrApplicationTag: CFStringRef;
        static kSecAttrAccessControl: CFStringRef;
        static kSecAttrAccessible: CFStringRef;
        static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: CFStringRef;
        static kSecAttrComment: CFStringRef;
        static kSecAttrIsExtractable: CFStringRef;
        static kSecAttrIsPermanent: CFStringRef;
        static kSecAttrKeyClass: CFStringRef;
        static kSecAttrKeyClassPrivate: CFStringRef;
        static kSecAttrKeySizeInBits: CFStringRef;
        static kSecAttrKeyType: CFStringRef;
        static kSecAttrKeyTypeECSECPrimeRandom: CFStringRef;
        static kSecAttrKeyTypeEd25519: CFStringRef;
        static kSecAttrLabel: CFStringRef;
        static kSecAttrTokenID: CFStringRef;
        static kSecAttrTokenIDSecureEnclave: CFStringRef;
        static kSecMatchLimit: CFStringRef;
        static kSecMatchLimitOne: CFStringRef;
        static kSecPrivateKeyAttrs: CFStringRef;
        static kSecReturnRef: CFStringRef;
        static kSecUseDataProtectionKeychain: CFStringRef;
        static kSecKeyAlgorithmEdDSASignatureMessageCurve25519SHA512: CFStringRef;
        static kSecKeyAlgorithmECDSASignatureMessageX962SHA256: CFStringRef;

        fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
        fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;
        fn SecAccessControlCreateWithFlags(
            allocator: CFAllocatorRef,
            protection: CFTypeRef,
            flags: SecAccessControlCreateFlags,
            error: *mut CFErrorRef,
        ) -> SecAccessControlRef;
        fn SecKeyCopyExternalRepresentation(key: SecKeyRef, error: *mut CFErrorRef) -> CFDataRef;
        fn SecKeyCopyPublicKey(key: SecKeyRef) -> SecKeyRef;
        fn SecKeyCreateRandomKey(parameters: CFDictionaryRef, error: *mut CFErrorRef) -> SecKeyRef;
        fn SecKeyCreateSignature(
            key: SecKeyRef,
            algorithm: SecKeyAlgorithm,
            data_to_sign: CFDataRef,
            error: *mut CFErrorRef,
        ) -> CFDataRef;
        fn SecKeyIsAlgorithmSupported(
            key: SecKeyRef,
            operation: SecKeyOperationType,
            algorithm: SecKeyAlgorithm,
        ) -> Boolean;

    }

    struct CfOwned {
        ptr: CFTypeRef,
    }

    impl CfOwned {
        fn new(ptr: CFTypeRef, profile: AppleKeychainProfile) -> Result<Self, CryptoError> {
            if ptr.is_null() {
                return Err(CryptoError::StorageBackendUnavailable {
                    backend: profile.storage_backend_id().to_owned(),
                });
            }
            Ok(Self { ptr })
        }

        fn as_type(&self) -> CFTypeRef {
            self.ptr
        }

        fn as_dictionary(&self) -> CFDictionaryRef {
            self.ptr.cast()
        }

        fn as_data(&self) -> CFDataRef {
            self.ptr.cast()
        }

        fn as_key(&self) -> SecKeyRef {
            self.ptr.cast()
        }
    }

    impl Drop for CfOwned {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe {
                    CFRelease(self.ptr);
                }
            }
        }
    }

    pub(super) fn backend_status(profile: AppleKeychainProfile) -> DevicePrivateKeyStoreStatus {
        match profile {
            AppleKeychainProfile::Ed25519V1 => {
                DevicePrivateKeyStoreStatus::apple_keychain_v1_compiled()
            }
            AppleKeychainProfile::P256V1 => {
                DevicePrivateKeyStoreStatus::apple_keychain_p256_v1_runtime_available()
            }
            AppleKeychainProfile::SecureEnclaveP256V1 => {
                DevicePrivateKeyStoreStatus::apple_secure_enclave_p256_v1_compiled()
            }
        }
    }

    pub(super) fn create_signing_key(
        store: &impl AppleKeychainStore,
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        store.ensure_not_revoked(device_id, signing_key_id)?;

        let private_key = create_private_key(store, signing_key_id, created_at_ms)?;
        public_key_from_private(
            store.profile(),
            device_id,
            signing_key_id,
            created_at_ms,
            None,
            &private_key,
        )
    }

    pub(super) fn handle(
        store: &impl AppleKeychainStore,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        store.ensure_not_revoked(device_id, signing_key_id)?;
        let _private_key = load_private_key(store, signing_key_id)?;
        match store.profile() {
            AppleKeychainProfile::Ed25519V1 => {
                DeviceSigningKeyHandle::apple_keychain(device_id, signing_key_id, 0)
            }
            AppleKeychainProfile::P256V1 => {
                DeviceSigningKeyHandle::apple_keychain_p256(device_id, signing_key_id, 0)
            }
            AppleKeychainProfile::SecureEnclaveP256V1 => {
                DeviceSigningKeyHandle::apple_secure_enclave_p256(device_id, signing_key_id, 0)
            }
        }
    }

    pub(super) fn public_key(
        store: &impl AppleKeychainStore,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_apple_handle(store.profile(), handle)?;
        store.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let private_key = load_private_key(store, &handle.signing_key_id)?;
        public_key_from_private(
            store.profile(),
            &handle.device_id,
            &handle.signing_key_id,
            handle.created_at_ms,
            handle.revoked_at_ms,
            &private_key,
        )
    }

    pub(super) fn sign(
        store: &impl AppleKeychainStore,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        validate_apple_handle(store.profile(), handle)?;
        validate_non_empty_bytes("canonical_bytes", canonical_bytes)?;
        store.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let private_key = load_private_key(store, &handle.signing_key_id)?;
        let algorithm = signature_algorithm(store.profile());
        let is_supported = unsafe {
            SecKeyIsAlgorithmSupported(private_key.as_key(), SEC_KEY_OPERATION_SIGN, algorithm)
        };
        if is_supported == 0 {
            return Err(CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: store.profile().signature_algorithm_id().to_owned(),
            });
        }

        let data_to_sign = cf_data(store.profile(), canonical_bytes)?;
        let mut error = ptr::null();
        let signature_data = unsafe {
            SecKeyCreateSignature(
                private_key.as_key(),
                algorithm,
                data_to_sign.as_data(),
                &mut error,
            )
        };
        let error_status = cf_error_status(error);
        release_error(error);
        let signature_data =
            CfOwned::new(signature_data.cast(), store.profile()).map_err(|_| {
                error_status
                    .map(|status| map_status(store.profile(), &handle.signing_key_id, status))
                    .unwrap_or_else(|| CryptoError::PrivateKeyAccessDenied {
                        key_id: handle.signing_key_id.clone(),
                        reason: PrivateKeyAccessDeniedReason::Unspecified,
                    })
            })?;
        let signature = normalize_signature(store.profile(), &cf_data_bytes(&signature_data)?)
            .map_err(|_| CryptoError::PrivateKeyCorrupted {
                key_id: handle.signing_key_id.clone(),
            })?;
        DeviceSignature::new_for_algorithm(
            store.profile().signature_algorithm(),
            handle.signing_key_id.clone(),
            handle.device_id.clone(),
            signature,
        )
    }

    pub(super) fn verify_private_key_non_exportable(
        store: &impl AppleKeychainStore,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<(), CryptoError> {
        validate_apple_handle(store.profile(), handle)?;
        if store.profile() != AppleKeychainProfile::SecureEnclaveP256V1 {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: store.profile().storage_backend_id().to_owned(),
                message: "private export-block verification requires Secure Enclave".to_owned(),
            });
        }
        store.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let private_key = load_private_key(store, &handle.signing_key_id)?;
        let mut error = ptr::null();
        let private_data =
            unsafe { SecKeyCopyExternalRepresentation(private_key.as_key(), &mut error) };
        release_error(error);
        if private_data.is_null() {
            return Ok(());
        }
        unsafe {
            CFRelease(private_data.cast());
        }
        Err(CryptoError::BackendCapabilityMismatch {
            backend: store.profile().storage_backend_id().to_owned(),
            message: "Secure Enclave private key unexpectedly allowed external representation"
                .to_owned(),
        })
    }

    pub(super) fn delete_or_revoke(
        store: &impl AppleKeychainStore,
        handle: &DeviceSigningKeyHandle,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        validate_apple_handle(store.profile(), handle)?;
        let query = key_delete_query(store, &handle.signing_key_id)?;
        let status = unsafe { SecItemDelete(query.as_dictionary()) };
        match status {
            ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => {
                store.mark_revoked(&handle.device_id, &handle.signing_key_id, revoked_at_ms)
            }
            other => Err(map_status(store.profile(), &handle.signing_key_id, other)),
        }
    }

    fn key_type(profile: AppleKeychainProfile) -> CFStringRef {
        match profile {
            AppleKeychainProfile::Ed25519V1 => unsafe { kSecAttrKeyTypeEd25519 },
            AppleKeychainProfile::P256V1 | AppleKeychainProfile::SecureEnclaveP256V1 => unsafe {
                kSecAttrKeyTypeECSECPrimeRandom
            },
        }
    }

    fn signature_algorithm(profile: AppleKeychainProfile) -> SecKeyAlgorithm {
        match profile {
            AppleKeychainProfile::Ed25519V1 => unsafe {
                kSecKeyAlgorithmEdDSASignatureMessageCurve25519SHA512
            },
            AppleKeychainProfile::P256V1 | AppleKeychainProfile::SecureEnclaveP256V1 => unsafe {
                kSecKeyAlgorithmECDSASignatureMessageX962SHA256
            },
        }
    }

    fn normalize_signature(profile: AppleKeychainProfile, signature: &[u8]) -> Result<Vec<u8>, ()> {
        match profile {
            AppleKeychainProfile::Ed25519V1 if signature.len() == ED25519_SIGNATURE_LEN => {
                Ok(signature.to_vec())
            }
            AppleKeychainProfile::P256V1 | AppleKeychainProfile::SecureEnclaveP256V1 => {
                P256Signature::from_der(signature)
                    .map(|signature| signature.to_bytes().to_vec())
                    .map_err(|_| ())
            }
            _ => Err(()),
        }
    }

    fn create_private_key(
        store: &impl AppleKeychainStore,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<CfOwned, CryptoError> {
        let profile = store.profile();
        let tag = cf_data(profile, &store.key_tag(signing_key_id))?;
        let label = cf_string(profile, store.label())?;
        let created_at = cf_string(profile, &created_at_ms.to_string())?;
        let key_size = cf_number_i32(profile, SIGNING_KEY_SIZE_BITS)?;
        let mut private_entries = vec![
            (unsafe { kSecAttrIsPermanent }, unsafe { kCFBooleanTrue }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
            (unsafe { kSecAttrLabel }, label.as_type()),
        ];
        let _access_control = if profile == AppleKeychainProfile::SecureEnclaveP256V1 {
            let access_control = create_secure_enclave_access_control(profile)?;
            private_entries.push((unsafe { kSecAttrAccessControl }, access_control.as_type()));
            Some(access_control)
        } else {
            private_entries.push((unsafe { kSecAttrAccessible }, unsafe {
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            }));
            if profile == AppleKeychainProfile::Ed25519V1 {
                private_entries
                    .push((unsafe { kSecAttrIsExtractable }, unsafe { kCFBooleanFalse }));
                private_entries.push((unsafe { kSecAttrComment }, created_at.as_type()));
            }
            None
        };
        let private_attrs = cf_dictionary(profile, &private_entries)?;
        let mut parameter_entries = vec![
            (unsafe { kSecAttrKeyType }, key_type(store.profile())),
            (unsafe { kSecAttrKeySizeInBits }, key_size.as_type()),
            (unsafe { kSecPrivateKeyAttrs }, private_attrs.as_type()),
        ];
        add_data_protection_domain(profile, &mut parameter_entries);
        if profile == AppleKeychainProfile::SecureEnclaveP256V1 {
            parameter_entries.push((unsafe { kSecAttrTokenID }, unsafe {
                kSecAttrTokenIDSecureEnclave
            }));
        }
        let parameters = cf_dictionary(profile, &parameter_entries)?;

        let mut error = ptr::null();
        let private_key = unsafe { SecKeyCreateRandomKey(parameters.as_dictionary(), &mut error) };
        let error_status = cf_error_status(error);
        release_error(error);
        CfOwned::new(private_key.cast(), profile).map_err(|_| {
            error_status
                .map(|status| map_status(store.profile(), signing_key_id, status))
                .unwrap_or_else(|| CryptoError::UnsupportedSignatureAlgorithm {
                    algorithm: store.profile().signature_algorithm_id().to_owned(),
                })
        })
    }

    fn load_private_key(
        store: &impl AppleKeychainStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let query = key_query(store, signing_key_id)?;
        let mut result = ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_dictionary(), &mut result) };
        if status != ERR_SEC_SUCCESS {
            return Err(map_status(store.profile(), signing_key_id, status));
        }
        CfOwned::new(result, store.profile())
    }

    fn public_key_from_private(
        profile: AppleKeychainProfile,
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
        revoked_at_ms: Option<i64>,
        private_key: &CfOwned,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        let public_key = unsafe { SecKeyCopyPublicKey(private_key.as_key()) };
        let public_key = CfOwned::new(public_key.cast(), profile).map_err(|_| {
            CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            }
        })?;
        let mut error = ptr::null();
        let public_data =
            unsafe { SecKeyCopyExternalRepresentation(public_key.as_key(), &mut error) };
        release_error(error);
        let public_data = CfOwned::new(public_data.cast(), profile).map_err(|_| {
            CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            }
        })?;
        let public_bytes = cf_data_bytes(&public_data)?;
        let expected_len = match profile {
            AppleKeychainProfile::Ed25519V1 => ED25519_PUBLIC_KEY_LEN,
            AppleKeychainProfile::P256V1 | AppleKeychainProfile::SecureEnclaveP256V1 => {
                P256_PUBLIC_KEY_LEN
            }
        };
        if public_bytes.len() != expected_len {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            });
        }
        DeviceSigningPublicKey::new(
            device_id,
            signing_key_id,
            profile.signature_algorithm(),
            public_bytes,
            created_at_ms,
            revoked_at_ms,
        )
    }

    fn key_query(
        store: &impl AppleKeychainStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let profile = store.profile();
        let tag = cf_data(profile, &store.key_tag(signing_key_id))?;
        let mut entries = vec![
            (unsafe { kSecClass }, unsafe { kSecClassKey }),
            (unsafe { kSecAttrKeyType }, key_type(store.profile())),
            (unsafe { kSecAttrKeyClass }, unsafe {
                kSecAttrKeyClassPrivate
            }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
            (unsafe { kSecReturnRef }, unsafe { kCFBooleanTrue }),
            (unsafe { kSecMatchLimit }, unsafe { kSecMatchLimitOne }),
        ];
        add_data_protection_domain(profile, &mut entries);
        add_secure_enclave_token(profile, &mut entries);
        cf_dictionary(profile, &entries)
    }

    fn key_delete_query(
        store: &impl AppleKeychainStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let profile = store.profile();
        let tag = cf_data(profile, &store.key_tag(signing_key_id))?;
        let mut entries = vec![
            (unsafe { kSecClass }, unsafe { kSecClassKey }),
            (unsafe { kSecAttrKeyType }, key_type(store.profile())),
            (unsafe { kSecAttrKeyClass }, unsafe {
                kSecAttrKeyClassPrivate
            }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
            (unsafe { kSecMatchLimit }, unsafe { kSecMatchLimitOne }),
        ];
        add_data_protection_domain(profile, &mut entries);
        add_secure_enclave_token(profile, &mut entries);
        cf_dictionary(profile, &entries)
    }

    fn add_data_protection_domain(
        profile: AppleKeychainProfile,
        entries: &mut Vec<(CFTypeRef, CFTypeRef)>,
    ) {
        if matches!(
            profile,
            AppleKeychainProfile::P256V1 | AppleKeychainProfile::SecureEnclaveP256V1
        ) {
            entries.push((unsafe { kSecUseDataProtectionKeychain }, unsafe {
                kCFBooleanTrue
            }));
        }
    }

    fn add_secure_enclave_token(
        profile: AppleKeychainProfile,
        entries: &mut Vec<(CFTypeRef, CFTypeRef)>,
    ) {
        if profile == AppleKeychainProfile::SecureEnclaveP256V1 {
            entries.push((unsafe { kSecAttrTokenID }, unsafe {
                kSecAttrTokenIDSecureEnclave
            }));
        }
    }

    fn create_secure_enclave_access_control(
        profile: AppleKeychainProfile,
    ) -> Result<CfOwned, CryptoError> {
        let mut error = ptr::null();
        let access_control = unsafe {
            SecAccessControlCreateWithFlags(
                ptr::null(),
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly.cast(),
                SEC_ACCESS_CONTROL_PRIVATE_KEY_USAGE,
                &mut error,
            )
        };
        let error_status = cf_error_status(error);
        release_error(error);
        CfOwned::new(access_control.cast(), profile).map_err(|_| {
            error_status
                .map(|status| map_status(profile, "access-control", status))
                .unwrap_or_else(|| CryptoError::StorageBackendUnavailable {
                    backend: profile.storage_backend_id().to_owned(),
                })
        })
    }

    fn validate_apple_handle(
        profile: AppleKeychainProfile,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<(), CryptoError> {
        handle.validate()?;
        if handle.storage_backend != profile.storage_backend()
            || handle.signature_algorithm != profile.signature_algorithm()
        {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: handle.storage_backend.as_str().to_owned(),
                message: format!(
                    "handle must use {} with {}",
                    profile.storage_backend_id(),
                    profile.signature_algorithm_id()
                ),
            });
        }
        Ok(())
    }

    fn map_status(
        profile: AppleKeychainProfile,
        signing_key_id: &str,
        status: OSStatus,
    ) -> CryptoError {
        match status {
            ERR_SEC_ITEM_NOT_FOUND => CryptoError::PrivateKeyUnavailable {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_NOT_AVAILABLE => CryptoError::StorageBackendUnavailable {
                backend: profile.storage_backend_id().to_owned(),
            },
            ERR_SEC_INTERACTION_NOT_ALLOWED => CryptoError::PrivateKeyLocked {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_INTERACTION_REQUIRED => CryptoError::PrivateKeyUserPresenceRequired {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_AUTH_FAILED => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::AuthenticationFailed,
            },
            ERR_SEC_WR_PERM => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::WritePermission,
            },
            ERR_SEC_READ_ONLY => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::ReadOnly,
            },
            ERR_SEC_MISSING_ENTITLEMENT => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::MissingEntitlement,
            },
            ERR_SEC_RESTRICTED_API => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::RestrictedApi,
            },
            ERR_SEC_DECODE => CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_UNIMPLEMENTED | ERR_SEC_PARAM
                if profile == AppleKeychainProfile::SecureEnclaveP256V1 =>
            {
                CryptoError::StorageBackendUnavailable {
                    backend: profile.storage_backend_id().to_owned(),
                }
            }
            ERR_SEC_UNIMPLEMENTED | ERR_SEC_PARAM => CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: profile.signature_algorithm_id().to_owned(),
            },
            _ => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::UnclassifiedPlatformStatus(status),
            },
        }
    }

    fn cf_string(profile: AppleKeychainProfile, value: &str) -> Result<CfOwned, CryptoError> {
        validate_required("cf_string", value)?;
        let string = unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                value.as_ptr(),
                value.len() as CFIndex,
                CF_STRING_ENCODING_UTF8,
                0,
            )
        };
        CfOwned::new(string.cast(), profile)
    }

    fn cf_data(profile: AppleKeychainProfile, bytes: &[u8]) -> Result<CfOwned, CryptoError> {
        validate_non_empty_bytes("cf_data", bytes)?;
        let data = unsafe { CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex) };
        CfOwned::new(data.cast(), profile)
    }

    fn cf_number_i32(profile: AppleKeychainProfile, value: i32) -> Result<CfOwned, CryptoError> {
        let number = unsafe {
            CFNumberCreate(
                ptr::null(),
                CF_NUMBER_SINT32_TYPE,
                (&value as *const i32).cast::<c_void>(),
            )
        };
        CfOwned::new(number.cast(), profile)
    }

    fn cf_dictionary(
        profile: AppleKeychainProfile,
        entries: &[(CFTypeRef, CFTypeRef)],
    ) -> Result<CfOwned, CryptoError> {
        let keys: Vec<CFTypeRef> = entries.iter().map(|(key, _)| *key).collect();
        let values: Vec<CFTypeRef> = entries.iter().map(|(_, value)| *value).collect();
        let dictionary = unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                entries.len() as CFIndex,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
        };
        CfOwned::new(dictionary.cast(), profile)
    }

    fn cf_data_bytes(data: &CfOwned) -> Result<Vec<u8>, CryptoError> {
        let len = unsafe { CFDataGetLength(data.as_data()) };
        if len < 0 {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: "unknown".to_owned(),
            });
        }
        let ptr = unsafe { CFDataGetBytePtr(data.as_data()) };
        if ptr.is_null() {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: "unknown".to_owned(),
            });
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        Ok(bytes.to_vec())
    }

    fn cf_error_status(error: CFErrorRef) -> Option<OSStatus> {
        if error.is_null() {
            return None;
        }
        let code = unsafe { CFErrorGetCode(error) };
        i32::try_from(code).ok()
    }

    fn release_error(error: CFErrorRef) {
        if !error.is_null() {
            unsafe {
                CFRelease(error.cast());
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn p256_der_signature_is_converted_to_fixed_width_p1363() {
            let raw = [1u8; 64];
            let signature = P256Signature::from_slice(&raw).expect("valid synthetic scalars");
            let normalized =
                normalize_signature(AppleKeychainProfile::P256V1, signature.to_der().as_bytes())
                    .expect("strict DER conversion");
            assert_eq!(normalized, raw);
            assert!(normalize_signature(AppleKeychainProfile::P256V1, b"not-der").is_err());
        }

        #[test]
        fn p256_status_codes_map_to_structured_fail_closed_errors() {
            let profile = AppleKeychainProfile::P256V1;
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_ITEM_NOT_FOUND),
                CryptoError::PrivateKeyUnavailable { .. }
            ));
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_INTERACTION_NOT_ALLOWED),
                CryptoError::PrivateKeyLocked { .. }
            ));
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_AUTH_FAILED),
                CryptoError::PrivateKeyAccessDenied { .. }
            ));
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_MISSING_ENTITLEMENT),
                CryptoError::PrivateKeyAccessDenied { .. }
            ));
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_RESTRICTED_API),
                CryptoError::PrivateKeyAccessDenied { .. }
            ));
            assert!(matches!(
                map_status(profile, "synthetic-key", ERR_SEC_INTERACTION_REQUIRED),
                CryptoError::PrivateKeyUserPresenceRequired { .. }
            ));
            assert_eq!(
                map_status(profile, "synthetic-key", ERR_SEC_NOT_AVAILABLE),
                CryptoError::StorageBackendUnavailable {
                    backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_P256_V1.to_owned(),
                }
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    pub(super) fn backend_status(profile: AppleKeychainProfile) -> DevicePrivateKeyStoreStatus {
        match profile {
            AppleKeychainProfile::Ed25519V1 => DevicePrivateKeyStoreStatus::apple_keychain_v1(),
            AppleKeychainProfile::P256V1 => DevicePrivateKeyStoreStatus::apple_keychain_p256_v1(),
            AppleKeychainProfile::SecureEnclaveP256V1 => {
                DevicePrivateKeyStoreStatus::apple_secure_enclave_p256_v1()
            }
        }
    }

    pub(super) fn create_signing_key(
        store: &impl AppleKeychainStore,
        _device_id: &str,
        _signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: store.profile().storage_backend_id().to_owned(),
        })
    }

    pub(super) fn handle(
        _store: &impl AppleKeychainStore,
        _device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        Err(CryptoError::PrivateKeyUnavailable {
            key_id: signing_key_id.to_owned(),
        })
    }

    pub(super) fn public_key(
        store: &impl AppleKeychainStore,
        _handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: store.profile().storage_backend_id().to_owned(),
        })
    }

    pub(super) fn sign(
        store: &impl AppleKeychainStore,
        _handle: &DeviceSigningKeyHandle,
        _canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: store.profile().storage_backend_id().to_owned(),
        })
    }

    pub(super) fn verify_private_key_non_exportable(
        store: &impl AppleKeychainStore,
        _handle: &DeviceSigningKeyHandle,
    ) -> Result<(), CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: store.profile().storage_backend_id().to_owned(),
        })
    }

    pub(super) fn delete_or_revoke(
        store: &impl AppleKeychainStore,
        _handle: &DeviceSigningKeyHandle,
        _revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: store.profile().storage_backend_id().to_owned(),
        })
    }
}

pub struct AppleKeychainDeviceKeyStore {
    service: String,
    label: String,
    revoked_keys: Mutex<BTreeSet<(String, String)>>,
}

impl AppleKeychainDeviceKeyStore {
    pub fn new() -> Self {
        Self {
            service: DEFAULT_KEYCHAIN_SERVICE.to_owned(),
            label: DEFAULT_KEYCHAIN_LABEL.to_owned(),
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
        platform::backend_status(AppleKeychainProfile::Ed25519V1)
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

impl AppleKeychainStore for AppleKeychainDeviceKeyStore {
    fn profile(&self) -> AppleKeychainProfile {
        AppleKeychainProfile::Ed25519V1
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn key_tag(&self, signing_key_id: &str) -> Vec<u8> {
        AppleKeychainDeviceKeyStore::key_tag(self, signing_key_id)
    }

    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError> {
        AppleKeychainDeviceKeyStore::ensure_not_revoked(self, device_id, signing_key_id)
    }

    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        AppleKeychainDeviceKeyStore::mark_revoked(self, device_id, signing_key_id, revoked_at_ms)
    }
}

impl Default for AppleKeychainDeviceKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AppleKeychainDeviceKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let revoked_key_count = self
            .revoked_keys
            .lock()
            .map(|revoked_keys| revoked_keys.len())
            .unwrap_or_default();
        f.debug_struct("AppleKeychainDeviceKeyStore")
            .field("storage_backend", &DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1)
            .field("revoked_key_count", &revoked_key_count)
            .finish()
    }
}

pub struct AppleKeychainP256DeviceKeyStore {
    service: String,
    label: String,
    revoked_keys: Mutex<BTreeSet<(String, String)>>,
}

impl AppleKeychainP256DeviceKeyStore {
    pub fn new() -> Self {
        Self {
            service: DEFAULT_P256_KEYCHAIN_SERVICE.to_owned(),
            label: DEFAULT_P256_KEYCHAIN_LABEL.to_owned(),
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
        platform::backend_status(AppleKeychainProfile::P256V1)
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

impl AppleKeychainStore for AppleKeychainP256DeviceKeyStore {
    fn profile(&self) -> AppleKeychainProfile {
        AppleKeychainProfile::P256V1
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn key_tag(&self, signing_key_id: &str) -> Vec<u8> {
        AppleKeychainP256DeviceKeyStore::key_tag(self, signing_key_id)
    }

    fn ensure_not_revoked(&self, device_id: &str, signing_key_id: &str) -> Result<(), CryptoError> {
        AppleKeychainP256DeviceKeyStore::ensure_not_revoked(self, device_id, signing_key_id)
    }

    fn mark_revoked(
        &self,
        device_id: &str,
        signing_key_id: &str,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        AppleKeychainP256DeviceKeyStore::mark_revoked(
            self,
            device_id,
            signing_key_id,
            revoked_at_ms,
        )
    }
}

impl Default for AppleKeychainP256DeviceKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AppleKeychainP256DeviceKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let revoked_key_count = self
            .revoked_keys
            .lock()
            .map(|revoked_keys| revoked_keys.len())
            .unwrap_or_default();
        f.debug_struct("AppleKeychainP256DeviceKeyStore")
            .field("storage_backend", &DEVICE_KEY_STORE_APPLE_KEYCHAIN_P256_V1)
            .field(
                "signature_algorithm",
                &SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1,
            )
            .field("revoked_key_count", &revoked_key_count)
            .finish()
    }
}
