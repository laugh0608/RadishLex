use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    NegativeFeedbackDraft, PrivacyLevel, SelectionEventDraft, TermSource, TermStatus, UserTerm,
};

use super::connection::MAX_LEARNING_COUNT;
use super::identity::{
    clear_deleted_tombstones, has_deleted_tombstone_on, latest_deleted_tombstone_time,
};
use super::{
    normalized_optional, normalized_required, user_term_from_row, validate_required, UserDb,
};

impl UserDb {
    pub fn add_term(
        &mut self,
        input_code: impl AsRef<str>,
        text: impl AsRef<str>,
        reading: Option<&str>,
        source: TermSource,
    ) -> UserDbResult<UserTerm> {
        let input_code = normalized_required("input_code", input_code.as_ref())?;
        let text = normalized_required("text", text.as_ref())?;
        let reading = normalized_optional(reading);
        let now = now_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        if has_deleted_tombstone_on(&transaction, &input_code, &text, &reading)?
            || fetch_term_status_on(&transaction, &input_code, &text, &reading)?
                == Some(TermStatus::Deleted)
        {
            return Err(UserDbError::invalid_input(
                "term",
                "term is deleted; use explicit restore instead of add",
            ));
        }

        transaction.execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms,
                updated_at_ms, last_used_at_ms, restored_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, 1.0, 'active', ?5, ?5, NULL, NULL)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                source = CASE
                    WHEN user_terms.status = 'suppressed' THEN user_terms.source
                    ELSE excluded.source
                END,
                status = CASE
                    WHEN user_terms.status = 'suppressed' THEN 'suppressed'
                    ELSE 'active'
                END,
                weight = CASE
                    WHEN user_terms.status = 'suppressed' THEN user_terms.weight
                    ELSE MAX(user_terms.weight, 1.0)
                END,
                updated_at_ms = excluded.updated_at_ms",
            params![text, reading, input_code, source.as_str(), now],
        )?;
        let term = fetch_term_on(&transaction, &input_code, &text, &reading)?
            .ok_or_else(|| UserDbError::invalid_input("term", "term was not stored"))?;
        transaction.commit()?;
        Ok(term)
    }

    pub fn restore_term(
        &mut self,
        input_code: impl AsRef<str>,
        text: impl AsRef<str>,
        reading: Option<&str>,
    ) -> UserDbResult<UserTerm> {
        let input_code = normalized_required("input_code", input_code.as_ref())?;
        let text = normalized_required("text", text.as_ref())?;
        let reading = normalized_optional(reading);
        let requested_at_ms = now_ms()?;
        self.restore_term_at_normalized(&input_code, &text, &reading, requested_at_ms, true)
    }

    #[cfg(test)]
    pub(super) fn restore_term_at(
        &mut self,
        input_code: &str,
        text: &str,
        reading: Option<&str>,
        restored_at_ms: i64,
    ) -> UserDbResult<UserTerm> {
        let input_code = normalized_required("input_code", input_code)?;
        let text = normalized_required("text", text)?;
        let reading = normalized_optional(reading);
        self.restore_term_at_normalized(&input_code, &text, &reading, restored_at_ms, false)
    }

    fn restore_term_at_normalized(
        &mut self,
        input_code: &str,
        text: &str,
        reading: &str,
        requested_at_ms: i64,
        advance_if_needed: bool,
    ) -> UserDbResult<UserTerm> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = fetch_term_on(&transaction, input_code, text, reading)?;
        let deleted_at_ms = latest_deleted_tombstone_time(&transaction, input_code, text, reading)?;
        let restorable = deleted_at_ms.is_some()
            || current.as_ref().is_some_and(|term| {
                matches!(term.status, TermStatus::Deleted | TermStatus::Suppressed)
            });
        if !restorable {
            return Err(UserDbError::invalid_input(
                "term",
                "explicit restore requires a deleted or suppressed term",
            ));
        }

        let latest_state_ms = current
            .as_ref()
            .map(|term| term.updated_at_ms)
            .into_iter()
            .chain(deleted_at_ms)
            .max()
            .unwrap_or(i64::MIN);
        let restored_at_ms = if requested_at_ms > latest_state_ms {
            requested_at_ms
        } else if advance_if_needed {
            latest_state_ms.checked_add(1).ok_or_else(|| {
                UserDbError::invalid_input("restored_at_ms", "version timestamp overflow")
            })?
        } else {
            return Err(UserDbError::invalid_input(
                "restored_at_ms",
                format!(
                    "explicit restore version {requested_at_ms} must be newer than {latest_state_ms}"
                ),
            ));
        };

        clear_deleted_tombstones(&transaction, input_code, text, reading)?;
        transaction.execute(
            "UPDATE user_terms
             SET source = 'manual_add', weight = 1.0, status = 'active',
                 updated_at_ms = ?4, restored_at_ms = ?4
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading, restored_at_ms],
        )?;
        transaction.execute(
            "UPDATE ranker_weights
             SET frequency = 0, last_used_at_ms = NULL, negative_score = 0.0,
                 updated_at_ms = ?4
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading, restored_at_ms],
        )?;
        let term = fetch_term_on(&transaction, input_code, text, reading)?
            .ok_or_else(|| UserDbError::invalid_input("term", "restored term is missing"))?;
        transaction.commit()?;
        Ok(term)
    }

    pub fn record_selection(&mut self, event: SelectionEventDraft) -> UserDbResult<Option<i64>> {
        if event.privacy == PrivacyLevel::P0NeverLearn {
            return Ok(None);
        }
        validate_selection_event(&event)?;

        let session_id = normalized_required("session_id", &event.session_id)?;
        let input_code = normalized_required("input_code", &event.input_code)?;
        let selected_text = normalized_required("selected_text", &event.selected_text)?;
        let reading = normalized_optional(event.selected_reading.as_deref());
        let context_kind = normalized_required("context_kind", &event.context_kind)?;
        let candidate_index = i64::try_from(event.candidate_index).map_err(|_| {
            UserDbError::invalid_input("candidate_index", "value exceeds SQLite integer range")
        })?;
        let candidate_count = i64::try_from(event.candidate_count).map_err(|_| {
            UserDbError::invalid_input("candidate_count", "value exceeds SQLite integer range")
        })?;
        let now = now_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO selection_events (
                session_id, input_code, selected_text, selected_reading, candidate_index,
                candidate_count, context_kind, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                input_code,
                selected_text,
                reading,
                candidate_index,
                candidate_count,
                context_kind,
                now
            ],
        )?;
        let event_id = transaction.last_insert_rowid();

        let status = fetch_term_status_on(&transaction, &input_code, &selected_text, &reading)?;
        let blocked =
            has_deleted_tombstone_on(&transaction, &input_code, &selected_text, &reading)?
                || matches!(status, Some(TermStatus::Deleted | TermStatus::Suppressed));
        if !blocked {
            upsert_term_from_selection(&transaction, &input_code, &selected_text, &reading, now)?;
            upsert_ranker_selection(
                &transaction,
                &input_code,
                &selected_text,
                &reading,
                &context_kind,
                now,
            )?;
        }

        transaction.commit()?;
        Ok(Some(event_id))
    }

    pub fn record_negative_feedback(
        &mut self,
        feedback: NegativeFeedbackDraft,
    ) -> UserDbResult<Option<i64>> {
        if feedback.privacy == PrivacyLevel::P0NeverLearn {
            return Ok(None);
        }

        let input_code = normalized_required("input_code", &feedback.input_code)?;
        let text = normalized_required("text", &feedback.text)?;
        let reading = normalized_optional(feedback.reading.as_deref());
        let context_kind = normalized_required("context_kind", &feedback.context_kind)?;
        let now = now_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO negative_feedback (
                input_code, text, reading, reason, context_kind, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input_code,
                text,
                reading,
                feedback.reason.as_str(),
                context_kind,
                now
            ],
        )?;
        let feedback_id = transaction.last_insert_rowid();

        let deleted = has_deleted_tombstone_on(&transaction, &input_code, &text, &reading)?
            || fetch_term_status_on(&transaction, &input_code, &text, &reading)?
                == Some(TermStatus::Deleted);
        if !deleted {
            transaction.execute(
                "INSERT INTO user_terms (
                    text, reading, input_code, source, weight, status, created_at_ms,
                    updated_at_ms, last_used_at_ms, restored_at_ms
                 ) VALUES (?1, ?2, ?3, 'engine_selection', 0.0, 'suppressed', ?4, ?4, NULL, NULL)
                 ON CONFLICT(input_code, text, reading) DO UPDATE SET
                    status = 'suppressed', updated_at_ms = excluded.updated_at_ms",
                params![text, reading, input_code, now],
            )?;
            upsert_ranker_weight_penalty(
                &transaction,
                &input_code,
                &text,
                &reading,
                &context_kind,
                now,
            )?;
        }

        transaction.commit()?;
        Ok(Some(feedback_id))
    }

    pub fn delete_term(
        &mut self,
        input_code: impl AsRef<str>,
        text: impl AsRef<str>,
        reading: Option<&str>,
    ) -> UserDbResult<()> {
        let input_code = normalized_required("input_code", input_code.as_ref())?;
        let text = normalized_required("text", text.as_ref())?;
        let reading = normalized_optional(reading);
        let requested_at_ms = now_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = fetch_term_on(&transaction, &input_code, &text, &reading)?;
        let prior_delete =
            latest_deleted_tombstone_time(&transaction, &input_code, &text, &reading)?;
        let latest_state_ms = current
            .as_ref()
            .map(|term| term.updated_at_ms)
            .into_iter()
            .chain(prior_delete)
            .max()
            .unwrap_or(i64::MIN);
        let deleted_at_ms = if requested_at_ms > latest_state_ms {
            requested_at_ms
        } else {
            latest_state_ms.checked_add(1).ok_or_else(|| {
                UserDbError::invalid_input("deleted_at_ms", "version timestamp overflow")
            })?
        };

        transaction.execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms,
                updated_at_ms, last_used_at_ms, restored_at_ms
             ) VALUES (?1, ?2, ?3, 'manual_add', 0.0, 'deleted', ?4, ?4, NULL, NULL)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                status = 'deleted', weight = 0.0, updated_at_ms = excluded.updated_at_ms,
                last_used_at_ms = NULL, restored_at_ms = NULL",
            params![text, reading, input_code, deleted_at_ms],
        )?;
        let term_id: i64 = transaction.query_row(
            "SELECT id FROM user_terms WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO deleted_terms (
                term_id, input_code, text, reading, deleted_at_ms, reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'manual_delete')
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                term_id = excluded.term_id,
                deleted_at_ms = MAX(deleted_terms.deleted_at_ms, excluded.deleted_at_ms),
                reason = excluded.reason",
            params![term_id, input_code, text, reading, deleted_at_ms],
        )?;
        transaction.execute(
            "DELETE FROM ranker_weights
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
        )?;
        transaction.execute(
            "INSERT INTO negative_feedback (
                input_code, text, reading, reason, context_kind, created_at_ms
             ) VALUES (?1, ?2, ?3, 'manual_delete', 'general', ?4)",
            params![input_code, text, reading, deleted_at_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

pub(super) fn now_ms() -> UserDbResult<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
    i64::try_from(duration.as_millis()).map_err(|_| {
        UserDbError::invalid_input("timestamp", "system time exceeds SQLite integer range")
    })
}

