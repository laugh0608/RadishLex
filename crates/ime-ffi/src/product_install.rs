use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use radishlex_ime_product_install::{
    inspect_install_startup_gate_with, InstallStartupGateDecision, InstallStartupGateErrorCode,
    InstallState,
};

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::ffi_status;

pub const RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_REQUEST_VERSION: u32 = 1;
pub const RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_RESULT_VERSION: u32 = 1;

pub const RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH: u32 = 1;
pub const RADISHLEX_INSTALL_GATE_ALLOWED_NO_INSTALL_STATE: u32 = 2;
pub const RADISHLEX_INSTALL_GATE_ALLOWED_TERMINAL_RECEIPT: u32 = 3;
pub const RADISHLEX_INSTALL_GATE_BLOCKED_IN_PROGRESS: u32 = 4;
pub const RADISHLEX_INSTALL_GATE_FAILED_CLOSED: u32 = 5;

pub const RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED: u32 = 10;
pub const RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED: u32 = 11;
pub const RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK: u32 = 14;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RadishLexProductInstallStartupGateRequest {
    pub version: u32,
    pub data_root_path: *const c_char,
    pub expected_owner_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexProductInstallStartupGateResult {
    pub version: u32,
    pub decision: u32,
    pub error_code: u32,
    pub receipt_state: u32,
}

impl RadishLexProductInstallStartupGateResult {
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
/// Performs the read-only outer product-install startup gate.
///
/// The running program identity is derived from the current executable and its
/// fixed installed bundle. It is never accepted from the caller.
///
/// # Safety
/// The request path must be a live NUL-terminated byte string. `result_out`
/// must be writable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_product_install_startup_gate(
    request: *const RadishLexProductInstallStartupGateRequest,
    result_out: *mut RadishLexProductInstallStartupGateResult,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if request.is_null() || result_out.is_null() {
            return Err(FfiError::invalid_argument(
                "install startup gate request or output is null",
            ));
        }
        unsafe { *result_out = RadishLexProductInstallStartupGateResult::empty() };
        let request = unsafe { &*request };
        if request.version != RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_REQUEST_VERSION
            || request.data_root_path.is_null()
        {
            return Err(FfiError::invalid_argument(
                "install startup gate request version or path is invalid",
            ));
        }
        let path_bytes = unsafe { CStr::from_ptr(request.data_root_path) }.to_bytes();
        if path_bytes.is_empty() {
            return Err(FfiError::invalid_argument(
                "install startup gate data root path is empty",
            ));
        }
        let data_root = Path::new(std::ffi::OsStr::from_bytes(path_bytes));
        let outcome =
            inspect_install_startup_gate_with(data_root, request.expected_owner_id, || {
                running_program_identity(data_root)
            });
        unsafe {
            *result_out = RadishLexProductInstallStartupGateResult {
                version: RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_RESULT_VERSION,
                decision: decision_code(outcome.decision()),
                error_code: error_code(outcome.error_code()),
                receipt_state: outcome.receipt_state().map_or(0, state_code),
            };
        }
        Ok(())
    })
}

