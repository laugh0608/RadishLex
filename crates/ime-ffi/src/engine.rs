use crate::error::FfiError;

pub const RADISHLEX_SESSION_OPTIONS_VERSION: u32 = 1;

pub const RADISHLEX_ENGINE_KIND_DEMO: u32 = 1;
pub const RADISHLEX_ENGINE_KIND_RIME: u32 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexSessionOptions {
    pub version: u32,
    pub engine_kind: u32,
}

impl RadishLexSessionOptions {
    pub const fn demo() -> Self {
        Self {
            version: RADISHLEX_SESSION_OPTIONS_VERSION,
            engine_kind: RADISHLEX_ENGINE_KIND_DEMO,
        }
    }
}

pub fn validate_session_options(options: RadishLexSessionOptions) -> Result<u32, FfiError> {
    if options.version != RADISHLEX_SESSION_OPTIONS_VERSION {
        return Err(FfiError::invalid_argument(format!(
            "unsupported session options version {}",
            options.version
        )));
    }

    match options.engine_kind {
        RADISHLEX_ENGINE_KIND_DEMO => Ok(options.engine_kind),
        RADISHLEX_ENGINE_KIND_RIME => Err(FfiError::invalid_state(
            "rime engine is not available through ime-ffi yet",
        )),
        other => Err(FfiError::invalid_argument(format!(
            "unknown engine kind {other}"
        ))),
    }
}
