use std::ffi::c_void;
use std::fmt;
use std::ptr;

use crate::{
    CryptoError, DeviceKeyAgreementKeyHandle, DeviceKeyAgreementPublicKey, EcdhSharedSecret,
    PrivateKeyAccessDeniedReason, P256_KEY_AGREEMENT_PUBLIC_KEY_LEN,
};

pub const APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND: &str =
    "apple-secure-enclave-p256-key-agreement-v1";

const DEFAULT_SERVICE: &str = "local.radishlex.sync.key-agreement.secure-enclave.p256.v1";
const DEFAULT_LABEL: &str = "RadishLex Secure Enclave P-256 Key Agreement Key";

pub struct AppleSecureEnclaveP256KeyAgreementStore {
    service: String,
    label: String,
}

impl AppleSecureEnclaveP256KeyAgreementStore {
    pub fn new() -> Self {
        Self {
            service: DEFAULT_SERVICE.to_owned(),
            label: DEFAULT_LABEL.to_owned(),
        }
    }

    pub fn with_service(
        service: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Self, CryptoError> {
        let service = service.into();
        let label = label.into();
        validate_text("key_agreement_service", &service)?;
        validate_text("key_agreement_label", &label)?;
        Ok(Self { service, label })
    }

    pub fn create_key(
        &self,
        device_id: &str,
        key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        platform::create_key(self, device_id, key_id, created_at_ms)
    }

    pub fn handle(
        &self,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        platform::handle(self, device_id, key_id)
    }

    pub fn public_key(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        platform::public_key(self, handle)
    }

    pub fn derive_shared_secret(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        platform::derive_shared_secret(self, handle, peer_public_key)
    }

    pub fn delete_key(&self, handle: &DeviceKeyAgreementKeyHandle) -> Result<(), CryptoError> {
        platform::delete_key(self, handle)
    }

    fn key_tag(&self, key_id: &str) -> Vec<u8> {
        format!("{}:{key_id}", self.service).into_bytes()
    }
}

impl Default for AppleSecureEnclaveP256KeyAgreementStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AppleSecureEnclaveP256KeyAgreementStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppleSecureEnclaveP256KeyAgreementStore")
            .field(
                "storage_backend",
                &APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
            )
            .finish()
    }
}

