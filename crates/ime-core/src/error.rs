use std::fmt;

/// Shared result type for core input operations.
pub type CoreResult<T> = Result<T, CoreError>;

/// Errors surfaced by the Rust core boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    EmptySchemaId,
    InvalidCompositionCursor { cursor: usize, byte_len: usize },
    InvalidCandidateIndex { index: usize, len: usize },
    EngineFailure { message: String },
}

impl CoreError {
    /// Wraps an underlying engine failure without exposing engine-private state.
    pub fn engine(message: impl Into<String>) -> Self {
        Self::EngineFailure {
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySchemaId => write!(f, "schema id cannot be empty"),
            Self::InvalidCompositionCursor { cursor, byte_len } => write!(
                f,
                "composition cursor {cursor} is not a valid UTF-8 boundary for {byte_len} bytes"
            ),
            Self::InvalidCandidateIndex { index, len } => {
                write!(
                    f,
                    "candidate index {index} is out of range for {len} candidates"
                )
            }
            Self::EngineFailure { message } => write!(f, "engine failure: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}
