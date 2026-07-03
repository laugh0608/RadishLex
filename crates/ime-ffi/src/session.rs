use std::thread::{self, ThreadId};

use radishlex_ime_core::{
    Candidate, Commit, Composition, CoreResult, Engine, InputSession, KeyEvent, KeyOutcome,
    SchemaId, SessionState,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::{RimeEngine, RimeEngineConfig};

use crate::demo_engine::FfiDemoEngine;
use crate::engine::RADISHLEX_ENGINE_KIND_DEMO;
#[cfg(feature = "native-rime")]
use crate::engine::RADISHLEX_ENGINE_KIND_RIME;
use crate::error::FfiError;

pub struct RadishLexSession {
    inner: InputSession<SessionEngine>,
    owner_thread: ThreadId,
}

impl RadishLexSession {
    pub fn new() -> Self {
        Self::new_with_engine_kind(RADISHLEX_ENGINE_KIND_DEMO)
    }

    pub fn new_with_engine_kind(engine_kind: u32) -> Self {
        debug_assert_eq!(engine_kind, RADISHLEX_ENGINE_KIND_DEMO);
        Self {
            inner: InputSession::new(SessionEngine::Demo(FfiDemoEngine::new())),
            owner_thread: thread::current().id(),
        }
    }

    #[cfg(feature = "native-rime")]
    pub fn new_rime(config: RimeEngineConfig) -> Result<Self, FfiError> {
        Ok(Self {
            inner: InputSession::new(SessionEngine::Rime(RimeEngine::new(config)?)),
            owner_thread: thread::current().id(),
        })
    }

    pub fn ensure_owner_thread(&self) -> Result<(), FfiError> {
        if thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(FfiError::invalid_state(
                "session handle must be used on the thread that created it",
            ))
        }
    }

    pub(crate) fn inner_mut(&mut self) -> &mut InputSession<SessionEngine> {
        &mut self.inner
    }

    pub fn engine_kind(&self) -> u32 {
        self.inner.engine().engine_kind()
    }

    pub fn push_char(&mut self, ch: char) -> radishlex_ime_core::CoreResult<()> {
        self.push_key_event(KeyEvent::press_char(ch))
    }

    pub fn push_key_event(&mut self, key: KeyEvent) -> radishlex_ime_core::CoreResult<()> {
        self.inner.push_key(key)?;
        Ok(())
    }

    pub fn state(&self) -> radishlex_ime_core::CoreResult<SessionState> {
        self.inner.state()
    }

    pub fn snapshot_text(&self) -> radishlex_ime_core::CoreResult<String> {
        render_snapshot(&self.inner.state()?)
    }
}

pub(crate) enum SessionEngine {
    Demo(FfiDemoEngine),
    #[cfg(feature = "native-rime")]
    Rime(RimeEngine),
}

impl SessionEngine {
    fn engine_kind(&self) -> u32 {
        match self {
            Self::Demo(_) => RADISHLEX_ENGINE_KIND_DEMO,
            #[cfg(feature = "native-rime")]
            Self::Rime(_) => RADISHLEX_ENGINE_KIND_RIME,
        }
    }
}

impl Engine for SessionEngine {
    fn reset(&mut self) -> CoreResult<()> {
        match self {
            Self::Demo(engine) => engine.reset(),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.reset(),
        }
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        match self {
            Self::Demo(engine) => engine.push_key(key),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.push_key(key),
        }
    }

    fn composition(&self) -> CoreResult<Composition> {
        match self {
            Self::Demo(engine) => engine.composition(),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.composition(),
        }
    }

    fn candidates(&self) -> CoreResult<Vec<Candidate>> {
        match self {
            Self::Demo(engine) => engine.candidates(),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.candidates(),
        }
    }

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        match self {
            Self::Demo(engine) => engine.commit_candidate(index),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.commit_candidate(index),
        }
    }

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        match self {
            Self::Demo(engine) => engine.set_schema(schema),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.set_schema(schema),
        }
    }

    fn schema(&self) -> CoreResult<SchemaId> {
        match self {
            Self::Demo(engine) => engine.schema(),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.schema(),
        }
    }
}

impl Default for RadishLexSession {
    fn default() -> Self {
        Self::new()
    }
}

fn render_snapshot(state: &SessionState) -> radishlex_ime_core::CoreResult<String> {
    let mut output = String::new();
    output.push_str(&format!("schema: {}\n", state.schema().as_str()));
    output.push_str(&format!("composition: {}\n", state.composition().preedit()));
    output.push_str(&format!("cursor: {}\n", state.composition().cursor()));
    output.push_str("candidates:\n");

    if state.candidates().is_empty() {
        output.push_str("  <none>\n");
    } else {
        for (index, candidate) in state.candidates().iter().enumerate() {
            output.push_str(&format!("  {index}. {}", candidate.text()));
            if let Some(reading) = candidate.reading() {
                output.push_str(&format!(" [{reading}]"));
            }
            if let Some(annotation) = candidate.annotation() {
                output.push_str(&format!(" - {annotation}"));
            }
            output.push('\n');
        }
    }

    Ok(output)
}
