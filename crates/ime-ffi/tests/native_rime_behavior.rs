#![cfg(feature = "native-rime")]

use std::env;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;
use std::slice;

use radishlex_ime_ffi::*;

#[test]
#[ignore = "requires RADISHLEX_RIME_SHARED_DATA and RADISHLEX_RIME_USER_DATA"]
fn rime_session_native_smoke_uses_ffi_entrypoint() {
    let shared_data = env::var("RADISHLEX_RIME_SHARED_DATA")
        .expect("RADISHLEX_RIME_SHARED_DATA must point to isolated Rime shared data");
    let user_data = env::var("RADISHLEX_RIME_USER_DATA")
        .expect("RADISHLEX_RIME_USER_DATA must point to isolated Rime user data");
    let schema =
        env::var("RADISHLEX_RIME_SCHEMA").unwrap_or_else(|_| "radishlex_pinyin".to_owned());
    let userdb_path = PathBuf::from(&user_data).join("userdb.sqlite3");

    let shared_data = CString::new(shared_data).expect("shared data path");
    let user_data = CString::new(user_data).expect("user data path");
    let schema = CString::new(schema).expect("schema");
    let userdb_path_text = userdb_path.to_string_lossy().into_owned();
    let userdb_path = CString::new(userdb_path_text.as_bytes()).expect("userdb path");
    let session_id = CString::new("native-rime-personalized-smoke").expect("session id");
    let mut error = ptr::null_mut();

    let options = RadishLexPersonalizedRimeSessionOptions {
        version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared_data.as_ptr(),
        user_data_dir: user_data.as_ptr(),
        schema: schema.as_ptr(),
        log_dir: ptr::null(),
        deploy_on_start: 1,
        userdb_path: userdb_path.as_ptr(),
        session_id: session_id.as_ptr(),
    };
    let session = radishlex_session_new_personalized_rime(&options, &mut error);
    assert!(
        !session.is_null(),
        "Rime session should be created: {}",
        unsafe { error_message(error) }
    );
    assert_eq!(
        radishlex_session_engine_kind(session),
        RADISHLEX_ENGINE_KIND_RIME
    );
    set_learning_context(session, false, false, true, &mut error);

    push_text(session, "luobo", &mut error);
    let snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert!(
        !snapshot.is_null(),
        "snapshot should be created: {}",
        unsafe { error_message(error) }
    );
    assert_expected_candidate_page_size(snapshot);
    assert_eq!(
        radishlex_snapshot_personalization_status(snapshot),
        RADISHLEX_PERSONALIZATION_STATUS_READY
    );
    assert!(radishlex_snapshot_candidate_count(snapshot) > 1);
    let partial_candidate = unsafe { candidate_text(snapshot, 1, &mut error) };
    let mut partial_selection = ptr::null_mut();
    assert_eq!(
        unsafe {
            radishlex_session_select_candidate(session, 1, &mut partial_selection, &mut error)
        },
        RadishLexStatusCode::Ok,
        "partial candidate selection: {}",
        unsafe { error_message(error) }
    );
    assert_eq!(
        unsafe { radishlex_key_result_consumed(partial_selection) },
        1
    );
    assert_eq!(
        unsafe { radishlex_key_result_commit_present(partial_selection) },
        0
    );
    assert_eq!(
        unsafe { radishlex_key_result_learning_disposition(partial_selection) },
        RADISHLEX_LEARNING_DEFERRED
    );
    let partial_preedit = unsafe { result_preedit(partial_selection) };
    assert!(partial_preedit.starts_with(&partial_candidate));
    unsafe {
        radishlex_key_result_free(partial_selection);
        radishlex_snapshot_free(snapshot);
    }
    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );

    push_text(session, "shi", &mut error);
    let snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert!(radishlex_snapshot_candidate_count(snapshot) > 1);
    let (expected_non_first, expected_non_first_reading) =
        unsafe { candidate_identity(snapshot, 1, &mut error) };
    let mut non_first = ptr::null_mut();
    assert_eq!(
        unsafe { radishlex_session_select_candidate(session, 1, &mut non_first, &mut error) },
        RadishLexStatusCode::Ok,
        "non-first candidate selection: {}",
        unsafe { error_message(error) }
    );
    assert_eq!(unsafe { radishlex_key_result_commit_present(non_first) }, 1);
    assert_eq!(
        unsafe { radishlex_key_result_learning_disposition(non_first) },
        RADISHLEX_LEARNING_RECORDED
    );
    assert_eq!(
        unsafe { view_to_string(radishlex_key_result_commit(non_first)) },
        expected_non_first
    );
    unsafe {
        radishlex_key_result_free(non_first);
        radishlex_snapshot_free(snapshot);
    }

    let manager_summary = manager_learning_status(&userdb_path, &mut error);
    let learned_selection_count = manager_summary.selection_events;
    assert_eq!(learned_selection_count, 1);
    assert!(manager_summary.active_user_terms > 0);
    unsafe {
        radishlex_session_free(session);
    }

    let second_session_id = CString::new("native-rime-manager-observer").expect("session id");
    let second_options = RadishLexPersonalizedRimeSessionOptions {
        version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared_data.as_ptr(),
        user_data_dir: user_data.as_ptr(),
        schema: schema.as_ptr(),
        log_dir: ptr::null(),
        deploy_on_start: 1,
        userdb_path: userdb_path.as_ptr(),
        session_id: second_session_id.as_ptr(),
    };
    let second_session = radishlex_session_new_personalized_rime(&second_options, &mut error);
    assert!(
        !second_session.is_null(),
        "second runtime session opens: {}",
        unsafe { error_message(error) }
    );
    set_learning_context(second_session, false, false, true, &mut error);
    push_text(second_session, "shi", &mut error);
    let second_snapshot = radishlex_session_snapshot_new(second_session, &mut error);
    assert_eq!(
        unsafe { candidate_text(second_snapshot, 0, &mut error) },
        expected_non_first,
        "a second runtime session must observe the learned order"
    );
    unsafe {
        radishlex_snapshot_free(second_snapshot);
    }
    assert_eq!(
        radishlex_session_reset(second_session, &mut error),
        RadishLexStatusCode::Ok
    );
    unsafe {
        radishlex_session_free(second_session);
    }

    let input_code = CString::new("shi").expect("input code");
    let selected_text = CString::new(expected_non_first.as_bytes()).expect("candidate text");
    let selected_reading = expected_non_first_reading
        .as_ref()
        .map(|value| CString::new(value.as_bytes()).expect("candidate reading"));
    assert_eq!(
        radishlex_userdb_delete_term(
            userdb_path.as_ptr(),
            input_code.as_ptr(),
            selected_text.as_ptr(),
            selected_reading
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            &mut error,
        ),
        RadishLexStatusCode::Ok,
        "Manager delete endpoint must accept the learned identity"
    );
    let deleted_summary = manager_learning_status(&userdb_path, &mut error);
    assert_eq!(deleted_summary.deleted_term_tombstones, 1);

    let deleted_observer_id = CString::new("native-rime-deleted-observer").expect("session id");
    let deleted_observer_options = RadishLexPersonalizedRimeSessionOptions {
        version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared_data.as_ptr(),
        user_data_dir: user_data.as_ptr(),
        schema: schema.as_ptr(),
        log_dir: ptr::null(),
        deploy_on_start: 1,
        userdb_path: userdb_path.as_ptr(),
        session_id: deleted_observer_id.as_ptr(),
    };
    let deleted_observer =
        radishlex_session_new_personalized_rime(&deleted_observer_options, &mut error);
    assert!(
        !deleted_observer.is_null(),
        "deleted observer session opens: {}",
        unsafe { error_message(error) }
    );
    set_learning_context(deleted_observer, false, false, true, &mut error);
    push_text(deleted_observer, "shi", &mut error);
    let deleted_snapshot = radishlex_session_snapshot_new(deleted_observer, &mut error);
    assert_ne!(
        unsafe { candidate_text(deleted_snapshot, 0, &mut error) },
        expected_non_first,
        "Manager deletion must remove the stale learned promotion"
    );
    unsafe {
        radishlex_snapshot_free(deleted_snapshot);
        radishlex_session_free(deleted_observer);
    }

    assert_eq!(
        radishlex_userdb_restore_term(
            userdb_path.as_ptr(),
            input_code.as_ptr(),
            selected_text.as_ptr(),
            selected_reading
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            &mut error,
        ),
        RadishLexStatusCode::Ok,
        "Manager explicit restore endpoint must reactivate the identity"
    );
    assert_eq!(
        manager_learning_status(&userdb_path, &mut error).deleted_term_tombstones,
        0
    );

    let restored_session_id = CString::new("native-rime-restored-observer").expect("session id");
    let restored_session_options = RadishLexPersonalizedRimeSessionOptions {
        version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared_data.as_ptr(),
        user_data_dir: user_data.as_ptr(),
        schema: schema.as_ptr(),
        log_dir: ptr::null(),
        deploy_on_start: 1,
        userdb_path: userdb_path.as_ptr(),
        session_id: restored_session_id.as_ptr(),
    };
    let session = radishlex_session_new_personalized_rime(&restored_session_options, &mut error);
    assert!(
        !session.is_null(),
        "restored observer session opens: {}",
        unsafe { error_message(error) }
    );
    set_learning_context(session, false, false, true, &mut error);
    push_text(session, "shi", &mut error);
    let restored_snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert_eq!(
        unsafe { candidate_text(restored_snapshot, 0, &mut error) },
        expected_non_first,
        "explicit restore must be visible to a new runtime session"
    );
    unsafe {
        radishlex_snapshot_free(restored_snapshot);
    }
    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );

    set_learning_context(session, false, true, true, &mut error);
    push_text(session, "shi", &mut error);
    let private_snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert_eq!(
        radishlex_snapshot_personalization_status(private_snapshot),
        RADISHLEX_PERSONALIZATION_STATUS_READY
    );
    let mut private_selection = ptr::null_mut();
    assert_eq!(
        unsafe {
            radishlex_session_select_candidate(session, 0, &mut private_selection, &mut error)
        },
        RadishLexStatusCode::Ok
    );
    assert_eq!(
        unsafe { radishlex_key_result_learning_disposition(private_selection) },
        RADISHLEX_LEARNING_SKIPPED_BY_POLICY
    );
    unsafe {
        radishlex_key_result_free(private_selection);
        radishlex_snapshot_free(private_snapshot);
    }
    assert_eq!(
        manager_learning_status(&userdb_path, &mut error).selection_events,
        learned_selection_count
    );

    set_learning_context(session, true, false, true, &mut error);
    push_text(session, "shi", &mut error);
    let blocked_snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert_eq!(
        radishlex_snapshot_personalization_status(blocked_snapshot),
        RADISHLEX_PERSONALIZATION_STATUS_POLICY_BLOCKED
    );
    unsafe {
        radishlex_snapshot_free(blocked_snapshot);
    }
    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );
    set_learning_context(session, false, false, true, &mut error);

    push_text(session, "nihao", &mut error);
    let snapshot = radishlex_session_snapshot_new(session, &mut error);
    let before_backspace = unsafe { view_to_string(radishlex_snapshot_preedit(snapshot)) };
    unsafe {
        radishlex_snapshot_free(snapshot);
    }
    let backspace = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_BACKSPACE, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(backspace) }, 1);
    let after_backspace = unsafe { result_preedit(backspace) };
    assert!(!after_backspace.is_empty());
    assert!(after_backspace.len() < before_backspace.len());
    unsafe {
        radishlex_key_result_free(backspace);
    }
    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );

    push_text(session, "nihao", &mut error);
    let escape = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_ESCAPE, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(escape) }, 1);
    assert!(unsafe { result_preedit(escape) }.is_empty());
    unsafe {
        radishlex_key_result_free(escape);
    }

    push_text(session, "nihao", &mut error);
    let enter = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_ENTER, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(enter) }, 1);
    assert_eq!(unsafe { radishlex_key_result_commit_present(enter) }, 1);
    assert_eq!(
        unsafe { view_to_string(radishlex_key_result_commit(enter)) },
        "nihao"
    );
    assert!(unsafe { result_preedit(enter) }.is_empty());
    unsafe {
        radishlex_key_result_free(enter);
    }

    push_text(session, "shi", &mut error);
    let snapshot = radishlex_session_snapshot_new(session, &mut error);
    let first_page_candidate = unsafe { candidate_text(snapshot, 0, &mut error) };
    unsafe {
        radishlex_snapshot_free(snapshot);
    }
    let page_down = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_PAGE_DOWN, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(page_down) }, 1);
    let page_down_snapshot = unsafe { radishlex_key_result_snapshot(page_down) };
    assert!(radishlex_snapshot_candidate_count(page_down_snapshot) > 0);
    let second_page_candidate = unsafe { candidate_text(page_down_snapshot, 0, &mut error) };
    assert_ne!(second_page_candidate, first_page_candidate);
    unsafe {
        radishlex_key_result_free(page_down);
    }
    let page_up = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_PAGE_UP, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(page_up) }, 1);
    unsafe {
        radishlex_key_result_free(page_up);
    }
    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );

    // This tail checks raw Rime highlight keys, whose indices follow engine
    // order. Product shells own their visible cursor and use display-index
    // selection (covered above); learned ranker order may differ from Rime.
    set_learning_context(session, false, false, false, &mut error);
    push_text(session, "shi", &mut error);
    let snapshot = radishlex_session_snapshot_new(session, &mut error);
    assert_eq!(
        radishlex_snapshot_personalization_status(snapshot),
        RADISHLEX_PERSONALIZATION_STATUS_POLICY_BLOCKED
    );
    assert!(radishlex_snapshot_candidate_count(snapshot) > 1);
    let expected_highlighted = unsafe { candidate_text(snapshot, 1, &mut error) };
    unsafe {
        radishlex_snapshot_free(snapshot);
    }
    let arrow_down = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_ARROW_DOWN, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(arrow_down) }, 1);
    unsafe {
        radishlex_key_result_free(arrow_down);
    }
    let space = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_SPACE, &mut error) };
    assert_eq!(unsafe { radishlex_key_result_consumed(space) }, 1);
    assert_eq!(unsafe { radishlex_key_result_commit_present(space) }, 1);
    assert_eq!(
        unsafe { view_to_string(radishlex_key_result_commit(space)) },
        expected_highlighted
    );
    assert!(unsafe { result_preedit(space) }.is_empty());

    unsafe {
        radishlex_key_result_free(space);
    }
    assert_restricted_segment_and_automatic_commits(session, &userdb_path, &mut error);
    unsafe {
        radishlex_session_free(session);
    }
    assert_eq!(
        unsafe { radishlex_rime_runtime_shutdown(&mut error) },
        RadishLexStatusCode::Ok
    );
}

