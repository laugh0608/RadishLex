use std::fmt;

use radishlex_ime_core::CoreError;

pub type RimeEngineResult<T> = Result<T, RimeEngineError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RimeEngineError {
    MissingConfigPath {
        field: &'static str,
    },
    EmptyCandidateText,
    NativeFeatureDisabled,
    NullApi,
    MissingApiFunction {
        name: &'static str,
    },
    NativeProbeFailed {
        message: String,
    },
    FfiFailure {
        stage: &'static str,
        message: String,
    },
    EncodingFailure {
        field: &'static str,
        message: String,
    },
    Core(CoreError),
}

impl fmt::Display for RimeEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfigPath { field } => {
                write!(f, "Rime config path cannot be empty: {field}")
            }
            Self::EmptyCandidateText => write!(f, "Rime candidate text cannot be empty"),
            Self::NativeFeatureDisabled => write!(
                f,
                "native Rime support is disabled; rebuild with the native-rime feature"
            ),
            Self::NullApi => write!(f, "rime_get_api returned a null pointer"),
            Self::MissingApiFunction { name } => {
                write!(f, "native librime is missing required API function: {name}")
            }
            Self::NativeProbeFailed { message } => {
                write!(f, "failed to locate native librime: {message}")
            }
            Self::FfiFailure { stage, message } => {
                write!(f, "Rime FFI failure during {stage}: {message}")
            }
            Self::EncodingFailure { field, message } => {
                write!(f, "failed to decode Rime {field}: {message}")
            }
            Self::Core(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RimeEngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CoreError> for RimeEngineError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

#[cfg(test)]
mod tests {
    use super::RimeEngineError;

    #[test]
    fn errors_include_stage_context() {
        let error = RimeEngineError::FfiFailure {
            stage: "create_session",
            message: "null session".to_owned(),
        };

        assert_eq!(
            error.to_string(),
            "Rime FFI failure during create_session: null session"
        );
    }
}
