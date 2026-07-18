use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use crate::apple_p256_product::{
    RadishLexAppleP256ProductSmokeSummary, RadishLexAppleP256ProductStatus,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE, RADISHLEX_APPLE_P256_ERROR_NONE,
    RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
    RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT, RADISHLEX_APPLE_P256_SMOKE_PASSED,
};
#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
use crate::apple_p256_product::{
    RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED, RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE,
    RADISHLEX_APPLE_P256_ERROR_CORRUPTED, RADISHLEX_APPLE_P256_ERROR_DETAIL_ACCESS_UNSPECIFIED,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_AUTHENTICATION_FAILED,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_MISSING_ENTITLEMENT,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_READ_ONLY, RADISHLEX_APPLE_P256_ERROR_DETAIL_RESTRICTED_API,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_UNCLASSIFIED_PLATFORM_STATUS,
    RADISHLEX_APPLE_P256_ERROR_DETAIL_WRITE_PERMISSION, RADISHLEX_APPLE_P256_ERROR_LOCKED,
    RADISHLEX_APPLE_P256_ERROR_MISSING, RADISHLEX_APPLE_P256_ERROR_OTHER,
    RADISHLEX_APPLE_P256_ERROR_REVOKED, RADISHLEX_APPLE_P256_ERROR_UNSUPPORTED,
    RADISHLEX_APPLE_P256_ERROR_USER_PRESENCE_REQUIRED,
    RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH, RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED,
    RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED,
    RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED,
    RADISHLEX_APPLE_P256_SMOKE_GO_VERIFY_FAILED, RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED,
    RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED, RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED,
    RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY,
};

pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_STATUS_VERSION: u32 = 1;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_SMOKE_VERSION: u32 = 1;

pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_LIFECYCLE: u32 = 0;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_DENIED_CREATE: u32 = 1;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_PREPARE_LOCKED_SIGN: u32 = 2;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_LOCKED_SIGN: u32 = 3;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_CLEANUP_LOCKED_SIGN: u32 = 4;
pub const RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_UNSUPPORTED_CREATE: u32 = 5;

fn current_status() -> RadishLexAppleP256ProductStatus {
    #[cfg(feature = "apple-keychain")]
    {
        use radishlex_ime_crypto::AppleSecureEnclaveP256DeviceKeyStore;

        let status = AppleSecureEnclaveP256DeviceKeyStore::new().backend_status();
        RadishLexAppleP256ProductStatus {
            version: RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_STATUS_VERSION,
            compiled: flag(status.compiled),
            runtime_available: flag(status.available),
            can_create_signing_keys: flag(status.can_create_signing_keys),
            can_sign: flag(status.can_sign),
            product_qualified: flag(status.product_qualified),
            user_sync_enabled: 0,
            exportable: flag(status.capabilities.exportable),
            hardware_backed: flag(status.capabilities.hardware_backed),
            user_presence_required: flag(status.capabilities.user_presence_required),
            backup_migratable: flag(status.capabilities.backup_migratable),
        }
    }

    #[cfg(not(feature = "apple-keychain"))]
    RadishLexAppleP256ProductStatus {
        version: RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_STATUS_VERSION,
        compiled: 0,
        runtime_available: 0,
        can_create_signing_keys: 0,
        can_sign: 0,
        product_qualified: 0,
        user_sync_enabled: 0,
        exportable: 0,
        hardware_backed: 0,
        user_presence_required: 0,
        backup_migratable: 0,
    }
}

fn summary(scenario: u32, result: u32) -> RadishLexAppleP256ProductSmokeSummary {
    let status = current_status();
    RadishLexAppleP256ProductSmokeSummary {
        version: RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_SMOKE_VERSION,
        result,
        scenario,
        error_category: RADISHLEX_APPLE_P256_ERROR_NONE,
        error_detail: RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
        platform_status: 0,
        compiled: status.compiled,
        runtime_available: status.runtime_available,
        can_create_signing_keys: status.can_create_signing_keys,
        can_sign: status.can_sign,
        product_qualified: status.product_qualified,
        user_sync_enabled: status.user_sync_enabled,
        exportable: status.exportable,
        hardware_backed: status.hardware_backed,
        user_presence_required: status.user_presence_required,
        backup_migratable: status.backup_migratable,
        created: 0,
        reloaded: 0,
        rust_verified: 0,
        go_verified: 0,
        deleted: 0,
        missing_confirmed: 0,
        fail_closed: 0,
        expected_failure_confirmed: 0,
        cleanup_required: 0,
        cleanup_attempted: 0,
    }
}

