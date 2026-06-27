//! C ABI boundary for RadishLex host smoke tests.

mod abi;
mod buffer;
mod contract;
mod demo_engine;
mod dictionary;
mod engine;
mod error;
mod key;
mod session;
mod snapshot;
mod sync_status;

pub use abi::*;
pub use buffer::RadishLexBuffer;
pub use contract::{
    RadishLexFfiContract, RADISHLEX_ABI_CONTRACT_VERSION,
    RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND, RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD,
};
pub use dictionary::{
    RadishLexDictionaryExportSummary, RadishLexDictionaryImportSummary,
    RadishLexDictionaryInspectSummary, RadishLexImportBatchList, RadishLexImportBatchView,
    RadishLexUserTermList, RadishLexUserTermView, RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1,
    RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC, RADISHLEX_TERM_SOURCE_ENGINE_SELECTION,
    RADISHLEX_TERM_SOURCE_MANUAL_ADD, RADISHLEX_TERM_SOURCE_MANUAL_IMPORT,
    RADISHLEX_TERM_SOURCE_PHRASE_LEARNING, RADISHLEX_TERM_STATUS_ACTIVE,
    RADISHLEX_TERM_STATUS_DELETED, RADISHLEX_TERM_STATUS_SUPPRESSED,
};
pub use engine::{
    RadishLexRimeSessionOptions, RadishLexSessionOptions, RADISHLEX_ENGINE_KIND_DEMO,
    RADISHLEX_ENGINE_KIND_RIME, RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
    RADISHLEX_SESSION_OPTIONS_VERSION,
};
pub use error::{RadishLexError, RadishLexStatusCode};
pub use key::*;
pub use session::RadishLexSession;
pub use snapshot::{
    RadishLexCandidateView, RadishLexSnapshot, RadishLexStringView,
    RADISHLEX_CANDIDATE_SOURCE_ENGINE, RADISHLEX_CANDIDATE_SOURCE_PERSONALIZED,
    RADISHLEX_CANDIDATE_SOURCE_SYSTEM, RADISHLEX_CANDIDATE_SOURCE_USER_DICTIONARY,
};
pub use sync_status::RadishLexSyncPreflightSummary;
