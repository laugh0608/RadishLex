use std::ffi::CStr;
use std::os::raw::c_char;
use std::slice;

use radishlex_ime_sync_runtime::{
    QualificationError, QualificationErrorCode, QualificationRequest, QualificationRun,
    QualificationRunSnapshot, QUALIFICATION_REQUEST_VERSION, QUALIFICATION_SNAPSHOT_VERSION,
};

use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::{ffi_ptr, ffi_release, ffi_status};

pub const RADISHLEX_MANAGER_SYNC_QUALIFICATION_REQUEST_VERSION: u32 = QUALIFICATION_REQUEST_VERSION;
pub const RADISHLEX_MANAGER_SYNC_QUALIFICATION_SNAPSHOT_VERSION: u32 =
    QUALIFICATION_SNAPSHOT_VERSION;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RadishLexManagerSyncQualificationRequest {
    pub version: u32,
    pub endpoint: *const c_char,
    pub access_token_data: *const u8,
    pub access_token_len: usize,
    pub local_ca_der_data: *const u8,
    pub local_ca_der_len: usize,
    pub timeout_ms: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexManagerSyncQualificationSnapshot {
    pub version: u32,
    pub state: u32,
    pub phase: u32,
    pub discovered: u64,
    pub downloaded: u64,
    pub applied: u64,
    pub uploaded: u64,
    pub conflicts: u64,
    pub retries: u64,
    pub convergence_rounds: u64,
    pub temporary_files_cleaned: u32,
    pub worker_stopped: u32,
    pub transient_inputs_cleared: u32,
    pub error_code: u32,
    pub error_phase: u32,
    pub error_retryable: u32,
}

impl RadishLexManagerSyncQualificationSnapshot {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            state: 0,
            phase: 0,
            discovered: 0,
            downloaded: 0,
            applied: 0,
            uploaded: 0,
            conflicts: 0,
            retries: 0,
            convergence_rounds: 0,
            temporary_files_cleaned: 0,
            worker_stopped: 0,
            transient_inputs_cleared: 0,
            error_code: 0,
            error_phase: 0,
            error_retryable: 0,
        }
    }
}

impl From<QualificationRunSnapshot> for RadishLexManagerSyncQualificationSnapshot {
    fn from(snapshot: QualificationRunSnapshot) -> Self {
        let error = snapshot.error;
        Self {
            version: snapshot.version,
            state: snapshot.state as u32,
            phase: snapshot.phase as u32,
            discovered: count(snapshot.discovered),
            downloaded: count(snapshot.downloaded),
            applied: count(snapshot.applied),
            uploaded: count(snapshot.uploaded),
            conflicts: count(snapshot.conflicts),
            retries: count(snapshot.retries),
            convergence_rounds: count(snapshot.convergence_rounds),
            temporary_files_cleaned: flag(snapshot.temporary_files_cleaned),
            worker_stopped: flag(snapshot.worker_stopped),
            transient_inputs_cleared: flag(snapshot.transient_inputs_cleared),
            error_code: error
                .map(|value| value.code as u32)
                .unwrap_or(QualificationErrorCode::None as u32),
            error_phase: error.map(|value| value.phase as u32).unwrap_or(0),
            error_retryable: error.map(|value| flag(value.retryable)).unwrap_or(0),
        }
    }
}

pub struct RadishLexManagerSyncQualificationRun {
    run: QualificationRun,
}

impl RadishLexManagerSyncQualificationRun {
    fn new(run: QualificationRun) -> Self {
        Self { run }
    }
}