pub unsafe fn write_product_status(status_out: *mut RadishLexAppleP256ProductStatus) -> u32 {
    if status_out.is_null() {
        return RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        ptr::write(status_out, current_status());
    })) {
        Ok(()) => RADISHLEX_APPLE_P256_SMOKE_PASSED,
        Err(_) => RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
    }
}

pub unsafe fn run_product_smoke(
    scenario: u32,
    go_server_dir: *const c_char,
    summary_out: *mut RadishLexAppleP256ProductSmokeSummary,
) -> u32 {
    if summary_out.is_null() {
        return RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        if std::env::var("RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_P256_SMOKE").as_deref()
            != Ok("1")
        {
            return summary(scenario, RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED);
        }

        #[cfg(all(feature = "apple-keychain", target_os = "macos"))]
        {
            let go_server_dir = match read_go_server_dir(go_server_dir) {
                Some(path) => path,
                None => return summary(scenario, RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT),
            };
            run_macos_product_smoke(scenario, &go_server_dir)
        }

        #[cfg(not(all(feature = "apple-keychain", target_os = "macos")))]
        {
            let _ = go_server_dir;
            summary(
                scenario,
                crate::apple_p256_product::RADISHLEX_APPLE_P256_SMOKE_UNSUPPORTED_BUILD,
            )
        }
    }))
    .unwrap_or_else(|_| summary(scenario, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR));
    let result_code = result.result;
    ptr::write(summary_out, result);
    result_code
}

