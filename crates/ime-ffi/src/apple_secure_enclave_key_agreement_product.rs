use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use crate::apple_p256_product::{
    RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE, RADISHLEX_APPLE_P256_ERROR_NONE,
    RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
    RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT, RADISHLEX_APPLE_P256_SMOKE_PASSED,
};

pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_STATUS_VERSION: u32 = 1;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_SMOKE_VERSION: u32 = 1;

pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_LIFECYCLE: u32 = 0;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_DENIED_CREATE: u32 = 1;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_PREPARE_LOCKED_DERIVE: u32 = 2;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_LOCKED_DERIVE: u32 = 3;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_CLEANUP_LOCKED_DERIVE: u32 = 4;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_UNSUPPORTED_CREATE: u32 = 5;

pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CAPABILITY_MISMATCH: u32 = 4;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED: u32 = 5;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_RELOAD_FAILED: u32 = 6;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DERIVE_FAILED: u32 = 7;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED: u32 = 8;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DELETE_FAILED: u32 = 9;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_MISSING_CHECK_FAILED: u32 = 10;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED: u32 = 11;
pub const RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_UNEXPECTED_ERROR_CATEGORY: u32 = 12;

/// Metadata-only key-agreement product status. No platform item API is called.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexAppleSecureEnclaveKeyAgreementProductStatus {
    pub version: u32,
    pub compiled: u32,
    pub runtime_qualified: u32,
    pub product_qualified: u32,
    pub user_sync_enabled: u32,
    pub exportable: u32,
    pub hardware_backed: u32,
    pub user_presence_required: u32,
    pub backup_migratable: u32,
}

/// Fixed redacted result. It never contains identifiers, key bytes, or ciphertext.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    pub version: u32,
    pub result: u32,
    pub scenario: u32,
    pub error_category: u32,
    pub error_detail: u32,
    pub platform_status: i32,
    pub compiled: u32,
    pub runtime_qualified: u32,
    pub product_qualified: u32,
    pub user_sync_enabled: u32,
    pub exportable: u32,
    pub hardware_backed: u32,
    pub user_presence_required: u32,
    pub backup_migratable: u32,
    pub created: u32,
    pub reloaded: u32,
    pub public_key_matched: u32,
    pub shared_secret_derived: u32,
    pub wrapped_epoch_verified: u32,
    pub deleted: u32,
    pub missing_confirmed: u32,
    pub fail_closed: u32,
    pub expected_failure_confirmed: u32,
    pub cleanup_required: u32,
    pub cleanup_attempted: u32,
}

pub(crate) const fn current_status() -> RadishLexAppleSecureEnclaveKeyAgreementProductStatus {
    RadishLexAppleSecureEnclaveKeyAgreementProductStatus {
        version: RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_STATUS_VERSION,
        compiled: cfg!(all(feature = "apple-keychain", target_os = "macos")) as u32,
        runtime_qualified: 0,
        product_qualified: 0,
        user_sync_enabled: 0,
        exportable: 0,
        hardware_backed: 0,
        user_presence_required: 0,
        backup_migratable: 0,
    }
}

const fn summary(
    scenario: u32,
    result: u32,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    let status = current_status();
    RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
        version: RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_SMOKE_VERSION,
        result,
        scenario,
        error_category: RADISHLEX_APPLE_P256_ERROR_NONE,
        error_detail: RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
        platform_status: 0,
        compiled: status.compiled,
        runtime_qualified: status.runtime_qualified,
        product_qualified: status.product_qualified,
        user_sync_enabled: status.user_sync_enabled,
        exportable: status.exportable,
        hardware_backed: status.hardware_backed,
        user_presence_required: status.user_presence_required,
        backup_migratable: status.backup_migratable,
        created: 0,
        reloaded: 0,
        public_key_matched: 0,
        shared_secret_derived: 0,
        wrapped_epoch_verified: 0,
        deleted: 0,
        missing_confirmed: 0,
        fail_closed: 0,
        expected_failure_confirmed: 0,
        cleanup_required: 0,
        cleanup_attempted: 0,
    }
}

/// Writes the metadata-only product status.
///
/// # Safety
/// `status_out` must point to writable memory for one status value.
pub unsafe fn write_product_status(
    status_out: *mut RadishLexAppleSecureEnclaveKeyAgreementProductStatus,
) -> u32 {
    if status_out.is_null() {
        return RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        ptr::write(status_out, current_status())
    })) {
        Ok(()) => RADISHLEX_APPLE_P256_SMOKE_PASSED,
        Err(_) => RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
    }
}

