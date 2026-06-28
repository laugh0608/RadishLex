//! Local user dictionary and learning event store for RadishLex.

#![forbid(unsafe_code)]

mod error;
mod model;
mod store;
mod sync_decode;

pub use error::{UserDbError, UserDbResult};
pub use model::{
    DictionaryImportBatch, DictionaryImportSummary, DictionaryTermRecord, DictionaryTermsDocument,
    DictionaryTermsFormat, LearningStatusSummary, NegativeFeedbackDraft, NegativeFeedbackReason,
    PrivacyLevel, SelectionEventDraft, SyncPreflightSummary, TermSource, TermStatus,
    UserDbSyncPayloadObjectType, UserDbSyncPlaintextPayload, UserTerm,
    USERDB_SYNC_PAYLOAD_SCHEMA_VERSION,
};
pub use store::{
    decode_dictionary_terms_tsv, decode_dictionary_terms_tsv_document, encode_dictionary_terms_tsv,
    RankerWeight, UserDb,
};
pub use sync_decode::{
    decode_userdb_sync_objects, UserDbDecodedSyncPayloadBatch, UserDbDecryptedSyncObject,
    UserDbSyncDeletedTermRecord, UserDbSyncRankerWeightRecord, UserDbSyncUserTermRecord,
};
