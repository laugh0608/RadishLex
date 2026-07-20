use std::slice;
use std::str;

#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::RimeEngineConfig;
use radishlex_ime_runtime::{LearningContext, LearningDisposition, PersonalizationStatus};

use crate::abi::{parse_rime_session_options, read_required_utf8, ParsedRimeSessionOptions};
use crate::engine::{
    validate_personalized_rime_session_options_version, RadishLexPersonalizedRimeSessionOptions,
    RadishLexRimeSessionOptions, RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
};
use crate::error::{FfiError, RadishLexError, RadishLexStatusCode};
use crate::ffi_support::{ffi_ptr, ffi_status};
use crate::session::{session_mut, RadishLexSession};
use crate::snapshot::RadishLexStringView;

pub const RADISHLEX_LEARNING_CONTEXT_VERSION: u32 = 1;

pub const RADISHLEX_PERSONALIZATION_STATUS_NOT_ENABLED: u32 = 0;
pub const RADISHLEX_PERSONALIZATION_STATUS_READY: u32 = 1;
pub const RADISHLEX_PERSONALIZATION_STATUS_POLICY_BLOCKED: u32 = 2;
pub const RADISHLEX_PERSONALIZATION_STATUS_STORAGE_UNAVAILABLE: u32 = 3;
pub const RADISHLEX_PERSONALIZATION_STATUS_READ_FAILED: u32 = 4;
pub const RADISHLEX_PERSONALIZATION_STATUS_RANK_FAILED: u32 = 5;

pub const RADISHLEX_LEARNING_NOT_APPLICABLE: u32 = 0;
pub const RADISHLEX_LEARNING_RECORDED: u32 = 1;
pub const RADISHLEX_LEARNING_DEFERRED: u32 = 2;
pub const RADISHLEX_LEARNING_SKIPPED_BY_POLICY: u32 = 3;
pub const RADISHLEX_LEARNING_FAILED: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexLearningContext {
    pub version: u32,
    pub secure_input: u8,
    pub sensitive_application: u8,
    pub privacy_mode: u8,
    pub context_known: u8,
    pub context_kind: RadishLexStringView,
}

struct ParsedPersonalizedRimeSessionOptions<'a> {
    rime: ParsedRimeSessionOptions<'a>,
    userdb_path: &'a str,
    session_id: &'a str,
}

#[no_mangle]
pub extern "C" fn radishlex_session_new_personalized_rime(
    options: *const RadishLexPersonalizedRimeSessionOptions,
    error_out: *mut *mut RadishLexError,
) -> *mut RadishLexSession {
    ffi_ptr(error_out, || {
        let options = parse_personalized_rime_session_options(options)?;
        new_personalized_rime_session(options)
    })
}

#[no_mangle]
/// # Safety
/// `context.context_kind` must be readable for the duration of this call.
pub unsafe extern "C" fn radishlex_session_set_learning_context(
    session: *mut RadishLexSession,
    context: RadishLexLearningContext,
    error_out: *mut *mut RadishLexError,
) -> RadishLexStatusCode {
    ffi_status(error_out, || {
        let context = unsafe { context.to_runtime()? };
        session_mut(session)?.set_learning_context(context)
    })
}

impl RadishLexLearningContext {
    /// # Safety
    /// The context kind view must be readable for the duration of this call.
    pub unsafe fn to_runtime(self) -> Result<LearningContext, FfiError> {
        if self.version != RADISHLEX_LEARNING_CONTEXT_VERSION {
            return Err(FfiError::invalid_argument(format!(
                "unsupported learning context version {}",
                self.version
            )));
        }
        let secure_input = ffi_bool(self.secure_input, "secure_input")?;
        let sensitive_application = ffi_bool(self.sensitive_application, "sensitive_application")?;
        let privacy_mode = ffi_bool(self.privacy_mode, "privacy_mode")?;
        let context_known = ffi_bool(self.context_known, "context_known")?;
        let context_kind = unsafe { read_context_kind(self.context_kind)? };
        Ok(LearningContext::new(context_kind)?
            .with_secure_input(secure_input)
            .with_sensitive_application(sensitive_application)
            .with_privacy_mode(privacy_mode)
            .with_context_known(context_known))
    }
}

