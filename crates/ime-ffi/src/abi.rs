use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

use radishlex_ime_core::SchemaId;
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::RimeEngineConfig;

use crate::apple_p256_product::{
    run_product_smoke, write_product_status, RadishLexAppleP256ProductSmokeSummary,
    RadishLexAppleP256ProductStatus,
};
use crate::apple_secure_enclave_product::{
    run_product_smoke as run_secure_enclave_product_smoke,
    write_product_status as write_secure_enclave_product_status,
};
use crate::buffer::RadishLexBuffer;
use crate::contract::RadishLexFfiContract;
use crate::dictionary::{
    export_dictionary_file, import_dictionary_file, inspect_dictionary_file, list_import_batches,
    RadishLexDictionaryExportSummary, RadishLexDictionaryImportSummary,
    RadishLexDictionaryInspectSummary, RadishLexImportBatchList, RadishLexImportBatchView,
};
use crate::engine::{
    validate_rime_session_options_version, validate_session_options, RadishLexRimeSessionOptions,
    RadishLexSessionOptions,
};
use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::{ffi_ptr, ffi_release, ffi_status};
use crate::key::RadishLexKeyEvent;
use crate::learning_status::{learning_status_for_path, RadishLexLearningStatusSummary};
use crate::rank_explain::{rank_explain_for_path, RadishLexRankExplain, RadishLexRankExplainView};
use crate::session::{session_mut, session_ref, RadishLexSession};
use crate::snapshot::{RadishLexCandidateView, RadishLexSnapshot, RadishLexStringView};
use crate::sync_status::{sync_preflight_for_path, RadishLexSyncPreflightSummary};

#[path = "abi/user_terms.rs"]
mod user_terms;
pub use user_terms::*;

#[no_mangle]
/// Returns compile/runtime capability and independent product/user-sync gates.
///
/// This validation ABI never returns key, canonical, or signature bytes.
///
/// # Safety
/// `status_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_p256_product_status(
    status_out: *mut RadishLexAppleP256ProductStatus,
) -> u32 {
    unsafe { write_product_status(status_out) }
}

#[no_mangle]
/// Runs the explicitly gated manager product-process Apple P-256 smoke.
///
/// The returned summary contains fixed flags only. Private key material,
/// canonical bytes, public key bytes, and signature bytes stay inside the
/// native validation path and are never returned to Dart.
///
/// # Safety
/// `go_server_dir` must be null or point to a NUL-terminated UTF-8 string;
/// `summary_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_p256_product_smoke(
    scenario: u32,
    go_server_dir: *const c_char,
    summary_out: *mut RadishLexAppleP256ProductSmokeSummary,
) -> u32 {
    unsafe { run_product_smoke(scenario, go_server_dir, summary_out) }
}

#[no_mangle]
/// Returns compiled Secure Enclave capability and closed runtime/product gates.
///
/// This validation ABI never returns key, canonical, or signature bytes.
///
/// # Safety
/// `status_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_secure_enclave_p256_product_status(
    status_out: *mut RadishLexAppleP256ProductStatus,
) -> u32 {
    unsafe { write_secure_enclave_product_status(status_out) }
}

#[no_mangle]
/// Runs the explicitly gated manager product-process Secure Enclave P-256 smoke.
///
/// Private key material, public key bytes, canonical bytes, and signature bytes
/// stay inside the native validation path and are never returned to Dart.
///
/// # Safety
/// `go_server_dir` must be null or point to a NUL-terminated UTF-8 string;
/// `summary_out` must be writable.
pub unsafe extern "C" fn radishlex_apple_secure_enclave_p256_product_smoke(
    scenario: u32,
    go_server_dir: *const c_char,
    summary_out: *mut RadishLexAppleP256ProductSmokeSummary,
) -> u32 {
    unsafe { run_secure_enclave_product_smoke(scenario, go_server_dir, summary_out) }
}

