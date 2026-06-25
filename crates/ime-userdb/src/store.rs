use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    NegativeFeedbackDraft, PrivacyLevel, SelectionEventDraft, TermSource, TermStatus, UserTerm,
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct RankerWeight {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
    pub frequency: i64,
    pub recency_score: f64,
    pub negative_score: f64,
    pub context_kind: String,
}

pub struct UserDb {
    connection: Connection,
}

impl UserDb {
    pub fn open(path: impl AsRef<Path>) -> UserDbResult<Self> {
        let connection = Connection::open(path)?;
        let db = Self { connection };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> UserDbResult<Self> {
        let connection = Connection::open_in_memory()?;
        let db = Self { connection };
        db.migrate()?;
        Ok(db)
    }

    pub fn schema_version(&self) -> UserDbResult<i64> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(Into::into)
    }

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

        if self.has_deleted_tombstone(&input_code, &text, &reading)? {
            if source != TermSource::ManualAdd {
                return self
                    .fetch_term(&input_code, &text, &reading)?
                    .ok_or_else(|| {
                        UserDbError::invalid_input(
                            "term",
                            "term is blocked by a deletion tombstone",
                        )
                    });
            }

            self.clear_deleted_tombstones(&input_code, &text, &reading)?;
        }

        self.connection.execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, 1.0, 'active', ?5, ?5, NULL)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                source = excluded.source,
                status = 'active',
                weight = CASE
                    WHEN user_terms.status = 'deleted' THEN excluded.weight
                    ELSE user_terms.weight + 1.0
                END,
                updated_at_ms = excluded.updated_at_ms",
            params![text, reading, input_code, source.as_str(), now],
        )?;

        self.fetch_term(&input_code, &text, &reading)?
            .ok_or_else(|| UserDbError::invalid_input("term", "term was not stored"))
    }

    pub fn list_active_terms(&self) -> UserDbResult<Vec<UserTerm>> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             FROM user_terms
             WHERE status != 'deleted'
             ORDER BY input_code, text, reading",
        )?;
        let terms = statement
            .query_map([], user_term_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(terms)
    }

    pub fn fetch_term(
        &self,
        input_code: &str,
        text: &str,
        reading: &str,
    ) -> UserDbResult<Option<UserTerm>> {
        self.connection
            .query_row(
                "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
                 FROM user_terms
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
                params![input_code, text, reading],
                user_term_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn record_selection(&mut self, event: SelectionEventDraft) -> UserDbResult<Option<i64>> {
        if event.privacy == PrivacyLevel::P0NeverLearn {
            return Ok(None);
        }

        validate_selection_event(&event)?;
        let reading = normalized_optional(event.selected_reading.as_deref());
        let now = now_ms()?;

        self.connection.execute(
            "INSERT INTO selection_events (
                session_id, input_code, selected_text, selected_reading, candidate_index,
                candidate_count, context_kind, created_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.session_id,
                event.input_code,
                event.selected_text,
                reading,
                event.candidate_index as i64,
                event.candidate_count as i64,
                event.context_kind,
                now
            ],
        )?;
        let event_id = self.connection.last_insert_rowid();

        if !self.has_deleted_tombstone(&event.input_code, &event.selected_text, &reading)? {
            self.upsert_term_from_selection(
                &event.input_code,
                &event.selected_text,
                &reading,
                &event.context_kind,
                now,
            )?;
        }

        Ok(Some(event_id))
    }

    pub fn record_negative_feedback(
        &mut self,
        feedback: NegativeFeedbackDraft,
    ) -> UserDbResult<Option<i64>> {
        if feedback.privacy == PrivacyLevel::P0NeverLearn {
            return Ok(None);
        }

        validate_required("input_code", &feedback.input_code)?;
        validate_required("text", &feedback.text)?;
        validate_required("context_kind", &feedback.context_kind)?;

        let reading = normalized_optional(feedback.reading.as_deref());
        let now = now_ms()?;

        self.connection.execute(
            "INSERT INTO negative_feedback (
                input_code, text, reading, reason, context_kind, created_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                feedback.input_code,
                feedback.text,
                reading,
                feedback.reason.as_str(),
                feedback.context_kind,
                now
            ],
        )?;
        let feedback_id = self.connection.last_insert_rowid();

        self.connection.execute(
            "UPDATE user_terms
             SET status = 'suppressed', weight = MAX(weight - 1.0, 0.0), updated_at_ms = ?4
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3 AND status != 'deleted'",
            params![feedback.input_code, feedback.text, reading, now],
        )?;

        self.upsert_ranker_weight_penalty(
            &feedback.input_code,
            &feedback.text,
            &reading,
            &feedback.context_kind,
            now,
        )?;

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
        let now = now_ms()?;

        let term_id = self
            .fetch_term(&input_code, &text, &reading)?
            .map(|term| term.id);

        self.connection.execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, 'manual_add', 0.0, 'deleted', ?4, ?4, NULL)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                status = 'deleted',
                weight = 0.0,
                updated_at_ms = excluded.updated_at_ms",
            params![text, reading, input_code, now],
        )?;

        self.connection.execute(
            "INSERT INTO deleted_terms (
                term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
             )
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual_delete')",
            params![
                term_id,
                stable_hash_hex(&text),
                stable_hash_hex(&reading),
                stable_hash_hex(&input_code),
                now
            ],
        )?;

        self.connection.execute(
            "DELETE FROM ranker_weights
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
        )?;

        Ok(())
    }

    pub fn selection_event_count(&self) -> UserDbResult<i64> {
        count_rows(&self.connection, "selection_events")
    }

    pub fn negative_feedback_count(&self) -> UserDbResult<i64> {
        count_rows(&self.connection, "negative_feedback")
    }

    pub fn deleted_term_count(&self) -> UserDbResult<i64> {
        count_rows(&self.connection, "deleted_terms")
    }

    pub fn ranker_weight(
        &self,
        input_code: &str,
        text: &str,
        reading: Option<&str>,
        context_kind: &str,
    ) -> UserDbResult<Option<RankerWeight>> {
        let reading = normalized_optional(reading);
        self.connection
            .query_row(
                "SELECT input_code, text, reading, frequency, recency_score, negative_score, context_kind
                 FROM ranker_weights
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3 AND context_kind = ?4",
                params![input_code, text, reading, context_kind],
                |row| {
                    let reading: String = row.get(2)?;
                    Ok(RankerWeight {
                        input_code: row.get(0)?,
                        text: row.get(1)?,
                        reading: optional_from_storage(&reading),
                        frequency: row.get(3)?,
                        recency_score: row.get(4)?,
                        negative_score: row.get(5)?,
                        context_kind: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn migrate(&self) -> UserDbResult<()> {
        self.connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS user_terms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                reading TEXT NOT NULL DEFAULT '',
                input_code TEXT NOT NULL,
                source TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 0.0,
                status TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                last_used_at_ms INTEGER
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_user_terms_identity
                ON user_terms(input_code, text, reading);

            CREATE TABLE IF NOT EXISTS selection_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                input_code TEXT NOT NULL,
                selected_text TEXT NOT NULL,
                selected_reading TEXT NOT NULL DEFAULT '',
                candidate_index INTEGER NOT NULL,
                candidate_count INTEGER NOT NULL,
                context_kind TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS negative_feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                input_code TEXT NOT NULL,
                text TEXT NOT NULL,
                reading TEXT NOT NULL DEFAULT '',
                reason TEXT NOT NULL,
                context_kind TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS deleted_terms (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                term_id INTEGER,
                text_hash TEXT NOT NULL,
                reading_hash TEXT NOT NULL,
                input_code_hash TEXT NOT NULL,
                deleted_at_ms INTEGER NOT NULL,
                reason TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS ranker_weights (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                input_code TEXT NOT NULL,
                text TEXT NOT NULL,
                reading TEXT NOT NULL DEFAULT '',
                frequency INTEGER NOT NULL DEFAULT 0,
                recency_score REAL NOT NULL DEFAULT 0.0,
                negative_score REAL NOT NULL DEFAULT 0.0,
                context_kind TEXT NOT NULL DEFAULT 'general',
                updated_at_ms INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_ranker_weights_identity
                ON ranker_weights(input_code, text, reading, context_kind);

            CREATE TABLE IF NOT EXISTS import_batches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_name TEXT NOT NULL,
                term_count INTEGER NOT NULL,
                created_at_ms INTEGER NOT NULL,
                notes TEXT NOT NULL DEFAULT ''
            );

            PRAGMA user_version = 1;
            ",
        )?;

        let version = self.schema_version()?;
        if version != SCHEMA_VERSION {
            return Err(UserDbError::invalid_input(
                "schema_version",
                format!("expected {SCHEMA_VERSION}, got {version}"),
            ));
        }

        Ok(())
    }

    fn has_deleted_tombstone(
        &self,
        input_code: &str,
        text: &str,
        reading: &str,
    ) -> UserDbResult<bool> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*)
             FROM deleted_terms
             WHERE input_code_hash = ?1 AND text_hash = ?2 AND reading_hash = ?3",
            params![
                stable_hash_hex(input_code),
                stable_hash_hex(text),
                stable_hash_hex(reading)
            ],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn clear_deleted_tombstones(
        &self,
        input_code: &str,
        text: &str,
        reading: &str,
    ) -> UserDbResult<()> {
        self.connection.execute(
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

    fn upsert_term_from_selection(
        &self,
        input_code: &str,
        text: &str,
        reading: &str,
        context_kind: &str,
        now: i64,
    ) -> UserDbResult<()> {
        self.connection.execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, 'engine_selection', 1.0, 'active', ?4, ?4, ?4)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                source = excluded.source,
                weight = CASE
                    WHEN user_terms.status = 'deleted' THEN user_terms.weight
                    ELSE user_terms.weight + 1.0
                END,
                status = CASE
                    WHEN user_terms.status = 'deleted' THEN user_terms.status
                    ELSE 'active'
                END,
                updated_at_ms = excluded.updated_at_ms,
                last_used_at_ms = excluded.last_used_at_ms",
            params![text, reading, input_code, now],
        )?;

        self.connection.execute(
            "INSERT INTO ranker_weights (
                input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
             )
             VALUES (?1, ?2, ?3, 1, ?5, 0.0, ?4, ?5)
             ON CONFLICT(input_code, text, reading, context_kind) DO UPDATE SET
                frequency = ranker_weights.frequency + 1,
                recency_score = excluded.recency_score,
                updated_at_ms = excluded.updated_at_ms",
            params![input_code, text, reading, context_kind, now as f64],
        )?;

        Ok(())
    }

    fn upsert_ranker_weight_penalty(
        &self,
        input_code: &str,
        text: &str,
        reading: &str,
        context_kind: &str,
        now: i64,
    ) -> UserDbResult<()> {
        self.connection.execute(
            "INSERT INTO ranker_weights (
                input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
             )
             VALUES (?1, ?2, ?3, 0, 0.0, 1.0, ?4, ?5)
             ON CONFLICT(input_code, text, reading, context_kind) DO UPDATE SET
                negative_score = ranker_weights.negative_score + 1.0,
                updated_at_ms = excluded.updated_at_ms",
            params![input_code, text, reading, context_kind, now],
        )?;

        Ok(())
    }
}

fn user_term_from_row(row: &Row<'_>) -> rusqlite::Result<UserTerm> {
    let reading: String = row.get(2)?;
    let source: String = row.get(4)?;
    let status: String = row.get(6)?;

    let source = TermSource::from_str(&source).map_err(to_sqlite_conversion_failure)?;
    let status = TermStatus::from_str(&status).map_err(to_sqlite_conversion_failure)?;

    Ok(UserTerm {
        id: row.get(0)?,
        text: row.get(1)?,
        reading: optional_from_storage(&reading),
        input_code: row.get(3)?,
        source,
        weight: row.get(5)?,
        status,
        created_at_ms: row.get(7)?,
        updated_at_ms: row.get(8)?,
        last_used_at_ms: row.get(9)?,
    })
}

fn to_sqlite_conversion_failure(error: UserDbError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn count_rows(connection: &Connection, table: &str) -> UserDbResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(Into::into)
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

fn normalized_required(field: &'static str, value: &str) -> UserDbResult<String> {
    validate_required(field, value)?;
    Ok(value.trim().to_owned())
}

fn validate_required(field: &'static str, value: &str) -> UserDbResult<()> {
    if value.trim().is_empty() {
        return Err(UserDbError::invalid_input(field, "value cannot be empty"));
    }
    Ok(())
}

fn normalized_optional(value: Option<&str>) -> String {
    value.map(str::trim).unwrap_or_default().to_owned()
}

fn optional_from_storage(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn now_ms() -> UserDbResult<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(duration.as_millis() as i64)
}

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::UserDb;
    use crate::{
        NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel, SelectionEventDraft,
        TermSource, TermStatus,
    };

    #[test]
    fn migration_initializes_empty_database() {
        let db = UserDb::open_in_memory().expect("userdb opens");

        assert_eq!(db.schema_version().expect("schema version"), 1);
        assert!(db.list_active_terms().expect("terms").is_empty());
    }

    #[test]
    fn add_query_and_delete_term_records_tombstone() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");

        let term = db
            .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");
        assert_eq!(term.status, TermStatus::Active);
        assert_eq!(term.reading.as_deref(), Some("luo bo"));

        let terms = db.list_active_terms().expect("terms");
        assert_eq!(terms.len(), 1);

        db.delete_term("luobo", "萝卜", Some("luo bo"))
            .expect("term is deleted");

        assert!(db.list_active_terms().expect("terms").is_empty());
        assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
    }

    #[test]
    fn selection_event_updates_term_and_ranker_summary() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        let event = SelectionEventDraft::new("session-1", "luobo", "萝卜", 0, 5)
            .with_reading("luo bo")
            .with_context_kind("chat");

        assert!(db.record_selection(event.clone()).expect("event").is_some());
        assert!(db.record_selection(event).expect("event").is_some());

        assert_eq!(db.selection_event_count().expect("event count"), 2);
        let term = db
            .fetch_term("luobo", "萝卜", "luo bo")
            .expect("term lookup")
            .expect("term exists");
        assert_eq!(term.weight, 2.0);

        let weight = db
            .ranker_weight("luobo", "萝卜", Some("luo bo"), "chat")
            .expect("ranker weight")
            .expect("ranker weight exists");
        assert_eq!(weight.frequency, 2);
        assert_eq!(weight.negative_score, 0.0);
    }

    #[test]
    fn p0_selection_is_not_recorded() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        let event = SelectionEventDraft::new("session-1", "secret", "敏感", 0, 1)
            .with_privacy(PrivacyLevel::P0NeverLearn);

        assert_eq!(db.record_selection(event).expect("event"), None);
        assert_eq!(db.selection_event_count().expect("event count"), 0);
        assert!(db.list_active_terms().expect("terms").is_empty());
    }

    #[test]
    fn negative_feedback_suppresses_term_and_records_penalty() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");

        let feedback =
            NegativeFeedbackDraft::new("luobo", "萝卜", NegativeFeedbackReason::ManualSuppress)
                .with_reading("luo bo")
                .with_context_kind("general");
        assert!(db
            .record_negative_feedback(feedback)
            .expect("feedback")
            .is_some());

        assert_eq!(
            db.fetch_term("luobo", "萝卜", "luo bo")
                .expect("term lookup")
                .expect("term exists")
                .status,
            TermStatus::Suppressed
        );
        assert_eq!(db.negative_feedback_count().expect("count"), 1);

        let weight = db
            .ranker_weight("luobo", "萝卜", Some("luo bo"), "general")
            .expect("ranker weight")
            .expect("ranker weight exists");
        assert_eq!(weight.negative_score, 1.0);
    }

    #[test]
    fn deleted_term_is_not_revived_by_later_selection_event() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");
        db.delete_term("luobo", "萝卜", Some("luo bo"))
            .expect("term is deleted");

        let event =
            SelectionEventDraft::new("session-2", "luobo", "萝卜", 0, 5).with_reading("luo bo");
        db.record_selection(event).expect("event is recorded");

        assert_eq!(db.selection_event_count().expect("event count"), 1);
        assert!(db.list_active_terms().expect("terms").is_empty());
        assert_eq!(
            db.fetch_term("luobo", "萝卜", "luo bo")
                .expect("term lookup")
                .expect("term exists")
                .status,
            TermStatus::Deleted
        );
        assert!(db
            .ranker_weight("luobo", "萝卜", Some("luo bo"), "general")
            .expect("ranker weight")
            .is_none());
    }

    #[test]
    fn manual_import_does_not_revive_deleted_term() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");
        db.delete_term("luobo", "萝卜", Some("luo bo"))
            .expect("term is deleted");

        let term = db
            .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualImport)
            .expect("import respects tombstone");

        assert_eq!(term.status, TermStatus::Deleted);
        assert!(db.list_active_terms().expect("terms").is_empty());
        assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
    }

    #[test]
    fn manual_add_can_restore_deleted_term() {
        let mut db = UserDb::open_in_memory().expect("userdb opens");
        db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("term is added");
        db.delete_term("luobo", "萝卜", Some("luo bo"))
            .expect("term is deleted");

        let term = db
            .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
            .expect("manual add restores term");

        assert_eq!(term.status, TermStatus::Active);
        assert_eq!(db.list_active_terms().expect("terms").len(), 1);
        assert_eq!(db.deleted_term_count().expect("deleted count"), 0);
    }
}
