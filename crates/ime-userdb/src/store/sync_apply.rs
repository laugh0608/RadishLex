use std::collections::BTreeMap;

use radishlex_ime_sync::{
    ClientSyncMergeResult, DictionaryDeletedTermMergeRecord, DictionaryUserTermMergeRecord,
    RankerWeightMergeRecord, SyncMergeDecisionKind,
};
use rusqlite::{params, Transaction};

use crate::error::{UserDbError, UserDbResult};
use crate::model::TermSource;
use crate::sync_decode::{
    UserDbDecodedSyncPayloadBatch, UserDbSyncDeletedTermRecord, UserDbSyncRankerWeightRecord,
    UserDbSyncUserTermRecord,
};

use super::{stable_hash_hex, UserDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserDbSyncApplySummary {
    pub user_terms_written: usize,
    pub deleted_terms_written: usize,
    pub ranker_weights_written: usize,
    pub blocked_by_tombstone: usize,
    pub tombstones_cleared_by_restore: usize,
    pub stale_weights_blocked: usize,
}

pub(super) fn apply_decoded_sync_payload_batch(
    db: &mut UserDb,
    batch: &UserDbDecodedSyncPayloadBatch,
) -> UserDbResult<UserDbSyncApplySummary> {
    let merge_result = batch
        .to_merge_input()?
        .merge()
        .map_err(|error| UserDbError::invalid_input("sync_merge", error.to_string()))?;
    let mut summary = summary_from_merge_result(&merge_result);

    let user_terms = accepted_user_terms(batch, &merge_result)?;
    let deleted_terms = accepted_deleted_terms(batch, &merge_result)?;
    let ranker_weights = accepted_ranker_weights(batch, &merge_result)?;

    let transaction = db.connection.transaction()?;
    let (user_terms, ranker_weights) =
        filter_local_tombstone_conflicts(&transaction, user_terms, ranker_weights, &mut summary)?;
    summary.user_terms_written = user_terms.len();
    summary.deleted_terms_written = deleted_terms.len();
    summary.ranker_weights_written = ranker_weights.len();

    for tombstone in &deleted_terms {
        apply_deleted_term(&transaction, tombstone)?;
    }
    for term in &user_terms {
        apply_user_term(&transaction, term)?;
    }
    for weight in &ranker_weights {
        apply_ranker_weight(&transaction, weight)?;
    }
    transaction.commit()?;

    Ok(summary)
}

fn summary_from_merge_result(result: &ClientSyncMergeResult) -> UserDbSyncApplySummary {
    let mut summary = UserDbSyncApplySummary {
        user_terms_written: result.user_terms.len(),
        deleted_terms_written: result.deleted_terms.len(),
        ranker_weights_written: result.ranker_weights.len(),
        blocked_by_tombstone: 0,
        tombstones_cleared_by_restore: 0,
        stale_weights_blocked: 0,
    };

    for decision in &result.decisions {
        match decision.kind {
            SyncMergeDecisionKind::BlockedByTombstone => summary.blocked_by_tombstone += 1,
            SyncMergeDecisionKind::ClearedTombstoneByExplicitRestore => {
                summary.tombstones_cleared_by_restore += 1
            }
            SyncMergeDecisionKind::BlockedStaleWeightBeforeRestore => {
                summary.stale_weights_blocked += 1
            }
        }
    }

    summary
}

fn accepted_user_terms(
    batch: &UserDbDecodedSyncPayloadBatch,
    result: &ClientSyncMergeResult,
) -> UserDbResult<Vec<UserDbSyncUserTermRecord>> {
    let mut records = batch
        .user_terms
        .iter()
        .map(|record| (TermApplyKey::from_user_detail(record), record.clone()))
        .collect::<BTreeMap<_, _>>();

    result
        .user_terms
        .iter()
        .map(|record| {
            let key = TermApplyKey::from_user_merge(record);
            records
                .remove(&key)
                .ok_or_else(|| missing_detail("dictionary.user_terms"))
        })
        .collect()
}

fn accepted_deleted_terms(
    batch: &UserDbDecodedSyncPayloadBatch,
    result: &ClientSyncMergeResult,
) -> UserDbResult<Vec<UserDbSyncDeletedTermRecord>> {
    let mut records = batch
        .deleted_terms
        .iter()
        .map(|record| (TermApplyKey::from_deleted_detail(record), record.clone()))
        .collect::<BTreeMap<_, _>>();

    result
        .deleted_terms
        .iter()
        .map(|record| {
            let key = TermApplyKey::from_deleted_merge(record);
            records
                .remove(&key)
                .ok_or_else(|| missing_detail("dictionary.deleted_terms"))
        })
        .collect()
}

fn accepted_ranker_weights(
    batch: &UserDbDecodedSyncPayloadBatch,
    result: &ClientSyncMergeResult,
) -> UserDbResult<Vec<UserDbSyncRankerWeightRecord>> {
    let mut records = batch
        .ranker_weights
        .iter()
        .map(|record| (WeightApplyKey::from_weight_detail(record), record.clone()))
        .collect::<BTreeMap<_, _>>();

    result
        .ranker_weights
        .iter()
        .map(|record| {
            let key = WeightApplyKey::from_weight_merge(record);
            records
                .remove(&key)
                .ok_or_else(|| missing_detail("ranker.weights"))
        })
        .collect()
}

fn filter_local_tombstone_conflicts(
    transaction: &Transaction<'_>,
    user_terms: Vec<UserDbSyncUserTermRecord>,
    ranker_weights: Vec<UserDbSyncRankerWeightRecord>,
    summary: &mut UserDbSyncApplySummary,
) -> UserDbResult<(
    Vec<UserDbSyncUserTermRecord>,
    Vec<UserDbSyncRankerWeightRecord>,
)> {
    let mut restored_terms = BTreeMap::new();
    let mut accepted_user_terms = Vec::new();

    for term in user_terms {
        let key = LocalTermKey::from_user_term(&term);
        if let Some(deleted_at_ms) =
            local_deleted_tombstone_time(transaction, &term.input_code, &term.text, &term.reading)?
        {
            if term_restores_local_tombstone(&term, deleted_at_ms) {
                restored_terms.insert(key, term.updated_at_ms);
                accepted_user_terms.push(term);
            } else {
                summary.blocked_by_tombstone += 1;
            }
        } else {
            accepted_user_terms.push(term);
        }
    }

    let mut accepted_ranker_weights = Vec::new();
    for weight in ranker_weights {
        let key = LocalTermKey::from_ranker_weight(&weight);
        if local_deleted_tombstone_time(
            transaction,
            &weight.input_code,
            &weight.text,
            &weight.reading,
        )?
        .is_some()
        {
            if restored_terms
                .get(&key)
                .is_some_and(|restored_at_ms| weight.updated_at_ms >= *restored_at_ms)
            {
                accepted_ranker_weights.push(weight);
            } else {
                summary.blocked_by_tombstone += 1;
            }
        } else {
            accepted_ranker_weights.push(weight);
        }
    }

    Ok((accepted_user_terms, accepted_ranker_weights))
}

fn term_restores_local_tombstone(term: &UserDbSyncUserTermRecord, deleted_at_ms: i64) -> bool {
    term.source == TermSource::ManualAdd && term.updated_at_ms > deleted_at_ms
}

fn apply_user_term(
    transaction: &Transaction<'_>,
    term: &UserDbSyncUserTermRecord,
) -> UserDbResult<()> {
    clear_deleted_tombstones(transaction, &term.input_code, &term.text, &term.reading)?;
    transaction.execute(
        "INSERT INTO user_terms (
            text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(input_code, text, reading) DO UPDATE SET
            source = excluded.source,
            weight = excluded.weight,
            status = excluded.status,
            created_at_ms = excluded.created_at_ms,
            updated_at_ms = excluded.updated_at_ms,
            last_used_at_ms = excluded.last_used_at_ms",
        params![
            term.text,
            term.reading,
            term.input_code,
            term.source.as_str(),
            term.weight,
            term.status.as_str(),
            term.created_at_ms,
            term.updated_at_ms,
            term.last_used_at_ms
        ],
    )?;
    Ok(())
}

fn apply_deleted_term(
    transaction: &Transaction<'_>,
    tombstone: &UserDbSyncDeletedTermRecord,
) -> UserDbResult<()> {
    transaction.execute(
        "INSERT INTO user_terms (
            text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
         )
         VALUES (?1, ?2, ?3, 'manual_add', 0.0, 'deleted', ?4, ?4, NULL)
         ON CONFLICT(input_code, text, reading) DO UPDATE SET
            status = 'deleted',
            weight = 0.0,
            updated_at_ms = excluded.updated_at_ms,
            last_used_at_ms = NULL",
        params![
            tombstone.text,
            tombstone.reading,
            tombstone.input_code,
            tombstone.deleted_at_ms
        ],
    )?;

    let term_id = transaction.query_row(
        "SELECT id
         FROM user_terms
         WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
        params![tombstone.input_code, tombstone.text, tombstone.reading],
        |row| row.get::<_, i64>(0),
    )?;
    transaction.execute(
        "DELETE FROM deleted_terms
         WHERE input_code_hash = ?1 AND text_hash = ?2 AND reading_hash = ?3
           AND deleted_at_ms = ?4 AND reason = ?5",
        params![
            stable_hash_hex(&tombstone.input_code),
            stable_hash_hex(&tombstone.text),
            stable_hash_hex(&tombstone.reading),
            tombstone.deleted_at_ms,
            tombstone.reason
        ],
    )?;
    transaction.execute(
        "INSERT INTO deleted_terms (
            term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            term_id,
            stable_hash_hex(&tombstone.text),
            stable_hash_hex(&tombstone.reading),
            stable_hash_hex(&tombstone.input_code),
            tombstone.deleted_at_ms,
            tombstone.reason
        ],
    )?;
    transaction.execute(
        "DELETE FROM ranker_weights
         WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
        params![tombstone.input_code, tombstone.text, tombstone.reading],
    )?;
    Ok(())
}