#[no_mangle]
/// # Safety
/// `contract_out` must be writable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_ffi_contract(
    contract_out: *mut RadishLexFfiContract,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if contract_out.is_null() {
            return Err(FfiError::invalid_argument(
                "FFI contract output pointer is null",
            ));
        }

        unsafe {
            *contract_out = RadishLexFfiContract::current();
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_new(
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSession {
    ffi_ptr(error_out, || {
        Ok(Box::into_raw(Box::new(RadishLexSession::new())))
    })
}

#[no_mangle]
/// # Safety
/// `options` must be null or readable; `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_session_new_with_options(
    options: *const RadishLexSessionOptions,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSession {
    ffi_ptr(error_out, || {
        if options.is_null() {
            return Err(FfiError::invalid_argument(
                "session options pointer is null",
            ));
        }

        let engine_kind = validate_session_options(unsafe { *options })?;
        Ok(Box::into_raw(Box::new(
            RadishLexSession::new_with_engine_kind(engine_kind),
        )))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_new_rime(
    options: *const RadishLexRimeSessionOptions,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSession {
    ffi_ptr(error_out, || {
        let options = parse_rime_session_options(options)?;
        new_rime_session(options)
    })
}

#[no_mangle]
/// Releases a session on the thread that created it.
///
/// # Safety
///
/// `session` must be null or a live handle returned by a RadishLex session
/// constructor. Passing an already released pointer is invalid. A call from a
/// non-owner thread is ignored so the owner thread can still release the handle.
pub unsafe extern "C" fn radishlex_session_free(session: *mut RadishLexSession) {
    ffi_release(|| {
        if session.is_null() {
            return;
        }
        if (&*session).ensure_owner_thread().is_err() {
            return;
        }
        let _ = Box::from_raw(session);
    });
}

#[no_mangle]
pub extern "C" fn radishlex_session_engine_kind(session: *const RadishLexSession) -> u32 {
    session_ref(session).map_or(0, RadishLexSession::engine_kind)
}

#[no_mangle]
pub extern "C" fn radishlex_session_reset(
    session: *mut RadishLexSession,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        session_mut(session)?.reset()?;
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
        session_mut(session)?.set_schema(schema)?;
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
        let event = RadishLexKeyEvent::press_char(char::from_u32(codepoint).ok_or_else(|| {
            FfiError::invalid_argument("key codepoint is not a valid Unicode scalar value")
        })?);
        session_mut(session)?.push_key_event(event.try_into()?)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_push_key_event(
    session: *mut RadishLexSession,
    event: RadishLexKeyEvent,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        session_mut(session)?.push_key_event(event.try_into()?)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_snapshot(
    session: *mut RadishLexSession,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexBuffer {
    ffi_ptr(error_out, || {
        let snapshot = session_mut(session)?.snapshot_text()?;
        Ok(RadishLexBuffer::from_string(snapshot))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_session_snapshot_new(
    session: *mut RadishLexSession,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSnapshot {
    ffi_ptr(error_out, || {
        let snapshot = session_mut(session)?.snapshot()?;
        Ok(Box::into_raw(Box::new(snapshot)))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_snapshot_schema(
    snapshot: *const RadishLexSnapshot,
) -> RadishLexStringView {
    snapshot_ref(snapshot).map_or_else(
        |_| RadishLexStringView::empty(),
        |snapshot| snapshot.schema(),
    )
}

#[no_mangle]
pub extern "C" fn radishlex_snapshot_preedit(
    snapshot: *const RadishLexSnapshot,
) -> RadishLexStringView {
    snapshot_ref(snapshot).map_or_else(
        |_| RadishLexStringView::empty(),
        |snapshot| snapshot.preedit(),
    )
}

#[no_mangle]
pub extern "C" fn radishlex_snapshot_cursor(snapshot: *const RadishLexSnapshot) -> usize {
    snapshot_ref(snapshot).map_or(0, RadishLexSnapshot::cursor)
}

#[no_mangle]
pub extern "C" fn radishlex_snapshot_candidate_count(snapshot: *const RadishLexSnapshot) -> usize {
    snapshot_ref(snapshot).map_or(0, RadishLexSnapshot::candidate_count)
}

#[no_mangle]
pub extern "C" fn radishlex_snapshot_personalization_status(
    snapshot: *const RadishLexSnapshot,
) -> u32 {
    snapshot_ref(snapshot).map_or(0, RadishLexSnapshot::personalization_status)
}

#[no_mangle]
/// # Safety
/// Handles must be live and `candidate_out`/`error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_snapshot_candidate(
    snapshot: *const RadishLexSnapshot,
    index: usize,
    candidate_out: *mut RadishLexCandidateView,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if candidate_out.is_null() {
            return Err(FfiError::invalid_argument(
                "candidate output pointer is null",
            ));
        }

        let view = snapshot_ref(snapshot)?.candidate_view(index)?;
        unsafe {
            *candidate_out = view;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// `snapshot` must be null or a live independent snapshot released exactly once.
pub unsafe extern "C" fn radishlex_snapshot_free(snapshot: *mut RadishLexSnapshot) {
    ffi_release(|| {
        RadishLexSnapshot::free(snapshot);
    });
}

#[no_mangle]
/// # Safety
/// Input strings must be readable and output pointers, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_sync_preflight(
    db_path: *const c_char,
    summary_out: *mut RadishLexSyncPreflightSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "sync preflight summary output pointer is null",
            ));
        }

        let db_path = read_utf8(db_path, "db_path")?;
        let summary = sync_preflight_for_path(db_path)?;
        unsafe {
            *summary_out = summary;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// Input strings must be readable and output pointers, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_learning_status(
    db_path: *const c_char,
    summary_out: *mut RadishLexLearningStatusSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "learning status summary output pointer is null",
            ));
        }

        let db_path = read_utf8(db_path, "db_path")?;
        let summary = learning_status_for_path(db_path)?;
        unsafe {
            *summary_out = summary;
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_rank_explain_new(
    db_path: *const c_char,
    input_code: *const c_char,
    candidate_text: *const c_char,
    reading: *const c_char,
    context_kind: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexRankExplain {
    ffi_ptr(error_out, || {
        let explain = rank_explain_for_path(
            read_utf8(db_path, "db_path")?,
            read_utf8(input_code, "input_code")?,
            read_utf8(candidate_text, "candidate_text")?,
            read_optional_utf8(reading, "reading")?,
            read_optional_utf8(context_kind, "context_kind")?,
        )?;
        Ok(Box::into_raw(Box::new(explain)))
    })
}

#[no_mangle]
/// # Safety
/// `explain` must be live and `view_out`/`error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_rank_explain_view(
    explain: *const RadishLexRankExplain,
    view_out: *mut RadishLexRankExplainView,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if view_out.is_null() {
            return Err(FfiError::invalid_argument(
                "rank explain view output pointer is null",
            ));
        }

        let view = rank_explain_ref(explain)?.view();
        unsafe {
            *view_out = view;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// `explain` must be null or a live handle released exactly once.
pub unsafe extern "C" fn radishlex_userdb_rank_explain_free(explain: *mut RadishLexRankExplain) {
    ffi_release(|| {
        RadishLexRankExplain::free(explain);
    });
}

#[no_mangle]
/// # Safety
/// Input strings must be readable and output pointers, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_dictionary_inspect(
    file_path: *const c_char,
    summary_out: *mut RadishLexDictionaryInspectSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "dictionary inspect summary output pointer is null",
            ));
        }

        let summary = inspect_dictionary_file(read_utf8(file_path, "file_path")?)?;
        unsafe {
            *summary_out = summary;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// Input strings must be readable and output pointers, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_dictionary_export(
    db_path: *const c_char,
    file_path: *const c_char,
    summary_out: *mut RadishLexDictionaryExportSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "dictionary export summary output pointer is null",
            ));
        }

        let summary = export_dictionary_file(
            read_utf8(db_path, "db_path")?,
            read_utf8(file_path, "file_path")?,
        )?;
        unsafe {
            *summary_out = summary;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// Input strings must be readable and output pointers, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_dictionary_import(
    db_path: *const c_char,
    file_path: *const c_char,
    source_name: *const c_char,
    dry_run: u8,
    summary_out: *mut RadishLexDictionaryImportSummary,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if summary_out.is_null() {
            return Err(FfiError::invalid_argument(
                "dictionary import summary output pointer is null",
            ));
        }

        let summary = import_dictionary_file(
            read_utf8(db_path, "db_path")?,
            read_utf8(file_path, "file_path")?,
            read_optional_utf8(source_name, "source_name")?,
            read_ffi_bool(dry_run, "dry_run")?,
        )?;
        unsafe {
            *summary_out = summary;
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_import_batches_new(
    db_path: *const c_char,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexImportBatchList {
    ffi_ptr(error_out, || {
        let batches = list_import_batches(read_utf8(db_path, "db_path")?)?;
        Ok(Box::into_raw(Box::new(batches)))
    })
}

#[no_mangle]
pub extern "C" fn radishlex_userdb_import_batches_count(
    batches: *const RadishLexImportBatchList,
) -> usize {
    import_batch_list_ref(batches).map_or(0, RadishLexImportBatchList::len)
}

#[no_mangle]
/// # Safety
/// `batches` must be live and `batch_out`/`error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_userdb_import_batches_get(
    batches: *const RadishLexImportBatchList,
    index: usize,
    batch_out: *mut RadishLexImportBatchView,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if batch_out.is_null() {
            return Err(FfiError::invalid_argument(
                "import batch output pointer is null",
            ));
        }

        let view = import_batch_list_ref(batches)?.batch_view(index)?;
        unsafe {
            *batch_out = view;
        }
        Ok(())
    })
}

#[no_mangle]
/// # Safety
/// `batches` must be null or a live handle released exactly once.
pub unsafe extern "C" fn radishlex_userdb_import_batches_free(
    batches: *mut RadishLexImportBatchList,
) {
    ffi_release(|| {
        RadishLexImportBatchList::free(batches);
    });
}

#[no_mangle]
/// # Safety
/// `buffer` must be null or a live buffer handle.
pub unsafe extern "C" fn radishlex_buffer_data(buffer: *const RadishLexBuffer) -> *const u8 {
    if buffer.is_null() {
        return ptr::null();
    }
    unsafe { (*buffer).data() }
}

#[no_mangle]
/// # Safety
/// `buffer` must be null or a live buffer handle.
pub unsafe extern "C" fn radishlex_buffer_len(buffer: *const RadishLexBuffer) -> usize {
    if buffer.is_null() {
        return 0;
    }
    unsafe { (*buffer).len() }
}

#[no_mangle]
/// # Safety
/// `buffer` must be null or a live buffer handle released exactly once.
pub unsafe extern "C" fn radishlex_buffer_free(buffer: *mut RadishLexBuffer) {
    ffi_release(|| {
        RadishLexBuffer::free(buffer);
    });
}

#[no_mangle]
/// # Safety
/// `error` must be null or a live error handle.
pub unsafe extern "C" fn radishlex_error_code(error: *const RadishLexError) -> RadishLexStatusCode {
    if error.is_null() {
        return RadishLexStatusCode::InternalError;
    }
    unsafe { (*error).code() }
}

#[no_mangle]
/// # Safety
/// `error` must be null or a live error handle.
pub unsafe extern "C" fn radishlex_error_message(error: *const RadishLexError) -> *const c_char {
    if error.is_null() {
        return ptr::null();
    }
    unsafe { (*error).message() }
}

#[no_mangle]
/// # Safety
/// `error` must be null or a live error handle released exactly once.
pub unsafe extern "C" fn radishlex_error_free(error: *mut RadishLexError) {
    ffi_release(|| {
        RadishLexError::free(error);
    });
}

fn snapshot_ref<'a>(snapshot: *const RadishLexSnapshot) -> Result<&'a RadishLexSnapshot, FfiError> {
    if snapshot.is_null() {
        return Err(FfiError::invalid_argument("snapshot handle is null"));
    }
    Ok(unsafe { &*snapshot })
}

fn import_batch_list_ref<'a>(
    batches: *const RadishLexImportBatchList,
) -> Result<&'a RadishLexImportBatchList, FfiError> {
    if batches.is_null() {
        return Err(FfiError::invalid_argument(
            "import batch list handle is null",
        ));
    }
    Ok(unsafe { &*batches })
}

fn rank_explain_ref<'a>(
    explain: *const RadishLexRankExplain,
) -> Result<&'a RadishLexRankExplain, FfiError> {
    if explain.is_null() {
        return Err(FfiError::invalid_argument("rank explain handle is null"));
    }
    Ok(unsafe { &*explain })
}

fn read_utf8<'a>(value: *const c_char, field: &'static str) -> Result<&'a str, FfiError> {
    if value.is_null() {
        return Err(FfiError::invalid_argument(format!("{field} is null")));
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|_| FfiError::invalid_argument(format!("{field} must be valid UTF-8")))
}

pub(crate) fn read_required_utf8<'a>(
    value: *const c_char,
    field: &'static str,
) -> Result<&'a str, FfiError> {
    let value = read_utf8(value, field)?;
    if value.is_empty() {
        return Err(FfiError::invalid_argument(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value)
}

fn read_optional_utf8<'a>(
    value: *const c_char,
    field: &'static str,
) -> Result<Option<&'a str>, FfiError> {
    if value.is_null() {
        return Ok(None);
    }
    read_utf8(value, field).map(Some)
}

fn read_optional_nonempty_utf8<'a>(
    value: *const c_char,
    field: &'static str,
) -> Result<Option<&'a str>, FfiError> {
    let Some(value) = read_optional_utf8(value, field)? else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err(FfiError::invalid_argument(format!(
            "{field} cannot be empty when provided"
        )));
    }
    Ok(Some(value))
}