/// Runs one explicitly gated product smoke scenario.
///
/// # Safety
/// `summary_out` must point to writable memory for one summary value.
pub unsafe fn run_product_smoke(
    scenario: u32,
    summary_out: *mut RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> u32 {
    if summary_out.is_null() {
        return RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
    }
    let value = catch_unwind(AssertUnwindSafe(|| {
        if std::env::var("RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SMOKE")
            .as_deref()
            != Ok("1")
        {
            return summary(scenario, RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED);
        }

        #[cfg(all(feature = "apple-keychain", target_os = "macos"))]
        {
            run_macos_product_smoke(scenario)
        }

        #[cfg(not(all(feature = "apple-keychain", target_os = "macos")))]
        {
            summary(
                scenario,
                crate::apple_p256_product::RADISHLEX_APPLE_P256_SMOKE_UNSUPPORTED_BUILD,
            )
        }
    }))
    .unwrap_or_else(|_| summary(scenario, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR));
    let result = value.result;
    ptr::write(summary_out, value);
    result
}

#[no_mangle]
/// Returns metadata-only Secure Enclave key-agreement qualification gates.
///
/// # Safety
/// `status_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_secure_enclave_key_agreement_product_status(
    status_out: *mut RadishLexAppleSecureEnclaveKeyAgreementProductStatus,
) -> u32 {
    unsafe { write_product_status(status_out) }
}

