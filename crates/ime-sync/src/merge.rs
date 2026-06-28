use std::collections::BTreeMap;

use crate::model::{SyncObjectType, SyncPayloadError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SyncTermIdentity {
    pub input_code: String,
    pub text: String,
    pub reading: String,
}

impl SyncTermIdentity {
    pub fn new(
        input_code: impl Into<String>,
        text: impl Into<String>,
        reading: impl Into<String>,
    ) -> Result<Self, SyncPayloadError> {
        let identity = Self {
            input_code: input_code.into(),
            text: text.into(),
            reading: reading.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        validate_required("input_code", &self.input_code)?;
        validate_required("text", &self.text)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SyncRankerWeightIdentity {
    pub term: SyncTermIdentity,
    pub context_kind: String,
}

impl SyncRankerWeightIdentity {
    pub fn new(
        term: SyncTermIdentity,
        context_kind: impl Into<String>,
    ) -> Result<Self, SyncPayloadError> {
        let identity = Self {
            term,
            context_kind: context_kind.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.term.validate()?;
        validate_required("context_kind", &self.context_kind)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTermMergeIntent {
    SyncedTerm,
    ExplicitRestore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryUserTermMergeRecord {
    pub identity: SyncTermIdentity,
    pub key_epoch: u64,
    pub updated_at_ms: i64,
    pub intent: UserTermMergeIntent,
}

impl DictionaryUserTermMergeRecord {
    pub fn new(
        identity: SyncTermIdentity,
        key_epoch: u64,
        updated_at_ms: i64,
        intent: UserTermMergeIntent,
    ) -> Result<Self, SyncPayloadError> {
        let record = Self {
            identity,
            key_epoch,
            updated_at_ms,
            intent,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.identity.validate()?;
        validate_key_epoch(self.key_epoch)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryDeletedTermMergeRecord {
    pub identity: SyncTermIdentity,
    pub key_epoch: u64,
    pub deleted_at_ms: i64,
}

impl DictionaryDeletedTermMergeRecord {
    pub fn new(
        identity: SyncTermIdentity,
        key_epoch: u64,
        deleted_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let record = Self {
            identity,
            key_epoch,
            deleted_at_ms,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.identity.validate()?;
        validate_key_epoch(self.key_epoch)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankerWeightMergeRecord {
    pub identity: SyncRankerWeightIdentity,
    pub key_epoch: u64,
    pub updated_at_ms: i64,
}

impl RankerWeightMergeRecord {
    pub fn new(
        identity: SyncRankerWeightIdentity,
        key_epoch: u64,
        updated_at_ms: i64,
    ) -> Result<Self, SyncPayloadError> {
        let record = Self {
            identity,
            key_epoch,
            updated_at_ms,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), SyncPayloadError> {
        self.identity.validate()?;
        validate_key_epoch(self.key_epoch)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSyncMergeInput {
    pub user_terms: Vec<DictionaryUserTermMergeRecord>,
    pub deleted_terms: Vec<DictionaryDeletedTermMergeRecord>,
    pub ranker_weights: Vec<RankerWeightMergeRecord>,
}

impl ClientSyncMergeInput {
    pub fn new(
        user_terms: Vec<DictionaryUserTermMergeRecord>,
        deleted_terms: Vec<DictionaryDeletedTermMergeRecord>,
        ranker_weights: Vec<RankerWeightMergeRecord>,
    ) -> Self {
        Self {
            user_terms,
            deleted_terms,
            ranker_weights,
        }
    }

    pub fn merge(self) -> Result<ClientSyncMergeResult, SyncPayloadError> {
        let tombstones = dominant_tombstones(self.deleted_terms)?;
        let user_terms = dominant_user_terms(self.user_terms)?;
        let ranker_weights = dominant_ranker_weights(self.ranker_weights)?;

        let mut accepted_user_terms = Vec::new();
        let mut accepted_restores = BTreeMap::new();
        let mut decisions = Vec::new();

        for (identity, term) in user_terms {
            if let Some(tombstone) = tombstones.get(&identity) {
                if term_explicitly_restores(&term, tombstone) {
                    decisions.push(SyncMergeDecision::cleared_tombstone(
                        SyncObjectType::DictionaryUserTerms,
                        identity.clone(),
                        term.key_epoch,
                        term.updated_at_ms,
                        tombstone,
                    ));
                    accepted_restores.insert(identity.clone(), term.clone());
                    accepted_user_terms.push(term);
                } else {
                    decisions.push(SyncMergeDecision::blocked_by_tombstone(
                        SyncObjectType::DictionaryUserTerms,
                        identity,
                        term.key_epoch,
                        term.updated_at_ms,
                        tombstone,
                    ));
                }
            } else {
                accepted_user_terms.push(term);
            }
        }

        let accepted_deleted_terms = tombstones
            .iter()
            .filter_map(|(identity, tombstone)| {
                if accepted_restores.contains_key(identity) {
                    None
                } else {
                    Some(tombstone.clone())
                }
            })
            .collect::<Vec<_>>();

        let mut accepted_ranker_weights = Vec::new();
        for (identity, weight) in ranker_weights {
            if let Some(tombstone) = tombstones.get(&identity.term) {
                if let Some(restore) = accepted_restores.get(&identity.term) {
                    if record_happened_after_or_at(
                        weight.key_epoch,
                        weight.updated_at_ms,
                        restore.key_epoch,
                        restore.updated_at_ms,
                    ) {
                        accepted_ranker_weights.push(weight);
                    } else {
                        decisions.push(SyncMergeDecision::stale_weight_before_restore(
                            identity.term,
                            weight.key_epoch,
                            weight.updated_at_ms,
                            tombstone,
                        ));
                    }
                } else {
                    decisions.push(SyncMergeDecision::blocked_by_tombstone(
                        SyncObjectType::RankerWeights,
                        identity.term,
                        weight.key_epoch,
                        weight.updated_at_ms,
                        tombstone,
                    ));
                }
            } else {
                accepted_ranker_weights.push(weight);
            }
        }

        Ok(ClientSyncMergeResult {
            user_terms: accepted_user_terms,
            deleted_terms: accepted_deleted_terms,
            ranker_weights: accepted_ranker_weights,
            decisions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSyncMergeResult {
    pub user_terms: Vec<DictionaryUserTermMergeRecord>,
    pub deleted_terms: Vec<DictionaryDeletedTermMergeRecord>,
    pub ranker_weights: Vec<RankerWeightMergeRecord>,
    pub decisions: Vec<SyncMergeDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMergeDecisionKind {
    BlockedByTombstone,
    ClearedTombstoneByExplicitRestore,
    BlockedStaleWeightBeforeRestore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMergeDecision {
    pub kind: SyncMergeDecisionKind,
    pub object_type: SyncObjectType,
    pub identity: SyncTermIdentity,
    pub object_key_epoch: u64,
    pub object_updated_at_ms: i64,
    pub tombstone_key_epoch: u64,
    pub tombstone_deleted_at_ms: i64,
}

impl SyncMergeDecision {
    fn blocked_by_tombstone(
        object_type: SyncObjectType,
        identity: SyncTermIdentity,
        object_key_epoch: u64,
        object_updated_at_ms: i64,
        tombstone: &DictionaryDeletedTermMergeRecord,
    ) -> Self {
        Self {
            kind: SyncMergeDecisionKind::BlockedByTombstone,
            object_type,
            identity,
            object_key_epoch,
            object_updated_at_ms,
            tombstone_key_epoch: tombstone.key_epoch,
            tombstone_deleted_at_ms: tombstone.deleted_at_ms,
        }
    }

    fn cleared_tombstone(
        object_type: SyncObjectType,
        identity: SyncTermIdentity,
        object_key_epoch: u64,
        object_updated_at_ms: i64,
        tombstone: &DictionaryDeletedTermMergeRecord,
    ) -> Self {
        Self {
            kind: SyncMergeDecisionKind::ClearedTombstoneByExplicitRestore,
            object_type,
            identity,
            object_key_epoch,
            object_updated_at_ms,
            tombstone_key_epoch: tombstone.key_epoch,
            tombstone_deleted_at_ms: tombstone.deleted_at_ms,
        }
    }

    fn stale_weight_before_restore(
        identity: SyncTermIdentity,
        object_key_epoch: u64,
        object_updated_at_ms: i64,
        tombstone: &DictionaryDeletedTermMergeRecord,
    ) -> Self {
        Self {
            kind: SyncMergeDecisionKind::BlockedStaleWeightBeforeRestore,
            object_type: SyncObjectType::RankerWeights,
            identity,
            object_key_epoch,
            object_updated_at_ms,
            tombstone_key_epoch: tombstone.key_epoch,
            tombstone_deleted_at_ms: tombstone.deleted_at_ms,
        }
    }
}

fn dominant_tombstones(
    records: Vec<DictionaryDeletedTermMergeRecord>,
) -> Result<BTreeMap<SyncTermIdentity, DictionaryDeletedTermMergeRecord>, SyncPayloadError> {
    let mut result = BTreeMap::new();
    for record in records {
        record.validate()?;
        upsert_latest(
            &mut result,
            record.identity.clone(),
            record,
            |record| record.key_epoch,
            |record| record.deleted_at_ms,
        );
    }
    Ok(result)
}

fn dominant_user_terms(
    records: Vec<DictionaryUserTermMergeRecord>,
) -> Result<BTreeMap<SyncTermIdentity, DictionaryUserTermMergeRecord>, SyncPayloadError> {
    let mut result = BTreeMap::new();
    for record in records {
        record.validate()?;
        upsert_latest(
            &mut result,
            record.identity.clone(),
            record,
            |record| record.key_epoch,
            |record| record.updated_at_ms,
        );
    }
    Ok(result)
}

fn dominant_ranker_weights(
    records: Vec<RankerWeightMergeRecord>,
) -> Result<BTreeMap<SyncRankerWeightIdentity, RankerWeightMergeRecord>, SyncPayloadError> {
    let mut result = BTreeMap::new();
    for record in records {
        record.validate()?;
        upsert_latest(
            &mut result,
            record.identity.clone(),
            record,
            |record| record.key_epoch,
            |record| record.updated_at_ms,
        );
    }
    Ok(result)
}

fn upsert_latest<K, V, Epoch, Timestamp>(
    records: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    epoch: Epoch,
    timestamp: Timestamp,
) where
    K: Ord,
    Epoch: Fn(&V) -> u64,
    Timestamp: Fn(&V) -> i64,
{
    let should_replace = records
        .get(&key)
        .map(|current| {
            record_happened_after_or_at(
                epoch(&value),
                timestamp(&value),
                epoch(current),
                timestamp(current),
            )
        })
        .unwrap_or(true);

    if should_replace {
        records.insert(key, value);
    }
}

fn term_explicitly_restores(
    term: &DictionaryUserTermMergeRecord,
    tombstone: &DictionaryDeletedTermMergeRecord,
) -> bool {
    term.intent == UserTermMergeIntent::ExplicitRestore
        && record_happened_after(
            term.key_epoch,
            term.updated_at_ms,
            tombstone.key_epoch,
            tombstone.deleted_at_ms,
        )
}

fn record_happened_after(key_epoch: u64, timestamp_ms: i64, base_epoch: u64, base_ms: i64) -> bool {
    key_epoch > base_epoch || (key_epoch == base_epoch && timestamp_ms > base_ms)
}

fn record_happened_after_or_at(
    key_epoch: u64,
    timestamp_ms: i64,
    base_epoch: u64,
    base_ms: i64,
) -> bool {
    key_epoch > base_epoch || (key_epoch == base_epoch && timestamp_ms >= base_ms)
}

fn validate_key_epoch(key_epoch: u64) -> Result<(), SyncPayloadError> {
    if key_epoch == 0 {
        return Err(SyncPayloadError::InvalidField {
            field: "key_epoch",
            message: "value must be greater than 0".to_owned(),
        });
    }
    Ok(())
}

fn validate_required(field: &'static str, value: &str) -> Result<(), SyncPayloadError> {
    if value.trim().is_empty() {
        return Err(SyncPayloadError::InvalidField {
            field,
            message: "value cannot be empty".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tombstone_blocks_old_user_term_and_ranker_weight() {
        let term = term_record("luobo", "萝卜", "", 1, 100, UserTermMergeIntent::SyncedTerm);
        let weight = weight_record("luobo", "萝卜", "", "chat", 1, 110);
        let tombstone = tombstone_record("luobo", "萝卜", "", 2, 120);

        let result = ClientSyncMergeInput::new(vec![term], vec![tombstone], vec![weight])
            .merge()
            .expect("merge result");

        assert!(result.user_terms.is_empty());
        assert_eq!(result.deleted_terms.len(), 1);
        assert!(result.ranker_weights.is_empty());
        assert_eq!(result.decisions.len(), 2);
        assert_eq!(
            result.decisions[0].kind,
            SyncMergeDecisionKind::BlockedByTombstone
        );
        assert_eq!(
            result.decisions[0].object_type,
            SyncObjectType::DictionaryUserTerms
        );
        assert_eq!(
            result.decisions[1].object_type,
            SyncObjectType::RankerWeights
        );
    }

    #[test]
    fn old_epoch_upload_cannot_revive_deleted_term_even_with_later_clock() {
        let old_device_term = term_record(
            "luobo",
            "萝卜",
            "",
            1,
            1_000,
            UserTermMergeIntent::ExplicitRestore,
        );
        let tombstone = tombstone_record("luobo", "萝卜", "", 2, 200);

        let result = ClientSyncMergeInput::new(vec![old_device_term], vec![tombstone], vec![])
            .merge()
            .expect("merge result");

        assert!(result.user_terms.is_empty());
        assert_eq!(result.deleted_terms.len(), 1);
        assert_eq!(
            result.decisions[0].kind,
            SyncMergeDecisionKind::BlockedByTombstone
        );
    }

    #[test]
    fn explicit_restore_after_tombstone_clears_delete_intent() {
        let restored = term_record(
            "luobo",
            "萝卜",
            "",
            2,
            300,
            UserTermMergeIntent::ExplicitRestore,
        );
        let old_weight = weight_record("luobo", "萝卜", "", "chat", 2, 250);
        let new_weight = weight_record("luobo", "萝卜", "", "work", 2, 310);
        let tombstone = tombstone_record("luobo", "萝卜", "", 2, 200);

        let result = ClientSyncMergeInput::new(
            vec![restored],
            vec![tombstone],
            vec![old_weight, new_weight],
        )
        .merge()
        .expect("merge result");

        assert_eq!(result.user_terms.len(), 1);
        assert!(result.deleted_terms.is_empty());
        assert_eq!(result.ranker_weights.len(), 1);
        assert_eq!(result.ranker_weights[0].identity.context_kind, "work");
        assert_eq!(
            result.decisions[0].kind,
            SyncMergeDecisionKind::ClearedTombstoneByExplicitRestore
        );
        assert_eq!(
            result.decisions[1].kind,
            SyncMergeDecisionKind::BlockedStaleWeightBeforeRestore
        );
    }

    #[test]
    fn merge_keeps_newest_record_per_identity() {
        let stale = term_record("luobo", "萝卜", "", 1, 100, UserTermMergeIntent::SyncedTerm);
        let latest = term_record("luobo", "萝卜", "", 1, 200, UserTermMergeIntent::SyncedTerm);
        let stale_tombstone = tombstone_record("old", "旧词", "", 1, 100);
        let latest_tombstone = tombstone_record("old", "旧词", "", 2, 90);

        let result = ClientSyncMergeInput::new(
            vec![stale, latest],
            vec![stale_tombstone, latest_tombstone],
            vec![],
        )
        .merge()
        .expect("merge result");

        assert_eq!(result.user_terms.len(), 1);
        assert_eq!(result.user_terms[0].updated_at_ms, 200);
        assert_eq!(result.deleted_terms.len(), 1);
        assert_eq!(result.deleted_terms[0].key_epoch, 2);
    }

    #[test]
    fn invalid_merge_record_is_rejected() {
        let invalid_identity = SyncTermIdentity {
            input_code: " ".to_owned(),
            text: "萝卜".to_owned(),
            reading: String::new(),
        };
        let invalid = DictionaryUserTermMergeRecord {
            identity: invalid_identity,
            key_epoch: 1,
            updated_at_ms: 10,
            intent: UserTermMergeIntent::SyncedTerm,
        };

        let error = ClientSyncMergeInput::new(vec![invalid], vec![], vec![])
            .merge()
            .expect_err("invalid record fails");
        assert!(error.to_string().contains("input_code"));
    }

    fn term_record(
        input_code: &str,
        text: &str,
        reading: &str,
        key_epoch: u64,
        updated_at_ms: i64,
        intent: UserTermMergeIntent,
    ) -> DictionaryUserTermMergeRecord {
        DictionaryUserTermMergeRecord::new(
            term_identity(input_code, text, reading),
            key_epoch,
            updated_at_ms,
            intent,
        )
        .expect("term record")
    }

    fn tombstone_record(
        input_code: &str,
        text: &str,
        reading: &str,
        key_epoch: u64,
        deleted_at_ms: i64,
    ) -> DictionaryDeletedTermMergeRecord {
        DictionaryDeletedTermMergeRecord::new(
            term_identity(input_code, text, reading),
            key_epoch,
            deleted_at_ms,
        )
        .expect("tombstone")
    }

    fn weight_record(
        input_code: &str,
        text: &str,
        reading: &str,
        context_kind: &str,
        key_epoch: u64,
        updated_at_ms: i64,
    ) -> RankerWeightMergeRecord {
        RankerWeightMergeRecord::new(
            SyncRankerWeightIdentity::new(term_identity(input_code, text, reading), context_kind)
                .expect("weight identity"),
            key_epoch,
            updated_at_ms,
        )
        .expect("weight")
    }

    fn term_identity(input_code: &str, text: &str, reading: &str) -> SyncTermIdentity {
        SyncTermIdentity::new(input_code, text, reading).expect("term identity")
    }
}