#[cfg(feature = "apple-keychain")]
fn flag(value: bool) -> u32 {
    u32::from(value)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn read_go_server_dir(value: *const c_char) -> Option<std::path::PathBuf> {
    use std::ffi::CStr;

    if value.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(value) }.to_str().ok()?;
    let path = std::path::PathBuf::from(value);
    if !path.is_dir() || !path.join("go.mod").is_file() {
        return None;
    }
    Some(path)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_macos_product_smoke(
    scenario: u32,
    go_server_dir: &std::path::Path,
) -> RadishLexAppleP256ProductSmokeSummary {
    let mut result = summary(scenario, RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR);
    if result.compiled != 1
        || result.runtime_available != 1
        || result.can_create_signing_keys != 1
        || result.can_sign != 1
        || result.product_qualified != 0
        || result.user_sync_enabled != 0
        || result.exportable != 0
        || result.hardware_backed != 1
        || result.user_presence_required != 0
        || result.backup_migratable != 0
    {
        result.result = RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH;
        return result;
    }
    match scenario {
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_LIFECYCLE => {
            run_lifecycle_smoke(result, go_server_dir)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_DENIED_CREATE => {
            run_expected_create_failure(result, RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_PREPARE_LOCKED_SIGN => {
            run_prepare_locked_sign(result)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_LOCKED_SIGN => {
            run_expected_locked_sign(result)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_CLEANUP_LOCKED_SIGN => {
            run_cleanup_locked_sign(result)
        }
        RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_UNSUPPORTED_CREATE => {
            run_expected_create_failure(result, RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE)
        }
        _ => {
            result.result = RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
            result
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_lifecycle_smoke(
    mut result: RadishLexAppleP256ProductSmokeSummary,
    go_server_dir: &std::path::Path,
) -> RadishLexAppleP256ProductSmokeSummary {
    use std::process::{Command, Stdio};

    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleSecureEnclaveP256DeviceKeyStore, CryptoError,
        DeviceSigningKeyHandle, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(value) => value,
        None => return result,
    };
    let device_id = format!(
        "radishlex-manager-secure-enclave-smoke-device-{}",
        std::process::id()
    );
    let signing_key_id = format!("radishlex-manager-secure-enclave-smoke-key-{now_ms}");
    let cleanup_handle = match DeviceSigningKeyHandle::apple_secure_enclave_p256(
        &device_id,
        &signing_key_id,
        now_ms,
    ) {
        Ok(handle) => handle,
        Err(_) => return result,
    };
    let _cleanup = SecureEnclaveSmokeCleanup::new(cleanup_handle.clone(), now_ms + 1);
    result.cleanup_attempted = 1;

    let store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let public_key = match store.create_signing_key(&device_id, &signing_key_id, now_ms) {
        Ok(value) => value,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED;
            return result;
        }
    };
    result.created = 1;
    let reloaded_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let handle = match reloaded_store.handle(&device_id, &signing_key_id) {
        Ok(value) => value,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
    };
    let loaded_public_key = match reloaded_store.public_key(&handle) {
        Ok(value) if value.public_key == public_key.public_key => value,
        Ok(_) => {
            result.error_category = RADISHLEX_APPLE_P256_ERROR_CORRUPTED;
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
    };
    result.reloaded = 1;

    let canonical = canonical_signature_bytes(
        "apple_secure_enclave_p256_manager_product_smoke",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = match reloaded_store.sign(&handle, &canonical) {
        Ok(value) => value,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
            return result;
        }
    };
    if signature
        .verify_at(&loaded_public_key, &canonical, now_ms)
        .is_err()
    {
        result.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        result.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
        return result;
    }
    result.rust_verified = 1;
    let go_status = Command::new("go")
        .args([
            "test",
            "./internal/storage",
            "-run",
            "^TestExternalP256SignatureSmoke$",
            "-count=1",
        ])
        .current_dir(go_server_dir)
        .env("GOCACHE", "/tmp/radishlex-manager-secure-enclave-go-cache")
        .env("RADISHLEX_GO_P256_SMOKE", "1")
        .env(
            "RADISHLEX_GO_P256_PUBLIC_KEY_HEX",
            hex(&loaded_public_key.public_key),
        )
        .env("RADISHLEX_GO_P256_SIGNATURE_HEX", hex(&signature.signature))
        .env("RADISHLEX_GO_P256_CANONICAL_HEX", hex(&canonical))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !matches!(go_status, Ok(status) if status.success()) {
        result.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        result.result = RADISHLEX_APPLE_P256_SMOKE_GO_VERIFY_FAILED;
        return result;
    }
    result.go_verified = 1;
    if let Err(error) = reloaded_store.verify_private_key_non_exportable(&handle) {
        record_product_error(&mut result, &error);
        result.result = RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH;
        return result;
    }
    result.expected_failure_confirmed = 1;

    if let Err(error) = reloaded_store.delete_or_revoke(&handle, now_ms + 1) {
        record_product_error(&mut result, &error);
        result.result = RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED;
        return result;
    }
    result.deleted = 1;
    let revoked_failed_closed = matches!(
        reloaded_store.sign(&handle, &canonical),
        Err(CryptoError::PrivateKeyRevoked { .. })
    );
    let missing = AppleSecureEnclaveP256DeviceKeyStore::new().handle(&device_id, &signing_key_id);
    if !revoked_failed_closed || !matches!(missing, Err(CryptoError::PrivateKeyUnavailable { .. }))
    {
        result.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        result.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        return result;
    }
    result.missing_confirmed = 1;
    result.fail_closed = 1;
    result.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
    result
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_create_failure(
    mut result: RadishLexAppleP256ProductSmokeSummary,
    expected_category: u32,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{AppleSecureEnclaveP256DeviceKeyStore, DeviceSigningKeyHandle};

    let now_ms = match product_smoke_now_ms() {
        Some(value) => value,
        None => return result,
    };
    let device_id = format!(
        "radishlex-manager-secure-enclave-failure-device-{}",
        std::process::id()
    );
    let signing_key_id = format!("radishlex-manager-secure-enclave-failure-key-{now_ms}");
    let cleanup_handle = match DeviceSigningKeyHandle::apple_secure_enclave_p256(
        &device_id,
        &signing_key_id,
        now_ms,
    ) {
        Ok(handle) => handle,
        Err(_) => return result,
    };
    let _cleanup = SecureEnclaveSmokeCleanup::new(cleanup_handle, now_ms + 1);
    result.cleanup_attempted = 1;
    match AppleSecureEnclaveP256DeviceKeyStore::new().create_signing_key(
        &device_id,
        &signing_key_id,
        now_ms,
    ) {
        Ok(_) => {
            result.created = 1;
            result.result = RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED;
        }
        Err(error) => {
            record_product_error(&mut result, &error);
            result.fail_closed = 1;
            if result.error_category == expected_category {
                result.expected_failure_confirmed = 1;
                result.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
            } else {
                result.result = RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY;
            }
        }
    }
    result
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_prepare_locked_sign(
    mut result: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleSecureEnclaveP256DeviceKeyStore, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(value) => value,
        None => return result,
    };
    let handle = match locked_matrix_handle(now_ms) {
        Some(value) => value,
        None => return result,
    };
    let cleanup_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let _ = cleanup_store.delete_or_revoke(&handle, now_ms);
    let store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let public_key = match store.create_signing_key(
        LOCKED_MATRIX_DEVICE_ID,
        LOCKED_MATRIX_SIGNING_KEY_ID,
        now_ms,
    ) {
        Ok(value) => value,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED;
            return result;
        }
    };
    result.created = 1;
    result.cleanup_required = 1;
    let reloaded_store = AppleSecureEnclaveP256DeviceKeyStore::new();
    let handle = match reloaded_store.handle(LOCKED_MATRIX_DEVICE_ID, LOCKED_MATRIX_SIGNING_KEY_ID)
    {
        Ok(value) => value,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
    };
    let loaded_public = match reloaded_store.public_key(&handle) {
        Ok(value) if value.public_key == public_key.public_key => value,
        Ok(_) => {
            result.error_category = RADISHLEX_APPLE_P256_ERROR_CORRUPTED;
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return result;
        }
    };
    result.reloaded = 1;
    let canonical = canonical_signature_bytes(
        "apple_secure_enclave_p256_manager_locked_matrix",
        &[SignatureField::text("smoke", "synthetic")],
    );
    match reloaded_store.sign(&handle, &canonical) {
        Ok(signature)
            if signature
                .verify_at(&loaded_public, &canonical, now_ms)
                .is_ok() =>
        {
            result.rust_verified = 1;
            result.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
        }
        Ok(_) => {
            result.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
            result.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
        }
        Err(error) => {
            record_product_error(&mut result, &error);
            result.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
        }
    }
    result
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_locked_sign(
    mut result: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleSecureEnclaveP256DeviceKeyStore, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(value) => value,
        None => return result,
    };
    let handle = match locked_matrix_handle(now_ms) {
        Some(value) => value,
        None => return result,
    };
    result.cleanup_required = 1;
    let canonical = canonical_signature_bytes(
        "apple_secure_enclave_p256_manager_locked_matrix",
        &[SignatureField::text("smoke", "synthetic")],
    );
    match AppleSecureEnclaveP256DeviceKeyStore::new().sign(&handle, &canonical) {
        Ok(_) => result.result = RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED,
        Err(error) => {
            record_product_error(&mut result, &error);
            result.fail_closed = 1;
            if result.error_category == RADISHLEX_APPLE_P256_ERROR_LOCKED {
                result.expected_failure_confirmed = 1;
                result.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
            } else {
                result.result = RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY;
            }
        }
    }
    result
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_cleanup_locked_sign(
    mut result: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{AppleSecureEnclaveP256DeviceKeyStore, CryptoError};

    let now_ms = match product_smoke_now_ms() {
        Some(value) => value,
        None => return result,
    };
    let handle = match locked_matrix_handle(now_ms) {
        Some(value) => value,
        None => return result,
    };
    result.cleanup_attempted = 1;
    let store = AppleSecureEnclaveP256DeviceKeyStore::new();
    if let Err(error) = store.delete_or_revoke(&handle, now_ms + 1) {
        record_product_error(&mut result, &error);
        result.cleanup_required = 1;
        result.result = RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED;
        return result;
    }
    result.deleted = 1;
    match AppleSecureEnclaveP256DeviceKeyStore::new()
        .handle(LOCKED_MATRIX_DEVICE_ID, LOCKED_MATRIX_SIGNING_KEY_ID)
    {
        Err(CryptoError::PrivateKeyUnavailable { .. }) => {
            result.missing_confirmed = 1;
            result.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
        }
        Err(error) => {
            record_product_error(&mut result, &error);
            result.cleanup_required = 1;
            result.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        }
        Ok(_) => {
            result.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
            result.cleanup_required = 1;
            result.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        }
    }
    result
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_MATRIX_DEVICE_ID: &str = "radishlex-manager-secure-enclave-locked-device-v1";
#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_MATRIX_SIGNING_KEY_ID: &str = "radishlex-manager-secure-enclave-locked-key-v1";

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn locked_matrix_handle(
    created_at_ms: i64,
) -> Option<radishlex_ime_crypto::DeviceSigningKeyHandle> {
    radishlex_ime_crypto::DeviceSigningKeyHandle::apple_secure_enclave_p256(
        LOCKED_MATRIX_DEVICE_ID,
        LOCKED_MATRIX_SIGNING_KEY_ID,
        created_at_ms,
    )
    .ok()
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn product_smoke_now_ms() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as i64)
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn record_product_error(
    result: &mut RadishLexAppleP256ProductSmokeSummary,
    error: &radishlex_ime_crypto::CryptoError,
) {
    let (category, detail, platform_status) = product_error(error);
    result.error_category = category;
    result.error_detail = detail;
    result.platform_status = platform_status;
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn product_error(error: &radishlex_ime_crypto::CryptoError) -> (u32, u32, i32) {
    use radishlex_ime_crypto::{CryptoError, PrivateKeyAccessDeniedReason};

    match error {
        CryptoError::StorageBackendUnavailable { .. }
        | CryptoError::UnsupportedStorageBackend { .. } => (
            RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            -25291,
        ),
        CryptoError::PrivateKeyLocked { .. } => (
            RADISHLEX_APPLE_P256_ERROR_LOCKED,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            -25308,
        ),
        CryptoError::PrivateKeyAccessDenied { reason, .. } => {
            let (detail, status) = match reason {
                PrivateKeyAccessDeniedReason::Unspecified => {
                    (RADISHLEX_APPLE_P256_ERROR_DETAIL_ACCESS_UNSPECIFIED, 0)
                }
                PrivateKeyAccessDeniedReason::AuthenticationFailed => (
                    RADISHLEX_APPLE_P256_ERROR_DETAIL_AUTHENTICATION_FAILED,
                    -25293,
                ),
                PrivateKeyAccessDeniedReason::WritePermission => {
                    (RADISHLEX_APPLE_P256_ERROR_DETAIL_WRITE_PERMISSION, -61)
                }
                PrivateKeyAccessDeniedReason::ReadOnly => {
                    (RADISHLEX_APPLE_P256_ERROR_DETAIL_READ_ONLY, -25292)
                }
                PrivateKeyAccessDeniedReason::MissingEntitlement => (
                    RADISHLEX_APPLE_P256_ERROR_DETAIL_MISSING_ENTITLEMENT,
                    -34018,
                ),
                PrivateKeyAccessDeniedReason::RestrictedApi => {
                    (RADISHLEX_APPLE_P256_ERROR_DETAIL_RESTRICTED_API, -34020)
                }
                PrivateKeyAccessDeniedReason::UnclassifiedPlatformStatus(status) => (
                    RADISHLEX_APPLE_P256_ERROR_DETAIL_UNCLASSIFIED_PLATFORM_STATUS,
                    *status,
                ),
            };
            (RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED, detail, status)
        }
        CryptoError::PrivateKeyUserPresenceRequired { .. } => (
            RADISHLEX_APPLE_P256_ERROR_USER_PRESENCE_REQUIRED,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            -25315,
        ),
        CryptoError::PrivateKeyUnavailable { .. } => (
            RADISHLEX_APPLE_P256_ERROR_MISSING,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            -25300,
        ),
        CryptoError::PrivateKeyCorrupted { .. } => (
            RADISHLEX_APPLE_P256_ERROR_CORRUPTED,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            -26275,
        ),
        CryptoError::UnsupportedSignatureAlgorithm { .. } => (
            RADISHLEX_APPLE_P256_ERROR_UNSUPPORTED,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            0,
        ),
        CryptoError::PrivateKeyRevoked { .. } => (
            RADISHLEX_APPLE_P256_ERROR_REVOKED,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            0,
        ),
        _ => (
            RADISHLEX_APPLE_P256_ERROR_OTHER,
            RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
            0,
        ),
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
struct SecureEnclaveSmokeCleanup {
    handle: radishlex_ime_crypto::DeviceSigningKeyHandle,
    revoked_at_ms: i64,
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
impl SecureEnclaveSmokeCleanup {
    fn new(handle: radishlex_ime_crypto::DeviceSigningKeyHandle, revoked_at_ms: i64) -> Self {
        Self {
            handle,
            revoked_at_ms,
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
impl Drop for SecureEnclaveSmokeCleanup {
    fn drop(&mut self) {
        let store = radishlex_ime_crypto::AppleSecureEnclaveP256DeviceKeyStore::new();
        let _ = store.delete_or_revoke(&self.handle, self.revoked_at_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_evidenced_runtime_but_keeps_product_gate_closed() {
        let status = current_status();
        let apple_runtime = u32::from(cfg!(all(feature = "apple-keychain", target_os = "macos")));
        assert_eq!(status.compiled, apple_runtime);
        assert_eq!(status.runtime_available, apple_runtime);
        assert_eq!(status.can_create_signing_keys, apple_runtime);
        assert_eq!(status.can_sign, apple_runtime);
        assert_eq!(status.product_qualified, 0);
        assert_eq!(status.user_sync_enabled, 0);
        assert_eq!(status.exportable, 0);
        assert_eq!(status.hardware_backed, apple_runtime);
        assert_eq!(status.user_presence_required, 0);
        assert_eq!(status.backup_migratable, 0);
    }

    #[test]
    fn summary_contains_only_fixed_flags() {
        let value = summary(
            RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_LIFECYCLE,
            RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED,
        );
        let debug = format!("{value:?}");
        assert!(!debug.contains("canonical"));
        assert!(!debug.contains("signature"));
        assert!(!debug.contains("private"));
        assert_eq!(
            value.version,
            RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_SMOKE_VERSION
        );
    }
}
