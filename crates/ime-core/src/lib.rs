//! Core input model and engine boundary for RadishLex.

#![forbid(unsafe_code)]

mod engine;
mod error;
mod key;
mod model;
mod session;

pub use engine::{Engine, KeyOutcome};
pub use error::{CoreError, CoreResult};
pub use key::{Key, KeyEvent, KeyModifiers, KeyPhase, NamedKey};
pub use model::{
    Candidate, CandidateSource, Commit, CommitSource, Composition, SchemaId, SessionState,
};
pub use session::InputSession;
