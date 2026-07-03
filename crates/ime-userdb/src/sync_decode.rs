use std::fmt;

use serde_json::{Map, Value};

use radishlex_ime_sync::{
    ClientSyncMergeInput, DictionaryDeletedTermMergeRecord, DictionaryUserTermMergeRecord,
    RankerWeightMergeRecord, SyncRankerWeightIdentity, SyncTermIdentity, UserTermMergeIntent,
};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    TermSource, TermStatus, UserDbSyncPayloadObjectType, UserDbSyncPlaintextPayload,
    USERDB_SYNC_PAYLOAD_SCHEMA_VERSION,
};

#[derive(Clone, PartialEq, Eq)]
pub struct UserDbDecryptedSyncObject {
    pub object_type: UserDbSyncPayloadObjectType,
    pub key_epoch: u64,
    pub bytes: Vec<u8>,
}

impl UserDbDecryptedSyncObject {
    pub fn new(
        object_type: UserDbSyncPayloadObjectType,
        key_epoch: u64,
        bytes: impl Into<Vec<u8>>,
    ) -> UserDbResult<Self> {
        if key_epoch == 0 {
            return Err(invalid_payload("key_epoch", "value must be greater than 0"));
        }
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(invalid_payload(
                "plaintext_payload",
                "value cannot be empty",
            ));
        }
        Ok(Self {
            object_type,
            key_epoch,
            bytes,
        })
    }

    pub fn from_plaintext_payload(
        payload: &UserDbSyncPlaintextPayload,
        key_epoch: u64,
    ) -> UserDbResult<Self> {
        Self::new(payload.object_type, key_epoch, payload.bytes.clone())
    }
}

impl fmt::Debug for UserDbDecryptedSyncObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDbDecryptedSyncObject")
            .field("object_type", &self.object_type)
            .field("key_epoch", &self.key_epoch)
            .field("bytes", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct UserDbDecodedSyncPayloadBatch {
    pub user_terms: Vec<UserDbSyncUserTermRecord>,
    pub deleted_terms: Vec<UserDbSyncDeletedTermRecord>,
    pub ranker_weights: Vec<UserDbSyncRankerWeightRecord>,
}

impl UserDbDecodedSyncPayloadBatch {
    pub fn to_merge_input(&self) -> UserDbResult<ClientSyncMergeInput> {
        let user_terms = self
            .user_terms
            .iter()
            .map(UserDbSyncUserTermRecord::to_merge_record)
            .collect::<UserDbResult<Vec<_>>>()?;
        let deleted_terms = self
            .deleted_terms
            .iter()
            .map(UserDbSyncDeletedTermRecord::to_merge_record)
            .collect::<UserDbResult<Vec<_>>>()?;
        let ranker_weights = self
            .ranker_weights
            .iter()
            .map(UserDbSyncRankerWeightRecord::to_merge_record)
            .collect::<UserDbResult<Vec<_>>>()?;

        Ok(ClientSyncMergeInput::new(
            user_terms,
            deleted_terms,
            ranker_weights,
        ))
    }
}

impl fmt::Debug for UserDbDecodedSyncPayloadBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDbDecodedSyncPayloadBatch")
            .field("user_terms", &self.user_terms.len())
            .field("deleted_terms", &self.deleted_terms.len())
            .field("ranker_weights", &self.ranker_weights.len())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct UserDbSyncUserTermRecord {
    pub input_code: String,
    pub text: String,
    pub reading: String,
    pub source: TermSource,
    pub weight: f64,
    pub status: TermStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_used_at_ms: Option<i64>,
    pub key_epoch: u64,
}

impl UserDbSyncUserTermRecord {
    fn to_merge_record(&self) -> UserDbResult<DictionaryUserTermMergeRecord> {
        let intent = if self.source == TermSource::ManualAdd {
            UserTermMergeIntent::ExplicitRestore
        } else {
            UserTermMergeIntent::SyncedTerm
        };
        DictionaryUserTermMergeRecord::new(
            term_identity(&self.input_code, &self.text, &self.reading)?,
            self.key_epoch,
            self.updated_at_ms,
            intent,
        )
        .map_err(sync_merge_error)
    }
}

impl fmt::Debug for UserDbSyncUserTermRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDbSyncUserTermRecord")
            .field("identity", &"[redacted]")
            .field("source", &self.source)
            .field("weight", &self.weight)
            .field("status", &self.status)
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .field("last_used_at_ms", &self.last_used_at_ms)
            .field("key_epoch", &self.key_epoch)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserDbSyncDeletedTermRecord {
    pub input_code: String,
    pub text: String,
    pub reading: String,
    pub deleted_at_ms: i64,
    pub reason: String,
    pub key_epoch: u64,
}

impl UserDbSyncDeletedTermRecord {
    fn to_merge_record(&self) -> UserDbResult<DictionaryDeletedTermMergeRecord> {
        DictionaryDeletedTermMergeRecord::new(
            term_identity(&self.input_code, &self.text, &self.reading)?,
            self.key_epoch,
            self.deleted_at_ms,
        )
        .map_err(sync_merge_error)
    }
}

impl fmt::Debug for UserDbSyncDeletedTermRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDbSyncDeletedTermRecord")
            .field("identity", &"[redacted]")
            .field("reason", &self.reason)
            .field("deleted_at_ms", &self.deleted_at_ms)
            .field("key_epoch", &self.key_epoch)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct UserDbSyncRankerWeightRecord {
    pub input_code: String,
    pub text: String,
    pub reading: String,
    pub frequency: i64,
    pub recency_score: f64,
    pub negative_score: f64,
    pub context_kind: String,
    pub updated_at_ms: i64,
    pub key_epoch: u64,
}

impl fmt::Debug for UserDbSyncRankerWeightRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDbSyncRankerWeightRecord")
            .field("identity", &"[redacted]")
            .field("frequency", &self.frequency)
            .field("recency_score", &self.recency_score)
            .field("negative_score", &self.negative_score)
            .field("context_kind", &self.context_kind)
            .field("updated_at_ms", &self.updated_at_ms)
            .field("key_epoch", &self.key_epoch)
            .finish()
    }
}

