#[cfg(feature = "apple-keychain")]
mod apple_key_agreement;
mod device;
mod epoch_material;
mod model;
mod recovery;
mod signing;

#[cfg(feature = "apple-keychain")]
pub use apple_key_agreement::{
    AppleSecureEnclaveP256KeyAgreementStore, APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
};
pub use device::{
    DeviceKeyDescriptor, DeviceWrappingKeyMaterial, DeviceWrappingRecord, RecoveryAssociatedData,
    RecoveryMaterial,
};
pub use epoch_material::{
    DeviceKeyAgreementKeyHandle, DeviceKeyAgreementPublicKey, EcdhSharedSecret,
    WrappedEpochMaterial, KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1, MAX_WRAPPED_EPOCH_MATERIAL_BYTES,
    P256_KEY_AGREEMENT_PUBLIC_KEY_LEN,
    WRAPPED_EPOCH_ALGORITHM_P256_ECDH_HKDF_SHA256_XCHACHA20POLY1305_V1,
    WRAPPED_EPOCH_SCHEMA_VERSION,
};
pub use model::{
    AlgorithmId, AssociatedData, CiphertextHash, CryptoError, CryptoObjectType,
    EncryptedObjectEnvelope, KeyDescriptor, KeyRole, Nonce, NonceTracker, ObjectKeyMaterial,
    PlaintextPayload, PrivateKeyAccessDeniedReason, SyncMasterKeyMaterial,
    ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256, ENVELOPE_SCHEMA_VERSION, XCHACHA20POLY1305_NONCE_LEN,
};
pub use recovery::{
    RecoveryCode, RecoveryKdfProfile, RecoveryWrappingKeyMaterial,
    RECOVERY_ACTIVATION_ALGORITHM_ED25519_V1, RECOVERY_CODE_PREFIX, RECOVERY_CODE_SECRET_LEN,
    RECOVERY_KDF_ID_ARGON2ID_V1, RECOVERY_KDF_VERSION_ARGON2ID_V1,
    RECOVERY_RECORD_SCHEMA_VERSION_V2, RECOVERY_SALT_LEN, RECOVERY_WRAPPING_KEY_LEN,
};
#[cfg(feature = "android-keystore")]
pub use signing::{
    android_keystore_alias, validate_android_keystore_public_key,
    validate_android_keystore_signature, AndroidKeystoreBridgeErrorCode,
    AndroidKeystoreBridgeOperation, AndroidKeystoreBridgeRequest, AndroidKeystoreDeviceKeyStore,
    ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION, ANDROID_KEYSTORE_JNI_BRIDGE_CLASS,
    ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR,
    ANDROID_KEYSTORE_JNI_CREATE_SIGNING_KEY_METHOD, ANDROID_KEYSTORE_JNI_DELETE_SIGNING_KEY_METHOD,
    ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD,
    ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD, ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD,
    ANDROID_KEYSTORE_JNI_KEY_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_JNI_LOAD_PUBLIC_KEY_METHOD,
    ANDROID_KEYSTORE_JNI_RESULT_CLASS, ANDROID_KEYSTORE_JNI_SIGN_METHOD,
    ANDROID_KEYSTORE_JNI_SIGN_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_PROVIDER,
    ANDROID_KEYSTORE_SIGNATURE_ALGORITHM, DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX,
};
pub use signing::{
    canonical_signature_bytes, verify_device_signature, DevicePrivateKeyStoreStatus,
    DeviceSignature, DeviceSigningBackendCapabilities, DeviceSigningKeyHandle,
    DeviceSigningPublicKey, DeviceSigningStorageBackend, RecoveredDeviceActivationManifest,
    SignatureAlgorithmId, SignatureField, SignedRecoveredDeviceActivation,
    SignedRecoveryRecordManifest, SignedSyncObjectManifest, TestMemoryDeviceKeyStore,
    UnavailableDeviceKeyStore, DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
    DEVICE_KEY_STORE_APPLE_KEYCHAIN_P256_V1, DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1,
    DEVICE_KEY_STORE_APPLE_SECURE_ENCLAVE_P256_V1, DEVICE_KEY_STORE_LINUX_SECRET_SERVICE_V1,
    DEVICE_KEY_STORE_TEST_MEMORY_V1, DEVICE_KEY_STORE_UNAVAILABLE, DEVICE_KEY_STORE_WINDOWS_CNG_V1,
    ED25519_PUBLIC_KEY_LEN, ED25519_SIGNATURE_LEN, P256_PUBLIC_KEY_LEN, P256_SIGNATURE_LEN,
    SIGNATURE_ALGORITHM_ECDSA_P256_SHA256_V1, SIGNATURE_ALGORITHM_ED25519_V1,
    SIGNATURE_SCHEMA_VERSION,
};
#[cfg(feature = "apple-keychain")]
pub use signing::{
    AppleKeychainDeviceKeyStore, AppleKeychainP256DeviceKeyStore,
    AppleSecureEnclaveP256DeviceKeyStore,
};