#[no_mangle]
/// Starts one isolated local HTTPS synthetic-sync qualification run.
///
/// # Safety
/// `request` and all non-empty views must be readable for this call;
/// `error_out`, when non-null, must be writable.
pub unsafe extern "C" fn radishlex_manager_sync_qualification_start(
    request: *const RadishLexManagerSyncQualificationRequest,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexManagerSyncQualificationRun {
    ffi_ptr(error_out, || {
        let request = parse_request(request)?;
        let run = QualificationRun::start(request).map_err(map_start_error)?;
        Ok(Box::into_raw(Box::new(
            RadishLexManagerSyncQualificationRun::new(run),
        )))
    })
}

#[no_mangle]
/// Copies the current fixed, redacted run snapshot.
///
/// # Safety
/// `run` must be a live qualification handle and `snapshot_out` must be writable.
pub unsafe extern "C" fn radishlex_manager_sync_qualification_poll(
    run: *const RadishLexManagerSyncQualificationRun,
    snapshot_out: *mut RadishLexManagerSyncQualificationSnapshot,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if run.is_null() || snapshot_out.is_null() {
            return Err(FfiError::invalid_argument(
                "sync qualification poll requires run and snapshot output",
            ));
        }
        let run = unsafe { &*run };
        unsafe {
            *snapshot_out = run.run.poll().into();
        }
        Ok(())
    })
}

#[no_mangle]
/// Requests cancellation. Repeated requests are safe.
///
/// # Safety
/// `run` must be a live qualification handle and `requested_out` must be writable.
pub unsafe extern "C" fn radishlex_manager_sync_qualification_cancel(
    run: *const RadishLexManagerSyncQualificationRun,
    requested_out: *mut u32,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        if run.is_null() || requested_out.is_null() {
            return Err(FfiError::invalid_argument(
                "sync qualification cancel requires run and output",
            ));
        }
        let run = unsafe { &*run };
        unsafe {
            *requested_out = flag(run.run.cancel());
        }
        Ok(())
    })
}

#[no_mangle]
/// Cancels an active run, joins its worker, and releases the handle.
///
/// # Safety
/// `run` must be null or a live handle returned by qualification start. The
/// caller must not overlap this release with poll or cancel.
pub unsafe extern "C" fn radishlex_manager_sync_qualification_free(
    run: *mut RadishLexManagerSyncQualificationRun,
) {
    ffi_release(|| {
        if run.is_null() {
            return;
        }
        let _ = unsafe { Box::from_raw(run) };
    });
}

fn parse_request(
    request: *const RadishLexManagerSyncQualificationRequest,
) -> Result<QualificationRequest, FfiError> {
    if request.is_null() {
        return Err(FfiError::invalid_argument(
            "sync qualification request pointer is null",
        ));
    }
    let request = unsafe { *request };
    if request.endpoint.is_null()
        || request.access_token_data.is_null()
        || request.access_token_len == 0
        || request.access_token_len > 4_096
        || request.local_ca_der_len > 64 * 1024
        || (request.local_ca_der_len > 0 && request.local_ca_der_data.is_null())
    {
        return Err(FfiError::invalid_argument(
            "sync qualification request contains an invalid view",
        ));
    }
    let endpoint = unsafe { CStr::from_ptr(request.endpoint) }
        .to_str()
        .map_err(|_| FfiError::invalid_argument("sync qualification endpoint is not UTF-8"))?
        .to_owned();
    let token =
        unsafe { slice::from_raw_parts(request.access_token_data, request.access_token_len) };
    let token = std::str::from_utf8(token)
        .map_err(|_| FfiError::invalid_argument("sync qualification token is not UTF-8"))?
        .to_owned();
    let local_ca_der = if request.local_ca_der_len == 0 {
        None
    } else {
        Some(
            unsafe { slice::from_raw_parts(request.local_ca_der_data, request.local_ca_der_len) }
                .to_vec(),
        )
    };
    QualificationRequest::with_version(
        request.version,
        endpoint,
        token,
        local_ca_der,
        request.timeout_ms,
    )
    .map_err(|_| FfiError::invalid_argument("sync qualification request is invalid"))
}

