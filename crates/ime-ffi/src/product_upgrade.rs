use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use radishlex_ime_product_upgrade::{
    inspect_startup_gate, StartupGateDecision, StartupGateErrorCode, UpgradeState,
    UPGRADE_VALIDATION_EVIDENCE_VERSION,
};
use radishlex_ime_userdb::UserDb;

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::ffi_status;

pub const RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION: u32 = 1;
pub const RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_RESULT_VERSION: u32 = 1;
pub const RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION: u32 = 1;
pub const RADISHLEX_INPUT_METHOD_UPGRADE_VALIDATION_REQUEST_VERSION: u32 = 1;

pub const RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH: u32 = 1;
pub const RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE: u32 = 2;
pub const RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT: u32 = 3;
pub const RADISHLEX_STARTUP_GATE_BLOCKED_UPGRADE_IN_PROGRESS: u32 = 4;
pub const RADISHLEX_STARTUP_GATE_FAILED_CLOSED: u32 = 5;

pub const RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED: u32 = 9;
pub const RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED: u32 = 10;
pub const RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK: u32 = 12;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RadishLexProductUpgradeStartupGateRequest {
    pub version: u32,
    pub data_root_path: *const c_char,
    pub expected_owner_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexProductUpgradeStartupGateResult {
    pub version: u32,
    pub decision: u32,
    pub error_code: u32,
    pub receipt_state: u32,
}

#[repr(C)]
pub struct RadishLexManagerUpgradeValidationRequest {
    pub version: u32,
    pub candidate_path: *const c_char,
    pub settings_path: *const c_char,
}

#[repr(C)]
pub struct RadishLexInputMethodUpgradeValidationRequest {
    pub version: u32,
    pub candidate_path: *const c_char,
    pub shared_data_path: *const c_char,
    pub validation_user_data_path: *const c_char,
    pub schema: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RadishLexUpgradeValidationSummary {
    pub version: u32,
    pub schema_version: i64,
    pub management_queries_checked: u32,
    pub settings_checked: u32,
    pub personalized_runtime_checked: u32,
    pub candidate_signals_read: u32,
}

impl RadishLexProductUpgradeStartupGateResult {
    const fn empty() -> Self {
        Self {
            version: 0,
            decision: 0,
            error_code: 0,
            receipt_state: 0,
        }
    }
}

#[no_mangle]
/// Performs the read-only product startup gate.
///
/// # Safety
/// The request path must be a live NUL-terminated byte string. `result_out`
/// must be writable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_product_upgrade_startup_gate(
    request: *const RadishLexProductUpgradeStartupGateRequest,
    result_out: *mut RadishLexProductUpgradeStartupGateResult,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if request.is_null() || result_out.is_null() {
            return Err(FfiError::invalid_argument(
                "startup gate request or output is null",
            ));
        }
        unsafe { *result_out = RadishLexProductUpgradeStartupGateResult::empty() };
        let request = unsafe { &*request };
        if request.version != RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION
            || request.data_root_path.is_null()
        {
            return Err(FfiError::invalid_argument(
                "startup gate request version or path is invalid",
            ));
        }
        let path_bytes = unsafe { CStr::from_ptr(request.data_root_path) }.to_bytes();
        if path_bytes.is_empty() {
            return Err(FfiError::invalid_argument(
                "startup gate data root path is empty",
            ));
        }
        let outcome = inspect_startup_gate(
            Path::new(std::ffi::OsStr::from_bytes(path_bytes)),
            request.expected_owner_id,
        );
        unsafe {
            *result_out = RadishLexProductUpgradeStartupGateResult {
                version: RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_RESULT_VERSION,
                decision: decision_code(outcome.decision()),
                error_code: error_code(outcome.error_code()),
                receipt_state: outcome.receipt_state().map_or(0, state_code),
            };
        }
        Ok(())
    })
}

