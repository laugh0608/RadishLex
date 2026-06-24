use crate::error::CoreResult;
use crate::key::KeyEvent;
use crate::model::{Candidate, Commit, Composition, SchemaId};

/// Input engine boundary consumed by RadishLex core.
pub trait Engine {
    fn reset(&mut self) -> CoreResult<()>;

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome>;

    fn composition(&self) -> CoreResult<Composition>;

    fn candidates(&self) -> CoreResult<Vec<Candidate>>;

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit>;

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()>;

    fn schema(&self) -> CoreResult<SchemaId>;
}

/// Result of handling one key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyOutcome {
    consumed: bool,
    commit: Option<Commit>,
}

impl KeyOutcome {
    pub fn new(consumed: bool, commit: Option<Commit>) -> Self {
        Self { consumed, commit }
    }

    pub fn ignored() -> Self {
        Self::new(false, None)
    }

    pub fn consumed() -> Self {
        Self::new(true, None)
    }

    pub fn committed(commit: Commit) -> Self {
        Self::new(true, Some(commit))
    }

    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    pub fn commit(&self) -> Option<&Commit> {
        self.commit.as_ref()
    }
}