fn map_start_error(error: QualificationError) -> FfiError {
    match error.code {
        QualificationErrorCode::InvalidRequest => {
            FfiError::invalid_argument("sync qualification request is invalid")
        }
        QualificationErrorCode::AlreadyRunning => {
            FfiError::invalid_state("a sync qualification run is already active")
        }
        _ => FfiError::new(
            RadishLexStatusCode::SyncError,
            "sync qualification run could not start",
        ),
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

const fn flag(value: bool) -> u32 {
    value as u32
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::ptr;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn invalid_request_fails_without_exposing_token() {
        let endpoint = CString::new("http://localhost:7319").expect("endpoint");
        let token = b"qualification-secret-token-0000000000000000";
        let request = RadishLexManagerSyncQualificationRequest {
            version: RADISHLEX_MANAGER_SYNC_QUALIFICATION_REQUEST_VERSION,
            endpoint: endpoint.as_ptr(),
            access_token_data: token.as_ptr(),
            access_token_len: token.len(),
            local_ca_der_data: ptr::null(),
            local_ca_der_len: 0,
            timeout_ms: 1_000,
        };
        let mut error = ptr::null_mut();
        let run = unsafe { radishlex_manager_sync_qualification_start(&request, &mut error) };
        assert!(run.is_null());
        assert!(!error.is_null());
        let message = unsafe { CStr::from_ptr((*error).message()) }.to_string_lossy();
        assert!(!message.contains("qualification-secret-token"));
        unsafe { RadishLexError::free(error) };
    }

    #[test]
    fn run_can_be_serially_polled_and_released_from_another_thread() {
        let endpoint = CString::new("https://127.0.0.1:9").expect("endpoint");
        let token = b"qualification-secret-token-1111111111111111";
        let request = RadishLexManagerSyncQualificationRequest {
            version: RADISHLEX_MANAGER_SYNC_QUALIFICATION_REQUEST_VERSION,
            endpoint: endpoint.as_ptr(),
            access_token_data: token.as_ptr(),
            access_token_len: token.len(),
            local_ca_der_data: ptr::null(),
            local_ca_der_len: 0,
            timeout_ms: 500,
        };
        let mut error = ptr::null_mut();
        let run = unsafe { radishlex_manager_sync_qualification_start(&request, &mut error) };
        assert!(!run.is_null());
        assert!(error.is_null());
        let run_address = run as usize;
        let snapshot = std::thread::spawn(move || {
            let run = run_address as *mut RadishLexManagerSyncQualificationRun;
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut error = ptr::null_mut();
            let mut snapshot = RadishLexManagerSyncQualificationSnapshot::empty();
            loop {
                assert_eq!(
                    unsafe {
                        radishlex_manager_sync_qualification_poll(run, &mut snapshot, &mut error)
                    },
                    RadishLexStatusCode::Ok
                );
                if matches!(snapshot.state, 4..=6) {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            unsafe { radishlex_manager_sync_qualification_free(run) };
            snapshot
        })
        .join()
        .expect("serial qualification caller thread");
        assert_eq!(
            snapshot.version,
            RADISHLEX_MANAGER_SYNC_QUALIFICATION_SNAPSHOT_VERSION
        );
        assert_eq!(snapshot.state, 5);
        assert_eq!(snapshot.temporary_files_cleaned, 1);
        assert_eq!(snapshot.worker_stopped, 1);
        assert_eq!(snapshot.transient_inputs_cleared, 1);
    }

    #[test]
    fn null_poll_cancel_and_free_are_contained() {
        let mut error = ptr::null_mut();
        let mut snapshot = RadishLexManagerSyncQualificationSnapshot::empty();
        assert_eq!(
            unsafe {
                radishlex_manager_sync_qualification_poll(ptr::null(), &mut snapshot, &mut error)
            },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe { RadishLexError::free(error) };
        error = ptr::null_mut();
        let mut requested = 0;
        assert_eq!(
            unsafe {
                radishlex_manager_sync_qualification_cancel(ptr::null(), &mut requested, &mut error)
            },
            RadishLexStatusCode::InvalidArgument
        );
        unsafe {
            RadishLexError::free(error);
            radishlex_manager_sync_qualification_free(ptr::null_mut());
        }
    }
}