#[no_mangle]
/// Validates the fixed Manager migration candidate through read-only queries.
///
/// # Safety
/// Request strings must be live NUL-terminated byte strings. `summary_out`
/// must be writable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_manager_upgrade_validate_candidate(
    request: *const RadishLexManagerUpgradeValidationRequest,
    summary_out: *mut RadishLexUpgradeValidationSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if request.is_null() || summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "manager validation request or output is null",
            ));
        }
        unsafe { *summary_out = RadishLexUpgradeValidationSummary::default() };
        let request = unsafe { &*request };
        if request.version != RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION {
            return Err(FfiError::invalid_argument(
                "manager validation request version is invalid",
            ));
        }
        let candidate = ffi_path(request.candidate_path, "candidate")?;
        let settings = ffi_path(request.settings_path, "settings")?;
        let db = UserDb::open_read_only_current(&candidate)?;
        let schema_version = db.schema_version()?;
        let _ = db.list_active_terms()?;
        let _ = db.list_deleted_term_tombstones()?;
        let _ = db.list_import_batches()?;
        let _ = db.learning_status_summary()?;
        validate_manager_settings(&settings)?;
        unsafe {
            *summary_out = RadishLexUpgradeValidationSummary {
                version: UPGRADE_VALIDATION_EVIDENCE_VERSION,
                schema_version,
                management_queries_checked: 1,
                settings_checked: 1,
                ..Default::default()
            };
        }
        Ok(())
    })
}

#[no_mangle]
/// Validates the fixed InputMethod migration candidate through native Rime.
///
/// # Safety
/// Request strings must be live NUL-terminated byte strings. `summary_out`
/// must be writable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_input_method_upgrade_validate_candidate(
    request: *const RadishLexInputMethodUpgradeValidationRequest,
    summary_out: *mut RadishLexUpgradeValidationSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || unsafe {
        validate_input_method_candidate(request, summary_out)
    })
}

#[cfg(feature = "native-rime")]
unsafe fn validate_input_method_candidate(
    request: *const RadishLexInputMethodUpgradeValidationRequest,
    summary_out: *mut RadishLexUpgradeValidationSummary,
) -> Result<(), FfiError> {
    use radishlex_ime_core::{KeyEvent, SchemaId};
    use radishlex_ime_engine_rime::{shutdown_process_runtime, RimeEngine, RimeEngineConfig};
    use radishlex_ime_runtime::{LearningContext, PersonalizationStatus, PersonalizedInputSession};

    if request.is_null() || summary_out.is_null() {
        return Err(FfiError::invalid_argument(
            "input method validation request or output is null",
        ));
    }
    *summary_out = RadishLexUpgradeValidationSummary::default();
    let request = &*request;
    if request.version != RADISHLEX_INPUT_METHOD_UPGRADE_VALIDATION_REQUEST_VERSION {
        return Err(FfiError::invalid_argument(
            "input method validation request version is invalid",
        ));
    }
    let candidate = ffi_path(request.candidate_path, "candidate")?;
    let shared = ffi_path(request.shared_data_path, "shared data")?;
    let user_data = ffi_path(request.validation_user_data_path, "validation user data")?;
    let schema = ffi_string(request.schema, "schema")?;
    let db = UserDb::open_read_only_current(&candidate)?;
    let schema_version = db.schema_version()?;
    let config = RimeEngineConfig::new(shared, user_data, SchemaId::new(schema)?)?
        .with_deploy_on_start(true);
    let engine = RimeEngine::new(config)?;
    let mut session = PersonalizedInputSession::with_userdb(engine, db, "upgrade-validation-v1")?;
    session.set_learning_context(LearningContext::new("general")?.with_privacy_mode(true));
    for value in "luobo".chars() {
        let _ = session.handle_key(KeyEvent::press_char(value))?;
    }
    let snapshot = session.snapshot()?;
    if snapshot.personalization_status() != PersonalizationStatus::Ready {
        return Err(FfiError::invalid_state(
            "candidate signals were not available read-only",
        ));
    }
    drop(session);
    shutdown_process_runtime()?;
    *summary_out = RadishLexUpgradeValidationSummary {
        version: UPGRADE_VALIDATION_EVIDENCE_VERSION,
        schema_version,
        personalized_runtime_checked: 1,
        candidate_signals_read: 1,
        ..Default::default()
    };
    Ok(())
}

#[cfg(not(feature = "native-rime"))]
unsafe fn validate_input_method_candidate(
    request: *const RadishLexInputMethodUpgradeValidationRequest,
    summary_out: *mut RadishLexUpgradeValidationSummary,
) -> Result<(), FfiError> {
    let _ = (request, summary_out);
    Err(FfiError::invalid_state(
        "native Rime validation is unavailable",
    ))
}