fn validate_selection_event(event: &SelectionEventDraft) -> UserDbResult<()> {
    validate_required("session_id", &event.session_id)?;
    validate_required("input_code", &event.input_code)?;
    validate_required("selected_text", &event.selected_text)?;
    validate_required("context_kind", &event.context_kind)?;
    if event.candidate_index >= event.candidate_count {
        return Err(UserDbError::invalid_input(
            "candidate_index",
            format!(
                "{} is out of range for {} candidates",
                event.candidate_index, event.candidate_count
            ),
        ));
    }
    Ok(())
}

fn fetch_term_on(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<UserTerm>> {
    connection
        .query_row(
            "SELECT id, text, reading, input_code, source, weight, status, created_at_ms,
                    updated_at_ms, last_used_at_ms, restored_at_ms
             FROM user_terms
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
            user_term_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_term_status_on(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<TermStatus>> {
    Ok(fetch_term_on(connection, input_code, text, reading)?.map(|term| term.status))
}

fn upsert_term_from_selection(
    transaction: &Transaction<'_>,
    input_code: &str,
    text: &str,
    reading: &str,
    now: i64,
) -> UserDbResult<()> {
    transaction.execute(
        "INSERT INTO user_terms (
            text, reading, input_code, source, weight, status, created_at_ms,
            updated_at_ms, last_used_at_ms, restored_at_ms
         ) VALUES (?1, ?2, ?3, 'engine_selection', 1.0, 'active', ?4, ?4, ?4, NULL)
         ON CONFLICT(input_code, text, reading) DO UPDATE SET
            updated_at_ms = excluded.updated_at_ms,
            last_used_at_ms = excluded.last_used_at_ms",
        params![text, reading, input_code, now],
    )?;
    Ok(())
}

fn upsert_ranker_selection(
    transaction: &Transaction<'_>,
    input_code: &str,
    text: &str,
    reading: &str,
    context_kind: &str,
    now: i64,
) -> UserDbResult<()> {
    transaction.execute(
        "INSERT INTO ranker_weights (
            input_code, text, reading, frequency, last_used_at_ms, negative_score,
            context_kind, updated_at_ms
         ) VALUES (?1, ?2, ?3, 1, ?5, 0.0, ?4, ?5)
         ON CONFLICT(input_code, text, reading, context_kind) DO UPDATE SET
            frequency = CASE
                WHEN ranker_weights.frequency < ?6 THEN ranker_weights.frequency + 1
                ELSE ?6
            END,
            last_used_at_ms = excluded.last_used_at_ms,
            updated_at_ms = excluded.updated_at_ms",
        params![
            input_code,
            text,
            reading,
            context_kind,
            now,
            MAX_LEARNING_COUNT
        ],
    )?;
    Ok(())
}

fn upsert_ranker_weight_penalty(
    transaction: &Transaction<'_>,
    input_code: &str,
    text: &str,
    reading: &str,
    context_kind: &str,
    now: i64,
) -> UserDbResult<()> {
    transaction.execute(
        "INSERT INTO ranker_weights (
            input_code, text, reading, frequency, last_used_at_ms, negative_score,
            context_kind, updated_at_ms
         ) VALUES (?1, ?2, ?3, 0, NULL, 1.0, ?4, ?5)
         ON CONFLICT(input_code, text, reading, context_kind) DO UPDATE SET
            negative_score = MIN(ranker_weights.negative_score + 1.0, ?6),
            updated_at_ms = excluded.updated_at_ms",
        params![
            input_code,
            text,
            reading,
            context_kind,
            now,
            MAX_LEARNING_COUNT as f64
        ],
    )?;
    Ok(())
}
