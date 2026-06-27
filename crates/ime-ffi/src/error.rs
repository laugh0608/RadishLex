use std::ffi::CString;
use std::os::raw::c_char;

use radishlex_ime_core::CoreError;
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::RimeEngineError;
use radishlex_ime_userdb::UserDbError;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadishLexStatusCode {
    Ok = 0,
    InvalidArgument = 1,
    InvalidState = 2,
    EngineError = 3,
    UserDbError = 4,
    RankerError = 5,
    SyncError = 6,
    InternalError = 255,
}

#[repr(C)]
pub struct RadishLexError {
    code: RadishLexStatusCode,
    message: *mut c_char,
}

impl RadishLexError {
    pub fn new(code: RadishLexStatusCode, message: impl AsRef<str>) -> Self {
        let sanitized = message.as_ref().replace('\0', "\\0");
        let message = CString::new(sanitized)
            .expect("sanitized FFI error message must not contain interior NUL")
            .into_raw();
        Self { code, message }
    }

    pub fn code(&self) -> RadishLexStatusCode {
        self.code
    }

    pub fn message(&self) -> *const c_char {
        self.message.cast_const()
    }

    pub unsafe fn free(error: *mut Self) {
        if error.is_null() {
            return;
        }

        let error = Box::from_raw(error);
        if !error.message.is_null() {
            let _ = CString::from_raw(error.message);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiError {
    pub code: RadishLexStatusCode,
    pub message: String,
}

impl FfiError {
    pub fn new(code: RadishLexStatusCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(RadishLexStatusCode::InvalidArgument, message)
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(RadishLexStatusCode::InvalidState, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RadishLexStatusCode::InternalError, message)
    }

    pub fn into_raw_error(self) -> *mut RadishLexError {
        Box::into_raw(Box::new(RadishLexError::new(self.code, self.message)))
    }
}

impl From<CoreError> for FfiError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::EmptySchemaId => Self::invalid_argument(error.to_string()),
            CoreError::InvalidCandidateIndex { .. } => Self::invalid_argument(error.to_string()),
            CoreError::InvalidCompositionCursor { .. } => Self::internal(error.to_string()),
            CoreError::EngineFailure { .. } => {
                Self::new(RadishLexStatusCode::EngineError, error.to_string())
            }
        }
    }
}

impl From<UserDbError> for FfiError {
    fn from(error: UserDbError) -> Self {
        match error {
            UserDbError::InvalidInput { .. } => Self::invalid_argument(error.to_string()),
            UserDbError::Sqlite(_) | UserDbError::Time(_) => {
                Self::new(RadishLexStatusCode::UserDbError, error.to_string())
            }
        }
    }
}

#[cfg(feature = "native-rime")]
impl From<RimeEngineError> for FfiError {
    fn from(error: RimeEngineError) -> Self {
        match error {
            RimeEngineError::MissingConfigPath { .. } | RimeEngineError::EncodingFailure { .. } => {
                Self::invalid_argument(error.to_string())
            }
            RimeEngineError::Core(core_error) => Self::from(core_error),
            RimeEngineError::NativeFeatureDisabled
            | RimeEngineError::NullApi
            | RimeEngineError::MissingApiFunction { .. }
            | RimeEngineError::NativeProbeFailed { .. }
            | RimeEngineError::FfiFailure { .. }
            | RimeEngineError::EmptyCandidateText => {
                Self::new(RadishLexStatusCode::EngineError, error.to_string())
            }
        }
    }
}