#[cfg(target_os = "macos")]
fn running_program_identity(
    data_root: &Path,
) -> Option<radishlex_ime_product_install::RunningProgramIdentity> {
    use radishlex_ime_product_install::ProgramComponent;
    use radishlex_macos_product_install::inspect_running_program_identity;

    let home = fixed_home_from_data_root(data_root)?;
    let executable = std::fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    for (component, bundle) in [
        (
            ProgramComponent::Manager,
            home.join("Applications/RadishLex Manager.app"),
        ),
        (
            ProgramComponent::InputMethod,
            home.join("Library/Input Methods/RadishLexInputMethod.app"),
        ),
    ] {
        let canonical_bundle = std::fs::canonicalize(&bundle).ok()?;
        if canonical_bundle != bundle || !is_direct_bundle_executable(&executable, &bundle) {
            continue;
        }
        return inspect_running_program_identity(&bundle, component).ok();
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn running_program_identity(
    _data_root: &Path,
) -> Option<radishlex_ime_product_install::RunningProgramIdentity> {
    None
}

#[cfg(target_os = "macos")]
fn fixed_home_from_data_root(data_root: &Path) -> Option<&Path> {
    let home = data_root.parent()?.parent()?.parent()?;
    (home.join("Library/Application Support/RadishLex") == data_root).then_some(home)
}

#[cfg(target_os = "macos")]
fn is_direct_bundle_executable(executable: &Path, bundle: &Path) -> bool {
    let Ok(relative) = executable.strip_prefix(bundle) else {
        return false;
    };
    let mut components = relative.components();
    matches!(
        (
            components.next(),
            components.next(),
            components.next(),
            components.next()
        ),
        (
            Some(std::path::Component::Normal(contents)),
            Some(std::path::Component::Normal(macos)),
            Some(std::path::Component::Normal(_)),
            None
        ) if contents == "Contents" && macos == "MacOS"
    )
}

const fn decision_code(decision: InstallStartupGateDecision) -> u32 {
    match decision {
        InstallStartupGateDecision::AllowedFirstLaunch => {
            RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH
        }
        InstallStartupGateDecision::AllowedNoInstallState => {
            RADISHLEX_INSTALL_GATE_ALLOWED_NO_INSTALL_STATE
        }
        InstallStartupGateDecision::AllowedTerminalReceipt => {
            RADISHLEX_INSTALL_GATE_ALLOWED_TERMINAL_RECEIPT
        }
        InstallStartupGateDecision::BlockedInstallInProgress => {
            RADISHLEX_INSTALL_GATE_BLOCKED_IN_PROGRESS
        }
        InstallStartupGateDecision::FailedClosed => RADISHLEX_INSTALL_GATE_FAILED_CLOSED,
    }
}

const fn error_code(code: InstallStartupGateErrorCode) -> u32 {
    match code {
        InstallStartupGateErrorCode::None => crate::RADISHLEX_STARTUP_GATE_ERROR_NONE,
        InstallStartupGateErrorCode::InstallInProgress => 1,
        InstallStartupGateErrorCode::ActiveGuard => 2,
        InstallStartupGateErrorCode::UnsafeDataRoot => 3,
        InstallStartupGateErrorCode::UnsafeStateDirectory => 4,
        InstallStartupGateErrorCode::InterruptedReceipt => 5,
        InstallStartupGateErrorCode::InvalidReceipt => 6,
        InstallStartupGateErrorCode::UnexpectedStateObject => 7,
        InstallStartupGateErrorCode::RootIdentityChanged => 8,
        InstallStartupGateErrorCode::ProgramIdentityChanged => 9,
        InstallStartupGateErrorCode::RemovedProgram => 10,
        InstallStartupGateErrorCode::Io => 11,
    }
}

const fn state_code(state: InstallState) -> u32 {
    match state {
        InstallState::Prepared => 1,
        InstallState::Quiesced => 2,
        InstallState::TargetStaged => 3,
        InstallState::SourcePreserved => 4,
        InstallState::ManagerCommitted => 5,
        InstallState::ProgramsCommitted => 6,
        InstallState::DataCoordinating => 7,
        InstallState::DataSettled => 8,
        InstallState::FinalVerified => 9,
        InstallState::Completed => RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED,
        InstallState::AbortedPreserved => RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED,
        InstallState::RollbackRequired => 12,
        InstallState::ProgramsRestored => 13,
        InstallState::RolledBack => RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn ffi_allows_absent_root_and_rejects_unknown_request_version() {
        let root = temp_path("absent");
        let path = CString::new(root.as_os_str().as_bytes()).expect("path");
        let mut result = RadishLexProductInstallStartupGateResult::empty();
        let mut error = std::ptr::null_mut();
        let request = RadishLexProductInstallStartupGateRequest {
            version: RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_REQUEST_VERSION,
            data_root_path: path.as_ptr(),
            expected_owner_id: fs::metadata(std::env::temp_dir())
                .expect("temp metadata")
                .uid(),
        };
        assert_eq!(
            unsafe { radishlex_product_install_startup_gate(&request, &mut result, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert!(error.is_null());
        assert_eq!(
            result,
            RadishLexProductInstallStartupGateResult {
                version: RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_RESULT_VERSION,
                decision: RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH,
                error_code: 0,
                receipt_state: 0,
            }
        );

        let invalid = RadishLexProductInstallStartupGateRequest {
            version: 99,
            ..request
        };
        assert_eq!(
            unsafe { radishlex_product_install_startup_gate(&invalid, &mut result, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(result.version, 0);
        unsafe { crate::radishlex_error_free(error) };
    }

    #[test]
    fn ffi_maps_blocked_failed_and_state_inputs_to_stable_numbers() {
        assert_eq!(
            decision_code(InstallStartupGateDecision::BlockedInstallInProgress),
            RADISHLEX_INSTALL_GATE_BLOCKED_IN_PROGRESS
        );
        assert_eq!(
            decision_code(InstallStartupGateDecision::FailedClosed),
            RADISHLEX_INSTALL_GATE_FAILED_CLOSED
        );
        assert_eq!(error_code(InstallStartupGateErrorCode::ActiveGuard), 2);
        assert_eq!(
            error_code(InstallStartupGateErrorCode::ProgramIdentityChanged),
            9
        );
        assert_eq!(state_code(InstallState::FinalVerified), 9);
        assert_eq!(state_code(InstallState::Completed), 10);
    }

    fn temp_path(label: &str) -> std::path::PathBuf {
        fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-install-ffi-{label}-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ))
    }
}
