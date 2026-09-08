use crate::{RuntimeError, RuntimeResult};

const ALLOWED_CONTEXT_KINDS: &[&str] = &[
    "general", "browser", "chat", "code", "editor", "office", "terminal", "other",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalizationPolicy {
    EngineOnly,
    ReadOnly,
    ReadWrite,
}

impl PersonalizationPolicy {
    pub(crate) fn restricted_by(self, other: Self) -> Self {
        match (self, other) {
            (Self::EngineOnly, _) | (_, Self::EngineOnly) => Self::EngineOnly,
            (Self::ReadOnly, _) | (_, Self::ReadOnly) => Self::ReadOnly,
            (Self::ReadWrite, Self::ReadWrite) => Self::ReadWrite,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningContext {
    secure_input: bool,
    sensitive_application: bool,
    privacy_mode: bool,
    context_known: bool,
    context_kind: String,
}

impl LearningContext {
    pub fn new(context_kind: impl Into<String>) -> RuntimeResult<Self> {
        let context_kind = context_kind.into();
        validate_context_kind(&context_kind)?;
        Ok(Self {
            secure_input: false,
            sensitive_application: false,
            privacy_mode: false,
            context_known: true,
            context_kind,
        })
    }

    pub fn with_secure_input(mut self, secure_input: bool) -> Self {
        self.secure_input = secure_input;
        self
    }

    pub fn with_sensitive_application(mut self, sensitive_application: bool) -> Self {
        self.sensitive_application = sensitive_application;
        self
    }

    pub fn with_privacy_mode(mut self, privacy_mode: bool) -> Self {
        self.privacy_mode = privacy_mode;
        self
    }

    pub fn with_context_known(mut self, context_known: bool) -> Self {
        self.context_known = context_known;
        self
    }

    pub fn context_kind(&self) -> &str {
        &self.context_kind
    }

    pub fn policy(&self) -> PersonalizationPolicy {
        if self.secure_input || self.sensitive_application || !self.context_known {
            PersonalizationPolicy::EngineOnly
        } else if self.privacy_mode {
            PersonalizationPolicy::ReadOnly
        } else {
            PersonalizationPolicy::ReadWrite
        }
    }
}

impl Default for LearningContext {
    fn default() -> Self {
        Self::new("general").expect("default learning context must be valid")
    }
}

fn validate_context_kind(value: &str) -> RuntimeResult<()> {
    if ALLOWED_CONTEXT_KINDS.contains(&value) {
        Ok(())
    } else {
        Err(RuntimeError::invalid_input(
            "context_kind",
            "value must be a supported coarse context category",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{LearningContext, PersonalizationPolicy};

    #[test]
    fn policy_priority_is_secure_then_privacy_then_normal() {
        assert_eq!(
            LearningContext::default().policy(),
            PersonalizationPolicy::ReadWrite
        );
        assert_eq!(
            LearningContext::new("editor")
                .expect("context")
                .with_privacy_mode(true)
                .policy(),
            PersonalizationPolicy::ReadOnly
        );
        assert_eq!(
            LearningContext::new("editor")
                .expect("context")
                .with_privacy_mode(true)
                .with_secure_input(true)
                .policy(),
            PersonalizationPolicy::EngineOnly
        );
        assert_eq!(
            LearningContext::new("editor")
                .expect("context")
                .with_context_known(false)
                .policy(),
            PersonalizationPolicy::EngineOnly
        );
    }

    #[test]
    fn rejects_raw_application_identifiers_as_context() {
        let error = LearningContext::new("com.example.editor").expect_err("raw app id rejects");
        assert!(error.to_string().contains("context_kind"));
    }
}