impl UserDbSyncRankerWeightRecord {
    fn to_merge_record(&self) -> UserDbResult<RankerWeightMergeRecord> {
        let identity = SyncRankerWeightIdentity::new(
            term_identity(&self.input_code, &self.text, &self.reading)?,
            self.context_kind.clone(),
        )
        .map_err(sync_merge_error)?;
        RankerWeightMergeRecord::new(identity, self.key_epoch, self.updated_at_ms)
            .map_err(sync_merge_error)
    }
}

pub fn decode_userdb_sync_objects(
    objects: impl IntoIterator<Item = UserDbDecryptedSyncObject>,
) -> UserDbResult<UserDbDecodedSyncPayloadBatch> {
    let mut batch = UserDbDecodedSyncPayloadBatch {
        user_terms: Vec::new(),
        deleted_terms: Vec::new(),
        ranker_weights: Vec::new(),
    };

    for object in objects {
        decode_object(object, &mut batch)?;
    }

    Ok(batch)
}

fn decode_object(
    object: UserDbDecryptedSyncObject,
    batch: &mut UserDbDecodedSyncPayloadBatch,
) -> UserDbResult<()> {
    let value: Value = serde_json::from_slice(&object.bytes)
        .map_err(|error| invalid_payload("plaintext_payload", error.to_string()))?;
    let root = expect_object(&value, "plaintext_payload")?;

    let array_field = match object.object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => "terms",
        UserDbSyncPayloadObjectType::RankerWeights => "weights",
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => "tombstones",
    };
    ensure_exact_keys(
        root,
        &["payload_schema_version", "object_type", array_field],
    )?;
    validate_payload_header(root, object.object_type)?;

    match object.object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => {
            for item in required_array(root, "terms")? {
                batch
                    .user_terms
                    .push(parse_user_term_record(item, object.key_epoch)?);
            }
        }
        UserDbSyncPayloadObjectType::RankerWeights => {
            for item in required_array(root, "weights")? {
                batch
                    .ranker_weights
                    .push(parse_ranker_weight_record(item, object.key_epoch)?);
            }
        }
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => {
            for item in required_array(root, "tombstones")? {
                batch
                    .deleted_terms
                    .push(parse_deleted_term_record(item, object.key_epoch)?);
            }
        }
    }

    Ok(())
}

