use std::ffi::{CStr, CString};
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_ffi::{
    radishlex_error_code, radishlex_error_free, radishlex_error_message, radishlex_ffi_contract,
    radishlex_session_engine_kind, radishlex_session_free, radishlex_session_new,
    radishlex_session_reset, radishlex_userdb_add_term, radishlex_userdb_dictionary_export,
    radishlex_userdb_dictionary_import, radishlex_userdb_dictionary_inspect,
    radishlex_userdb_import_batches_count, radishlex_userdb_import_batches_free,
    radishlex_userdb_import_batches_get, radishlex_userdb_import_batches_new,
    radishlex_userdb_learning_status, radishlex_userdb_terms_count, radishlex_userdb_terms_free,
    radishlex_userdb_terms_new, RadishLexError, RadishLexFfiContract, RadishLexImportBatchView,
    RadishLexLearningStatusSummary, RadishLexSession, RadishLexStatusCode, RadishLexStringView,
    RADISHLEX_ABI_CONTRACT_VERSION, RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1,
    RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND, RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD,
    RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_ffi::{
    radishlex_session_new_rime, RadishLexRimeSessionOptions, RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
};
use radishlex_ime_userdb::{
    NegativeFeedbackDraft, NegativeFeedbackReason, SelectionEventDraft, TermSource, UserDb,
};

#[test]
fn ffi_contract_reports_lifecycle_and_thread_policy() {
    let mut error = ptr::null_mut();
    let mut contract = RadishLexFfiContract::empty();

    assert_eq!(
        radishlex_ffi_contract(&mut contract, &mut error),
        RadishLexStatusCode::Ok
    );
    assert!(error.is_null());
    assert_eq!(contract.version, RADISHLEX_ABI_CONTRACT_VERSION);
    assert_eq!(
        contract.session_thread_policy,
        RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD
    );
    assert_eq!(
        contract.panic_boundary,
        RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND
    );

    assert_eq!(
        radishlex_ffi_contract(ptr::null_mut(), &mut error),
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        radishlex_error_code(error),
        RadishLexStatusCode::InvalidArgument
    );
    unsafe {
        radishlex_error_free(error);
    }
}

#[test]
fn session_handles_reject_non_owner_thread_use() {
    let mut error = ptr::null_mut();
    let session = radishlex_session_new(&mut error);
    assert!(!session.is_null());
    assert!(error.is_null());

    let session_addr = session as usize;
    let (status, code, message, engine_kind) = thread::spawn(move || {
        let session = session_addr as *mut RadishLexSession;
        let engine_kind = radishlex_session_engine_kind(session);
        let mut error: *mut RadishLexError = ptr::null_mut();
        let status = radishlex_session_reset(session, &mut error);
        let code = radishlex_error_code(error);
        let message = unsafe { error_message(error) };
        unsafe {
            radishlex_error_free(error);
        }
        (status, code, message, engine_kind)
    })
    .join()
    .expect("thread joins");

    assert_eq!(engine_kind, 0);
    assert_eq!(status, RadishLexStatusCode::InvalidState);
    assert_eq!(code, RadishLexStatusCode::InvalidState);
    assert!(message.contains("thread that created it"));

    assert_eq!(
        radishlex_session_reset(session, &mut error),
        RadishLexStatusCode::Ok
    );
    assert!(error.is_null());

    unsafe {
        radishlex_session_free(session);
    }
}

#[test]
fn userdb_dictionary_file_management_round_trips_p2_terms() {
    let source_path = temp_db_path("dictionary-file-source");
    let target_path = temp_db_path("dictionary-file-target");
    let export_path = temp_db_path("dictionary-file-export").with_extension("tsv");
    let source_db_path = CString::new(source_path.to_string_lossy().as_bytes()).expect("path");
    let target_db_path = CString::new(target_path.to_string_lossy().as_bytes()).expect("path");
    let export_file_path =
        CString::new(export_path.to_string_lossy().as_bytes()).expect("file path");
    let input_code = CString::new("luobo").expect("input");
    let text = CString::new("萝卜").expect("text");
    let reading = CString::new("luo bo").expect("reading");
    let source_name = CString::new("ffi-smoke").expect("source name");
    let mut error = ptr::null_mut();

    assert_eq!(
        radishlex_userdb_add_term(
            source_db_path.as_ptr(),
            input_code.as_ptr(),
            text.as_ptr(),
            reading.as_ptr(),
            &mut error,
        ),
        RadishLexStatusCode::Ok
    );

    let mut export_summary = radishlex_ime_ffi::RadishLexDictionaryExportSummary::empty();
    assert_eq!(
        radishlex_userdb_dictionary_export(
            source_db_path.as_ptr(),
            export_file_path.as_ptr(),
            &mut export_summary,
            &mut error,
        ),
        RadishLexStatusCode::Ok
    );
    assert_eq!(
        export_summary.format_version,
        RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1
    );
    assert_eq!(export_summary.exported_terms, 1);
    assert_eq!(
        export_summary.sync_class,
        RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC
    );

    let exported = fs::read_to_string(&export_path).expect("export file is readable");
    assert!(exported.contains("# radishlex-user-terms-v1"));
    assert!(!exported.contains("session-local"));

    let mut inspect_summary = radishlex_ime_ffi::RadishLexDictionaryInspectSummary::empty();
    assert_eq!(
        radishlex_userdb_dictionary_inspect(
            export_file_path.as_ptr(),
            &mut inspect_summary,
            &mut error,
        ),
        RadishLexStatusCode::Ok
    );
    assert_eq!(inspect_summary.record_count, 1);
    assert_eq!(
        inspect_summary.format_version,
        RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1
    );
    assert_eq!(
        inspect_summary.sync_class,
        RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC
    );

    let mut import_summary = radishlex_ime_ffi::RadishLexDictionaryImportSummary::empty();
    assert_eq!(
        radishlex_userdb_dictionary_import(
            target_db_path.as_ptr(),
            export_file_path.as_ptr(),
            source_name.as_ptr(),
            1,
            &mut import_summary,
            &mut error,
        ),
        RadishLexStatusCode::Ok
    );
    assert_eq!(import_summary.dry_run, 1);
    assert_eq!(import_summary.import_batch_id_present, 0);
    assert_eq!(import_summary.total_records, 1);
    assert_eq!(import_summary.inserted_terms, 1);

    let batches = radishlex_userdb_import_batches_new(target_db_path.as_ptr(), &mut error);
    assert!(!batches.is_null());
    assert_eq!(radishlex_userdb_import_batches_count(batches), 0);
    unsafe {
        radishlex_userdb_import_batches_free(batches);
    }

    assert_eq!(
        radishlex_userdb_dictionary_import(
            target_db_path.as_ptr(),
            export_file_path.as_ptr(),
            source_name.as_ptr(),
            0,
            &mut import_summary,
            &mut error,
        ),
        RadishLexStatusCode::Ok
    );
    assert_eq!(import_summary.dry_run, 0);
    assert_eq!(import_summary.import_batch_id_present, 1);
    assert_eq!(import_summary.imported_terms, 1);

    let batches = radishlex_userdb_import_batches_new(target_db_path.as_ptr(), &mut error);
    assert!(!batches.is_null());
    assert_eq!(radishlex_userdb_import_batches_count(batches), 1);

    let mut batch = RadishLexImportBatchView::empty();
    assert_eq!(
        radishlex_userdb_import_batches_get(batches, 0, &mut batch, &mut error),
        RadishLexStatusCode::Ok
    );
    assert_eq!(unsafe { view_to_string(batch.source_name) }, "ffi-smoke");
    assert_eq!(batch.total_records, 1);
    assert_eq!(batch.imported_terms, 1);
    assert_eq!(batch.inserted_terms, 1);
    assert_eq!(batch.skipped_deleted_terms, 0);
    assert_eq!(batch.skipped_duplicate_terms, 0);

    assert_eq!(
        radishlex_userdb_import_batches_get(batches, 1, &mut batch, &mut error),
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        radishlex_error_code(error),
        RadishLexStatusCode::InvalidArgument
    );
    unsafe {
        radishlex_error_free(error);
        radishlex_userdb_import_batches_free(batches);
    }

    error = ptr::null_mut();
    let terms = radishlex_userdb_terms_new(target_db_path.as_ptr(), &mut error);
    assert!(!terms.is_null());
    assert_eq!(radishlex_userdb_terms_count(terms), 1);
    unsafe {
        radishlex_userdb_terms_free(terms);
    }

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
    let _ = fs::remove_file(export_path);
}

#[test]
fn userdb_learning_status_reports_read_only_counts() {
    let db_path = temp_db_path("learning-status");
    {
        let mut db = UserDb::open(&db_path).expect("userdb opens");
        db.add_term("cihe", "词核", None, TermSource::ManualAdd)
            .expect("term is added");
        db.record_selection(
            SelectionEventDraft::new("session-private", "luobo", "萝卜", 0, 1)
                .with_context_kind("chat"),
        )
        .expect("selection is recorded");
        db.record_negative_feedback(
            NegativeFeedbackDraft::new("luobo", "萝卜", NegativeFeedbackReason::ManualSuppress)
                .with_context_kind("chat"),
        )
        .expect("feedback is recorded");
    }

    let db_path_c = CString::new(db_path.to_string_lossy().as_bytes()).expect("path");
    let mut error = ptr::null_mut();
    let mut summary = RadishLexLearningStatusSummary::empty();
    assert_eq!(
        radishlex_userdb_learning_status(db_path_c.as_ptr(), &mut summary, &mut error),
        RadishLexStatusCode::Ok
    );
    assert!(error.is_null());

    assert_eq!(summary.schema_version, 2);
    assert_eq!(summary.plaintext_payload, 0);
    assert_eq!(summary.p1_raw_details, 0);
    assert_eq!(summary.context_stats, 0);
    assert_eq!(summary.active_user_terms, 1);
    assert_eq!(summary.suppressed_user_terms, 1);
    assert_eq!(summary.ranker_weights, 1);
    assert_eq!(summary.deleted_term_tombstones, 0);
    assert_eq!(summary.selection_events, 1);
    assert_eq!(summary.negative_feedback, 1);
    assert_eq!(summary.import_batches, 0);
    assert_eq!(summary.latest_user_term_updated_at_present, 1);
    assert_eq!(summary.latest_selection_event_at_present, 1);
    assert_eq!(summary.latest_negative_feedback_at_present, 1);
    assert_eq!(summary.latest_deleted_term_at_present, 0);
    assert_eq!(summary.latest_import_batch_at_present, 0);
    assert_eq!(summary.latest_activity_at_present, 1);

    let debug = format!("{summary:?}");
    assert!(!debug.contains("session-private"));
    assert!(!debug.contains("chat"));
    assert!(!debug.contains("manual_suppress"));
    assert!(!debug.contains("萝卜"));
    assert!(!debug.contains("词核"));

    assert_eq!(
        radishlex_userdb_learning_status(db_path_c.as_ptr(), ptr::null_mut(), &mut error),
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        radishlex_error_code(error),
        RadishLexStatusCode::InvalidArgument
    );
    unsafe {
        radishlex_error_free(error);
    }

    let _ = fs::remove_file(db_path);
}

#[cfg(feature = "native-rime")]
#[test]
#[ignore = "requires RADISHLEX_RIME_SHARED_DATA and RADISHLEX_RIME_USER_DATA"]
fn rime_session_native_invalid_schema_reports_engine_error() {
    let shared_data = std::env::var("RADISHLEX_RIME_SHARED_DATA")
        .expect("RADISHLEX_RIME_SHARED_DATA must point to isolated Rime shared data");
    let user_data = std::env::var("RADISHLEX_RIME_USER_DATA")
        .expect("RADISHLEX_RIME_USER_DATA must point to isolated Rime user data");
    let shared_data = CString::new(shared_data).expect("shared data path");
    let user_data = CString::new(user_data).expect("user data path");
    let missing_schema =
        CString::new("radishlex_missing_schema_ffi_smoke").expect("missing schema");
    let mut error = ptr::null_mut();

    let options = RadishLexRimeSessionOptions {
        version: RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared_data.as_ptr(),
        user_data_dir: user_data.as_ptr(),
        schema: missing_schema.as_ptr(),
        log_dir: ptr::null(),
        deploy_on_start: 0,
    };

    let session = radishlex_session_new_rime(&options, &mut error);
    if !session.is_null() {
        unsafe {
            radishlex_session_free(session);
        }
        panic!("missing schema unexpectedly created a Rime session");
    }
    assert!(session.is_null());
    assert_eq!(
        radishlex_error_code(error),
        RadishLexStatusCode::EngineError
    );

    let message = unsafe { error_message(error) };
    assert!(message.contains("select_schema"));
    assert!(message.contains("radishlex_missing_schema_ffi_smoke"));
    unsafe {
        radishlex_error_free(error);
    }
}

unsafe fn error_message(error: *const RadishLexError) -> String {
    if error.is_null() {
        return "<none>".to_owned();
    }
    CStr::from_ptr(radishlex_error_message(error))
        .to_string_lossy()
        .into_owned()
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
        std::process::id()
    ))
}
