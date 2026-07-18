use std::os::raw::c_char;

use crate::dictionary::{
    add_user_term, delete_user_term, list_deleted_terms, list_user_terms, restore_user_term,
    RadishLexDeletedTermList, RadishLexDeletedTermView, RadishLexUserTermList,
    RadishLexUserTermView,
};
use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::{ffi_ptr, ffi_release, ffi_status};

use super::{read_optional_utf8, read_utf8};

#[no_mangle]
pub extern "C" fn radishlex_userdb_add_term(
    db_path: *const c_char,
    input_code: *const c_char,
    text: *const c_char,
    reading: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        add_user_term(
            read_utf8(db_path, "db_path")?,
            read_utf8(input_code, "input_code")?,
            read_utf8(text, "text")?,
            read_optional_utf8(reading, "reading")?,
        )
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_delete_term(
    db_path: *const c_char,
    input_code: *const c_char,
    text: *const c_char,
    reading: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        delete_user_term(
            read_utf8(db_path, "db_path")?,
            read_utf8(input_code, "input_code")?,
            read_utf8(text, "text")?,
            read_optional_utf8(reading, "reading")?,
        )
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_restore_term(
    db_path: *const c_char,
    input_code: *const c_char,
    text: *const c_char,
    reading: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        restore_user_term(
            read_utf8(db_path, "db_path")?,
            read_utf8(input_code, "input_code")?,
            read_utf8(text, "text")?,
            read_optional_utf8(reading, "reading")?,
        )
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_terms_new(
    db_path: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexUserTermList {
    ffi_ptr(error_out, || {
        let terms = list_user_terms(read_utf8(db_path, "db_path")?)?;
        Ok(Box::into_raw(Box::new(terms)))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_terms_count(terms: *const RadishLexUserTermList) -> usize {
    term_list_ref(terms).map_or(0, RadishLexUserTermList::len)
}

#[no_mangle]
/// # Safety
/// `terms` must be live and `term_out`/`error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_terms_get(
    terms: *const RadishLexUserTermList,
    index: usize,
    term_out: *mut RadishLexUserTermView,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if term_out.is_null() {
            return Err(FfiError::invalid_argument(
                "user term output pointer is null",
            ));
        }

        let view = term_list_ref(terms)?.term_view(index)?;
        unsafe {
            *term_out = view;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// `terms` must be null or a live handle released exactly once.
pub unsafe extern "C" fn radishlex_userdb_terms_free(terms: *mut RadishLexUserTermList) {
    ffi_release(|| {
        RadishLexUserTermList::free(terms);
    });
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_deleted_terms_new(
    db_path: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexDeletedTermList {
    ffi_ptr(error_out, || {
        let terms = list_deleted_terms(read_utf8(db_path, "db_path")?)?;
        Ok(Box::into_raw(Box::new(terms)))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_deleted_terms_count(
    terms: *const RadishLexDeletedTermList,
) -> usize {
    deleted_term_list_ref(terms).map_or(0, RadishLexDeletedTermList::len)
}

#[no_mangle]
/// # Safety
/// `terms` must be live and `term_out`/`error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_deleted_terms_get(
    terms: *const RadishLexDeletedTermList,
    index: usize,
    term_out: *mut RadishLexDeletedTermView,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if term_out.is_null() {
            return Err(FfiError::invalid_argument(
                "deleted term output pointer is null",
            ));
        }

        let term = deleted_term_list_ref(terms)?.tombstone_view(index)?;
        unsafe {
            *term_out = term;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// `terms` must be null or a live deleted term list handle released exactly once.
pub unsafe extern "C" fn radishlex_userdb_deleted_terms_free(terms: *mut RadishLexDeletedTermList) {
    ffi_release(|| {
        RadishLexDeletedTermList::free(terms);
    });
}

fn term_list_ref<'a>(
    terms: *const RadishLexUserTermList,
) -> Result<&'a RadishLexUserTermList, FfiError> {
    if terms.is_null() {
        return Err(FfiError::invalid_argument("user term list handle is null"));
    }
    Ok(unsafe { &*terms })
}

fn deleted_term_list_ref<'a>(
    terms: *const RadishLexDeletedTermList,
) -> Result<&'a RadishLexDeletedTermList, FfiError> {
    if terms.is_null() {
        return Err(FfiError::invalid_argument(
            "deleted term list handle is null",
        ));
    }
    Ok(unsafe { &*terms })
}