fn read_ffi_bool(value: u8, field: &'static str) -> Result<bool, FfiError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(FfiError::invalid_argument(format!(
            "{field} must be 0 or 1, got {other}"
        ))),
    }
}

pub(crate) struct ParsedRimeSessionOptions<'a> {
    pub(crate) shared_data_dir: &'a str,
    pub(crate) user_data_dir: &'a str,
    pub(crate) schema: SchemaId,
    pub(crate) log_dir: Option<&'a str>,
    pub(crate) deploy_on_start: bool,
}

pub(crate) fn parse_rime_session_options<'a>(
    options: *const RadishLexRimeSessionOptions,
) -> Result<ParsedRimeSessionOptions<'a>, FfiError> {
    if options.is_null() {
        return Err(FfiError::invalid_argument(
            "rime session options pointer is null",
        ));
    }

    let options = unsafe { *options };
    validate_rime_session_options_version(options)?;
    let shared_data_dir = read_required_utf8(options.shared_data_dir, "shared_data_dir")?;
    let user_data_dir = read_required_utf8(options.user_data_dir, "user_data_dir")?;
    let schema = SchemaId::new(read_required_utf8(options.schema, "schema")?)?;
    let log_dir = read_optional_nonempty_utf8(options.log_dir, "log_dir")?;
    let deploy_on_start = read_ffi_bool(options.deploy_on_start, "deploy_on_start")?;

    Ok(ParsedRimeSessionOptions {
        shared_data_dir,
        user_data_dir,
        schema,
        log_dir,
        deploy_on_start,
    })
}

