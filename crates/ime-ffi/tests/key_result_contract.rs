use std::ffi::CStr;
use std::ptr;
use std::slice;
use std::thread;

use radishlex_ime_ffi::{
    radishlex_error_code, radishlex_error_free, radishlex_error_message,
    radishlex_key_result_commit, radishlex_key_result_commit_present,
    radishlex_key_result_consumed, radishlex_key_result_free, radishlex_key_result_snapshot,
    radishlex_key_result_version, radishlex_session_free, radishlex_session_handle_key_event,
    radishlex_session_new, radishlex_snapshot_candidate_count, radishlex_snapshot_preedit,
    RadishLexError, RadishLexKeyEvent, RadishLexKeyResult, RadishLexSession, RadishLexStatusCode,
    RadishLexStringView, RADISHLEX_KEY_PHASE_RELEASE, RADISHLEX_KEY_RESULT_VERSION,
    RADISHLEX_NAMED_KEY_ENTER, RADISHLEX_NAMED_KEY_TAB,
};

#[test]
fn key_result_keeps_consumed_commit_and_snapshot_from_one_event() {
    let mut error = ptr::null_mut();
    let session = radishlex_session_new(&mut error);
    assert!(!session.is_null());
    assert!(error.is_null());

    let ignored = handle_key(
        session,
        RadishLexKeyEvent::press_named(RADISHLEX_NAMED_KEY_TAB),
        &mut error,
    );
    assert_eq!(
        unsafe { radishlex_key_result_version(ignored) },
        RADISHLEX_KEY_RESULT_VERSION
    );
    assert_eq!(unsafe { radishlex_key_result_consumed(ignored) }, 0);
    assert_eq!(unsafe { radishlex_key_result_commit_present(ignored) }, 0);
    assert!(unsafe { view_to_owned(radishlex_key_result_commit(ignored)) }.is_empty());
    let ignored_snapshot = unsafe { radishlex_key_result_snapshot(ignored) };
    assert!(!ignored_snapshot.is_null());
    assert!(unsafe { view_to_owned(radishlex_snapshot_preedit(ignored_snapshot)) }.is_empty());
    unsafe {
        radishlex_key_result_free(ignored);
    }

    for ch in "luobo".chars() {
        let result = handle_key(session, RadishLexKeyEvent::press_char(ch), &mut error);
        assert_eq!(unsafe { radishlex_key_result_consumed(result) }, 1);
        assert_eq!(unsafe { radishlex_key_result_commit_present(result) }, 0);
        unsafe {
            radishlex_key_result_free(result);
        }
    }

    let commit_result = handle_key(
        session,
        RadishLexKeyEvent::press_named(RADISHLEX_NAMED_KEY_ENTER),
        &mut error,
    );
    assert_eq!(unsafe { radishlex_key_result_consumed(commit_result) }, 1);
    assert_eq!(
        unsafe { radishlex_key_result_commit_present(commit_result) },
        1
    );
    assert_eq!(
        unsafe { view_to_owned(radishlex_key_result_commit(commit_result)) },
        "luobo"
    );

    let commit_snapshot = unsafe { radishlex_key_result_snapshot(commit_result) };
    assert!(!commit_snapshot.is_null());
    assert!(unsafe { view_to_owned(radishlex_snapshot_preedit(commit_snapshot)) }.is_empty());
    assert_eq!(radishlex_snapshot_candidate_count(commit_snapshot), 0);

    unsafe {
        radishlex_key_result_free(commit_result);
        radishlex_session_free(session);
    }
}

#[test]
fn failed_key_handling_returns_no_partial_result() {
    let mut error = ptr::null_mut();
    let session = radishlex_session_new(&mut error);
    assert!(!session.is_null());

    let mut result = session.cast::<RadishLexKeyResult>();
    let status = unsafe {
        radishlex_session_handle_key_event(
            session,
            RadishLexKeyEvent {
                phase: RADISHLEX_KEY_PHASE_RELEASE + 10,
                ..RadishLexKeyEvent::press_char('l')
            },
            &mut result,
            &mut error,
        )
    };

    assert_eq!(status, RadishLexStatusCode::InvalidArgument);
    assert!(result.is_null());
    assert_eq!(
        radishlex_error_code(error),
        RadishLexStatusCode::InvalidArgument
    );
    assert!(unsafe { error_message(error) }.contains("unknown key phase code"));
    unsafe {
        radishlex_error_free(error);
    }

    let status = unsafe {
        radishlex_session_handle_key_event(
            session,
            RadishLexKeyEvent::press_char('l'),
            ptr::null_mut(),
            &mut error,
        )
    };
    assert_eq!(status, RadishLexStatusCode::InvalidArgument);
    assert!(unsafe { error_message(error) }.contains("key result output pointer is null"));

    unsafe {
        radishlex_error_free(error);
        radishlex_session_free(session);
    }
}

#[test]
fn key_result_handler_rejects_non_owner_thread() {
    let mut error = ptr::null_mut();
    let session = radishlex_session_new(&mut error);
    assert!(!session.is_null());

    let session_addr = session as usize;
    let (status, result_is_null, code, message) = thread::spawn(move || {
        let session = session_addr as *mut RadishLexSession;
        let mut result = ptr::null_mut();
        let mut error: *mut RadishLexError = ptr::null_mut();
        let status = unsafe {
            radishlex_session_handle_key_event(
                session,
                RadishLexKeyEvent::press_char('l'),
                &mut result,
                &mut error,
            )
        };
        let code = radishlex_error_code(error);
        let message = unsafe { error_message(error) };
        unsafe {
            radishlex_error_free(error);
        }
        (status, result.is_null(), code, message)
    })
    .join()
    .expect("thread joins");

    assert_eq!(status, RadishLexStatusCode::InvalidState);
    assert!(result_is_null);
    assert_eq!(code, RadishLexStatusCode::InvalidState);
    assert!(message.contains("thread that created it"));

    unsafe {
        radishlex_session_free(session);
    }
}

#[test]
fn null_key_result_accessors_are_empty_and_release_is_safe() {
    unsafe {
        assert_eq!(radishlex_key_result_version(ptr::null()), 0);
        assert_eq!(radishlex_key_result_consumed(ptr::null()), 0);
        assert_eq!(radishlex_key_result_commit_present(ptr::null()), 0);
        assert!(radishlex_key_result_commit(ptr::null()).data.is_null());
        assert!(radishlex_key_result_snapshot(ptr::null()).is_null());
        radishlex_key_result_free(ptr::null_mut());
    }
}

fn handle_key(
    session: *mut RadishLexSession,
    event: RadishLexKeyEvent,
    error: *mut *mut RadishLexError,
) -> *mut RadishLexKeyResult {
    let mut result = ptr::null_mut();
    let status = unsafe { radishlex_session_handle_key_event(session, event, &mut result, error) };
    assert_eq!(status, RadishLexStatusCode::Ok);
    assert!(!result.is_null());
    result
}

unsafe fn view_to_owned(view: RadishLexStringView) -> String {
    if view.len == 0 {
        return String::new();
    }
    let bytes = slice::from_raw_parts(view.data, view.len);
    String::from_utf8(bytes.to_vec()).expect("view must be UTF-8")
}

unsafe fn error_message(error: *const RadishLexError) -> String {
    CStr::from_ptr(radishlex_error_message(error))
        .to_string_lossy()
        .into_owned()
}
