use std::ptr;

use radishlex_ime_core::{KeyOutcome, SessionState};

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::{ffi_release, ffi_status};
use crate::key::RadishLexKeyEvent;
use crate::session::{session_mut, RadishLexSession};
use crate::snapshot::{RadishLexSnapshot, RadishLexStringView};

pub const RADISHLEX_KEY_RESULT_VERSION: u32 = 1;

/// Rust-owned result of handling one platform key event.
///
/// This type intentionally does not implement `Debug`: an immediate commit may
/// contain user input and must not leak through generic diagnostics.
pub struct RadishLexKeyResult {
    version: u32,
    consumed: u8,
    commit: Option<String>,
    snapshot: RadishLexSnapshot,
}

impl RadishLexKeyResult {
    pub fn new(outcome: KeyOutcome, state: SessionState) -> Self {
        Self {
            version: RADISHLEX_KEY_RESULT_VERSION,
            consumed: u8::from(outcome.is_consumed()),
            commit: outcome.commit().map(|commit| commit.text().to_owned()),
            snapshot: RadishLexSnapshot::from_state(state),
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn consumed(&self) -> u8 {
        self.consumed
    }

    pub fn commit(&self) -> RadishLexStringView {
        self.commit
            .as_deref()
            .map_or_else(RadishLexStringView::empty, RadishLexStringView::from_str)
    }

    pub fn commit_present(&self) -> u8 {
        u8::from(self.commit.is_some())
    }

    pub fn snapshot(&self) -> *const RadishLexSnapshot {
        &self.snapshot
    }

    /// Releases a key result allocated by Rust.
    ///
    /// # Safety
    ///
    /// `result` must be null or a live pointer returned by the RadishLex key
    /// result ABI. A non-null pointer must be released exactly once and must not
    /// be used after this call.
    pub unsafe fn free(result: *mut Self) {
        if result.is_null() {
            return;
        }

        let _ = Box::from_raw(result);
    }
}

/// Handles one key event and returns an owned result from the same state transition.
///
/// # Safety
///
/// `session` must be null or a live `RadishLexSession` pointer owned by the
/// calling thread. `result_out` must point to writable storage for one result
/// pointer. `error_out` must be null or point to writable storage for one error
/// pointer. Output pointers must not alias each other.
#[no_mangle]
pub unsafe extern "C" fn radishlex_session_handle_key_event(
    session: *mut RadishLexSession,
    event: RadishLexKeyEvent,
    result_out: *mut *mut RadishLexKeyResult,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if result_out.is_null() {
            return Err(FfiError::invalid_argument(
                "key result output pointer is null",
            ));
        }

        unsafe {
            *result_out = ptr::null_mut();
        }

        let event = event.try_into()?;
        let session = session_mut(session)?;
        let outcome = session.push_key_event(event)?;
        let state = session.state()?;
        let result = Box::into_raw(Box::new(RadishLexKeyResult::new(outcome, state)));

        unsafe {
            *result_out = result;
        }
        Ok(())
    })
}

/// Selects a candidate from the current page and returns the resulting state.
///
/// A segmented engine can consume the selection while leaving `commit_present`
/// false and returning an updated composition in the snapshot.
///
/// # Safety
///
/// `session` must be null or a live `RadishLexSession` pointer owned by the
/// calling thread. `result_out` must point to writable storage for one result
/// pointer. `error_out` must be null or point to writable storage for one error
/// pointer. Output pointers must not alias each other.
#[no_mangle]
pub unsafe extern "C" fn radishlex_session_select_candidate(
    session: *mut RadishLexSession,
    index: usize,
    result_out: *mut *mut RadishLexKeyResult,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if result_out.is_null() {
            return Err(FfiError::invalid_argument(
                "candidate selection result output pointer is null",
            ));
        }

        unsafe {
            *result_out = ptr::null_mut();
        }

        let session = session_mut(session)?;
        let outcome = session.inner_mut().select_candidate(index)?;
        let state = session.state()?;
        let result = Box::into_raw(Box::new(RadishLexKeyResult::new(outcome, state)));

        unsafe {
            *result_out = result;
        }
        Ok(())
    })
}

/// Returns the schema version of a live key result.
///
/// # Safety
///
/// `result` must be null or point to a live `RadishLexKeyResult`.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_version(result: *const RadishLexKeyResult) -> u32 {
    key_result_ref(result).map_or(0, RadishLexKeyResult::version)
}

/// Returns whether the input method consumed the key (`0` or `1`).
///
/// # Safety
///
/// `result` must be null or point to a live `RadishLexKeyResult`.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_consumed(result: *const RadishLexKeyResult) -> u8 {
    key_result_ref(result).map_or(0, RadishLexKeyResult::consumed)
}

/// Returns the borrowed immediate commit view for a key result.
///
/// # Safety
///
/// `result` must be null or point to a live `RadishLexKeyResult`. The returned
/// view must be copied before the result is released.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_commit(
    result: *const RadishLexKeyResult,
) -> RadishLexStringView {
    key_result_ref(result).map_or_else(|_| RadishLexStringView::empty(), RadishLexKeyResult::commit)
}

/// Returns whether a key result has an immediate commit (`0` or `1`).
///
/// # Safety
///
/// `result` must be null or point to a live `RadishLexKeyResult`.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_commit_present(
    result: *const RadishLexKeyResult,
) -> u8 {
    key_result_ref(result).map_or(0, RadishLexKeyResult::commit_present)
}

/// Returns the snapshot borrowed from a key result.
///
/// # Safety
///
/// `result` must be null or point to a live `RadishLexKeyResult`. The returned
/// snapshot must not be freed separately and becomes invalid when the result is
/// released.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_snapshot(
    result: *const RadishLexKeyResult,
) -> *const RadishLexSnapshot {
    key_result_ref(result).map_or(ptr::null(), RadishLexKeyResult::snapshot)
}

/// Releases an owned key result and all of its borrowed views.
///
/// # Safety
///
/// `result` must be null or a live pointer returned through
/// `radishlex_session_handle_key_event`. A non-null result must be released
/// exactly once.
#[no_mangle]
pub unsafe extern "C" fn radishlex_key_result_free(result: *mut RadishLexKeyResult) {
    ffi_release(|| unsafe {
        RadishLexKeyResult::free(result);
    });
}

fn key_result_ref<'a>(
    result: *const RadishLexKeyResult,
) -> Result<&'a RadishLexKeyResult, FfiError> {
    if result.is_null() {
        return Err(FfiError::invalid_argument("key result handle is null"));
    }
    Ok(unsafe { &*result })
}
