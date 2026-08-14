use std::ffi::CStr;
use std::os::raw::c_char;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use radishlex_linux_product_install::{
    inspect_linux_startup, LinuxStartupBuildIdentity, LinuxStartupComponent, LinuxStartupPaths,
    LinuxSystemStartupPort,
};
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt;

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::ffi_status;

pub const RADISHLEX_LINUX_PRODUCT_STARTUP_REQUEST_VERSION: u32 = 1;
pub const RADISHLEX_LINUX_PRODUCT_STARTUP_RESULT_VERSION: u32 = 1;

pub const RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED: u32 = 1;
pub const RADISHLEX_LINUX_STARTUP_BUILD_DEBIAN_SYSTEM_PRODUCT: u32 = 2;

pub const RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER: u32 = 1;
pub const RADISHLEX_LINUX_STARTUP_COMPONENT_FCITX_ADDON: u32 = 2;

pub const RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT: u32 = 1;
pub const RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT: u32 = 2;
pub const RADISHLEX_LINUX_STARTUP_MAINTENANCE_REQUIRED: u32 = 3;
pub const RADISHLEX_LINUX_STARTUP_FAILED_CLOSED: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RadishLexLinuxProductStartupRequest {
    pub version: u32,
    pub build_identity: u32,
    pub component: u32,
    pub component_path: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexLinuxProductStartupResult {
    pub version: u32,
    pub decision: u32,
    pub reason: u32,
    pub receipt_state: u32,
}

impl RadishLexLinuxProductStartupResult {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            decision: 0,
            reason: 0,
            receipt_state: 0,
        }
    }
}

#[no_mangle]
/// Inspects the fixed Linux package receipt, dpkg state, and running component.
///
/// The operation is read-only. It never creates the state root, removes a stale
/// guard, opens user XDG data, or attempts package recovery.
///
/// # Safety
/// `request` and `result_out` must be live for the call. `component_path` must
/// be a live NUL-terminated path. `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_linux_product_startup_gate(
    request: *const RadishLexLinuxProductStartupRequest,
    result_out: *mut RadishLexLinuxProductStartupResult,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if request.is_null() || result_out.is_null() {
            return Err(FfiError::invalid_argument(
                "Linux startup request or output is null",
            ));
        }
        unsafe { *result_out = RadishLexLinuxProductStartupResult::empty() };
        let request = unsafe { &*request };
        if request.version != RADISHLEX_LINUX_PRODUCT_STARTUP_REQUEST_VERSION
            || request.component_path.is_null()
        {
            return Err(FfiError::invalid_argument(
                "Linux startup request version or component path is invalid",
            ));
        }
        let build_identity = parse_build_identity(request.build_identity)?;
        let component = parse_component(request.component)?;
        let component_path = unsafe { CStr::from_ptr(request.component_path) };
        if component_path.to_bytes().is_empty() {
            return Err(FfiError::invalid_argument(
                "Linux startup component path is empty",
            ));
        }
        let outcome =
            inspect_platform_startup(build_identity, component, component_path.to_bytes())?;
        unsafe {
            *result_out = RadishLexLinuxProductStartupResult {
                version: RADISHLEX_LINUX_PRODUCT_STARTUP_RESULT_VERSION,
                decision: outcome.0,
                reason: outcome.1,
                receipt_state: outcome.2,
            };
        }
        Ok(())
    })
}

