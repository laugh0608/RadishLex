//! Local user dictionary and learning event store for RadishLex.

#![forbid(unsafe_code)]

mod error;
mod model;
mod store;

pub use error::{UserDbError, UserDbResult};
pub use model::{
    NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel, SelectionEventDraft, TermSource,
    TermStatus, UserTerm,
};
pub use store::{RankerWeight, UserDb};
