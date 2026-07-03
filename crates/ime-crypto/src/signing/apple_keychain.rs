use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

use crate::model::{validate_non_empty_bytes, validate_required, CryptoError};

use super::{
    DevicePrivateKeyStoreStatus, DeviceSignature, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    DeviceSigningStorageBackend, SignatureAlgorithmId, DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1,
    ED25519_PUBLIC_KEY_LEN, ED25519_SIGNATURE_LEN, SIGNATURE_ALGORITHM_ED25519_V1,
};

const DEFAULT_KEYCHAIN_SERVICE: &str = "org.radishlex.sync.signing";
const DEFAULT_KEYCHAIN_LABEL: &str = "RadishLex Device Signing Key";

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
    type OSStatus = i32;
    type SecKeyAlgorithm = CFStringRef;
    type SecKeyOperationType = CFIndex;
    type CFHashCode = usize;

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
    const ED25519_KEY_SIZE_BITS: i32 = 256;

    const ERR_SEC_SUCCESS: OSStatus = 0;
    const ERR_SEC_UNIMPLEMENTED: OSStatus = -4;
    const ERR_SEC_PARAM: OSStatus = -50;
    const ERR_SEC_NOT_AVAILABLE: OSStatus = -25291;
    const ERR_SEC_AUTH_FAILED: OSStatus = -25293;
    const ERR_SEC_ITEM_NOT_FOUND: OSStatus = -25300;
    const ERR_SEC_INTERACTION_NOT_ALLOWED: OSStatus = -25308;
    const ERR_SEC_DECODE: OSStatus = -26275;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
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
    }

    #[allow(non_upper_case_globals)]
    #[link(name = "Security", kind = "framework")]
    extern "C" {
        static kSecClass: CFStringRef;
        static kSecClassKey: CFStringRef;
        static kSecAttrApplicationTag: CFStringRef;
        static kSecAttrComment: CFStringRef;
        static kSecAttrIsPermanent: CFStringRef;
        static kSecAttrKeyClass: CFStringRef;
        static kSecAttrKeyClassPrivate: CFStringRef;
        static kSecAttrKeySizeInBits: CFStringRef;
        static kSecAttrKeyType: CFStringRef;
        static kSecAttrKeyTypeEd25519: CFStringRef;
        static kSecAttrLabel: CFStringRef;
        static kSecMatchLimit: CFStringRef;
        static kSecMatchLimitOne: CFStringRef;
        static kSecPrivateKeyAttrs: CFStringRef;
        static kSecReturnRef: CFStringRef;
        static kSecKeyAlgorithmEdDSASignatureMessageCurve25519SHA512: CFStringRef;

        fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> OSStatus;
        fn SecItemDelete(query: CFDictionaryRef) -> OSStatus;
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
        fn new(ptr: CFTypeRef) -> Result<Self, CryptoError> {
            if ptr.is_null() {
                return Err(CryptoError::StorageBackendUnavailable {
                    backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
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

    pub(super) fn backend_status() -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::apple_keychain_v1()
    }

    pub(super) fn create_signing_key(
        store: &AppleKeychainDeviceKeyStore,
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        store.ensure_not_revoked(device_id, signing_key_id)?;

        let private_key = create_private_key(store, signing_key_id, created_at_ms)?;
        public_key_from_private(device_id, signing_key_id, created_at_ms, None, &private_key)
    }

    pub(super) fn handle(
        store: &AppleKeychainDeviceKeyStore,
        device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        validate_required("device_id", device_id)?;
        validate_required("signing_key_id", signing_key_id)?;
        store.ensure_not_revoked(device_id, signing_key_id)?;
        let _private_key = load_private_key(store, signing_key_id)?;
        DeviceSigningKeyHandle::apple_keychain(device_id, signing_key_id, 0)
    }

    pub(super) fn public_key(
        store: &AppleKeychainDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        validate_apple_handle(handle)?;
        store.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let private_key = load_private_key(store, &handle.signing_key_id)?;
        public_key_from_private(
            &handle.device_id,
            &handle.signing_key_id,
            handle.created_at_ms,
            handle.revoked_at_ms,
            &private_key,
        )
    }

    pub(super) fn sign(
        store: &AppleKeychainDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
        canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        validate_apple_handle(handle)?;
        validate_non_empty_bytes("canonical_bytes", canonical_bytes)?;
        store.ensure_not_revoked(&handle.device_id, &handle.signing_key_id)?;
        let private_key = load_private_key(store, &handle.signing_key_id)?;
        let algorithm = unsafe { kSecKeyAlgorithmEdDSASignatureMessageCurve25519SHA512 };
        let is_supported = unsafe {
            SecKeyIsAlgorithmSupported(private_key.as_key(), SEC_KEY_OPERATION_SIGN, algorithm)
        };
        if is_supported == 0 {
            return Err(CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            });
        }

        let data_to_sign = cf_data(canonical_bytes)?;
        let mut error = ptr::null();
        let signature_data = unsafe {
            SecKeyCreateSignature(
                private_key.as_key(),
                algorithm,
                data_to_sign.as_data(),
                &mut error,
            )
        };
        release_error(error);
        let signature_data = CfOwned::new(signature_data.cast()).map_err(|_| {
            CryptoError::PrivateKeyAccessDenied {
                key_id: handle.signing_key_id.clone(),
            }
        })?;
        let signature = cf_data_bytes(&signature_data)?;
        if signature.len() != ED25519_SIGNATURE_LEN {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: handle.signing_key_id.clone(),
            });
        }
        DeviceSignature::new(
            handle.signing_key_id.clone(),
            handle.device_id.clone(),
            signature,
        )
    }

    pub(super) fn delete_or_revoke(
        store: &AppleKeychainDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
        revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        validate_apple_handle(handle)?;
        let query = key_delete_query(store, &handle.signing_key_id)?;
        let status = unsafe { SecItemDelete(query.as_dictionary()) };
        store.mark_revoked(&handle.device_id, &handle.signing_key_id, revoked_at_ms)?;
        match status {
            ERR_SEC_SUCCESS | ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            other => Err(map_status(&handle.signing_key_id, other)),
        }
    }

    fn create_private_key(
        store: &AppleKeychainDeviceKeyStore,
        signing_key_id: &str,
        created_at_ms: i64,
    ) -> Result<CfOwned, CryptoError> {
        let tag = cf_data(&store.key_tag(signing_key_id))?;
        let label = cf_string(&store.label)?;
        let created_at = cf_string(&created_at_ms.to_string())?;
        let key_size = cf_number_i32(ED25519_KEY_SIZE_BITS)?;
        let private_attrs = cf_dictionary(&[
            (unsafe { kSecAttrIsPermanent }, unsafe { kCFBooleanTrue }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
            (unsafe { kSecAttrLabel }, label.as_type()),
            (unsafe { kSecAttrComment }, created_at.as_type()),
        ])?;
        let parameters = cf_dictionary(&[
            (unsafe { kSecAttrKeyType }, unsafe {
                kSecAttrKeyTypeEd25519
            }),
            (unsafe { kSecAttrKeySizeInBits }, key_size.as_type()),
            (unsafe { kSecPrivateKeyAttrs }, private_attrs.as_type()),
        ])?;

        let mut error = ptr::null();
        let private_key = unsafe { SecKeyCreateRandomKey(parameters.as_dictionary(), &mut error) };
        release_error(error);
        CfOwned::new(private_key.cast()).map_err(|_| CryptoError::UnsupportedSignatureAlgorithm {
            algorithm: SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
        })
    }

    fn load_private_key(
        store: &AppleKeychainDeviceKeyStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let query = key_query(store, signing_key_id)?;
        let mut result = ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_dictionary(), &mut result) };
        if status != ERR_SEC_SUCCESS {
            return Err(map_status(signing_key_id, status));
        }
        CfOwned::new(result)
    }

    fn public_key_from_private(
        device_id: &str,
        signing_key_id: &str,
        created_at_ms: i64,
        revoked_at_ms: Option<i64>,
        private_key: &CfOwned,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        let public_key = unsafe { SecKeyCopyPublicKey(private_key.as_key()) };
        let public_key =
            CfOwned::new(public_key.cast()).map_err(|_| CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            })?;
        let mut error = ptr::null();
        let public_data =
            unsafe { SecKeyCopyExternalRepresentation(public_key.as_key(), &mut error) };
        release_error(error);
        let public_data =
            CfOwned::new(public_data.cast()).map_err(|_| CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            })?;
        let public_bytes = cf_data_bytes(&public_data)?;
        if public_bytes.len() != ED25519_PUBLIC_KEY_LEN {
            return Err(CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            });
        }
        DeviceSigningPublicKey::new(
            device_id,
            signing_key_id,
            SignatureAlgorithmId::ed25519_v1(),
            public_bytes,
            created_at_ms,
            revoked_at_ms,
        )
    }

    fn key_query(
        store: &AppleKeychainDeviceKeyStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let tag = cf_data(&store.key_tag(signing_key_id))?;
        cf_dictionary(&[
            (unsafe { kSecClass }, unsafe { kSecClassKey }),
            (unsafe { kSecAttrKeyType }, unsafe {
                kSecAttrKeyTypeEd25519
            }),
            (unsafe { kSecAttrKeyClass }, unsafe {
                kSecAttrKeyClassPrivate
            }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
            (unsafe { kSecReturnRef }, unsafe { kCFBooleanTrue }),
            (unsafe { kSecMatchLimit }, unsafe { kSecMatchLimitOne }),
        ])
    }

    fn key_delete_query(
        store: &AppleKeychainDeviceKeyStore,
        signing_key_id: &str,
    ) -> Result<CfOwned, CryptoError> {
        let tag = cf_data(&store.key_tag(signing_key_id))?;
        cf_dictionary(&[
            (unsafe { kSecClass }, unsafe { kSecClassKey }),
            (unsafe { kSecAttrKeyType }, unsafe {
                kSecAttrKeyTypeEd25519
            }),
            (unsafe { kSecAttrKeyClass }, unsafe {
                kSecAttrKeyClassPrivate
            }),
            (unsafe { kSecAttrApplicationTag }, tag.as_type()),
        ])
    }

    fn validate_apple_handle(handle: &DeviceSigningKeyHandle) -> Result<(), CryptoError> {
        handle.validate()?;
        if handle.storage_backend != DeviceSigningStorageBackend::AppleKeychainV1 {
            return Err(CryptoError::BackendCapabilityMismatch {
                backend: handle.storage_backend.as_str().to_owned(),
                message: "handle must use apple-keychain-v1 backend".to_owned(),
            });
        }
        Ok(())
    }

    fn map_status(signing_key_id: &str, status: OSStatus) -> CryptoError {
        match status {
            ERR_SEC_ITEM_NOT_FOUND => CryptoError::PrivateKeyUnavailable {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_NOT_AVAILABLE => CryptoError::StorageBackendUnavailable {
                backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
            },
            ERR_SEC_INTERACTION_NOT_ALLOWED => CryptoError::PrivateKeyLocked {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_AUTH_FAILED => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_DECODE => CryptoError::PrivateKeyCorrupted {
                key_id: signing_key_id.to_owned(),
            },
            ERR_SEC_UNIMPLEMENTED | ERR_SEC_PARAM => CryptoError::UnsupportedSignatureAlgorithm {
                algorithm: SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            },
            _ => CryptoError::PrivateKeyAccessDenied {
                key_id: signing_key_id.to_owned(),
            },
        }
    }

    fn cf_string(value: &str) -> Result<CfOwned, CryptoError> {
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
        CfOwned::new(string.cast())
    }

    fn cf_data(bytes: &[u8]) -> Result<CfOwned, CryptoError> {
        validate_non_empty_bytes("cf_data", bytes)?;
        let data = unsafe { CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex) };
        CfOwned::new(data.cast())
    }

    fn cf_number_i32(value: i32) -> Result<CfOwned, CryptoError> {
        let number = unsafe {
            CFNumberCreate(
                ptr::null(),
                CF_NUMBER_SINT32_TYPE,
                (&value as *const i32).cast::<c_void>(),
            )
        };
        CfOwned::new(number.cast())
    }

    fn cf_dictionary(entries: &[(CFTypeRef, CFTypeRef)]) -> Result<CfOwned, CryptoError> {
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
        CfOwned::new(dictionary.cast())
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

    fn release_error(error: CFErrorRef) {
        if !error.is_null() {
            unsafe {
                CFRelease(error.cast());
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    pub(super) fn backend_status() -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus {
            storage_backend: DeviceSigningStorageBackend::AppleKeychainV1,
            available: false,
            can_create_signing_keys: false,
            can_sign: false,
            capabilities: super::super::DeviceSigningBackendCapabilities::apple_keychain_v1(),
        }
    }

    pub(super) fn create_signing_key(
        _store: &AppleKeychainDeviceKeyStore,
        _device_id: &str,
        _signing_key_id: &str,
        _created_at_ms: i64,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
        })
    }

    pub(super) fn handle(
        _store: &AppleKeychainDeviceKeyStore,
        _device_id: &str,
        signing_key_id: &str,
    ) -> Result<DeviceSigningKeyHandle, CryptoError> {
        Err(CryptoError::PrivateKeyUnavailable {
            key_id: signing_key_id.to_owned(),
        })
    }

    pub(super) fn public_key(
        _store: &AppleKeychainDeviceKeyStore,
        _handle: &DeviceSigningKeyHandle,
    ) -> Result<DeviceSigningPublicKey, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
        })
    }

    pub(super) fn sign(
        _store: &AppleKeychainDeviceKeyStore,
        _handle: &DeviceSigningKeyHandle,
        _canonical_bytes: &[u8],
    ) -> Result<DeviceSignature, CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
        })
    }

    pub(super) fn delete_or_revoke(
        _store: &AppleKeychainDeviceKeyStore,
        _handle: &DeviceSigningKeyHandle,
        _revoked_at_ms: i64,
    ) -> Result<(), CryptoError> {
        Err(CryptoError::UnsupportedStorageBackend {
            backend: DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1.to_owned(),
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
        platform::backend_status()
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
