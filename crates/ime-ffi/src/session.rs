use std::thread::{self, ThreadId};

use radishlex_ime_core::{
    Candidate, Composition, CoreResult, Engine, InputSession, KeyEvent, KeyOutcome, SchemaId,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::{RimeEngine, RimeEngineConfig};
use radishlex_ime_runtime::{LearningContext, LearningDisposition, PersonalizedInputSession};

use crate::demo_engine::FfiDemoEngine;
use crate::engine::RADISHLEX_ENGINE_KIND_DEMO;
#[cfg(feature = "native-rime")]
use crate::engine::RADISHLEX_ENGINE_KIND_RIME;
use crate::error::FfiError;
use crate::snapshot::RadishLexSnapshot;

pub struct RadishLexSession {
    inner: SessionInner,
    owner_thread: ThreadId,
}

pub(crate) struct SessionEvent {
    pub outcome: KeyOutcome,
    pub snapshot: RadishLexSnapshot,
    pub learning_disposition: LearningDisposition,
}

enum SessionInner {
    Legacy(InputSession<SessionEngine>),
    #[cfg_attr(not(feature = "native-rime"), allow(dead_code))]
    Personalized(Box<PersonalizedInputSession<SessionEngine>>),
}

impl RadishLexSession {
    pub fn new() -> Self {
        Self::new_with_engine_kind(RADISHLEX_ENGINE_KIND_DEMO)
    }

    pub fn new_with_engine_kind(engine_kind: u32) -> Self {
        debug_assert_eq!(engine_kind, RADISHLEX_ENGINE_KIND_DEMO);
        Self {
            inner: SessionInner::Legacy(InputSession::new(SessionEngine::Demo(
                FfiDemoEngine::new(),
            ))),
            owner_thread: thread::current().id(),
        }
    }

    #[cfg(feature = "native-rime")]
    pub fn new_rime(config: RimeEngineConfig) -> Result<Self, FfiError> {
        Ok(Self {
            inner: SessionInner::Legacy(InputSession::new(SessionEngine::Rime(RimeEngine::new(
                config,
            )?))),
            owner_thread: thread::current().id(),
        })
    }

    #[cfg(feature = "native-rime")]
    pub fn new_personalized_rime(
        config: RimeEngineConfig,
        userdb_path: &str,
        session_id: &str,
    ) -> Result<Self, FfiError> {
        let engine = SessionEngine::Rime(RimeEngine::new(config)?);
        Ok(Self {
            inner: SessionInner::Personalized(Box::new(PersonalizedInputSession::open(
                engine,
                userdb_path,
                session_id,
            )?)),
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

    pub fn engine_kind(&self) -> u32 {
        match &self.inner {
            SessionInner::Legacy(input) => input.engine().engine_kind(),
            SessionInner::Personalized(runtime) => runtime.engine().engine_kind(),
        }
    }

    pub fn push_char(&mut self, ch: char) -> Result<KeyOutcome, FfiError> {
        self.push_key_event(KeyEvent::press_char(ch))
    }

    pub fn push_key_event(&mut self, key: KeyEvent) -> Result<KeyOutcome, FfiError> {
        Ok(self.handle_key_event(key)?.outcome)
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> Result<SessionEvent, FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(input) => {
                let outcome = input.push_key(key)?;
                let snapshot = RadishLexSnapshot::from_state(input.state()?);
                Ok(SessionEvent {
                    outcome,
                    snapshot,
                    learning_disposition: LearningDisposition::NotApplicable,
                })
            }
            SessionInner::Personalized(runtime) => {
                let event = runtime.handle_key(key)?;
                Ok(SessionEvent {
                    outcome: event.outcome().clone(),
                    snapshot: RadishLexSnapshot::from_runtime(event.snapshot().clone()),
                    learning_disposition: event.learning_disposition(),
                })
            }
        }
    }

    pub(crate) fn select_candidate(
        &mut self,
        display_index: usize,
    ) -> Result<SessionEvent, FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(input) => {
                let outcome = input.select_candidate(display_index)?;
                let snapshot = RadishLexSnapshot::from_state(input.state()?);
                Ok(SessionEvent {
                    outcome,
                    snapshot,
                    learning_disposition: LearningDisposition::NotApplicable,
                })
            }
            SessionInner::Personalized(runtime) => {
                let event = runtime.select_candidate(display_index)?;
                Ok(SessionEvent {
                    outcome: event.outcome().clone(),
                    snapshot: RadishLexSnapshot::from_runtime(event.snapshot().clone()),
                    learning_disposition: event.learning_disposition(),
                })
            }
        }
    }

    pub(crate) fn snapshot(&mut self) -> Result<RadishLexSnapshot, FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(input) => Ok(RadishLexSnapshot::from_state(input.state()?)),
            SessionInner::Personalized(runtime) => {
                Ok(RadishLexSnapshot::from_runtime(runtime.snapshot()?))
            }
        }
    }

    pub(crate) fn reset(&mut self) -> Result<(), FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(input) => input.reset()?,
            SessionInner::Personalized(runtime) => runtime.reset()?,
        }
        Ok(())
    }

    pub(crate) fn set_schema(&mut self, schema: SchemaId) -> Result<(), FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(input) => input.set_schema(schema)?,
            SessionInner::Personalized(runtime) => runtime.set_schema(schema)?,
        }
        Ok(())
    }

    pub(crate) fn set_learning_context(
        &mut self,
        context: LearningContext,
    ) -> Result<(), FfiError> {
        match &mut self.inner {
            SessionInner::Legacy(_) => Err(FfiError::invalid_state(
                "learning context requires a personalized session",
            )),
            SessionInner::Personalized(runtime) => {
                runtime.set_learning_context(context);
                Ok(())
            }
        }
    }

    pub fn snapshot_text(&mut self) -> Result<String, FfiError> {
        Ok(self.snapshot()?.render_text())
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

    fn input_code(&self) -> CoreResult<String> {
        match self {
            Self::Demo(engine) => engine.input_code(),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.input_code(),
        }
    }

    fn select_candidate(&mut self, index: usize) -> CoreResult<KeyOutcome> {
        match self {
            Self::Demo(engine) => engine.select_candidate(index),
            #[cfg(feature = "native-rime")]
            Self::Rime(engine) => engine.select_candidate(index),
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

pub(crate) fn session_mut<'a>(
    session: *mut RadishLexSession,
) -> Result<&'a mut RadishLexSession, FfiError> {
    if session.is_null() {
        return Err(FfiError::invalid_argument("session handle is null"));
    }
    let session = unsafe { &mut *session };
    session.ensure_owner_thread()?;
    Ok(session)
}

pub(crate) fn session_ref<'a>(
    session: *const RadishLexSession,
) -> Result<&'a RadishLexSession, FfiError> {
    if session.is_null() {
        return Err(FfiError::invalid_argument("session handle is null"));
    }
    let session = unsafe { &*session };
    session.ensure_owner_thread()?;
    Ok(session)
}