fn validate_payload_header(
    root: &Map<String, Value>,
    object_type: UserDbSyncPayloadObjectType,
) -> UserDbResult<()> {
    let schema_version = required_u64(root, "payload_schema_version")?;
    if schema_version != u64::from(USERDB_SYNC_PAYLOAD_SCHEMA_VERSION) {
        return Err(invalid_payload(
            "payload_schema_version",
            format!("value must be {USERDB_SYNC_PAYLOAD_SCHEMA_VERSION}"),
        ));
    }
    let actual_object_type = required_string(root, "object_type")?;
    if actual_object_type != object_type.as_str() {
        return Err(invalid_payload(
            "object_type",
            format!("value must be {}", object_type.as_str()),
        ));
    }
    Ok(())
}

fn parse_user_term_record(value: &Value, key_epoch: u64) -> UserDbResult<UserDbSyncUserTermRecord> {
    let object = expect_object(value, "terms[]")?;
    ensure_exact_keys(
        object,
        &[
            "input_code",
            "text",
            "reading",
            "source",
            "weight",
            "status",
            "created_at_ms",
            "updated_at_ms",
            "last_used_at_ms",
        ],
    )?;

    let input_code = required_string(object, "input_code")?.to_owned();
    let text = required_string(object, "text")?.to_owned();
    let reading = required_string(object, "reading")?.to_owned();
    validate_required("input_code", &input_code)?;
    validate_required("text", &text)?;
    let source = TermSource::from_str(required_string(object, "source")?)?;
    let status = TermStatus::from_str(required_string(object, "status")?)?;
    if status == TermStatus::Deleted {
        return Err(invalid_payload(
            "status",
            "dictionary.user_terms cannot contain deleted terms",
        ));
    }

    let weight = required_f64(object, "weight")?;
    validate_non_negative_f64("weight", weight)?;
    let created_at_ms = required_i64(object, "created_at_ms")?;
    let updated_at_ms = required_i64(object, "updated_at_ms")?;
    if updated_at_ms < created_at_ms {
        return Err(invalid_payload(
            "updated_at_ms",
            "value must be greater than or equal to created_at_ms",
        ));
    }

    Ok(UserDbSyncUserTermRecord {
        input_code,
        text,
        reading,
        source,
        weight,
        status,
        created_at_ms,
        updated_at_ms,
        last_used_at_ms: optional_i64(object, "last_used_at_ms")?,
        key_epoch,
    })
}

fn parse_deleted_term_record(
    value: &Value,
    key_epoch: u64,
) -> UserDbResult<UserDbSyncDeletedTermRecord> {
    let object = expect_object(value, "tombstones[]")?;
    ensure_exact_keys(
        object,
        &["input_code", "text", "reading", "deleted_at_ms", "reason"],
    )?;

    let input_code = required_string(object, "input_code")?.to_owned();
    let text = required_string(object, "text")?.to_owned();
    let reading = required_string(object, "reading")?.to_owned();
    let reason = required_string(object, "reason")?.to_owned();
    validate_required("input_code", &input_code)?;
    validate_required("text", &text)?;
    validate_required("reason", &reason)?;

    Ok(UserDbSyncDeletedTermRecord {
        input_code,
        text,
        reading,
        deleted_at_ms: required_i64(object, "deleted_at_ms")?,
        reason,
        key_epoch,
    })
}