fn validate_text(field: &'static str, value: &str) -> Result<(), CryptoError> {
    if value.trim().is_empty() {
        return Err(CryptoError::InvalidField {
            field,
            message: "value cannot be empty".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    type Boolean = u8;
    type CFIndex = isize;
    type OSStatus = i32;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFDataRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFErrorRef = *const c_void;
    type SecKeyRef = *const c_void;
    type SecKeyAlgorithm = CFStringRef;
    type SecAccessControlRef = *const c_void;
    type SecAccessControlCreateFlags = usize;

    #[repr(C)]
    struct CFDictionaryKeyCallBacks {
        version: CFIndex,
        retain: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void) -> *const c_void>,
        release: Option<unsafe extern "C" fn(CFAllocatorRef, *const c_void)>,
        copy_description: Option<unsafe extern "C" fn(*const c_void) -> CFStringRef>,
        equal: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> Boolean>,
        hash: Option<unsafe extern "C" fn(*const c_void) -> usize>,
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
        static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
        static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;
        fn CFRelease(value: CFTypeRef);
        fn CFStringCreateWithBytes(
            allocator: CFAllocatorRef,
            bytes: *const u8,
            count: CFIndex,
            encoding: u32,
            external: Boolean,
        ) -> CFStringRef;
        fn CFDataCreate(allocator: CFAllocatorRef, bytes: *const u8, length: CFIndex) -> CFDataRef;
        fn CFDataGetLength(data: CFDataRef) -> CFIndex;
        fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
        fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            count: CFIndex,
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
        static kSecAttrAccessControl: CFStringRef;
        static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: CFStringRef;
        static kSecAttrApplicationTag: CFStringRef;
        static kSecAttrIsPermanent: CFStringRef;
        static kSecAttrKeyClass: CFStringRef;
        static kSecAttrKeyClassPrivate: CFStringRef;
        static kSecAttrKeyClassPublic: CFStringRef;
        static kSecAttrKeyType: CFStringRef;
        static kSecAttrKeyTypeECSECPrimeRandom: CFStringRef;
        static kSecAttrLabel: CFStringRef;
        static kSecAttrTokenID: CFStringRef;
        static kSecAttrTokenIDSecureEnclave: CFStringRef;
        static kSecMatchLimit: CFStringRef;
        static kSecMatchLimitOne: CFStringRef;
        static kSecPrivateKeyAttrs: CFStringRef;
        static kSecReturnRef: CFStringRef;
        static kSecUseDataProtectionKeychain: CFStringRef;
        static kSecKeyAlgorithmECDHKeyExchangeStandard: SecKeyAlgorithm;

        fn SecAccessControlCreateWithFlags(
            allocator: CFAllocatorRef,
            protection: CFTypeRef,
            flags: SecAccessControlCreateFlags,
            error: *mut CFErrorRef,
        ) -> SecAccessControlRef;
        fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
        fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;
        fn SecKeyCreateRandomKey(parameters: CFDictionaryRef, error: *mut CFErrorRef) -> SecKeyRef;
        fn SecKeyCopyPublicKey(key: SecKeyRef) -> SecKeyRef;
        fn SecKeyCopyExternalRepresentation(key: SecKeyRef, error: *mut CFErrorRef) -> CFDataRef;
        fn SecKeyCreateWithData(
            key_data: CFDataRef,
            attributes: CFDictionaryRef,
            error: *mut CFErrorRef,
        ) -> SecKeyRef;
        fn SecKeyCopyKeyExchangeResult(
            private_key: SecKeyRef,
            algorithm: SecKeyAlgorithm,
            public_key: SecKeyRef,
            parameters: CFDictionaryRef,
            error: *mut CFErrorRef,
        ) -> CFDataRef;
    }

    struct CfOwned(CFTypeRef);

    impl CfOwned {
        fn new(value: CFTypeRef, key_id: &str) -> Result<Self, CryptoError> {
            if value.is_null() {
                Err(CryptoError::PrivateKeyUnavailable {
                    key_id: key_id.to_owned(),
                })
            } else {
                Ok(Self(value))
            }
        }

        fn dictionary(&self) -> CFDictionaryRef {
            self.0.cast()
        }

        fn data(&self) -> CFDataRef {
            self.0.cast()
        }

        fn key(&self) -> SecKeyRef {
            self.0.cast()
        }
    }

    impl Drop for CfOwned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    pub(super) fn create_key(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        device_id: &str,
        key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        validate_text("device_id", device_id)?;
        validate_text("key_agreement_key_id", key_id)?;
        let tag = data(&store.key_tag(key_id), key_id)?;
        let label = string(&store.label, key_id)?;
        let access_control = access_control(key_id)?;
        let private_attrs = dictionary(
            &[
                (unsafe { kSecAttrIsPermanent }, unsafe { kCFBooleanTrue }),
                (unsafe { kSecAttrApplicationTag }, tag.0),
                (unsafe { kSecAttrLabel }, label.0),
                (unsafe { kSecAttrAccessControl }, access_control.0),
            ],
            key_id,
        )?;
        let parameters = dictionary(
            &[
                (unsafe { kSecAttrKeyType }, unsafe {
                    kSecAttrKeyTypeECSECPrimeRandom
                }),
                (unsafe { kSecPrivateKeyAttrs }, private_attrs.0),
                (unsafe { kSecAttrTokenID }, unsafe {
                    kSecAttrTokenIDSecureEnclave
                }),
                (unsafe { kSecUseDataProtectionKeychain }, unsafe {
                    kCFBooleanTrue
                }),
            ],
            key_id,
        )?;
        let mut error = ptr::null();
        let private = unsafe { SecKeyCreateRandomKey(parameters.dictionary(), &mut error) };
        let private = owned_or_error(private.cast(), error, key_id)?;
        public_from_private(device_id, key_id, created_at_ms, &private)
    }

    pub(super) fn handle(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        validate_text("device_id", device_id)?;
        let _private = load_private(store, key_id)?;
        DeviceKeyAgreementKeyHandle::p256(
            device_id,
            key_id,
            APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
        )
    }

    pub(super) fn public_key(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        validate_handle(handle)?;
        let private = load_private(store, &handle.key_id)?;
        public_from_private(&handle.device_id, &handle.key_id, 0, &private)
    }

    pub(super) fn derive_shared_secret(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        validate_handle(handle)?;
        let _ = DeviceKeyAgreementPublicKey::p256("peer", "ephemeral", peer_public_key, 0, None)?;
        let private = load_private(store, &handle.key_id)?;
        let peer_data = data(peer_public_key, &handle.key_id)?;
        let peer_attributes = dictionary(
            &[
                (unsafe { kSecAttrKeyType }, unsafe {
                    kSecAttrKeyTypeECSECPrimeRandom
                }),
                (unsafe { kSecAttrKeyClass }, unsafe {
                    kSecAttrKeyClassPublic
                }),
            ],
            &handle.key_id,
        )?;
        let mut error = ptr::null();
        let peer = unsafe {
            SecKeyCreateWithData(peer_data.data(), peer_attributes.dictionary(), &mut error)
        };
        let peer = owned_or_error(peer.cast(), error, &handle.key_id)?;
        let empty = dictionary(&[], &handle.key_id)?;
        let mut error = ptr::null();
        let result = unsafe {
            SecKeyCopyKeyExchangeResult(
                private.key(),
                kSecKeyAlgorithmECDHKeyExchangeStandard,
                peer.key(),
                empty.dictionary(),
                &mut error,
            )
        };
        let result = owned_or_error(result.cast(), error, &handle.key_id)?;
        let bytes = data_bytes(&result, &handle.key_id)?;
        let shared: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CryptoError::PrivateKeyCorrupted {
                key_id: handle.key_id.clone(),
            })?;
        EcdhSharedSecret::new(shared)
    }

    pub(super) fn delete_key(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<(), CryptoError> {
        validate_handle(handle)?;
        let query = key_query(store, &handle.key_id, false)?;
        match unsafe { SecItemDelete(query.dictionary()) } {
            ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            status => Err(map_status(&handle.key_id, status)),
        }
    }

    fn public_from_private(
        device_id: &str,
        key_id: &str,
        created_at_ms: i64,
        private: &CfOwned,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        let public = unsafe { SecKeyCopyPublicKey(private.key()) };
        let public = CfOwned::new(public.cast(), key_id)?;
        let mut error = ptr::null();
        let bytes = unsafe { SecKeyCopyExternalRepresentation(public.key(), &mut error) };
        let bytes = owned_or_error(bytes.cast(), error, key_id)?;
        let bytes = data_bytes(&bytes, key_id)?;
        if bytes.len() != P256_KEY_AGREEMENT_PUBLIC_KEY_LEN {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: key_id.to_owned(),
            });
        }
        DeviceKeyAgreementPublicKey::p256(device_id, key_id, bytes, created_at_ms, None)
    }

    fn load_private(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let query = key_query(store, key_id, true)?;
        let mut result = ptr::null();
        let status = unsafe { SecItemCopyMatching(query.dictionary(), &mut result) };
        if status != ERR_SEC_SUCCESS {
            return Err(map_status(key_id, status));
        }
        CfOwned::new(result, key_id)
    }

    fn key_query(
        store: &AppleSecureEnclaveP256KeyAgreementStore,
        key_id: &str,
        return_ref: bool,
    ) -> Result<CfOwned, CryptoError> {
        let tag = data(&store.key_tag(key_id), key_id)?;
        let mut entries = vec![
            (unsafe { kSecClass }, unsafe { kSecClassKey }),
            (unsafe { kSecAttrKeyType }, unsafe {
                kSecAttrKeyTypeECSECPrimeRandom
            }),
            (unsafe { kSecAttrKeyClass }, unsafe {
                kSecAttrKeyClassPrivate
            }),
            (unsafe { kSecAttrApplicationTag }, tag.0),
            (unsafe { kSecAttrTokenID }, unsafe {
                kSecAttrTokenIDSecureEnclave
            }),
            (unsafe { kSecUseDataProtectionKeychain }, unsafe {
                kCFBooleanTrue
            }),
            (unsafe { kSecMatchLimit }, unsafe { kSecMatchLimitOne }),
        ];
        if return_ref {
            entries.push((unsafe { kSecReturnRef }, unsafe { kCFBooleanTrue }));
        }
        dictionary(&entries, key_id)
    }

    fn access_control(key_id: &str) -> Result<CfOwned, CryptoError> {
        let mut error = ptr::null();
        let value = unsafe {
            SecAccessControlCreateWithFlags(
                ptr::null(),
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                SEC_ACCESS_CONTROL_PRIVATE_KEY_USAGE,
                &mut error,
            )
        };
        owned_or_error(value.cast(), error, key_id)
    }

    fn validate_handle(handle: &DeviceKeyAgreementKeyHandle) -> Result<(), CryptoError> {
        handle.validate()?;
        if handle.storage_backend != APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: handle.storage_backend.clone(),
                message: "key-agreement handle uses another backend".to_owned(),
            });
        }
        Ok(())
    }

    fn string(value: &str, key_id: &str) -> Result<CfOwned, CryptoError> {
        let value = unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                value.as_ptr(),
                value.len() as CFIndex,
                CF_STRING_ENCODING_UTF8,
                0,
            )
        };
        CfOwned::new(value.cast(), key_id)
    }

    fn data(value: &[u8], key_id: &str) -> Result<CfOwned, CryptoError> {
        if value.is_empty() {
            return Err(CryptoError::InvalidField {
                field: "key_agreement_bytes",
                message: "value cannot be empty".to_owned(),
            });
        }
        let value = unsafe { CFDataCreate(ptr::null(), value.as_ptr(), value.len() as CFIndex) };
        CfOwned::new(value.cast(), key_id)
    }

    fn dictionary(
        entries: &[(CFTypeRef, CFTypeRef)],
        key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let (keys, values): (Vec<_>, Vec<_>) = entries.iter().copied().unzip();
        let value = unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                entries.len() as CFIndex,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
        };
        CfOwned::new(value.cast(), key_id)
    }

    fn data_bytes(value: &CfOwned, key_id: &str) -> Result<Vec<u8>, CryptoError> {
        let len = unsafe { CFDataGetLength(value.data()) };
        if len <= 0 {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: key_id.to_owned(),
            });
        }
        let ptr = unsafe { CFDataGetBytePtr(value.data()) };
        if ptr.is_null() {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: key_id.to_owned(),
            });
        }
        Ok(unsafe { std::slice::from_raw_parts(ptr, len as usize) }.to_vec())
    }

    fn owned_or_error(
        value: CFTypeRef,
        error: CFErrorRef,
        key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        if !value.is_null() {
            release_error(error);
            return Ok(CfOwned(value));
        }
        let status = if error.is_null() {
            ERR_SEC_NOT_AVAILABLE
        } else {
            unsafe { CFErrorGetCode(error) as OSStatus }
        };
        release_error(error);
        Err(map_status(key_id, status))
    }

    fn release_error(error: CFErrorRef) {
        if !error.is_null() {
            unsafe { CFRelease(error.cast()) };
        }
    }

    fn map_status(key_id: &str, status: OSStatus) -> CryptoError {
        match status {
            ERR_SEC_ITEM_NOT_FOUND => CryptoError::PrivateKeyUnavailable {
                key_id: key_id.to_owned(),
            },
            ERR_SEC_NOT_AVAILABLE | ERR_SEC_UNIMPLEMENTED | ERR_SEC_PARAM => {
                CryptoError::StorageBackendUnavailable {
                    backend: APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND.to_owned(),
                }
            }
            ERR_SEC_INTERACTION_NOT_ALLOWED => CryptoError::PrivateKeyLocked {
                key_id: key_id.to_owned(),
            },
            ERR_SEC_INTERACTION_REQUIRED => CryptoError::PrivateKeyUserPresenceRequired {
                key_id: key_id.to_owned(),
            },
            ERR_SEC_AUTH_FAILED => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::AuthenticationFailed,
            },
            ERR_SEC_WR_PERM => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::WritePermission,
            },
            ERR_SEC_READ_ONLY => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::ReadOnly,
            },
            ERR_SEC_MISSING_ENTITLEMENT => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::MissingEntitlement,
            },
            ERR_SEC_RESTRICTED_API => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::RestrictedApi,
            },
            ERR_SEC_DECODE => CryptoError::PrivateKeyCorrupted {
                key_id: key_id.to_owned(),
            },
            other => CryptoError::PrivateKeyAccessDenied {
                key_id: key_id.to_owned(),
                reason: PrivateKeyAccessDeniedReason::UnclassifiedPlatformStatus(other),
            },
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    fn unavailable() -> CryptoError {
        CryptoError::StorageBackendUnavailable {
            backend: APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND.to_owned(),
        }
    }

    pub(super) fn create_key(
        _store: &AppleSecureEnclaveP256KeyAgreementStore,
        _device_id: &str,
        _key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        Err(unavailable())
    }

    pub(super) fn handle(
        _store: &AppleSecureEnclaveP256KeyAgreementStore,
        _device_id: &str,
        _key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        Err(unavailable())
    }

    pub(super) fn public_key(
        _store: &AppleSecureEnclaveP256KeyAgreementStore,
        _handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        Err(unavailable())
    }

    pub(super) fn derive_shared_secret(
        _store: &AppleSecureEnclaveP256KeyAgreementStore,
        _handle: &DeviceKeyAgreementKeyHandle,
        _peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        Err(unavailable())
    }

    pub(super) fn delete_key(
        _store: &AppleSecureEnclaveP256KeyAgreementStore,
        _handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<(), CryptoError> {
        Err(unavailable())
    }
}