#[cfg(target_os = "linux")]
fn parse_build_identity(value: u32) -> Result<LinuxStartupBuildIdentity, FfiError> {
    match value {
        RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED => {
            Ok(LinuxStartupBuildIdentity::DevelopmentStaged)
        }
        RADISHLEX_LINUX_STARTUP_BUILD_DEBIAN_SYSTEM_PRODUCT => {
            Ok(LinuxStartupBuildIdentity::DebianSystemProduct)
        }
        _ => Err(FfiError::invalid_argument(
            "Linux startup build identity is unknown",
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn parse_build_identity(value: u32) -> Result<u32, FfiError> {
    match value {
        RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED
        | RADISHLEX_LINUX_STARTUP_BUILD_DEBIAN_SYSTEM_PRODUCT => Ok(value),
        _ => Err(FfiError::invalid_argument(
            "Linux startup build identity is unknown",
        )),
    }
}

#[cfg(target_os = "linux")]
fn parse_component(value: u32) -> Result<LinuxStartupComponent, FfiError> {
    match value {
        RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER => Ok(LinuxStartupComponent::Manager),
        RADISHLEX_LINUX_STARTUP_COMPONENT_FCITX_ADDON => Ok(LinuxStartupComponent::FcitxAddon),
        _ => Err(FfiError::invalid_argument(
            "Linux startup component is unknown",
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn parse_component(value: u32) -> Result<u32, FfiError> {
    match value {
        RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER
        | RADISHLEX_LINUX_STARTUP_COMPONENT_FCITX_ADDON => Ok(value),
        _ => Err(FfiError::invalid_argument(
            "Linux startup component is unknown",
        )),
    }
}

#[cfg(target_os = "linux")]
fn inspect_platform_startup(
    build_identity: LinuxStartupBuildIdentity,
    component: LinuxStartupComponent,
    component_path: &[u8],
) -> Result<(u32, u32, u32), FfiError> {
    let component_path = PathBuf::from(std::ffi::OsStr::from_bytes(component_path));
    let paths = LinuxStartupPaths::system();
    let port = LinuxSystemStartupPort::new(
        paths.clone(),
        component_path,
        crate::RADISHLEX_ABI_CONTRACT_VERSION,
    );
    let outcome = inspect_linux_startup(&paths, build_identity, component, &port);
    Ok((
        outcome.decision() as u32,
        outcome.reason() as u32,
        outcome.receipt_state_code(),
    ))
}

#[cfg(not(target_os = "linux"))]
fn inspect_platform_startup(
    _build_identity: u32,
    _component: u32,
    _component_path: &[u8],
) -> Result<(u32, u32, u32), FfiError> {
    Err(FfiError::invalid_state(
        "Linux startup gate is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn ffi_rejects_unknown_compile_identity_and_component() {
        let component_path = CString::new("/synthetic/component").expect("component path");
        let mut result = RadishLexLinuxProductStartupResult::empty();
        let mut error = std::ptr::null_mut();
        let mut request = RadishLexLinuxProductStartupRequest {
            version: RADISHLEX_LINUX_PRODUCT_STARTUP_REQUEST_VERSION,
            build_identity: 99,
            component: RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER,
            component_path: component_path.as_ptr(),
        };
        assert_eq!(
            unsafe { radishlex_linux_product_startup_gate(&request, &mut result, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(result, RadishLexLinuxProductStartupResult::empty());
        unsafe { crate::radishlex_error_free(error) };

        error = std::ptr::null_mut();
        request.build_identity = RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED;
        request.component = 99;
        assert_eq!(
            unsafe { radishlex_linux_product_startup_gate(&request, &mut result, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(result, RadishLexLinuxProductStartupResult::empty());
        unsafe { crate::radishlex_error_free(error) };
    }

    #[test]
    fn stable_codes_match_the_rust_startup_contract() {
        assert_eq!(
            RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT,
            LinuxStartupDecisionCode::AllowedDevelopment as u32
        );
        assert_eq!(
            RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT,
            LinuxStartupDecisionCode::AllowedProduct as u32
        );
        assert_eq!(
            RADISHLEX_LINUX_STARTUP_MAINTENANCE_REQUIRED,
            LinuxStartupDecisionCode::MaintenanceRequired as u32
        );
        assert_eq!(
            RADISHLEX_LINUX_STARTUP_FAILED_CLOSED,
            LinuxStartupDecisionCode::FailedClosed as u32
        );
    }

    #[repr(u32)]
    enum LinuxStartupDecisionCode {
        AllowedDevelopment = 1,
        AllowedProduct = 2,
        MaintenanceRequired = 3,
        FailedClosed = 4,
    }
}