fn parse_personalized_rime_session_options<'a>(
    options: *const RadishLexPersonalizedRimeSessionOptions,
) -> Result<ParsedPersonalizedRimeSessionOptions<'a>, FfiError> {
    if options.is_null() {
        return Err(FfiError::invalid_argument(
            "personalized rime session options pointer is null",
        ));
    }

    let options = unsafe { *options };
    validate_personalized_rime_session_options_version(options)?;
    let rime_options = RadishLexRimeSessionOptions {
        version: RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: options.shared_data_dir,
        user_data_dir: options.user_data_dir,
        schema: options.schema,
        log_dir: options.log_dir,
        deploy_on_start: options.deploy_on_start,
    };
    let rime = parse_rime_session_options(&rime_options)?;
    let userdb_path = read_required_utf8(options.userdb_path, "userdb_path")?;
    let session_id = read_required_utf8(options.session_id, "session_id")?;
    Ok(ParsedPersonalizedRimeSessionOptions {
        rime,
        userdb_path,
        session_id,
    })
}

#[cfg(feature = "native-rime")]
fn new_personalized_rime_session(
    options: ParsedPersonalizedRimeSessionOptions<'_>,
) -> Result<*mut RadishLexSession, FfiError> {
    let mut config = RimeEngineConfig::new(
        options.rime.shared_data_dir,
        options.rime.user_data_dir,
        options.rime.schema,
    )?;
    if let Some(log_dir) = options.rime.log_dir {
        config = config.with_log_dir(log_dir)?;
    }
    config = config.with_deploy_on_start(options.rime.deploy_on_start);

    Ok(Box::into_raw(Box::new(
        RadishLexSession::new_personalized_rime(config, options.userdb_path, options.session_id)?,
    )))
}

#[cfg(not(feature = "native-rime"))]
fn new_personalized_rime_session(
    options: ParsedPersonalizedRimeSessionOptions<'_>,
) -> Result<*mut RadishLexSession, FfiError> {
    let _ = (
        options.rime.shared_data_dir,
        options.rime.user_data_dir,
        options.rime.schema,
        options.rime.log_dir,
        options.rime.deploy_on_start,
        options.userdb_path,
        options.session_id,
    );
    Err(FfiError::invalid_state(
        "personalized rime session requires radishlex-ime-ffi with the native-rime feature",
    ))
}

pub(crate) fn personalization_status_code(status: PersonalizationStatus) -> u32 {
    match status {
        PersonalizationStatus::Ready => RADISHLEX_PERSONALIZATION_STATUS_READY,
        PersonalizationStatus::PolicyBlocked => RADISHLEX_PERSONALIZATION_STATUS_POLICY_BLOCKED,
        PersonalizationStatus::StorageUnavailable => {
            RADISHLEX_PERSONALIZATION_STATUS_STORAGE_UNAVAILABLE
        }
        PersonalizationStatus::ReadFailed => RADISHLEX_PERSONALIZATION_STATUS_READ_FAILED,
        PersonalizationStatus::RankFailed => RADISHLEX_PERSONALIZATION_STATUS_RANK_FAILED,
    }
}

pub(crate) fn learning_disposition_code(disposition: LearningDisposition) -> u32 {
    match disposition {
        LearningDisposition::NotApplicable => RADISHLEX_LEARNING_NOT_APPLICABLE,
        LearningDisposition::Recorded => RADISHLEX_LEARNING_RECORDED,
        LearningDisposition::Deferred => RADISHLEX_LEARNING_DEFERRED,
        LearningDisposition::SkippedByPolicy => RADISHLEX_LEARNING_SKIPPED_BY_POLICY,
        LearningDisposition::Failed => RADISHLEX_LEARNING_FAILED,
    }
}

fn ffi_bool(value: u8, field: &'static str) -> Result<bool, FfiError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(FfiError::invalid_argument(format!(
            "{field} must be 0 or 1, got {other}"
        ))),
    }
}

unsafe fn read_context_kind(view: RadishLexStringView) -> Result<String, FfiError> {
    if view.len == 0 {
        return Err(FfiError::invalid_argument("context_kind cannot be empty"));
    }
    if view.len > 32 {
        return Err(FfiError::invalid_argument(
            "context_kind exceeds 32 UTF-8 bytes",
        ));
    }
    if view.data.is_null() {
        return Err(FfiError::invalid_argument(
            "context_kind data is null for non-empty view",
        ));
    }
    let bytes = unsafe { slice::from_raw_parts(view.data, view.len) };
    str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .map_err(|_| FfiError::invalid_argument("context_kind must be valid UTF-8"))
}