fn parse_ranker_weight_record(
    value: &Value,
    key_epoch: u64,
) -> UserDbResult<UserDbSyncRankerWeightRecord> {
    let object = expect_object(value, "weights[]")?;
    ensure_exact_keys(
        object,
        &[
            "input_code",
            "text",
            "reading",
            "frequency",
            "recency_score",
            "negative_score",
            "context_kind",
            "updated_at_ms",
        ],
    )?;

    let input_code = required_string(object, "input_code")?.to_owned();
    let text = required_string(object, "text")?.to_owned();
    let reading = required_string(object, "reading")?.to_owned();
    let context_kind = required_string(object, "context_kind")?.to_owned();
    validate_required("input_code", &input_code)?;
    validate_required("text", &text)?;
    validate_required("context_kind", &context_kind)?;

    let frequency = required_i64(object, "frequency")?;
    if frequency < 0 {
        return Err(invalid_payload("frequency", "value must be non-negative"));
    }
    let recency_score = required_f64(object, "recency_score")?;
    let negative_score = required_f64(object, "negative_score")?;
    validate_non_negative_f64("recency_score", recency_score)?;
    validate_non_negative_f64("negative_score", negative_score)?;

    Ok(UserDbSyncRankerWeightRecord {
        input_code,
        text,
        reading,
        frequency,
        recency_score,
        negative_score,
        context_kind,
        updated_at_ms: required_i64(object, "updated_at_ms")?,
        key_epoch,
    })
}

fn term_identity(input_code: &str, text: &str, reading: &str) -> UserDbResult<SyncTermIdentity> {
    SyncTermIdentity::new(input_code.to_owned(), text.to_owned(), reading.to_owned())
        .map_err(sync_merge_error)
}

fn expect_object<'a>(
    value: &'a Value,
    field: &'static str,
) -> UserDbResult<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid_payload(field, "value must be a JSON object"))
}

fn ensure_exact_keys(object: &Map<String, Value>, expected: &[&'static str]) -> UserDbResult<()> {
    let has_same_len = object.len() == expected.len();
    let has_expected = expected.iter().all(|key| object.contains_key(*key));
    if has_same_len && has_expected {
        return Ok(());
    }

    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();
    Err(invalid_payload(
        "payload_fields",
        format!(
            "expected fields {}, got {}",
            expected.join(","),
            actual.join(",")
        ),
    ))
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> UserDbResult<&'a Vec<Value>> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_payload(field, "value must be an array"))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> UserDbResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_payload(field, "value must be a string"))
}

fn required_i64(object: &Map<String, Value>, field: &'static str) -> UserDbResult<i64> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid_payload(field, "value must be a signed integer"))
}

fn required_u64(object: &Map<String, Value>, field: &'static str) -> UserDbResult<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_payload(field, "value must be an unsigned integer"))
}

fn required_f64(object: &Map<String, Value>, field: &'static str) -> UserDbResult<f64> {
    object
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid_payload(field, "value must be a number"))
}

fn optional_i64(object: &Map<String, Value>, field: &'static str) -> UserDbResult<Option<i64>> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| invalid_payload(field, "value must be null or a signed integer")),
        None => Err(invalid_payload(field, "field is required")),
    }
}

fn validate_required(field: &'static str, value: &str) -> UserDbResult<()> {
    if value.trim().is_empty() {
        return Err(invalid_payload(field, "value cannot be empty"));
    }
    Ok(())
}

fn validate_non_negative_f64(field: &'static str, value: f64) -> UserDbResult<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(invalid_payload(
            field,
            "value must be finite and non-negative",
        ));
    }
    Ok(())
}

fn sync_merge_error(error: radishlex_ime_sync::SyncPayloadError) -> UserDbError {
    UserDbError::invalid_input("sync_merge", error.to_string())
}