fn ffi_path(value: *const c_char, field: &'static str) -> Result<PathBuf, FfiError> {
    if value.is_null() {
        return Err(FfiError::invalid_argument(format!("{field} path is null")));
    }
    let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
    if bytes.is_empty() {
        return Err(FfiError::invalid_argument(format!("{field} path is empty")));
    }
    Ok(Path::new(std::ffi::OsStr::from_bytes(bytes)).to_path_buf())
}

#[cfg(feature = "native-rime")]
fn ffi_string(value: *const c_char, field: &'static str) -> Result<String, FfiError> {
    if value.is_null() {
        return Err(FfiError::invalid_argument(format!("{field} is null")));
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| FfiError::invalid_argument(format!("{field} is not UTF-8")))
}

fn validate_manager_settings(path: &Path) -> Result<(), FfiError> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path)
        .map_err(|_| FfiError::invalid_state("manager settings could not be read"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| FfiError::invalid_state("manager settings are invalid"))?;
    let object = value
        .as_object()
        .ok_or_else(|| FfiError::invalid_state("manager settings root is not an object"))?;
    if object
        .get("format_version")
        .and_then(serde_json::Value::as_i64)
        != Some(1)
    {
        return Err(FfiError::invalid_state(
            "manager settings format is unsupported",
        ));
    }
    for key in [
        "retain_sync_config",
        "privacy_mode",
        "diagnostics_export",
        "deployment_evidence_recorded",
        "access_token_configured",
    ] {
        if object.get(key).is_some_and(|value| !value.is_boolean()) {
            return Err(FfiError::invalid_state(
                "manager settings boolean field is invalid",
            ));
        }
    }
    for key in ["server_endpoint", "deployment_evidence_source"] {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Err(FfiError::invalid_state(
                "manager settings string field is invalid",
            ));
        }
    }
    Ok(())
}

const fn decision_code(value: StartupGateDecision) -> u32 {
    match value {
        StartupGateDecision::AllowedFirstLaunch => RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH,
        StartupGateDecision::AllowedNoUpgradeState => {
            RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE
        }
        StartupGateDecision::AllowedTerminalReceipt => {
            RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT
        }
        StartupGateDecision::BlockedUpgradeInProgress => {
            RADISHLEX_STARTUP_GATE_BLOCKED_UPGRADE_IN_PROGRESS
        }
        StartupGateDecision::FailedClosed => RADISHLEX_STARTUP_GATE_FAILED_CLOSED,
    }
}

const fn error_code(value: StartupGateErrorCode) -> u32 {
    match value {
        StartupGateErrorCode::None => crate::RADISHLEX_STARTUP_GATE_ERROR_NONE,
        StartupGateErrorCode::UpgradeInProgress => 1,
        StartupGateErrorCode::ActiveGuard => 2,
        StartupGateErrorCode::UnsafeDataRoot => 3,
        StartupGateErrorCode::UnsafeStateDirectory => 4,
        StartupGateErrorCode::InterruptedArtifact => 5,
        StartupGateErrorCode::InvalidReceipt => 6,
        StartupGateErrorCode::UnexpectedStateObject => 7,
        StartupGateErrorCode::IdentityChanged => 8,
        StartupGateErrorCode::Io => 9,
    }
}

