//! Rime adapter boundary for RadishLex.

#![cfg_attr(not(feature = "native-rime"), forbid(unsafe_code))]

mod config;
mod convert;
mod error;
#[cfg(feature = "native-rime")]
mod ffi;
mod keymap;
#[cfg(feature = "native-rime")]
mod runtime;
#[cfg(feature = "native-rime")]
mod runtime_config;
#[cfg(feature = "native-rime")]
mod session;

pub use config::RimeEngineConfig;
pub use convert::{candidate_from_view, RimeCandidateView};
pub use error::{RimeEngineError, RimeEngineResult};
pub use keymap::{classify_key_event, RimeKeyInput, RimeNamedKey};
#[cfg(feature = "native-rime")]
pub use session::RimeEngine;

#[cfg(feature = "native-rime")]
/// Finalizes the process Rime runtime after all sessions are dropped.
///
/// The call must run on the thread that initialized the runtime. It is
/// idempotent, but returns an error while any Rime session remains active.
pub fn shutdown_process_runtime() -> RimeEngineResult<()> {
    runtime::shutdown_process_runtime()
}

pub fn native_rime_enabled() -> bool {
    cfg!(feature = "native-rime")
}
