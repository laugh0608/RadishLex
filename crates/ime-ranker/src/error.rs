use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankerError {
    field: &'static str,
    message: String,
}

impl RankerError {
    pub(crate) fn invalid(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    pub fn field(&self) -> &'static str {
        self.field
    }
}

impl fmt::Display for RankerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.field, self.message)
    }
}

impl std::error::Error for RankerError {}
