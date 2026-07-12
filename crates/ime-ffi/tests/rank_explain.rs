use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::ptr;
use std::slice;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_ffi::{
    radishlex_error_code, radishlex_error_free, radishlex_userdb_rank_explain_free,
    radishlex_userdb_rank_explain_new, radishlex_userdb_rank_explain_view, RadishLexError,
    RadishLexRankExplainView, RadishLexStatusCode, RadishLexStringView,
};
use radishlex_ime_userdb::{SelectionEventDraft, TermSource, UserDb};

#[test]
fn userdb_rank_explain_reports_candidate_contributions() {
    let path = temp_db_path("rank-explain");
    {
        let mut db = UserDb::open(&path).expect("userdb opens");
        db.add_term(
            "luobo",
            "萝卜词核",
            Some("luo bo ci he"),
            TermSource::ManualAdd,
        )
        .expect("term is added");
        db.record_selection(
            SelectionEventDraft::new("session-local", "luobo", "萝卜词核", 1, 3)
                .with_reading("luo bo ci he")
                .with_context_kind("chat"),
        )
        .expect("selection is recorded");
    }

    let db_path = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let input_code = CString::new("luobo").expect("input");
    let text = CString::new("萝卜词核").expect("text");
    let reading = CString::new("luo bo ci he").expect("reading");
    let context = CString::new("chat").expect("context");
    let mut error = ptr::null_mut();

    let explain = radishlex_userdb_rank_explain_new(
        db_path.as_ptr(),
        input_code.as_ptr(),
        text.as_ptr(),
        reading.as_ptr(),
        context.as_ptr(),
        &mut error,
    );
    assert!(!explain.is_null());
    assert!(error.is_null());

    let mut view = RadishLexRankExplainView::empty();
    assert_eq!(
        unsafe { radishlex_userdb_rank_explain_view(explain, &mut view, &mut error) },
        RadishLexStatusCode::Ok
    );
    assert_eq!(unsafe { view_to_string(view.input_code) }, "luobo");
    assert_eq!(unsafe { view_to_string(view.candidate_text) }, "萝卜词核");
    assert_eq!(view.reading_present, 1);
    assert_eq!(unsafe { view_to_string(view.reading) }, "luo bo ci he");
    assert_eq!(unsafe { view_to_string(view.context_kind) }, "chat");
    assert_eq!(view.original_index, 0);
    assert!(view.user_term_boost > 0.0);
    assert!(view.frequency_boost > 0.0);
    assert!(view.recency_boost > 0.0);
    assert!(view.context_boost > 0.0);
    assert!(view.final_score > 0.0);

    unsafe {
        radishlex_userdb_rank_explain_free(explain);
    }

    let _ = fs::remove_file(path);
}

#[test]
fn userdb_rank_explain_reports_deleted_term_penalty() {
    let path = temp_db_path("rank-explain-deleted");
    {
        let mut db = UserDb::open(&path).expect("userdb opens");
        db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");
        db.delete_term("luobo", "萝卜", Some("luo bo"))
            .expect("term is deleted");
    }

    let db_path = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let input_code = CString::new("luobo").expect("input");
    let text = CString::new("萝卜").expect("text");
    let reading = CString::new("luo bo").expect("reading");
    let mut error = ptr::null_mut();

    let explain = radishlex_userdb_rank_explain_new(
        db_path.as_ptr(),
        input_code.as_ptr(),
        text.as_ptr(),
        reading.as_ptr(),
        ptr::null(),
        &mut error,
    );
    assert!(!explain.is_null());
    assert!(error.is_null());

    let mut view = RadishLexRankExplainView::empty();
    assert_eq!(
        unsafe { radishlex_userdb_rank_explain_view(explain, &mut view, &mut error) },
        RadishLexStatusCode::Ok
    );
    assert_eq!(unsafe { view_to_string(view.context_kind) }, "general");
    assert_eq!(view.user_term_boost, 0.0);
    assert!(view.deleted_penalty > 0.0);
    assert!(view.final_score < 0.0);

    unsafe {
        radishlex_userdb_rank_explain_free(explain);
    }

    let _ = fs::remove_file(path);
}

#[test]
fn userdb_rank_explain_rejects_invalid_arguments() {
    let path = temp_db_path("rank-explain-invalid");
    let db_path = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let text = CString::new("萝卜").expect("text");
    let mut error: *mut RadishLexError = ptr::null_mut();

    let explain = radishlex_userdb_rank_explain_new(
        db_path.as_ptr(),
        ptr::null(),
        text.as_ptr(),
        ptr::null(),
        ptr::null(),
        &mut error,
    );
    assert!(explain.is_null());
    assert_eq!(
        unsafe { radishlex_error_code(error) },
        RadishLexStatusCode::InvalidArgument
    );

    unsafe {
        radishlex_error_free(error);
    }

    error = ptr::null_mut();
    let mut view = RadishLexRankExplainView::empty();
    assert_eq!(
        unsafe { radishlex_userdb_rank_explain_view(ptr::null(), &mut view, &mut error) },
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        unsafe { radishlex_error_code(error) },
        RadishLexStatusCode::InvalidArgument
    );
    unsafe {
        radishlex_error_free(error);
    }

    let _ = fs::remove_file(path);
}

unsafe fn view_to_string(view: RadishLexStringView) -> String {
    let bytes = slice::from_raw_parts(view.data, view.len);
    String::from_utf8(bytes.to_vec()).expect("view must be UTF-8")
}

fn temp_db_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is valid")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "radishlex-ime-ffi-{name}-{}-{nanos}.sqlite",
        process::id()
    ))
}
