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