fn invalid_payload(field: &'static str, message: impl Into<String>) -> UserDbError {
    UserDbError::invalid_input(field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NegativeFeedbackDraft, NegativeFeedbackReason, SelectionEventDraft, UserDb};
    use radishlex_ime_sync::SyncMergeDecisionKind;

    #[test]
    fn decoded_payloads_feed_client_merge_model() {
        let objects = vec![
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryUserTerms,
                1,
                r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"luobo","text":"萝卜","reading":"","source":"engine_selection","weight":1,"status":"active","created_at_ms":10,"updated_at_ms":100,"last_used_at_ms":100}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::RankerWeights,
                1,
                r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"luobo","text":"萝卜","reading":"","frequency":2,"recency_score":100,"negative_score":0,"context_kind":"chat","updated_at_ms":130}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
                2,
                r#"{"payload_schema_version":1,"object_type":"dictionary.deleted_terms","tombstones":[{"input_code":"luobo","text":"萝卜","reading":"","deleted_at_ms":120,"reason":"manual_delete"}]}"#,
            ),
        ];

        let batch = decode_userdb_sync_objects(objects).expect("decoded payloads");
        let result = batch
            .to_merge_input()
            .expect("merge input")
            .merge()
            .expect("merge result");

        assert!(result.user_terms.is_empty());
        assert!(result.ranker_weights.is_empty());
        assert_eq!(result.deleted_terms.len(), 1);
        assert_eq!(result.decisions.len(), 2);
        assert_eq!(
            result.decisions[0].kind,
            SyncMergeDecisionKind::BlockedByTombstone
        );
        assert_eq!(
            result.decisions[1].kind,
            SyncMergeDecisionKind::BlockedByTombstone
        );
    }

    #[test]
    fn local_p2_payloads_decode_to_detailed_records_without_p1_sources() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term(
            "luo\\bo",
            "萝\"卜\\词",
            Some("luo\tbo\nline"),
            TermSource::ManualAdd,
        )
        .expect("term is added");
        db.record_selection(
            SelectionEventDraft::new("session-private", "cihe", "词核", 0, 1)
                .with_context_kind("chat"),
        )
        .expect("selection is recorded");
        db.record_negative_feedback(
            NegativeFeedbackDraft::new("cihe", "词核", NegativeFeedbackReason::ManualSuppress)
                .with_context_kind("chat"),
        )
        .expect("feedback is recorded");

        let objects = db
            .p2_plaintext_payloads()
            .expect("payloads")
            .map(|payload| UserDbDecryptedSyncObject::from_plaintext_payload(&payload, 3))
            .collect::<UserDbResult<Vec<_>>>()
            .expect("decrypted objects");

        let batch = decode_userdb_sync_objects(objects).expect("decoded payloads");
        assert_eq!(batch.user_terms.len(), 2);
        assert_eq!(batch.ranker_weights.len(), 1);
        assert!(batch.deleted_terms.is_empty());
        assert!(batch
            .user_terms
            .iter()
            .any(|term| term.text == "萝\"卜\\词" && term.reading == "luo\tbo\nline"));
        assert_eq!(batch.ranker_weights[0].context_kind, "chat");

        let debug = format!("{batch:?}");
        assert!(!debug.contains("session-private"));
        assert!(!debug.contains("manual_suppress"));
    }

    #[test]
    fn decoded_payloads_reject_schema_mismatch_and_invalid_records() {
        let error = UserDbDecryptedSyncObject::new(
            UserDbSyncPayloadObjectType::DictionaryUserTerms,
            0,
            b"{}".to_vec(),
        )
        .expect_err("zero key epoch fails");
        assert!(error.to_string().contains("key_epoch"));

        let object = decrypted(
            UserDbSyncPayloadObjectType::DictionaryUserTerms,
            1,
            r#"{"payload_schema_version":2,"object_type":"dictionary.user_terms","terms":[]}"#,
        );
        let error = decode_userdb_sync_objects([object]).expect_err("schema mismatch fails");
        assert!(error.to_string().contains("payload_schema_version"));

        let object = decrypted(
            UserDbSyncPayloadObjectType::DictionaryUserTerms,
            1,
            r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"x","text":"词","reading":"","source":"manual_add","weight":1,"status":"deleted","created_at_ms":10,"updated_at_ms":20,"last_used_at_ms":null}]}"#,
        );
        let error = decode_userdb_sync_objects([object]).expect_err("deleted user term fails");
        assert!(error.to_string().contains("status"));
    }

    fn decrypted(
        object_type: UserDbSyncPayloadObjectType,
        key_epoch: u64,
        bytes: &str,
    ) -> UserDbDecryptedSyncObject {
        UserDbDecryptedSyncObject::new(object_type, key_epoch, bytes.as_bytes().to_vec())
            .expect("decrypted object")
    }
}
