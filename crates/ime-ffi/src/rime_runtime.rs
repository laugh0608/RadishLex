use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::ffi_status;

#[no_mangle]
/// Finalizes the process Rime runtime after every Rime session is released.
///
/// # Safety
///
/// `error_out` must be null or point to writable storage for one
/// `RadishLexError*`. A non-null error returned through it must be released with
/// `radishlex_error_free`.
pub unsafe extern "C" fn radishlex_rime_runtime_shutdown(
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, shutdown_rime_runtime)
}

#[cfg(feature = "native-rime")]
fn shutdown_rime_runtime() -> Result<(), FfiError> {
    radishlex_ime_engine_rime::shutdown_process_runtime().map_err(FfiError::from)
}

#[cfg(not(feature = "native-rime"))]
fn shutdown_rime_runtime() -> Result<(), FfiError> {
    Err(FfiError::invalid_state(
        "rime runtime is not available; rebuild radishlex-ime-ffi with the native-rime feature",
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "native-rime"))]
    use std::ffi::CStr;
    use std::ptr;

    use super::*;
    use crate::error::RadishLexStatusCode;

    #[cfg(not(feature = "native-rime"))]
    #[test]
    fn shutdown_reports_unavailable_without_native_feature() {
        let mut error = ptr::null_mut();
        let status = unsafe { radishlex_rime_runtime_shutdown(&mut error) };

        assert_eq!(status, RadishLexStatusCode::InvalidState);
        assert_eq!(
            unsafe { (*error).code() },
            RadishLexStatusCode::InvalidState
        );
        let message = unsafe { CStr::from_ptr((*error).message()) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("native-rime feature"));
        unsafe {
            RadishLexError::free(error);
        }
    }

    #[cfg(feature = "native-rime")]
    #[test]
    fn shutdown_is_idempotent_before_runtime_initialization() {
        let mut error = ptr::null_mut();

        assert_eq!(
            unsafe { radishlex_rime_runtime_shutdown(&mut error) },
            RadishLexStatusCode::Ok
        );
        assert!(error.is_null());
    }
}
