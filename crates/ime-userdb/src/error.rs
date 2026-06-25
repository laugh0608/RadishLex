use std::fmt;

pub type UserDbResult<T> = Result<T, UserDbError>;

#[derive(Debug)]
pub enum UserDbError {
    InvalidInput {
        field: &'static str,
        message: String,
    },
    Sqlite(rusqlite::Error),
    Time(std::time::SystemTimeError),
}

impl UserDbError {
    pub fn invalid_input(field: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for UserDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field, message } => {
                write!(f, "invalid {field}: {message}")
            }
            Self::Sqlite(error) => write!(f, "sqlite failure: {error}"),
            Self::Time(error) => write!(f, "system time failure: {error}"),
        }
    }
}

impl std::error::Error for UserDbError {}

impl From<rusqlite::Error> for UserDbError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::time::SystemTimeError> for UserDbError {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self::Time(error)
    }
}