#[cfg(feature = "native-rime")]
fn new_rime_session(
    options: ParsedRimeSessionOptions<'_>,
) -> Result<*mut RadishLexSession, FfiError> {
    let mut config = RimeEngineConfig::new(
        options.shared_data_dir,
        options.user_data_dir,
        options.schema,
    )?;
    if let Some(log_dir) = options.log_dir {
        config = config.with_log_dir(log_dir)?;
    }
    config = config.with_deploy_on_start(options.deploy_on_start);

    Ok(Box::into_raw(Box::new(RadishLexSession::new_rime(config)?)))
}

#[cfg(not(feature = "native-rime"))]
fn new_rime_session(
    options: ParsedRimeSessionOptions<'_>,
) -> Result<*mut RadishLexSession, FfiError> {
    let ParsedRimeSessionOptions {
        shared_data_dir,
        user_data_dir,
        schema,
        log_dir,
        deploy_on_start,
    } = options;
    let _ = (
        shared_data_dir,
        user_data_dir,
        schema,
        log_dir,
        deploy_on_start,
    );
    Err(FfiError::invalid_state(
        "rime engine is not available through ime-ffi; rebuild radishlex-ime-ffi with the native-rime feature",
    ))
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::slice;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::dictionary::{
        RadishLexUserTermView, RADISHLEX_TERM_SOURCE_MANUAL_ADD, RADISHLEX_TERM_STATUS_ACTIVE,
    };
    use crate::engine::{
        RadishLexPersonalizedRimeSessionOptions, RadishLexRimeSessionOptions,
        RADISHLEX_ENGINE_KIND_DEMO, RADISHLEX_ENGINE_KIND_RIME,
        RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        RADISHLEX_RIME_SESSION_OPTIONS_VERSION, RADISHLEX_SESSION_OPTIONS_VERSION,
    };
    use crate::key::{
        RADISHLEX_KEY_MOD_SHIFT, RADISHLEX_KEY_PHASE_RELEASE, RADISHLEX_NAMED_KEY_BACKSPACE,
    };
    use crate::personalization::{
        radishlex_session_new_personalized_rime, radishlex_session_set_learning_context,
        RadishLexLearningContext,
    };
    use crate::snapshot::RADISHLEX_CANDIDATE_SOURCE_ENGINE;
    use crate::{
        radishlex_key_result_commit, radishlex_key_result_commit_present,
        radishlex_key_result_free, radishlex_session_select_candidate,
    };
    use radishlex_ime_userdb::{
        NegativeFeedbackDraft, NegativeFeedbackReason, SelectionEventDraft, TermSource, UserDb,
    };

    #[test]
    fn session_snapshot_and_commit_round_trip() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());
        assert!(error.is_null());
        assert_eq!(
            radishlex_session_engine_kind(session),
            RADISHLEX_ENGINE_KIND_DEMO
        );

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

        let mut result = ptr::null_mut();
        assert_eq!(
            unsafe { radishlex_session_select_candidate(session, 1, &mut result, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert!(!result.is_null());
        assert_eq!(unsafe { radishlex_key_result_commit_present(result) }, 1);
        assert_eq!(
            unsafe { view_to_string(radishlex_key_result_commit(result)) },
            "萝卜词核"
        );
        unsafe {
            radishlex_key_result_free(result);
            radishlex_session_free(session);
        }
    }

    #[test]
    fn session_options_select_demo_and_reject_unavailable_rime() {
        let mut error = ptr::null_mut();
        let options = RadishLexSessionOptions::demo();
        let session = unsafe { radishlex_session_new_with_options(&options, &mut error) };
        assert!(!session.is_null());
        assert_eq!(
            radishlex_session_engine_kind(session),
            RADISHLEX_ENGINE_KIND_DEMO
        );
        unsafe {
            radishlex_session_free(session);
        }

        let rime_options = RadishLexSessionOptions {
            version: RADISHLEX_SESSION_OPTIONS_VERSION,
            engine_kind: RADISHLEX_ENGINE_KIND_RIME,
        };
        let session = unsafe { radishlex_session_new_with_options(&rime_options, &mut error) };
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidState
        );
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("rime engine is not available"));
        unsafe {
            radishlex_error_free(error);
        }

        let bad_options = RadishLexSessionOptions {
            version: RADISHLEX_SESSION_OPTIONS_VERSION + 1,
            engine_kind: RADISHLEX_ENGINE_KIND_DEMO,
        };
        let session = unsafe { radishlex_session_new_with_options(&bad_options, &mut error) };
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe {
            radishlex_error_free(error);
        }
    }

    #[test]
    fn rime_session_options_reject_invalid_arguments_before_engine_selection() {
        let shared_data_dir = CString::new("/tmp/radishlex-rime/shared").expect("shared path");
        let user_data_dir = CString::new("/tmp/radishlex-rime/user").expect("user path");
        let schema = CString::new("luna_pinyin").expect("schema");
        let mut error = ptr::null_mut();

        let options = RadishLexRimeSessionOptions {
            version: RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
            shared_data_dir: shared_data_dir.as_ptr(),
            user_data_dir: user_data_dir.as_ptr(),
            schema: schema.as_ptr(),
            log_dir: ptr::null(),
            deploy_on_start: 0,
        };

        let bad_version = RadishLexRimeSessionOptions {
            version: RADISHLEX_RIME_SESSION_OPTIONS_VERSION + 1,
            ..options
        };
        let session = radishlex_session_new_rime(&bad_version, &mut error);
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe {
            radishlex_error_free(error);
        }

        error = ptr::null_mut();
        let bad_deploy_flag = RadishLexRimeSessionOptions {
            deploy_on_start: 2,
            ..options
        };
        let session = radishlex_session_new_rime(&bad_deploy_flag, &mut error);
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("deploy_on_start"));
        unsafe {
            radishlex_error_free(error);
        }
    }

    #[test]
    fn personalized_session_options_and_learning_context_are_versioned() {
        let shared_data_dir = CString::new("/tmp/radishlex-rime/shared").expect("shared path");
        let user_data_dir = CString::new("/tmp/radishlex-rime/user").expect("user path");
        let schema = CString::new("luna_pinyin").expect("schema");
        let userdb_path = CString::new("/tmp/radishlex-rime/userdb.sqlite3").expect("db path");
        let session_id = CString::new("ffi-personalized-test").expect("session id");
        let mut error = ptr::null_mut();
        let options = RadishLexPersonalizedRimeSessionOptions {
            version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION + 1,
            shared_data_dir: shared_data_dir.as_ptr(),
            user_data_dir: user_data_dir.as_ptr(),
            schema: schema.as_ptr(),
            log_dir: ptr::null(),
            deploy_on_start: 0,
            userdb_path: userdb_path.as_ptr(),
            session_id: session_id.as_ptr(),
        };
        let session = radishlex_session_new_personalized_rime(&options, &mut error);
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe { radishlex_error_free(error) };

        let legacy = radishlex_session_new(&mut error);
        assert!(!legacy.is_null());
        let context_kind = b"general";
        let context = RadishLexLearningContext {
            version: crate::personalization::RADISHLEX_LEARNING_CONTEXT_VERSION,
            secure_input: 0,
            sensitive_application: 0,
            privacy_mode: 0,
            context_known: 1,
            context_kind: RadishLexStringView {
                data: context_kind.as_ptr(),
                len: context_kind.len(),
            },
        };
        error = ptr::null_mut();
        assert_eq!(
            unsafe { radishlex_session_set_learning_context(legacy, context, &mut error) },
            RadishLexStatusCode::InvalidState
        );
        unsafe {
            radishlex_error_free(error);
            radishlex_session_free(legacy);
        }
    }

    #[cfg(not(feature = "native-rime"))]
    #[test]
    fn rime_session_options_return_unavailable_without_native_feature() {
        let shared_data_dir = CString::new("/tmp/radishlex-rime/shared").expect("shared path");
        let user_data_dir = CString::new("/tmp/radishlex-rime/user").expect("user path");
        let schema = CString::new("luna_pinyin").expect("schema");
        let log_dir = CString::new("/tmp/radishlex-rime/log").expect("log path");
        let mut error = ptr::null_mut();

        let options = RadishLexRimeSessionOptions {
            version: RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
            shared_data_dir: shared_data_dir.as_ptr(),
            user_data_dir: user_data_dir.as_ptr(),
            schema: schema.as_ptr(),
            log_dir: log_dir.as_ptr(),
            deploy_on_start: 0,
        };
        let session = radishlex_session_new_rime(&options, &mut error);
        assert!(session.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidState
        );
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("native-rime feature"));
        unsafe {
            radishlex_error_free(error);
        }
    }

    #[test]
    fn userdb_management_adds_lists_and_deletes_terms() {
        let path = temp_db_path("userdb-management");
        let db_path = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        let input_code = CString::new("luobo").expect("input");
        let text = CString::new("萝卜").expect("text");
        let reading = CString::new("luo bo").expect("reading");
        let mut error = ptr::null_mut();

        assert_eq!(
            radishlex_userdb_add_term(
                db_path.as_ptr(),
                input_code.as_ptr(),
                text.as_ptr(),
                reading.as_ptr(),
                &mut error,
            ),
            RadishLexStatusCode::Ok
        );

        let terms = radishlex_userdb_terms_new(db_path.as_ptr(), &mut error);
        assert!(!terms.is_null());
        assert_eq!(radishlex_userdb_terms_count(terms), 1);

        let mut term = RadishLexUserTermView::empty();
        assert_eq!(
            unsafe { radishlex_userdb_terms_get(terms, 0, &mut term, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(unsafe { view_to_string(term.input_code) }, "luobo");
        assert_eq!(unsafe { view_to_string(term.text) }, "萝卜");
        assert_eq!(term.reading_present, 1);
        assert_eq!(unsafe { view_to_string(term.reading) }, "luo bo");
        assert_eq!(term.source, RADISHLEX_TERM_SOURCE_MANUAL_ADD);
        assert_eq!(term.status, RADISHLEX_TERM_STATUS_ACTIVE);
        assert_eq!(term.last_used_at_present, 0);

        assert_eq!(
            unsafe { radishlex_userdb_terms_get(terms, 1, &mut term, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe {
            radishlex_error_free(error);
            radishlex_userdb_terms_free(terms);
        }

        error = ptr::null_mut();
        assert_eq!(
            radishlex_userdb_delete_term(
                db_path.as_ptr(),
                input_code.as_ptr(),
                text.as_ptr(),
                reading.as_ptr(),
                &mut error,
            ),
            RadishLexStatusCode::Ok
        );

        assert_eq!(
            radishlex_userdb_add_term(
                db_path.as_ptr(),
                input_code.as_ptr(),
                text.as_ptr(),
                reading.as_ptr(),
                &mut error,
            ),
            RadishLexStatusCode::InvalidArgument
        );
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("explicit restore"));
        unsafe {
            radishlex_error_free(error);
        }
        error = ptr::null_mut();

        let terms = radishlex_userdb_terms_new(db_path.as_ptr(), &mut error);
        assert!(!terms.is_null());
        assert_eq!(radishlex_userdb_terms_count(terms), 0);
        unsafe {
            radishlex_userdb_terms_free(terms);
        }

        let mut summary = RadishLexSyncPreflightSummary::empty();
        assert_eq!(
            unsafe { radishlex_userdb_sync_preflight(db_path.as_ptr(), &mut summary, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(summary.syncable_user_terms, 0);
        assert_eq!(summary.syncable_deleted_terms, 1);
        assert_eq!(summary.local_selection_events, 0);
        assert_eq!(summary.local_negative_feedback, 1);

        assert_eq!(
            radishlex_userdb_restore_term(
                db_path.as_ptr(),
                input_code.as_ptr(),
                text.as_ptr(),
                reading.as_ptr(),
                &mut error,
            ),
            RadishLexStatusCode::Ok
        );
        let terms = radishlex_userdb_terms_new(db_path.as_ptr(), &mut error);
        assert!(!terms.is_null());
        assert_eq!(radishlex_userdb_terms_count(terms), 1);
        unsafe {
            radishlex_userdb_terms_free(terms);
        }
        assert_eq!(
            unsafe { radishlex_userdb_sync_preflight(db_path.as_ptr(), &mut summary, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(summary.syncable_user_terms, 1);
        assert_eq!(summary.syncable_deleted_terms, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sync_preflight_returns_counts_without_payload() {
        let path = temp_db_path("sync-preflight");
        {
            let mut db = UserDb::open(&path).expect("userdb opens");
            db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
                .expect("term is added");
            db.record_selection(
                SelectionEventDraft::new("session-local", "cihe", "词核", 0, 1)
                    .with_context_kind("chat"),
            )
            .expect("selection is recorded");
            db.record_negative_feedback(
                NegativeFeedbackDraft::new("cihe", "词核", NegativeFeedbackReason::ManualSuppress)
                    .with_context_kind("chat"),
            )
            .expect("feedback is recorded");
            db.delete_term("luobo", "萝卜", Some("luo bo"))
                .expect("term is deleted");
        }

        let mut error = ptr::null_mut();
        let db_path = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        let mut summary = RadishLexSyncPreflightSummary::empty();
        assert_eq!(
            unsafe { radishlex_userdb_sync_preflight(db_path.as_ptr(), &mut summary, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert!(error.is_null());
        assert_eq!(summary.schema_version, 5);
        assert_eq!(summary.plaintext_payload, 0);
        assert_eq!(summary.syncable_user_terms, 1);
        assert_eq!(summary.syncable_ranker_weights, 1);
        assert_eq!(summary.syncable_deleted_terms, 1);
        assert_eq!(summary.local_selection_events, 1);
        assert_eq!(summary.local_negative_feedback, 2);
        assert_eq!(summary.local_import_batches, 0);

        assert_eq!(
            unsafe {
                radishlex_userdb_sync_preflight(db_path.as_ptr(), ptr::null_mut(), &mut error)
            },
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

    #[test]
    fn structured_snapshot_exposes_candidate_views() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());

        for ch in "luox".chars() {
            assert_eq!(
                radishlex_session_push_key_event(
                    session,
                    RadishLexKeyEvent::press_char(ch),
                    &mut error
                ),
                RadishLexStatusCode::Ok
            );
        }

        assert_eq!(
            radishlex_session_push_key_event(
                session,
                RadishLexKeyEvent::press_named(RADISHLEX_NAMED_KEY_BACKSPACE),
                &mut error
            ),
            RadishLexStatusCode::Ok
        );

        for ch in "bo".chars() {
            assert_eq!(
                radishlex_session_push_key_event(
                    session,
                    RadishLexKeyEvent {
                        modifiers: RADISHLEX_KEY_MOD_SHIFT,
                        ..RadishLexKeyEvent::press_char(ch)
                    },
                    &mut error,
                ),
                RadishLexStatusCode::Ok
            );
        }

        let snapshot = radishlex_session_snapshot_new(session, &mut error);
        assert!(!snapshot.is_null());
        assert_eq!(
            unsafe { view_to_string(radishlex_snapshot_schema(snapshot)) },
            "ffi.demo"
        );
        assert_eq!(
            unsafe { view_to_string(radishlex_snapshot_preedit(snapshot)) },
            "luobo"
        );
        assert_eq!(radishlex_snapshot_cursor(snapshot), 5);
        assert_eq!(radishlex_snapshot_candidate_count(snapshot), 2);

        let mut candidate = RadishLexCandidateView::empty();
        assert_eq!(
            unsafe { radishlex_snapshot_candidate(snapshot, 1, &mut candidate, &mut error) },
            RadishLexStatusCode::Ok
        );
        assert_eq!(candidate.index, 1);
        assert_eq!(candidate.engine_index, 1);
        assert_eq!(unsafe { view_to_string(candidate.text) }, "萝卜词核");
        assert_eq!(candidate.reading_present, 1);
        assert_eq!(unsafe { view_to_string(candidate.reading) }, "luobo");
        assert_eq!(candidate.annotation_present, 1);
        assert_eq!(
            unsafe { view_to_string(candidate.annotation) },
            "project term"
        );
        assert_eq!(candidate.source, RADISHLEX_CANDIDATE_SOURCE_ENGINE);

        assert_eq!(
            unsafe { radishlex_snapshot_candidate(snapshot, 2, &mut candidate, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(
            unsafe { radishlex_error_code(error) },
            RadishLexStatusCode::InvalidArgument
        );

        unsafe {
            radishlex_error_free(error);
            radishlex_snapshot_free(snapshot);
            radishlex_session_free(session);
        }
    }

    #[test]
    fn invalid_key_event_reports_argument_error() {
        let mut error = ptr::null_mut();
        let session = radishlex_session_new(&mut error);
        assert!(!session.is_null());

        let status = radishlex_session_push_key_event(
            session,
            RadishLexKeyEvent {
                phase: RADISHLEX_KEY_PHASE_RELEASE + 10,
                ..RadishLexKeyEvent::press_char('l')
            },
            &mut error,
        );

        assert_eq!(status, RadishLexStatusCode::InvalidArgument);
        let message = unsafe { CStr::from_ptr(radishlex_error_message(error)) }
            .to_string_lossy()
            .into_owned();
        assert!(message.contains("unknown key phase code"));

        unsafe {
            radishlex_error_free(error);
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
            unsafe { radishlex_error_code(error) },
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

        let mut result = ptr::null_mut();
        assert_eq!(
            unsafe { radishlex_session_select_candidate(session, 0, &mut result, &mut error) },
            RadishLexStatusCode::InvalidArgument
        );
        assert!(result.is_null());
        assert_eq!(
            unsafe { radishlex_error_code(error) },
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
            radishlex_snapshot_free(ptr::null_mut());
            radishlex_userdb_terms_free(ptr::null_mut());
            radishlex_userdb_deleted_terms_free(ptr::null_mut());
            radishlex_userdb_import_batches_free(ptr::null_mut());
            radishlex_userdb_rank_explain_free(ptr::null_mut());
        }
        assert!(unsafe { radishlex_buffer_data(ptr::null()) }.is_null());
        assert_eq!(unsafe { radishlex_buffer_len(ptr::null()) }, 0);
        assert!(unsafe { radishlex_error_message(ptr::null()) }.is_null());
        assert_eq!(radishlex_snapshot_cursor(ptr::null()), 0);
        assert_eq!(radishlex_snapshot_candidate_count(ptr::null()), 0);
        assert!(radishlex_snapshot_schema(ptr::null()).data.is_null());
        assert_eq!(radishlex_session_engine_kind(ptr::null()), 0);
        assert_eq!(radishlex_userdb_terms_count(ptr::null()), 0);
        assert_eq!(radishlex_userdb_deleted_terms_count(ptr::null()), 0);
        assert_eq!(radishlex_userdb_import_batches_count(ptr::null()), 0);
    }

    unsafe fn buffer_to_string(buffer: *mut RadishLexBuffer) -> String {
        let data = radishlex_buffer_data(buffer);
        let len = radishlex_buffer_len(buffer);
        let bytes = slice::from_raw_parts(data, len);
        String::from_utf8(bytes.to_vec()).expect("buffer must be UTF-8")
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
}
