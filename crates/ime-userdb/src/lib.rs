//! Local user dictionary and learning event store for RadishLex.

#![forbid(unsafe_code)]

mod error;
mod model;
mod store;

pub use error::{UserDbError, UserDbResult};
pub use model::{
    DictionaryImportBatch, DictionaryImportSummary, DictionaryTermRecord, DictionaryTermsDocument,
    DictionaryTermsFormat, NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel,
    SelectionEventDraft, SyncPreflightSummary, TermSource, TermStatus, UserTerm,
};
pub use store::{
    decode_dictionary_terms_tsv, decode_dictionary_terms_tsv_document, encode_dictionary_terms_tsv,
    RankerWeight, UserDb,
};