fn apply_ranker_weight(
    transaction: &Transaction<'_>,
    weight: &UserDbSyncRankerWeightRecord,
) -> UserDbResult<()> {
    transaction.execute(
        "INSERT INTO ranker_weights (
            input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(input_code, text, reading, context_kind) DO UPDATE SET
            frequency = excluded.frequency,
            recency_score = excluded.recency_score,
            negative_score = excluded.negative_score,
            updated_at_ms = excluded.updated_at_ms",
        params![
            weight.input_code,
            weight.text,
            weight.reading,
            weight.frequency,
            weight.recency_score,
            weight.negative_score,
            weight.context_kind,
            weight.updated_at_ms
        ],
    )?;
    Ok(())
}

fn clear_deleted_tombstones(
    transaction: &Transaction<'_>,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<()> {
    transaction.execute(
        "DELETE FROM deleted_terms
         WHERE input_code_hash = ?1 AND text_hash = ?2 AND reading_hash = ?3",
        params![
            stable_hash_hex(input_code),
            stable_hash_hex(text),
            stable_hash_hex(reading)
        ],
    )?;
    Ok(())
}

fn local_deleted_tombstone_time(
    transaction: &Transaction<'_>,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<i64>> {
    transaction
        .query_row(
            "SELECT MAX(deleted_at_ms)
             FROM deleted_terms
             WHERE input_code_hash = ?1 AND text_hash = ?2 AND reading_hash = ?3",
            params![
                stable_hash_hex(input_code),
                stable_hash_hex(text),
                stable_hash_hex(reading)
            ],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn missing_detail(object_type: &'static str) -> UserDbError {
    UserDbError::invalid_input(
        "sync_apply",
        format!("accepted {object_type} merge record has no decoded payload detail"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LocalTermKey {
    input_code: String,
    text: String,
    reading: String,
}

impl LocalTermKey {
    fn from_user_term(record: &UserDbSyncUserTermRecord) -> Self {
        Self {
            input_code: record.input_code.clone(),
            text: record.text.clone(),
            reading: record.reading.clone(),
        }
    }

    fn from_ranker_weight(record: &UserDbSyncRankerWeightRecord) -> Self {
        Self {
            input_code: record.input_code.clone(),
            text: record.text.clone(),
            reading: record.reading.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TermApplyKey {
    input_code: String,
    text: String,
    reading: String,
    key_epoch: u64,
    timestamp_ms: i64,
}

impl TermApplyKey {
    fn from_user_detail(record: &UserDbSyncUserTermRecord) -> Self {
        Self {
            input_code: record.input_code.clone(),
            text: record.text.clone(),
            reading: record.reading.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.updated_at_ms,
        }
    }

    fn from_user_merge(record: &DictionaryUserTermMergeRecord) -> Self {
        Self {
            input_code: record.identity.input_code.clone(),
            text: record.identity.text.clone(),
            reading: record.identity.reading.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.updated_at_ms,
        }
    }

    fn from_deleted_detail(record: &UserDbSyncDeletedTermRecord) -> Self {
        Self {
            input_code: record.input_code.clone(),
            text: record.text.clone(),
            reading: record.reading.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.deleted_at_ms,
        }
    }

    fn from_deleted_merge(record: &DictionaryDeletedTermMergeRecord) -> Self {
        Self {
            input_code: record.identity.input_code.clone(),
            text: record.identity.text.clone(),
            reading: record.identity.reading.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.deleted_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct WeightApplyKey {
    input_code: String,
    text: String,
    reading: String,
    context_kind: String,
    key_epoch: u64,
    timestamp_ms: i64,
}

impl WeightApplyKey {
    fn from_weight_detail(record: &UserDbSyncRankerWeightRecord) -> Self {
        Self {
            input_code: record.input_code.clone(),
            text: record.text.clone(),
            reading: record.reading.clone(),
            context_kind: record.context_kind.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.updated_at_ms,
        }
    }

    fn from_weight_merge(record: &RankerWeightMergeRecord) -> Self {
        Self {
            input_code: record.identity.term.input_code.clone(),
            text: record.identity.term.text.clone(),
            reading: record.identity.term.reading.clone(),
            context_kind: record.identity.context_kind.clone(),
            key_epoch: record.key_epoch,
            timestamp_ms: record.updated_at_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sync_decode::{decode_userdb_sync_objects, UserDbDecryptedSyncObject};
    use crate::{TermSource, TermStatus, UserDb, UserDbSyncPayloadObjectType};

    #[test]
    fn apply_decoded_sync_payload_batch_writes_terms_weights_and_tombstones() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term("luobo", "萝卜", None, TermSource::ManualAdd)
            .expect("local term");
        db.connection
            .execute(
                "INSERT INTO ranker_weights (
                    input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
                 )
                 VALUES (?1, ?2, '', 1, 10.0, 0.0, ?3, 10)",
                rusqlite::params!["luobo", "萝卜", "chat"],
            )
            .expect("local weight");
        let batch = decode_userdb_sync_objects([
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryUserTerms,
                2,
                r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"cihe","text":"词核","reading":"","source":"manual_import","weight":3.5,"status":"active","created_at_ms":20,"updated_at_ms":30,"last_used_at_ms":25}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::RankerWeights,
                2,
                r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"cihe","text":"词核","reading":"","frequency":4,"recency_score":30,"negative_score":1.5,"context_kind":"work","updated_at_ms":35}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
                3,
                r#"{"payload_schema_version":1,"object_type":"dictionary.deleted_terms","tombstones":[{"input_code":"luobo","text":"萝卜","reading":"","deleted_at_ms":40,"reason":"manual_delete"}]}"#,
            ),
        ])
        .expect("decoded batch");

        let summary = db
            .apply_decoded_sync_payload_batch(&batch)
            .expect("sync payload applied");

        assert_eq!(summary.user_terms_written, 1);
        assert_eq!(summary.deleted_terms_written, 1);
        assert_eq!(summary.ranker_weights_written, 1);
        let synced = db
            .fetch_term("cihe", "词核", "")
            .expect("term fetch")
            .expect("synced term");
        assert_eq!(synced.source, TermSource::ManualImport);
        assert_eq!(synced.status, TermStatus::Active);
        assert_eq!(synced.weight, 3.5);
        assert_eq!(synced.updated_at_ms, 30);
        assert_eq!(synced.last_used_at_ms, Some(25));

        let deleted = db
            .fetch_term("luobo", "萝卜", "")
            .expect("deleted fetch")
            .expect("deleted term");
        assert_eq!(deleted.status, TermStatus::Deleted);
        assert_eq!(deleted.weight, 0.0);
        assert_eq!(deleted.updated_at_ms, 40);
        assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
        assert!(db
            .ranker_weight("luobo", "萝卜", None, "chat")
            .expect("local weight")
            .is_none());

        let weight = db
            .ranker_weight("cihe", "词核", None, "work")
            .expect("weight fetch")
            .expect("synced weight");
        assert_eq!(weight.frequency, 4);
        assert_eq!(weight.recency_score, 30.0);
        assert_eq!(weight.negative_score, 1.5);
    }

    #[test]
    fn apply_decoded_sync_payload_batch_respects_merge_decisions() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        let batch = decode_userdb_sync_objects([
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryUserTerms,
                1,
                r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"luobo","text":"萝卜","reading":"","source":"engine_selection","weight":1,"status":"active","created_at_ms":10,"updated_at_ms":100,"last_used_at_ms":100}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::RankerWeights,
                1,
                r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"luobo","text":"萝卜","reading":"","frequency":3,"recency_score":100,"negative_score":0,"context_kind":"chat","updated_at_ms":110}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
                2,
                r#"{"payload_schema_version":1,"object_type":"dictionary.deleted_terms","tombstones":[{"input_code":"luobo","text":"萝卜","reading":"","deleted_at_ms":120,"reason":"manual_delete"}]}"#,
            ),
        ])
        .expect("decoded batch");

        let summary = db
            .apply_decoded_sync_payload_batch(&batch)
            .expect("sync payload applied");

        assert_eq!(summary.user_terms_written, 0);
        assert_eq!(summary.deleted_terms_written, 1);
        assert_eq!(summary.ranker_weights_written, 0);
        assert_eq!(summary.blocked_by_tombstone, 2);
        assert!(db
            .ranker_weight("luobo", "萝卜", None, "chat")
            .expect("weight fetch")
            .is_none());
        assert_eq!(
            db.fetch_term("luobo", "萝卜", "")
                .expect("term fetch")
                .expect("deleted term")
                .status,
            TermStatus::Deleted
        );
    }

    #[test]
    fn apply_decoded_sync_payload_batch_respects_existing_local_tombstone() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        mark_local_deleted_at(&mut db, "luobo", "萝卜", 200);
        let batch = decode_userdb_sync_objects([
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryUserTerms,
                3,
                r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"luobo","text":"萝卜","reading":"","source":"engine_selection","weight":4,"status":"active","created_at_ms":250,"updated_at_ms":300,"last_used_at_ms":300}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::RankerWeights,
                3,
                r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"luobo","text":"萝卜","reading":"","frequency":8,"recency_score":300,"negative_score":0,"context_kind":"chat","updated_at_ms":300}]}"#,
            ),
        ])
        .expect("decoded batch");

        let summary = db
            .apply_decoded_sync_payload_batch(&batch)
            .expect("sync payload applied");

        assert_eq!(summary.user_terms_written, 0);
        assert_eq!(summary.deleted_terms_written, 0);
        assert_eq!(summary.ranker_weights_written, 0);
        assert_eq!(summary.blocked_by_tombstone, 2);
        assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
        assert!(db
            .ranker_weight("luobo", "萝卜", None, "chat")
            .expect("weight fetch")
            .is_none());
        assert_eq!(
            db.fetch_term("luobo", "萝卜", "")
                .expect("term fetch")
                .expect("deleted term")
                .status,
            TermStatus::Deleted
        );
    }

    #[test]
    fn apply_decoded_sync_payload_batch_clears_tombstone_for_explicit_restore() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        mark_local_deleted_at(&mut db, "luobo", "萝卜", 100);
        let batch = decode_userdb_sync_objects([
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
                2,
                r#"{"payload_schema_version":1,"object_type":"dictionary.deleted_terms","tombstones":[{"input_code":"luobo","text":"萝卜","reading":"","deleted_at_ms":100,"reason":"manual_delete"}]}"#,
            ),
            decrypted(
                UserDbSyncPayloadObjectType::DictionaryUserTerms,
                2,
                r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"luobo","text":"萝卜","reading":"","source":"manual_add","weight":2,"status":"active","created_at_ms":105,"updated_at_ms":110,"last_used_at_ms":null}]}"#,
            ),
        ])
        .expect("decoded batch");

        let summary = db
            .apply_decoded_sync_payload_batch(&batch)
            .expect("sync payload applied");

        assert_eq!(summary.user_terms_written, 1);
        assert_eq!(summary.deleted_terms_written, 0);
        assert_eq!(summary.tombstones_cleared_by_restore, 1);
        assert_eq!(db.deleted_term_count().expect("deleted count"), 0);
        assert_eq!(
            db.fetch_term("luobo", "萝卜", "")
                .expect("term fetch")
                .expect("restored term")
                .status,
            TermStatus::Active
        );
    }

    fn decrypted(
        object_type: UserDbSyncPayloadObjectType,
        key_epoch: u64,
        bytes: &str,
    ) -> UserDbDecryptedSyncObject {
        UserDbDecryptedSyncObject::new(object_type, key_epoch, bytes.as_bytes().to_vec())
            .expect("decrypted object")
    }

    fn mark_local_deleted_at(db: &mut UserDb, input_code: &str, text: &str, deleted_at_ms: i64) {
        db.delete_term(input_code, text, None)
            .expect("local tombstone");
        db.connection
            .execute(
                "UPDATE user_terms
                 SET updated_at_ms = ?1
                 WHERE input_code = ?2 AND text = ?3 AND reading = ''",
                rusqlite::params![deleted_at_ms, input_code, text],
            )
            .expect("term timestamp update");
        db.connection
            .execute(
                "UPDATE deleted_terms
                 SET deleted_at_ms = ?1",
                rusqlite::params![deleted_at_ms],
            )
            .expect("tombstone timestamp update");
    }
}
