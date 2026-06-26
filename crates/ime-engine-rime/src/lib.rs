//! Rime adapter boundary for RadishLex.

#![cfg_attr(not(feature = "native-rime"), forbid(unsafe_code))]

mod config;
mod convert;
mod error;
#[cfg(feature = "native-rime")]
mod ffi;
mod keymap;
#[cfg(feature = "native-rime")]
mod session;

pub use config::RimeEngineConfig;
pub use convert::{candidate_from_view, RimeCandidateView};
pub use error::{RimeEngineError, RimeEngineResult};
pub use keymap::{classify_key_event, RimeKeyInput, RimeNamedKey};
#[cfg(feature = "native-rime")]
pub use session::RimeEngine;

pub fn native_rime_enabled() -> bool {
    cfg!(feature = "native-rime")
}