const fn state_code(value: UpgradeState) -> u32 {
    match value {
        UpgradeState::Preflighted => 1,
        UpgradeState::Quiesced => 2,
        UpgradeState::SnapshotReady => 3,
        UpgradeState::CandidateMigrated => 4,
        UpgradeState::CandidateVerified => 5,
        UpgradeState::SwitchPrepared => 6,
        UpgradeState::Switched => 7,
        UpgradeState::PostSwitchVerified => 8,
        UpgradeState::Completed => RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED,
        UpgradeState::AbortedPreserved => RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED,
        UpgradeState::RollbackRequired => 11,
        UpgradeState::RolledBack => RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::ptr;

    use super::*;

    #[test]
    fn absent_data_root_is_allowed_and_request_version_is_strict() {
        let path = CString::new(format!(
            "/tmp/radishlex-ffi-startup-gate-{}-absent",
            std::process::id()
        ))
        .expect("path");
        let mut result = RadishLexProductUpgradeStartupGateResult::empty();
        let mut error = ptr::null_mut();
        let request = RadishLexProductUpgradeStartupGateRequest {
            version: RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION,
            data_root_path: path.as_ptr(),
            expected_owner_id: 0,
        };
        let status =
            unsafe { radishlex_product_upgrade_startup_gate(&request, &mut result, &mut error) };
        assert_eq!(status, RadishLexStatusCode::Ok);
        assert!(error.is_null());
        assert_eq!(result.decision, RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH);

        let bad = RadishLexProductUpgradeStartupGateRequest {
            version: 2,
            ..request
        };
        assert_eq!(
            unsafe { radishlex_product_upgrade_startup_gate(&bad, &mut result, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe { RadishLexError::free(error) };
    }

    #[test]
    fn manager_validation_is_read_only_and_rejects_a_corrupt_candidate() {
        let root = std::env::temp_dir().join(format!(
            "radishlex-manager-upgrade-validation-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("root mode");
        let candidate = root.join("migration-candidate.sqlite3");
        drop(UserDb::open(&candidate).expect("candidate"));
        UserDb::migrate_and_validate(&candidate).expect("standalone candidate");
        let settings = root.join("source-settings.json");
        fs::write(
            &settings,
            b"{\"format_version\":1,\"privacy_mode\":false}\n",
        )
        .expect("settings");
        let before = fs::read(&candidate).expect("before");
        let candidate_c = CString::new(candidate.as_os_str().as_bytes()).expect("candidate path");
        let settings_c = CString::new(settings.as_os_str().as_bytes()).expect("settings path");
        let request = RadishLexManagerUpgradeValidationRequest {
            version: 1,
            candidate_path: candidate_c.as_ptr(),
            settings_path: settings_c.as_ptr(),
        };
        let mut summary = RadishLexUpgradeValidationSummary::default();
        let mut error = ptr::null_mut();
        assert_eq!(
            unsafe {
                radishlex_manager_upgrade_validate_candidate(&request, &mut summary, &mut error)
            },
            RadishLexStatusCode::Ok
        );
        assert!(error.is_null());
        assert_eq!(summary.management_queries_checked, 1);
        assert_eq!(fs::read(&candidate).expect("after"), before);
        for suffix in ["-wal", "-shm", "-journal"] {
            assert!(!root
                .join(format!("migration-candidate.sqlite3{suffix}"))
                .exists());
        }

        fs::write(&candidate, b"corrupt\n").expect("corrupt");
        assert_ne!(
            unsafe {
                radishlex_manager_upgrade_validate_candidate(&request, &mut summary, &mut error)
            },
            RadishLexStatusCode::Ok
        );
        unsafe { RadishLexError::free(error) };
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn both_validation_contracts_fail_closed_when_candidate_cannot_open() {
        let root = std::env::temp_dir().join(format!(
            "radishlex-upgrade-validation-open-failure-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("root");
        let candidate = root.join("missing-candidate.sqlite3");
        let candidate_c = CString::new(candidate.as_os_str().as_bytes()).expect("candidate path");
        let settings_c = CString::new(root.join("missing-settings.json").as_os_str().as_bytes())
            .expect("settings path");
        let shared_c =
            CString::new(root.join("RimeData").as_os_str().as_bytes()).expect("shared data path");
        let user_data_c =
            CString::new(root.join("RimeUser").as_os_str().as_bytes()).expect("user data path");
        let schema_c = CString::new("radishlex_pinyin").expect("schema");
        let manager_request = RadishLexManagerUpgradeValidationRequest {
            version: RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION,
            candidate_path: candidate_c.as_ptr(),
            settings_path: settings_c.as_ptr(),
        };
        let input_request = RadishLexInputMethodUpgradeValidationRequest {
            version: RADISHLEX_INPUT_METHOD_UPGRADE_VALIDATION_REQUEST_VERSION,
            candidate_path: candidate_c.as_ptr(),
            shared_data_path: shared_c.as_ptr(),
            validation_user_data_path: user_data_c.as_ptr(),
            schema: schema_c.as_ptr(),
        };
        let mut summary = RadishLexUpgradeValidationSummary::default();
        let mut error = ptr::null_mut();
        assert_ne!(
            unsafe {
                radishlex_manager_upgrade_validate_candidate(
                    &manager_request,
                    &mut summary,
                    &mut error,
                )
            },
            RadishLexStatusCode::Ok
        );
        unsafe { RadishLexError::free(error) };
        error = ptr::null_mut();
        assert_ne!(
            unsafe {
                radishlex_input_method_upgrade_validate_candidate(
                    &input_request,
                    &mut summary,
                    &mut error,
                )
            },
            RadishLexStatusCode::Ok
        );
        unsafe { RadishLexError::free(error) };
        assert!(!candidate.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
