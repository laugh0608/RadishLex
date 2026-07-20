use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

pub const RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION: u32 = 1;
pub const RADISHLEX_APPLE_P256_PRODUCT_SMOKE_VERSION: u32 = 4;

pub const RADISHLEX_APPLE_P256_SCENARIO_LIFECYCLE: u32 = 0;
pub const RADISHLEX_APPLE_P256_SCENARIO_EXPECT_DENIED_CREATE: u32 = 1;
pub const RADISHLEX_APPLE_P256_SCENARIO_PREPARE_LOCKED_SIGN: u32 = 2;
pub const RADISHLEX_APPLE_P256_SCENARIO_EXPECT_LOCKED_SIGN: u32 = 3;
pub const RADISHLEX_APPLE_P256_SCENARIO_CLEANUP_LOCKED_SIGN: u32 = 4;

pub const RADISHLEX_APPLE_P256_ERROR_NONE: u32 = 0;
pub const RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE: u32 = 1;
pub const RADISHLEX_APPLE_P256_ERROR_LOCKED: u32 = 2;
pub const RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED: u32 = 3;
pub const RADISHLEX_APPLE_P256_ERROR_USER_PRESENCE_REQUIRED: u32 = 4;
pub const RADISHLEX_APPLE_P256_ERROR_MISSING: u32 = 5;
pub const RADISHLEX_APPLE_P256_ERROR_CORRUPTED: u32 = 6;
pub const RADISHLEX_APPLE_P256_ERROR_UNSUPPORTED: u32 = 7;
pub const RADISHLEX_APPLE_P256_ERROR_REVOKED: u32 = 8;
pub const RADISHLEX_APPLE_P256_ERROR_OTHER: u32 = 255;

pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE: u32 = 0;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_ACCESS_UNSPECIFIED: u32 = 1;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_AUTHENTICATION_FAILED: u32 = 2;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_WRITE_PERMISSION: u32 = 3;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_READ_ONLY: u32 = 4;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_MISSING_ENTITLEMENT: u32 = 5;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_RESTRICTED_API: u32 = 6;
pub const RADISHLEX_APPLE_P256_ERROR_DETAIL_UNCLASSIFIED_PLATFORM_STATUS: u32 = 7;

pub const RADISHLEX_APPLE_P256_SMOKE_PASSED: u32 = 0;
pub const RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED: u32 = 1;
pub const RADISHLEX_APPLE_P256_SMOKE_UNSUPPORTED_BUILD: u32 = 2;
pub const RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT: u32 = 3;
pub const RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH: u32 = 4;
pub const RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED: u32 = 5;
pub const RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED: u32 = 6;
pub const RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED: u32 = 7;
pub const RADISHLEX_APPLE_P256_SMOKE_GO_VERIFY_FAILED: u32 = 8;
pub const RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED: u32 = 9;
pub const RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED: u32 = 10;
pub const RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED: u32 = 11;
pub const RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY: u32 = 12;
pub const RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR: u32 = 255;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexAppleP256ProductStatus {
    pub version: u32,
    pub compiled: u32,
    pub runtime_available: u32,
    pub can_create_signing_keys: u32,
    pub can_sign: u32,
    pub product_qualified: u32,
    pub user_sync_enabled: u32,
    pub exportable: u32,
    pub hardware_backed: u32,
    pub user_presence_required: u32,
    pub backup_migratable: u32,
}

