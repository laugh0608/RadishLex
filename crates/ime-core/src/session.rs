use crate::engine::{Engine, KeyOutcome};
use crate::error::CoreResult;
use crate::key::KeyEvent;
use crate::model::{Commit, SchemaId, SessionState};

/// Platform-neutral input session over a concrete engine implementation.
pub struct InputSession<E> {
    engine: E,
}

impl<E: Engine> InputSession<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    pub fn reset(&mut self) -> CoreResult<()> {
        self.engine.reset()
    }

    pub fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        self.engine.push_key(key)
    }

    pub fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        self.engine.commit_candidate(index)
    }

    pub fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        self.engine.set_schema(schema)
    }

    pub fn state(&self) -> CoreResult<SessionState> {
        Ok(SessionState::new(
            self.engine.composition()?,
            self.engine.candidates()?,
            self.engine.schema()?,
        ))
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    pub fn into_engine(self) -> E {
        self.engine
    }
}
