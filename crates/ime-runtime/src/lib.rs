//! Local product input runtime for RadishLex.

#![forbid(unsafe_code)]

mod context;
mod error;
mod session;

pub use context::{LearningContext, PersonalizationPolicy};
pub use error::{RuntimeError, RuntimeResult};
pub use session::{
    LearningDisposition, PersonalizationStatus, PersonalizedInputSession, RuntimeCandidate,
    RuntimeEventResult, RuntimeSnapshot,
};