impl RadishLexAppleP256ProductStatus {
    fn current() -> Self {
        #[cfg(feature = "apple-keychain")]
        {
            use radishlex_ime_crypto::AppleKeychainP256DeviceKeyStore;

            let status = AppleKeychainP256DeviceKeyStore::new().backend_status();
            Self {
                version: RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION,
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
        Self {
            version: RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION,
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
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexAppleP256ProductSmokeSummary {
    pub version: u32,
    pub result: u32,
    pub scenario: u32,
    pub error_category: u32,
    pub error_detail: u32,
    pub platform_status: i32,
    pub compiled: u32,
    pub runtime_available: u32,
    pub can_create_signing_keys: u32,
    pub can_sign: u32,
    pub product_qualified: u32,
    pub user_sync_enabled: u32,
    pub exportable: u32,
    pub hardware_backed: u32,
    pub user_presence_required: u32,
    pub backup_migratable: u32,
    pub created: u32,
    pub reloaded: u32,
    pub rust_verified: u32,
    pub go_verified: u32,
    pub deleted: u32,
    pub missing_confirmed: u32,
    pub fail_closed: u32,
    pub expected_failure_confirmed: u32,
    pub cleanup_required: u32,
    pub cleanup_attempted: u32,
}

impl RadishLexAppleP256ProductSmokeSummary {
    fn new(scenario: u32, result: u32) -> Self {
        let status = RadishLexAppleP256ProductStatus::current();
        Self {
            version: RADISHLEX_APPLE_P256_PRODUCT_SMOKE_VERSION,
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
}

pub unsafe fn write_product_status(status_out: *mut RadishLexAppleP256ProductStatus) -> u32 {
    if status_out.is_null() {
        return RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        ptr::write(status_out, RadishLexAppleP256ProductStatus::current());
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
        if std::env::var("RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE").as_deref() != Ok("1") {
            return RadishLexAppleP256ProductSmokeSummary::new(
                scenario,
                RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED,
            );
        }

        #[cfg(all(feature = "apple-keychain", target_os = "macos"))]
        {
            let go_server_dir = match read_go_server_dir(go_server_dir) {
                Some(path) => path,
                None => {
                    return RadishLexAppleP256ProductSmokeSummary::new(
                        scenario,
                        RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT,
                    );
                }
            };
            run_macos_product_smoke(scenario, &go_server_dir)
        }

        #[cfg(not(all(feature = "apple-keychain", target_os = "macos")))]
        {
            let _ = go_server_dir;
            RadishLexAppleP256ProductSmokeSummary::new(
                scenario,
                RADISHLEX_APPLE_P256_SMOKE_UNSUPPORTED_BUILD,
            )
        }
    }))
    .unwrap_or_else(|_| {
        RadishLexAppleP256ProductSmokeSummary::new(
            scenario,
            RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
        )
    });
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
    if value.is_empty() {
        return None;
    }
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
    let mut summary = RadishLexAppleP256ProductSmokeSummary::new(
        scenario,
        RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR,
    );
    if summary.compiled != 1
        || summary.runtime_available != 1
        || summary.can_create_signing_keys != 1
        || summary.can_sign != 1
        || summary.product_qualified != 0
        || summary.user_sync_enabled != 0
        || summary.exportable != 1
        || summary.hardware_backed != 0
        || summary.user_presence_required != 0
        || summary.backup_migratable != 0
    {
        summary.result = RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH;
        return summary;
    }

    if !matches!(
        scenario,
        RADISHLEX_APPLE_P256_SCENARIO_LIFECYCLE
            | RADISHLEX_APPLE_P256_SCENARIO_EXPECT_DENIED_CREATE
            | RADISHLEX_APPLE_P256_SCENARIO_PREPARE_LOCKED_SIGN
            | RADISHLEX_APPLE_P256_SCENARIO_EXPECT_LOCKED_SIGN
            | RADISHLEX_APPLE_P256_SCENARIO_CLEANUP_LOCKED_SIGN
    ) {
        summary.result = RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
        return summary;
    }

    match scenario {
        RADISHLEX_APPLE_P256_SCENARIO_LIFECYCLE => run_lifecycle_smoke(summary, go_server_dir),
        RADISHLEX_APPLE_P256_SCENARIO_EXPECT_DENIED_CREATE => run_expected_denied_create(summary),
        RADISHLEX_APPLE_P256_SCENARIO_PREPARE_LOCKED_SIGN => run_prepare_locked_sign(summary),
        RADISHLEX_APPLE_P256_SCENARIO_EXPECT_LOCKED_SIGN => run_expected_locked_sign(summary),
        RADISHLEX_APPLE_P256_SCENARIO_CLEANUP_LOCKED_SIGN => run_cleanup_locked_sign(summary),
        _ => {
            summary.result = RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT;
            summary
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_lifecycle_smoke(
    mut summary: RadishLexAppleP256ProductSmokeSummary,
    go_server_dir: &std::path::Path,
) -> RadishLexAppleP256ProductSmokeSummary {
    use std::process::{Command, Stdio};

    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleKeychainP256DeviceKeyStore, CryptoError,
        DeviceSigningKeyHandle, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(now_ms) => now_ms,
        None => return summary,
    };
    let device_id = format!(
        "radishlex-manager-product-smoke-device-{}",
        std::process::id()
    );
    let signing_key_id = format!("radishlex-manager-product-smoke-key-{now_ms}");
    let cleanup_handle =
        match DeviceSigningKeyHandle::apple_keychain_p256(&device_id, &signing_key_id, now_ms) {
            Ok(handle) => handle,
            Err(_) => return summary,
        };
    let _cleanup = ProductSmokeCleanup::new(cleanup_handle.clone(), now_ms + 1);
    summary.cleanup_attempted = 1;

    let store = AppleKeychainP256DeviceKeyStore::new();
    let public_key = match store.create_signing_key(&device_id, &signing_key_id, now_ms) {
        Ok(public_key) => public_key,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED;
            return summary;
        }
    };
    summary.created = 1;

    let reloaded_store = AppleKeychainP256DeviceKeyStore::new();
    let handle = match reloaded_store.handle(&device_id, &signing_key_id) {
        Ok(handle) => handle,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
    };
    let loaded_public_key = match reloaded_store.public_key(&handle) {
        Ok(loaded_public_key) if loaded_public_key.public_key == public_key.public_key => {
            loaded_public_key
        }
        Ok(_) => {
            summary.error_category = RADISHLEX_APPLE_P256_ERROR_CORRUPTED;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
    };
    summary.reloaded = 1;

    let canonical = canonical_signature_bytes(
        "apple_keychain_p256_manager_product_smoke",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = match reloaded_store.sign(&handle, &canonical) {
        Ok(signature) => signature,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
            return summary;
        }
    };
    if signature
        .verify_at(&loaded_public_key, &canonical, now_ms)
        .is_err()
    {
        summary.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        summary.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
        return summary;
    }
    summary.rust_verified = 1;

    let go_status = Command::new("go")
        .args([
            "test",
            "./internal/storage",
            "-run",
            "^TestExternalP256SignatureSmoke$",
            "-count=1",
        ])
        .current_dir(go_server_dir)
        .env("GOCACHE", "/tmp/radishlex-manager-apple-p256-go-cache")
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
        summary.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        summary.result = RADISHLEX_APPLE_P256_SMOKE_GO_VERIFY_FAILED;
        return summary;
    }
    summary.go_verified = 1;

    if let Err(error) = reloaded_store.delete_or_revoke(&handle, now_ms + 1) {
        record_product_error(&mut summary, &error);
        summary.result = RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED;
        return summary;
    }
    summary.deleted = 1;
    let revoked_failed_closed = matches!(
        reloaded_store.sign(&handle, &canonical),
        Err(CryptoError::PrivateKeyRevoked { .. })
    );
    let fresh_store = AppleKeychainP256DeviceKeyStore::new();
    let missing_result = fresh_store.handle(&device_id, &signing_key_id);
    let missing_confirmed = matches!(
        &missing_result,
        Err(CryptoError::PrivateKeyUnavailable { .. })
    );
    if !missing_confirmed || !revoked_failed_closed {
        if let Err(error) = missing_result {
            record_product_error(&mut summary, &error);
        } else {
            summary.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        }
        summary.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        return summary;
    }
    summary.missing_confirmed = 1;
    summary.fail_closed = 1;
    summary.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
    summary
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_denied_create(
    mut summary: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{AppleKeychainP256DeviceKeyStore, DeviceSigningKeyHandle};

    let now_ms = match product_smoke_now_ms() {
        Some(now_ms) => now_ms,
        None => return summary,
    };
    let device_id = format!(
        "radishlex-manager-denied-probe-device-{}",
        std::process::id()
    );
    let signing_key_id = format!("radishlex-manager-denied-probe-key-{now_ms}");
    let cleanup_handle =
        match DeviceSigningKeyHandle::apple_keychain_p256(&device_id, &signing_key_id, now_ms) {
            Ok(handle) => handle,
            Err(_) => return summary,
        };
    let _cleanup = ProductSmokeCleanup::new(cleanup_handle, now_ms + 1);
    summary.cleanup_attempted = 1;

    match AppleKeychainP256DeviceKeyStore::new().create_signing_key(
        &device_id,
        &signing_key_id,
        now_ms,
    ) {
        Ok(_) => {
            summary.created = 1;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED;
        }
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.fail_closed = 1;
            if summary.error_category == RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED {
                summary.expected_failure_confirmed = 1;
                summary.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
            } else {
                summary.result = RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY;
            }
        }
    }
    summary
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_prepare_locked_sign(
    mut summary: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleKeychainP256DeviceKeyStore, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(now_ms) => now_ms,
        None => return summary,
    };
    let cleanup_handle = match locked_matrix_handle(now_ms) {
        Some(handle) => handle,
        None => return summary,
    };
    if let Err(error) =
        AppleKeychainP256DeviceKeyStore::new().delete_or_revoke(&cleanup_handle, now_ms)
    {
        record_product_error(&mut summary, &error);
        summary.cleanup_required = 1;
        summary.result = RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED;
        return summary;
    }

    let store = AppleKeychainP256DeviceKeyStore::new();
    let public_key = match store.create_signing_key(
        LOCKED_MATRIX_DEVICE_ID,
        LOCKED_MATRIX_SIGNING_KEY_ID,
        now_ms,
    ) {
        Ok(public_key) => public_key,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED;
            return summary;
        }
    };
    summary.created = 1;
    summary.cleanup_required = 1;

    let reloaded_store = AppleKeychainP256DeviceKeyStore::new();
    let handle = match reloaded_store.handle(LOCKED_MATRIX_DEVICE_ID, LOCKED_MATRIX_SIGNING_KEY_ID)
    {
        Ok(handle) => handle,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
    };
    let loaded_public_key = match reloaded_store.public_key(&handle) {
        Ok(loaded_public_key) if loaded_public_key.public_key == public_key.public_key => {
            loaded_public_key
        }
        Ok(_) => {
            summary.error_category = RADISHLEX_APPLE_P256_ERROR_CORRUPTED;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED;
            return summary;
        }
    };
    summary.reloaded = 1;
    let canonical = canonical_signature_bytes(
        "apple_keychain_p256_manager_locked_matrix",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let signature = match reloaded_store.sign(&handle, &canonical) {
        Ok(signature) => signature,
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
            return summary;
        }
    };
    if signature
        .verify_at(&loaded_public_key, &canonical, now_ms)
        .is_err()
    {
        summary.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
        summary.result = RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED;
        return summary;
    }
    summary.rust_verified = 1;
    summary.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
    summary
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_expected_locked_sign(
    mut summary: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{
        canonical_signature_bytes, AppleKeychainP256DeviceKeyStore, SignatureField,
    };

    let now_ms = match product_smoke_now_ms() {
        Some(now_ms) => now_ms,
        None => return summary,
    };
    let handle = match locked_matrix_handle(now_ms) {
        Some(handle) => handle,
        None => return summary,
    };
    summary.cleanup_required = 1;
    let canonical = canonical_signature_bytes(
        "apple_keychain_p256_manager_locked_matrix",
        &[SignatureField::text("smoke", "synthetic")],
    );
    let store = AppleKeychainP256DeviceKeyStore::new();
    match store.sign(&handle, &canonical) {
        Ok(_) => {
            summary.result = RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED;
            summary.cleanup_attempted = 1;
            if store.delete_or_revoke(&handle, now_ms + 1).is_ok() {
                summary.deleted = 1;
                summary.cleanup_required = 0;
            }
        }
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.fail_closed = 1;
            if summary.error_category == RADISHLEX_APPLE_P256_ERROR_LOCKED {
                summary.expected_failure_confirmed = 1;
                summary.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
            } else {
                summary.result = RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY;
            }
        }
    }
    summary
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn run_cleanup_locked_sign(
    mut summary: RadishLexAppleP256ProductSmokeSummary,
) -> RadishLexAppleP256ProductSmokeSummary {
    use radishlex_ime_crypto::{AppleKeychainP256DeviceKeyStore, CryptoError};

    let now_ms = match product_smoke_now_ms() {
        Some(now_ms) => now_ms,
        None => return summary,
    };
    let handle = match locked_matrix_handle(now_ms) {
        Some(handle) => handle,
        None => return summary,
    };
    summary.cleanup_attempted = 1;
    let store = AppleKeychainP256DeviceKeyStore::new();
    if let Err(error) = store.delete_or_revoke(&handle, now_ms + 1) {
        record_product_error(&mut summary, &error);
        summary.cleanup_required = 1;
        summary.result = RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED;
        return summary;
    }
    summary.deleted = 1;
    match AppleKeychainP256DeviceKeyStore::new()
        .handle(LOCKED_MATRIX_DEVICE_ID, LOCKED_MATRIX_SIGNING_KEY_ID)
    {
        Err(CryptoError::PrivateKeyUnavailable { .. }) => {
            summary.missing_confirmed = 1;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_PASSED;
        }
        Err(error) => {
            record_product_error(&mut summary, &error);
            summary.cleanup_required = 1;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        }
        Ok(_) => {
            summary.error_category = RADISHLEX_APPLE_P256_ERROR_OTHER;
            summary.cleanup_required = 1;
            summary.result = RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED;
        }
    }
    summary
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_MATRIX_DEVICE_ID: &str = "radishlex-manager-locked-matrix-device-v1";
#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
const LOCKED_MATRIX_SIGNING_KEY_ID: &str = "radishlex-manager-locked-matrix-key-v1";

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
fn locked_matrix_handle(
    created_at_ms: i64,
) -> Option<radishlex_ime_crypto::DeviceSigningKeyHandle> {
    radishlex_ime_crypto::DeviceSigningKeyHandle::apple_keychain_p256(
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
    summary: &mut RadishLexAppleP256ProductSmokeSummary,
    error: &radishlex_ime_crypto::CryptoError,
) {
    let (category, detail, platform_status) = product_error(error);
    summary.error_category = category;
    summary.error_detail = detail;
    summary.platform_status = platform_status;
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
struct ProductSmokeCleanup {
    handle: radishlex_ime_crypto::DeviceSigningKeyHandle,
    revoked_at_ms: i64,
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
impl ProductSmokeCleanup {
    fn new(handle: radishlex_ime_crypto::DeviceSigningKeyHandle, revoked_at_ms: i64) -> Self {
        Self {
            handle,
            revoked_at_ms,
        }
    }
}

#[cfg(all(feature = "apple-keychain", target_os = "macos"))]
impl Drop for ProductSmokeCleanup {
    fn drop(&mut self) {
        let store = radishlex_ime_crypto::AppleKeychainP256DeviceKeyStore::new();
        let _ = store.delete_or_revoke(&self.handle, self.revoked_at_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_keeps_product_and_user_sync_gates_closed() {
        let status = RadishLexAppleP256ProductStatus::current();
        assert_eq!(status.version, RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION);
        assert_eq!(
            status.runtime_available,
            u32::from(cfg!(all(feature = "apple-keychain", target_os = "macos")))
        );
        assert_eq!(status.product_qualified, 0);
        assert_eq!(status.user_sync_enabled, 0);
        assert_eq!(
            status.exportable,
            u32::from(cfg!(feature = "apple-keychain"))
        );
        assert_eq!(status.hardware_backed, 0);
        assert_eq!(status.user_presence_required, 0);
        assert_eq!(status.backup_migratable, 0);
    }

    #[test]
    fn summary_contains_only_fixed_flags() {
        let summary = RadishLexAppleP256ProductSmokeSummary::new(
            RADISHLEX_APPLE_P256_SCENARIO_LIFECYCLE,
            RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED,
        );
        let debug = format!("{summary:?}");
        assert!(!debug.contains("canonical"));
        assert!(!debug.contains("signature"));
        assert!(!debug.contains("private"));
        assert!(!debug.contains("seed"));
        assert_eq!(summary.product_qualified, 0);
        assert_eq!(summary.user_sync_enabled, 0);
    }

    #[cfg(all(feature = "apple-keychain", target_os = "macos"))]
    #[test]
    fn product_errors_map_to_fixed_redacted_categories() {
        use radishlex_ime_crypto::{CryptoError, PrivateKeyAccessDeniedReason};

        assert_eq!(
            product_error(&CryptoError::PrivateKeyLocked {
                key_id: "synthetic-key".to_owned(),
            }),
            (
                RADISHLEX_APPLE_P256_ERROR_LOCKED,
                RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
                -25308,
            )
        );
        assert_eq!(
            product_error(&CryptoError::PrivateKeyAccessDenied {
                key_id: "synthetic-key".to_owned(),
                reason: PrivateKeyAccessDeniedReason::MissingEntitlement,
            }),
            (
                RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED,
                RADISHLEX_APPLE_P256_ERROR_DETAIL_MISSING_ENTITLEMENT,
                -34018,
            )
        );
        assert_eq!(
            product_error(&CryptoError::PrivateKeyUserPresenceRequired {
                key_id: "synthetic-key".to_owned(),
            }),
            (
                RADISHLEX_APPLE_P256_ERROR_USER_PRESENCE_REQUIRED,
                RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE,
                -25315,
            )
        );
    }
}