#[no_mangle]
/// Runs the independently gated Secure Enclave key-agreement product smoke.
///
/// No identifier, public key, shared secret, wrapping key, master key, nonce,
/// or ciphertext crosses this ABI.
///
/// # Safety
/// `summary_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_secure_enclave_key_agreement_product_smoke(
    scenario: u32,
    summary_out: *mut RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> u32 {
    unsafe { run_product_smoke(scenario, summary_out) }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_macos_product_smoke(
    scenario: u32,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    let value = summary(scenario, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR);
    if value.compiled != 1
        || value.runtime_qualified != 0
        || value.product_qualified != 0
        || value.user_sync_enabled != 0
        || value.exportable != 0
        || value.hardware_backed != 0
        || value.user_presence_required != 0
        || value.backup_migratable != 0
    {
        return with_result(
            value,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CAPABILITY_MISMATCH,
        );
    }
    match scenario {
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_LIFECYCLE => run_lifecycle(value),
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_DENIED_CREATE => {
            run_expected_create_failure(
                value,
                crate::apple_p256_product::RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED,
            )
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_PREPARE_LOCKED_DERIVE => {
            run_prepare_locked(value)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_LOCKED_DERIVE => {
            run_expected_locked(value)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_CLEANUP_LOCKED_DERIVE => {
            run_cleanup_locked(value)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_UNSUPPORTED_CREATE => {
            run_expected_create_failure(
                value,
                crate::apple_p256_product::RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE,
            )
        }
        _ => with_result(value, RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT),
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_lifecycle(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    use radishlex_ime_crypto::{
        AppleSecureEnclaveP256KeyAgreementStore, DeviceKeyAgreementKeyHandle, KeyDescriptor,
        KeyRole, Nonce, SyncMasterKeyMaterial, WrappedEpochMaterial,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    };

    let now_ms = now_ms();
    let device_id = format!("radishlex-key-agreement-smoke-device-{now_ms}");
    let key_id = format!("radishlex-key-agreement-smoke-key-{now_ms}");
    let store = AppleSecureEnclaveP256KeyAgreementStore::new();
    let cleanup_handle = match DeviceKeyAgreementKeyHandle::p256(
        &device_id,
        &key_id,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    ) {
        Ok(handle) => handle,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED,
                &error,
            )
        }
    };
    let _cleanup = KeyAgreementSmokeCleanup(cleanup_handle.clone());
    result.cleanup_attempted = 1;
    let created = match store.create_key(&device_id, &key_id, now_ms) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED,
                &error,
            )
        }
    };
    result.created = 1;

    let fresh = AppleSecureEnclaveP256KeyAgreementStore::new();
    let handle = match fresh.handle(&device_id, &key_id) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_RELOAD_FAILED,
                &error,
            )
        }
    };
    let reloaded = match fresh.public_key(&handle) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_RELOAD_FAILED,
                &error,
            )
        }
    };
    result.reloaded = 1;
    if created.device_id != reloaded.device_id
        || created.key_id != reloaded.key_id
        || created.algorithm != reloaded.algorithm
        || created.public_key != reloaded.public_key
        || created.revoked_at_ms != reloaded.revoked_at_ms
    {
        return with_result(result, RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_RELOAD_FAILED);
    }
    result.public_key_matched = 1;

    let descriptor =
        match KeyDescriptor::new("radishlex-smoke-object-key-v1", KeyRole::ObjectKey, 7) {
            Ok(value) => value,
            Err(error) => {
                return record_error(
                    result,
                    RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                    &error,
                )
            }
        };
    let master = match SyncMasterKeyMaterial::new([0x5a; 32]) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                &error,
            )
        }
    };
    let nonce = match Nonce::new([0x42; 24]) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                &error,
            )
        }
    };
    let wrapped = match WrappedEpochMaterial::seal_for_recipient(
        "radishlex-smoke-domain-v1",
        &reloaded,
        "radishlex-smoke-wrapping-key-v1",
        &descriptor,
        &master,
        nonce,
        now_ms,
    ) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                &error,
            )
        }
    };
    let peer_public_key = match wrapped.ephemeral_public_key() {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                &error,
            )
        }
    };
    let shared = match fresh.derive_shared_secret(&handle, peer_public_key) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DERIVE_FAILED,
                &error,
            )
        }
    };
    result.shared_secret_derived = 1;
    let (unwrapped_descriptor, unwrapped_master) = match wrapped.unwrap(&shared) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
                &error,
            )
        }
    };
    if unwrapped_descriptor != descriptor || unwrapped_master.as_bytes() != master.as_bytes() {
        return with_result(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_WRAPPED_EPOCH_VERIFY_FAILED,
        );
    }
    result.wrapped_epoch_verified = 1;
    if let Err(error) = fresh.delete_key(&handle) {
        result.cleanup_required = 1;
        return record_error(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DELETE_FAILED,
            &error,
        );
    }
    result.deleted = 1;
    let missing_store = AppleSecureEnclaveP256KeyAgreementStore::new();
    match missing_store.handle(&device_id, &key_id) {
        Err(radishlex_ime_crypto::CryptoError::PrivateKeyUnavailable { .. }) => {
            result.missing_confirmed = 1;
        }
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_MISSING_CHECK_FAILED,
                &error,
            )
        }
        Ok(_) => {
            return with_result(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_MISSING_CHECK_FAILED,
            )
        }
    }
    with_result(result, RADISHLEX_APPLE_P256_SMOKE_PASSED)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_create_failure(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
    expected_category: u32,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    use radishlex_ime_crypto::{
        AppleSecureEnclaveP256KeyAgreementStore, DeviceKeyAgreementKeyHandle,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    };

    let now_ms = now_ms();
    let device_id = format!("radishlex-key-agreement-failure-device-{now_ms}");
    let key_id = format!("radishlex-key-agreement-failure-key-{now_ms}");
    let store = AppleSecureEnclaveP256KeyAgreementStore::new();
    let cleanup_handle = match DeviceKeyAgreementKeyHandle::p256(
        &device_id,
        &key_id,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    ) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED,
                &error,
            )
        }
    };
    let _cleanup = KeyAgreementSmokeCleanup(cleanup_handle);
    result.cleanup_attempted = 1;
    match store.create_key(&device_id, &key_id, now_ms) {
        Ok(_) => {
            if let Ok(handle) = store.handle(&device_id, &key_id) {
                let _ = store.delete_key(&handle);
            }
            with_result(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED,
            )
        }
        Err(error) => {
            let (category, detail, platform_status) =
                crate::apple_secure_enclave_product::product_error(&error);
            result.error_category = category;
            result.error_detail = detail;
            result.platform_status = platform_status;
            result.fail_closed = 1;
            if category == expected_category {
                result.expected_failure_confirmed = 1;
                with_result(result, RADISHLEX_APPLE_P256_SMOKE_PASSED)
            } else {
                with_result(
                    result,
                    RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_UNEXPECTED_ERROR_CATEGORY,
                )
            }
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_DEVICE_ID: &str = "radishlex-manager-key-agreement-locked-device-v1";
#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_KEY_ID: &str = "radishlex-manager-key-agreement-locked-key-v1";

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_prepare_locked(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    use radishlex_ime_crypto::{
        AppleSecureEnclaveP256KeyAgreementStore, DeviceKeyAgreementKeyHandle,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    };

    let store = AppleSecureEnclaveP256KeyAgreementStore::new();
    let handle = match DeviceKeyAgreementKeyHandle::p256(
        LOCKED_DEVICE_ID,
        LOCKED_KEY_ID,
        APPLE_SECURE_ENCLAVE_P256_KEY_AGREEMENT_BACKEND,
    ) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED,
                &error,
            )
        }
    };
    result.cleanup_attempted = 1;
    let _ = store.delete_key(&handle);
    if let Err(error) = store.create_key(LOCKED_DEVICE_ID, LOCKED_KEY_ID, now_ms()) {
        return record_error(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_CREATE_FAILED,
            &error,
        );
    }
    result.created = 1;
    match store.derive_shared_secret(&handle, &P256_GENERATOR_PUBLIC_KEY) {
        Ok(_) => {
            result.shared_secret_derived = 1;
            result.cleanup_required = 1;
            with_result(result, RADISHLEX_APPLE_P256_SMOKE_PASSED)
        }
        Err(error) => {
            let _ = store.delete_key(&handle);
            record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DERIVE_FAILED,
                &error,
            )
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_locked(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    use radishlex_ime_crypto::AppleSecureEnclaveP256KeyAgreementStore;

    let store = AppleSecureEnclaveP256KeyAgreementStore::new();
    let handle = match store.handle(LOCKED_DEVICE_ID, LOCKED_KEY_ID) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_RELOAD_FAILED,
                &error,
            )
        }
    };
    result.cleanup_required = 1;
    match store.derive_shared_secret(&handle, &P256_GENERATOR_PUBLIC_KEY) {
        Ok(_) => with_result(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED,
        ),
        Err(error) => {
            let (category, detail, platform_status) =
                crate::apple_secure_enclave_product::product_error(&error);
            result.error_category = category;
            result.error_detail = detail;
            result.platform_status = platform_status;
            result.fail_closed = 1;
            if category == crate::apple_p256_product::RADISHLEX_APPLE_P256_ERROR_LOCKED {
                result.expected_failure_confirmed = 1;
                with_result(result, RADISHLEX_APPLE_P256_SMOKE_PASSED)
            } else {
                with_result(
                    result,
                    RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_UNEXPECTED_ERROR_CATEGORY,
                )
            }
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_cleanup_locked(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    use radishlex_ime_crypto::{AppleSecureEnclaveP256KeyAgreementStore, CryptoError};

    let store = AppleSecureEnclaveP256KeyAgreementStore::new();
    let handle = match store.handle(LOCKED_DEVICE_ID, LOCKED_KEY_ID) {
        Ok(value) => value,
        Err(error) => {
            return record_error(
                result,
                RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DELETE_FAILED,
                &error,
            )
        }
    };
    result.cleanup_attempted = 1;
    if let Err(error) = store.delete_key(&handle) {
        result.cleanup_required = 1;
        return record_error(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_DELETE_FAILED,
            &error,
        );
    }
    result.deleted = 1;
    let missing_store = AppleSecureEnclaveP256KeyAgreementStore::new();
    match missing_store.handle(LOCKED_DEVICE_ID, LOCKED_KEY_ID) {
        Err(CryptoError::PrivateKeyUnavailable { .. }) => {
            result.missing_confirmed = 1;
            with_result(result, RADISHLEX_APPLE_P256_SMOKE_PASSED)
        }
        Err(error) => record_error(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_MISSING_CHECK_FAILED,
            &error,
        ),
        Ok(_) => with_result(
            result,
            RADISHLEX_APPLE_KEY_AGREEMENT_SMOKE_MISSING_CHECK_FAILED,
        ),
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn record_error(
    mut result: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
    result_code: u32,
    error: &radishlex_ime_crypto::CryptoError,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    let (category, detail, platform_status) =
        crate::apple_secure_enclave_product::product_error(error);
    result.error_category = category;
    result.error_detail = detail;
    result.platform_status = platform_status;
    with_result(result, result_code)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const fn with_result(
    mut value: RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
    result: u32,
) -> RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
    value.result = result;
    value
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const P256_GENERATOR_PUBLIC_KEY: [u8; 65] = [
    0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
    0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2,
    0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e,
    0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51,
    0xf5,
];

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
struct KeyAgreementSmokeCleanup(radishlex_ime_crypto::DeviceKeyAgreementKeyHandle);

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
impl Drop for KeyAgreementSmokeCleanup {
    fn drop(&mut self) {
        let store = radishlex_ime_crypto::AppleSecureEnclaveP256KeyAgreementStore::new();
        let _ = store.delete_key(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_keeps_independent_qualification_and_user_sync_closed() {
        let status = current_status();
        assert_eq!(
            status.compiled,
            cfg!(all(feature = "apple-keychain", target_os = "macos")) as u32
        );
        assert_eq!(status.runtime_qualified, 0);
        assert_eq!(status.hardware_backed, 0);
        assert_eq!(status.product_qualified, 0);
        assert_eq!(status.user_sync_enabled, 0);
    }

    #[test]
    fn summary_is_fixed_and_redacted() {
        let value = summary(
            RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_LIFECYCLE,
            RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED,
        );
        assert_eq!(
            std::mem::size_of_val(&value),
            25 * std::mem::size_of::<u32>()
        );
        let debug = format!("{value:?}");
        for forbidden in [
            "device_id",
            "key_id",
            "public_key_bytes",
            "shared_secret_bytes",
            "master_key_bytes",
            "nonce_bytes",
            "ciphertext_bytes",
        ] {
            assert!(
                !debug.contains(forbidden),
                "summary leaked field {forbidden}"
            );
        }
    }

    #[test]
    fn null_outputs_fail_before_any_platform_access() {
        assert_eq!(
            unsafe { write_product_status(std::ptr::null_mut()) },
            RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                run_product_smoke(
                    RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_LIFECYCLE,
                    std::ptr::null_mut(),
                )
            },
            RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT
        );
    }
}
