use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};

pub(crate) fn ffi_status<F>(error_out: *mut *mut RadishLexError, f: F) -> RadishLexStatusCode
where
    F: FnOnce() -> Result<(), FfiError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => {
            clear_error(error_out);
            RadishLexStatusCode::Ok
        }
        Ok(Err(error)) => {
            let code = error.code;
            write_error(error_out, error);
            code
        }
        Err(_) => {
            let error = FfiError::internal("panic caught at FFI boundary");
            let code = error.code;
            write_error(error_out, error);
            code
        }
    }
}

pub(crate) fn ffi_ptr<T, F>(error_out: *mut *mut RadishLexError, f: F) -> *mut T
where
    F: FnOnce() -> Result<*mut T, FfiError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => {
            clear_error(error_out);
            value
        }
        Ok(Err(error)) => {
            write_error(error_out, error);
            ptr::null_mut()
        }
        Err(_) => {
            write_error(
                error_out,
                FfiError::internal("panic caught at FFI boundary"),
            );
            ptr::null_mut()
        }
    }
}

pub(crate) fn ffi_release<F>(f: F)
where
    F: FnOnce(),
{
    let _ = catch_unwind(AssertUnwindSafe(f));
}

fn clear_error(error_out: *mut *mut RadishLexError) {
    if !error_out.is_null() {
        unsafe {
            *error_out = ptr::null_mut();
        }
    }
}

fn write_error(error_out: *mut *mut RadishLexError, error: FfiError) {
    if !error_out.is_null() {
        unsafe {
            *error_out = error.into_raw_error();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;
    use std::ptr;

    use super::*;

    #[test]
    fn panic_is_mapped_without_exposing_payload() {
        let mut error = ptr::null_mut();
        let status = ffi_status(&mut error, || -> Result<(), FfiError> {
            panic!("synthetic-private-panic-payload")
        });

        assert_eq!(status, RadishLexStatusCode::InternalError);
        assert!(!error.is_null());
        let message = unsafe { CStr::from_ptr((*error).message()) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("panic caught at FFI boundary"));
        assert!(!message.contains("synthetic-private-panic-payload"));
        unsafe {
            RadishLexError::free(error);
        }
    }

    #[test]
    fn pointer_and_release_boundaries_contain_panics() {
        let mut error = ptr::null_mut();
        let value = ffi_ptr::<u8, _>(&mut error, || -> Result<*mut u8, FfiError> {
            panic!("synthetic-pointer-panic")
        });
        assert!(value.is_null());
        assert!(!error.is_null());
        unsafe {
            RadishLexError::free(error);
        }

        ffi_release(|| panic!("synthetic-release-panic"));
    }
}
