use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use radishlex_ime_core::SchemaId;

use crate::buffer::RadishLexBuffer;
use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::session::RadishLexSession;

#[no_mangle]
pub extern "C" fn radishlex_session_new(
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSession {
    ffi_ptr(error_out, || {
        Ok(Box::into_raw(Box::new(RadishLexSession::new())))
    })
}

#[no_mangle]
pub unsafe extern "C" fn radishlex_session_free(session: *mut RadishLexSession) {
    if session.is_null() {
        return;
    }
    let _ = Box::from_raw(session);
}

#[no_mangle]
pub extern "C" fn radishlex_session_reset(
    session: *mut RadishLexSession,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        session_mut(session)?.inner_mut().reset()?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_set_schema(
    session: *mut RadishLexSession,
    schema: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        let schema = read_utf8(schema, "schema")?;
        let schema = SchemaId::new(schema)?;
        session_mut(session)?.inner_mut().set_schema(schema)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_push_key(
    session: *mut RadishLexSession,
    codepoint: u32,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        let ch = char::from_u32(codepoint).ok_or_else(|| {
            FfiError::invalid_argument("key codepoint is not a valid Unicode scalar value")
        })?;
        session_mut(session)?.push_char(ch)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_snapshot(
    session: *mut RadishLexSession,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexBuffer {
    ffi_ptr(error_out, || {
        let snapshot = session_mut(session)?.snapshot()?;
        Ok(RadishLexBuffer::from_string(snapshot))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_commit_candidate(
    session: *mut RadishLexSession,
    index: usize,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexBuffer {
    ffi_ptr(error_out, || {
        let commit = session_mut(session)?.inner_mut().commit_candidate(index)?;
        Ok(RadishLexBuffer::from_string(commit.text().to_owned()))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_buffer_data(buffer: *const RadishLexBuffer) -> *const u8 {
    if buffer.is_null() {
        return ptr::null();
    }
    unsafe { (*buffer).data() }
}

#[no_mangle]
pub extern "C" fn radishlex_buffer_len(buffer: *const RadishLexBuffer) -> usize {
    if buffer.is_null() {
        return 0;
    }
    unsafe { (*buffer).len() }
}

#[no_mangle]
pub unsafe extern "C" fn radishlex_buffer_free(buffer: *mut RadishLexBuffer) {
    RadishLexBuffer::free(buffer);
}

#[no_mangle]
pub extern "C" fn radishlex_error_code(error: *const RadishLexError) -> RadishLexStatusCode {
    if error.is_null() {
        return RadishLexStatusCode::InternalError;
    }
    unsafe { (*error).code() }
}

#[no_mangle]
pub extern "C" fn radishlex_error_message(error: *const RadishLexError) -> *const c_char {
    if error.is_null() {
        return ptr::null();
    }
    unsafe { (*error).message() }
}

#[no_mangle]
pub unsafe extern "C" fn radishlex_error_free(error: *mut RadishLexError) {
    RadishLexError::free(error);
}

fn session_mut<'a>(session: *mut RadishLexSession) -> Result<&'a mut RadishLexSession, FfiError> {
    if session.is_null() {
        return Err(FfiError::invalid_argument("session handle is null"));
    }
    Ok(unsafe { &mut *session })
}

fn read_utf8<'a>(value: *const c_char, field: &'static str) -> Result<&'a str, FfiError> {
    if value.is_null() {
        return Err(FfiError::invalid_argument(format!("{field} is null")));
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|_| FfiError::invalid_argument(format!("{field} must be valid UTF-8")))
}

fn ffi_status<F>(error_out: *mut *mut RadishLexError, f: F) -> RadishLexStatusCode
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

fn ffi_ptr<T, F>(error_out: *mut *mut RadishLexError, f: F) -> *mut T
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
    use std::ffi::{CStr, CString};
    use std::slice;

    use super::*;

    #[test]
    fn session_snapshot_and_commit_round_trip() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());
        assert!(error.is_null());

        let schema = CString::new("ffi.demo").expect("schema");
        assert_eq!(
            radishlex_session_set_schema(session, schema.as_ptr(), &mut error),
            RadishLexStatusCode::Ok
        );

        for ch in "luobo".chars() {
            assert_eq!(
                radishlex_session_push_key(session, ch as u32, &mut error),
                RadishLexStatusCode::Ok
            );
        }

        let snapshot = radishlex_session_snapshot(session, &mut error);
        assert!(!snapshot.is_null());
        let snapshot_text = unsafe { buffer_to_string(snapshot) };
        assert!(snapshot_text.contains("schema: ffi.demo"));
        assert!(snapshot_text.contains("composition: luobo"));
        assert!(snapshot_text.contains("0. 萝卜 [luobo]"));
        unsafe {
            radishlex_buffer_free(snapshot);
        }

        let commit = radishlex_session_commit_candidate(session, 1, &mut error);
        assert!(!commit.is_null());
        let commit_text = unsafe { buffer_to_string(commit) };
        assert_eq!(commit_text, "萝卜词核");
        unsafe {
            radishlex_buffer_free(commit);
            radishlex_session_free(session);
        }
    }

    #[test]
    fn null_session_returns_invalid_argument_error() {
        let mut error = ptr::null_mut();
        let status = radishlex_session_reset(ptr::null_mut(), &mut error);

        assert_eq!(status, RadishLexStatusCode::InvalidArgument);
        assert!(!error.is_null());
        assert_eq!(
            radishlex_error_code(error),
            RadishLexStatusCode::InvalidArgument
        );

        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("session handle is null"));
        unsafe {
            radishlex_error_free(error);
        }
    }

    #[test]
    fn invalid_utf8_schema_reports_argument_error() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());

        let invalid = [0xff_u8, 0];
        let status =
            radishlex_session_set_schema(session, invalid.as_ptr().cast::<c_char>(), &mut error);

        assert_eq!(status, RadishLexStatusCode::InvalidArgument);
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("valid UTF-8"));
        unsafe {
            radishlex_error_free(error);
            radishlex_session_free(session);
        }
    }

    #[test]
    fn invalid_candidate_index_reports_argument_error() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());

        let commit = radishlex_session_commit_candidate(session, 0, &mut error);
        assert!(commit.is_null());
        assert_eq!(
            radishlex_error_code(error),
            RadishLexStatusCode::InvalidArgument
        );
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("candidate index 0 is out of range"));

        unsafe {
            radishlex_error_free(error);
            radishlex_session_free(session);
        }
    }

    #[test]
    fn release_functions_accept_null() {
        unsafe {
            radishlex_session_free(ptr::null_mut());
            radishlex_buffer_free(ptr::null_mut());
            radishlex_error_free(ptr::null_mut());
        }
        assert!(radishlex_buffer_data(ptr::null()).is_null());
        assert_eq!(radishlex_buffer_len(ptr::null()), 0);
        assert!(radishlex_error_message(ptr::null()).is_null());
    }

    unsafe fn buffer_to_string(buffer: *mut RadishLexBuffer) -> String {
        let data = radishlex_buffer_data(buffer);
        let len = radishlex_buffer_len(buffer);
        let bytes = slice::from_raw_parts(data, len);
        String::from_utf8(bytes.to_vec()).expect("buffer must be UTF-8")
    }
}
