use std::collections::BTreeSet;

use rusqlite::{params, TransactionBehavior};

use crate::error::UserDbResult;
use crate::model::UserTerm;

use super::{
    normalized_optional, normalized_required, optional_from_storage, user_term_from_row,
    RankerWeight, UserDb,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RankingCandidateIdentity {
    pub text: String,
    pub reading: Option<String>,
}

impl RankingCandidateIdentity {
    pub fn new(text: impl Into<String>, reading: Option<impl Into<String>>) -> Self {
        Self {
            text: text.into(),
            reading: reading.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedTermIdentity {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankingSignals {
    pub user_terms: Vec<UserTerm>,
    pub ranker_weights: Vec<RankerWeight>,
    pub deleted_terms: Vec<DeletedTermIdentity>,
}

impl UserDb {
    /// Reads all ranking inputs for one candidate page from a single snapshot.
    pub fn ranking_signals(
        &mut self,
        input_code: &str,
        candidates: &[RankingCandidateIdentity],
    ) -> UserDbResult<RankingSignals> {
        let input_code = normalized_required("input_code", input_code)?;
        let candidate_identities = candidates
            .iter()
            .map(|candidate| {
                Ok((
                    normalized_required("candidate_text", &candidate.text)?,
                    normalized_optional(candidate.reading.as_deref()),
                ))
            })
            .collect::<UserDbResult<BTreeSet<_>>>()?;

        if candidate_identities.is_empty() {
            return Ok(RankingSignals {
                user_terms: Vec::new(),
                ranker_weights: Vec::new(),
                deleted_terms: Vec::new(),
            });
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;

        let user_terms = {
            let mut statement = transaction.prepare(
                "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms, restored_at_ms, import_batch_id
                 FROM user_terms
                 WHERE input_code = ?1
                 ORDER BY id",
            )?;
            let terms = statement
                .query_map(params![input_code], user_term_from_row)?
                .filter_map(|row| match row {
                    Ok(term)
                        if candidate_identities.contains(&(
                            term.text.clone(),
                            normalized_optional(term.reading.as_deref()),
                        )) =>
                    {
                        Some(Ok(term))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            terms
        };

        let ranker_weights = {
            let mut statement = transaction.prepare(
                "SELECT input_code, text, reading, frequency, last_used_at_ms, negative_score, context_kind
                 FROM ranker_weights
                 WHERE input_code = ?1
                 ORDER BY id",
            )?;
            let weights = statement
                .query_map(params![input_code], |row| {
                    let reading: String = row.get(2)?;
                    Ok(RankerWeight {
                        input_code: row.get(0)?,
                        text: row.get(1)?,
                        reading: optional_from_storage(&reading),
                        frequency: row.get(3)?,
                        last_used_at_ms: row.get(4)?,
                        negative_score: row.get(5)?,
                        context_kind: row.get(6)?,
                    })
                })?
                .filter_map(|row| match row {
                    Ok(weight)
                        if candidate_identities.contains(&(
                            weight.text.clone(),
                            normalized_optional(weight.reading.as_deref()),
                        )) =>
                    {
                        Some(Ok(weight))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            weights
        };

        let deleted_terms = {
            let mut statement = transaction.prepare(
                "SELECT input_code, text, reading
                 FROM deleted_terms
                 WHERE input_code = ?1
                 ORDER BY id",
            )?;
            let terms = statement
                .query_map(params![input_code], |row| {
                    let reading: String = row.get(2)?;
                    Ok(DeletedTermIdentity {
                        input_code: row.get(0)?,
                        text: row.get(1)?,
                        reading: optional_from_storage(&reading),
                    })
                })?
                .filter_map(|row| match row {
                    Ok(term)
                        if candidate_identities.contains(&(
                            term.text.clone(),
                            normalized_optional(term.reading.as_deref()),
                        )) =>
                    {
                        Some(Ok(term))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            terms
        };

        transaction.commit()?;
        Ok(RankingSignals {
            user_terms,
            ranker_weights,
            deleted_terms,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{SelectionEventDraft, UserDb};

    use super::RankingCandidateIdentity;

    #[test]
    fn reads_only_current_candidate_identities_in_one_page_snapshot() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.record_selection(
            SelectionEventDraft::new("session", "luobo", "萝卜", 1, 3)
                .with_reading("luobo")
                .with_context_kind("general"),
        )
        .expect("selection records");
        db.record_selection(
            SelectionEventDraft::new("session", "luobo", "落泊", 2, 3)
                .with_reading("luobo")
                .with_context_kind("editor"),
        )
        .expect("selection records");
        db.delete_term("luobo", "落泊", Some("luobo"))
            .expect("term deletes");

        let signals = db
            .ranking_signals(
                "luobo",
                &[
                    RankingCandidateIdentity::new("萝卜", Some("luobo")),
                    RankingCandidateIdentity::new("落泊", Some("luobo")),
                ],
            )
            .expect("signals load");

        assert_eq!(signals.user_terms.len(), 2);
        assert!(signals
            .ranker_weights
            .iter()
            .all(|weight| matches!(weight.text.as_str(), "萝卜" | "落泊")));
        assert_eq!(signals.deleted_terms.len(), 1);
        assert_eq!(signals.deleted_terms[0].text, "落泊");
    }
}