fn assert_restricted_segment_and_automatic_commits(
    session: *mut RadishLexSession,
    db: &CString,
    error: &mut *mut RadishLexError,
) {
    for (secure, private, known) in [
        (false, true, true),
        (false, false, false),
        (true, false, true),
    ] {
        let count = manager_learning_status(db, error).selection_events;
        set_learning_context(session, secure, private, known, error);
        push_text(session, "luobo", error);
        let mut partial = ptr::null_mut();
        assert_eq!(
            unsafe { radishlex_session_select_candidate(session, 1, &mut partial, error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(unsafe { radishlex_key_result_commit_present(partial) }, 0);
        assert_eq!(
            unsafe { radishlex_key_result_learning_disposition(partial) },
            RADISHLEX_LEARNING_SKIPPED_BY_POLICY
        );
        unsafe {
            radishlex_key_result_free(partial);
        }
        set_learning_context(session, false, false, true, error);
        let mut committed = false;
        for _ in 0..5 {
            let mut result = ptr::null_mut();
            assert_eq!(
                unsafe { radishlex_session_select_candidate(session, 0, &mut result, error) },
                RadishLexStatusCode::Ok
            );
            assert_eq!(
                unsafe { radishlex_key_result_learning_disposition(result) },
                RADISHLEX_LEARNING_SKIPPED_BY_POLICY
            );
            committed = unsafe { radishlex_key_result_commit_present(result) } == 1;
            if committed {
                assert!(!unsafe { view_to_string(radishlex_key_result_commit(result)) }.is_empty());
                assert!(unsafe { result_preedit(result) }.is_empty());
            }
            unsafe {
                radishlex_key_result_free(result);
            }
            if committed {
                break;
            }
        }
        assert!(committed, "restricted segmented input must still finish");
        assert_eq!(manager_learning_status(db, error).selection_events, count);

        set_learning_context(session, secure, private, known, error);
        push_text(session, "nihao", error);
        set_learning_context(session, false, false, true, error);
        let enter = unsafe { handle_named(session, RADISHLEX_NAMED_KEY_ENTER, error) };
        assert_eq!(unsafe { radishlex_key_result_commit_present(enter) }, 1);
        assert_eq!(
            unsafe { view_to_string(radishlex_key_result_commit(enter)) },
            "nihao"
        );
        assert_eq!(
            unsafe { radishlex_key_result_learning_disposition(enter) },
            RADISHLEX_LEARNING_SKIPPED_BY_POLICY
        );
        unsafe {
            radishlex_key_result_free(enter);
        }
        assert_eq!(manager_learning_status(db, error).selection_events, count);

        push_text(session, "shi", error);
        let mut fresh = ptr::null_mut();
        assert_eq!(
            unsafe { radishlex_session_select_candidate(session, 0, &mut fresh, error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(
            unsafe { radishlex_key_result_learning_disposition(fresh) },
            RADISHLEX_LEARNING_RECORDED
        );
        unsafe {
            radishlex_key_result_free(fresh);
        }
        assert_eq!(
            manager_learning_status(db, error).selection_events,
            count + 1
        );
    }
}

fn set_learning_context(
    session: *mut RadishLexSession,
    secure_input: bool,
    privacy_mode: bool,
    context_known: bool,
    error: &mut *mut RadishLexError,
) {
    let context_kind = b"general";
    let context = RadishLexLearningContext {
        version: RADISHLEX_LEARNING_CONTEXT_VERSION,
        secure_input: u8::from(secure_input),
        sensitive_application: 0,
        privacy_mode: u8::from(privacy_mode),
        context_known: u8::from(context_known),
        context_kind: RadishLexStringView {
            data: context_kind.as_ptr(),
            len: context_kind.len(),
        },
    };
    assert_eq!(
        unsafe { radishlex_session_set_learning_context(session, context, error) },
        RadishLexStatusCode::Ok
    );
}

fn push_text(session: *mut RadishLexSession, text: &str, error: &mut *mut RadishLexError) {
    for ch in text.chars() {
        assert_eq!(
            radishlex_session_push_key(session, ch as u32, error),
            RadishLexStatusCode::Ok
        );
    }
}

fn assert_expected_candidate_page_size(snapshot: *const RadishLexSnapshot) {
    let Ok(expected) = env::var("RADISHLEX_EXPECTED_CANDIDATE_PAGE_SIZE") else {
        return;
    };
    let expected = expected
        .parse::<usize>()
        .expect("RADISHLEX_EXPECTED_CANDIDATE_PAGE_SIZE must be a positive integer");
    assert!(expected > 0);
    assert_eq!(
        radishlex_snapshot_candidate_count(snapshot),
        expected,
        "native Rime candidate page must match the product display contract"
    );
}

unsafe fn handle_named(
    session: *mut RadishLexSession,
    named_key: u32,
    error: &mut *mut RadishLexError,
) -> *mut RadishLexKeyResult {
    let mut result = ptr::null_mut();
    assert_eq!(
        radishlex_session_handle_key_event(
            session,
            RadishLexKeyEvent::press_named(named_key),
            &mut result,
            error,
        ),
        RadishLexStatusCode::Ok
    );
    assert!(!result.is_null());
    result
}

unsafe fn result_preedit(result: *const RadishLexKeyResult) -> String {
    let snapshot = radishlex_key_result_snapshot(result);
    assert!(!snapshot.is_null());
    view_to_string(radishlex_snapshot_preedit(snapshot))
}

unsafe fn candidate_text(
    snapshot: *const RadishLexSnapshot,
    index: usize,
    error: &mut *mut RadishLexError,
) -> String {
    let mut candidate = RadishLexCandidateView::empty();
    assert_eq!(
        radishlex_snapshot_candidate(snapshot, index, &mut candidate, error),
        RadishLexStatusCode::Ok
    );
    view_to_string(candidate.text)
}

unsafe fn candidate_identity(
    snapshot: *const RadishLexSnapshot,
    index: usize,
    error: &mut *mut RadishLexError,
) -> (String, Option<String>) {
    let mut candidate = RadishLexCandidateView::empty();
    assert_eq!(
        radishlex_snapshot_candidate(snapshot, index, &mut candidate, error),
        RadishLexStatusCode::Ok
    );
    let reading = (candidate.reading_present == 1).then(|| view_to_string(candidate.reading));
    (view_to_string(candidate.text), reading)
}

fn manager_learning_status(
    userdb_path: &CString,
    error: &mut *mut RadishLexError,
) -> RadishLexLearningStatusSummary {
    let mut summary = RadishLexLearningStatusSummary::empty();
    assert_eq!(
        unsafe { radishlex_userdb_learning_status(userdb_path.as_ptr(), &mut summary, error) },
        RadishLexStatusCode::Ok,
        "Manager learning summary must open the shared userdb"
    );
    summary
}

unsafe fn view_to_string(view: RadishLexStringView) -> String {
    let bytes = slice::from_raw_parts(view.data, view.len);
    String::from_utf8(bytes.to_vec()).expect("view must be UTF-8")
}

unsafe fn error_message(error: *const RadishLexError) -> String {
    if error.is_null() {
        return "<none>".to_owned();
    }
    CStr::from_ptr(radishlex_error_message(error))
        .to_string_lossy()
        .into_owned()
}
