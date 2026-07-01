mod device;
mod model;
mod recovery;
mod signing;

pub use device::{
    DeviceKeyDescriptor, DeviceWrappingKeyMaterial, DeviceWrappingRecord, RecoveryAssociatedData,
    RecoveryMaterial,
};
pub use model::{
    AlgorithmId, AssociatedData, CiphertextHash, CryptoError, CryptoObjectType,
    EncryptedObjectEnvelope, KeyDescriptor, KeyRole, Nonce, NonceTracker, ObjectKeyMaterial,
    PlaintextPayload, SyncMasterKeyMaterial, ALGORITHM_XCHACHA20POLY1305_HKDF_SHA256,
    ENVELOPE_SCHEMA_VERSION, XCHACHA20POLY1305_NONCE_LEN,
};
pub use recovery::{
    RecoveryCode, RecoveryKdfProfile, RecoveryWrappingKeyMaterial, RECOVERY_CODE_PREFIX,
    RECOVERY_CODE_SECRET_LEN, RECOVERY_KDF_ID_ARGON2ID_V1, RECOVERY_KDF_VERSION_ARGON2ID_V1,
    RECOVERY_SALT_LEN, RECOVERY_WRAPPING_KEY_LEN,
};
#[cfg(feature = "apple-keychain")]
pub use signing::AppleKeychainDeviceKeyStore;
#[cfg(feature = "android-keystore")]
pub use signing::{
    android_keystore_alias, validate_android_keystore_public_key,
    validate_android_keystore_signature, AndroidKeystoreBridgeErrorCode,
    AndroidKeystoreBridgeOperation, AndroidKeystoreBridgeRequest, AndroidKeystoreDeviceKeyStore,
    ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION, ANDROID_KEYSTORE_PROVIDER,
    ANDROID_KEYSTORE_SIGNATURE_ALGORITHM, DEFAULT_ANDROID_KEYSTORE_ALIAS_PREFIX,
};
pub use signing::{
    canonical_signature_bytes, DevicePrivateKeyStoreStatus, DeviceSignature,
    DeviceSigningBackendCapabilities, DeviceSigningKeyHandle, DeviceSigningPublicKey,
    DeviceSigningStorageBackend, SignatureAlgorithmId, SignatureField,
    SignedRecoveryRecordManifest, SignedSyncObjectManifest, TestMemoryDeviceKeyStore,
    UnavailableDeviceKeyStore, DEVICE_KEY_STORE_ANDROID_KEYSTORE_V1,
    DEVICE_KEY_STORE_APPLE_KEYCHAIN_V1, DEVICE_KEY_STORE_LINUX_SECRET_SERVICE_V1,
    DEVICE_KEY_STORE_TEST_MEMORY_V1, DEVICE_KEY_STORE_UNAVAILABLE, DEVICE_KEY_STORE_WINDOWS_CNG_V1,
    ED25519_PUBLIC_KEY_LEN, ED25519_SIGNATURE_LEN, SIGNATURE_ALGORITHM_ED25519_V1,
    SIGNATURE_SCHEMA_VERSION,
};
