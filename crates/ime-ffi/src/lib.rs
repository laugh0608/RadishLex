//! C ABI boundary for RadishLex host smoke tests.

mod abi;
mod buffer;
mod demo_engine;
mod engine;
mod error;
mod key;
mod session;
mod snapshot;
mod sync_status;

pub use abi::*;
pub use engine::{
    RadishLexSessionOptions, RADISHLEX_ENGINE_KIND_DEMO, RADISHLEX_ENGINE_KIND_RIME,
    RADISHLEX_SESSION_OPTIONS_VERSION,
};
pub use error::RadishLexStatusCode;
pub use key::*;
pub use snapshot::{
    RadishLexCandidateView, RadishLexSnapshot, RadishLexStringView,
    RADISHLEX_CANDIDATE_SOURCE_ENGINE, RADISHLEX_CANDIDATE_SOURCE_PERSONALIZED,
    RADISHLEX_CANDIDATE_SOURCE_SYSTEM, RADISHLEX_CANDIDATE_SOURCE_USER_DICTIONARY,
};
pub use sync_status::RadishLexSyncPreflightSummary;
