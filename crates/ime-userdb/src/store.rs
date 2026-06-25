use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    DictionaryImportSummary, DictionaryTermRecord, NegativeFeedbackDraft, PrivacyLevel,
    SelectionEventDraft, TermSource, TermStatus, UserTerm,
};

const SCHEMA_VERSION: i64 = 1;
const DICTIONARY_EXPORT_VERSION: &str = "# radishlex-user-terms-v1";
const DICTIONARY_EXPORT_HEADER: &str = "input_code\ttext\treading\tsource\tweight\tstatus";

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

    pub fn export_dictionary_records(&self) -> UserDbResult<Vec<DictionaryTermRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             FROM user_terms
             WHERE status IN ('active', 'suppressed')
             ORDER BY input_code, text, reading",
        )?;
        let terms = statement
            .query_map([], user_term_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(terms.iter().map(DictionaryTermRecord::from).collect())
    }

    pub fn import_dictionary_records(
        &mut self,
        records: &[DictionaryTermRecord],
        source_name: impl AsRef<str>,
    ) -> UserDbResult<DictionaryImportSummary> {
        let source_name = normalized_required("source_name", source_name.as_ref())?;
        let now = now_ms()?;
        let mut summary = DictionaryImportSummary {
            total_records: records.len(),
            imported_terms: 0,
            skipped_deleted_terms: 0,
        };

        let transaction = self.connection.transaction()?;
        for record in records {
            validate_dictionary_record(record)?;
            let input_code = normalized_required("input_code", &record.input_code)?;
            let text = normalized_required("text", &record.text)?;
            let reading = normalized_optional(record.reading.as_deref());

            if has_deleted_tombstone_on(&transaction, &input_code, &text, &reading)?
                || fetch_term_status_on(&transaction, &input_code, &text, &reading)?
                    == Some(TermStatus::Deleted)
            {
                summary.skipped_deleted_terms += 1;
                continue;
            }

            transaction.execute(
                "INSERT INTO user_terms (
                    text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, NULL)
                 ON CONFLICT(input_code, text, reading) DO UPDATE SET
                    source = excluded.source,
                    weight = excluded.weight,
                    status = excluded.status,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    text,
                    reading,
                    input_code,
                    record.source.as_str(),
                    record.weight,
                    record.status.as_str(),
                    now
                ],
            )?;
            summary.imported_terms += 1;
        }

        transaction.execute(
            "INSERT INTO import_batches (source_name, term_count, created_at_ms, notes)
             VALUES (?1, ?2, ?3, '')",
            params![source_name, summary.imported_terms as i64, now],
        )?;
        transaction.commit()?;

        Ok(summary)
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

    pub fn import_batch_count(&self) -> UserDbResult<i64> {
        count_rows(&self.connection, "import_batches")
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
        has_deleted_tombstone_on(&self.connection, input_code, text, reading)
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

pub fn encode_dictionary_terms_tsv(records: &[DictionaryTermRecord]) -> String {
    let mut output = String::new();
    output.push_str(DICTIONARY_EXPORT_VERSION);
    output.push('\n');
    output.push_str(DICTIONARY_EXPORT_HEADER);
    output.push('\n');

    for record in records {
        output.push_str(&escape_tsv_field(&record.input_code));
        output.push('\t');
        output.push_str(&escape_tsv_field(&record.text));
        output.push('\t');
        output.push_str(&escape_tsv_field(
            record.reading.as_deref().unwrap_or_default(),
        ));
        output.push('\t');
        output.push_str(record.source.as_str());
        output.push('\t');
        output.push_str(&record.weight.to_string());
        output.push('\t');
        output.push_str(record.status.as_str());
        output.push('\n');
    }

    output
}

pub fn decode_dictionary_terms_tsv(input: &str) -> UserDbResult<Vec<DictionaryTermRecord>> {
    let mut lines = input.lines();
    let version = lines
        .next()
        .ok_or_else(|| UserDbError::invalid_input("import_file", "file is empty"))?;
    if version != DICTIONARY_EXPORT_VERSION {
        return Err(UserDbError::invalid_input(
            "import_file",
            format!("expected version line {DICTIONARY_EXPORT_VERSION}"),
        ));
    }

    let header = lines.next().ok_or_else(|| {
        UserDbError::invalid_input("import_file", "missing dictionary field header")
    })?;
    if header != DICTIONARY_EXPORT_HEADER {
        return Err(UserDbError::invalid_input(
            "import_file",
            format!("expected header {DICTIONARY_EXPORT_HEADER}"),
        ));
    }

    let mut records = Vec::new();
    for (offset, line) in lines.enumerate() {
        let line_number = offset + 3;
        if line.trim().is_empty() {
            continue;
        }

        let fields = split_tsv_line(line, line_number)?;
        if fields.len() != 6 {
            return Err(UserDbError::invalid_input(
                "import_file",
                format!("line {line_number} has {} fields; expected 6", fields.len()),
            ));
        }

        let source = TermSource::from_str(&fields[3])?;
        let weight = fields[4].parse::<f64>().map_err(|_| {
            UserDbError::invalid_input(
                "weight",
                format!("line {line_number} has invalid weight {}", fields[4]),
            )
        })?;
        let status = TermStatus::from_str(&fields[5])?;

        let record = DictionaryTermRecord {
            input_code: fields[0].clone(),
            text: fields[1].clone(),
            reading: optional_from_storage(&fields[2]),
            source,
            weight,
            status,
        };
        validate_dictionary_record(&record)?;
        records.push(record);
    }

    Ok(records)
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

fn validate_dictionary_record(record: &DictionaryTermRecord) -> UserDbResult<()> {
    validate_required("input_code", &record.input_code)?;
    validate_required("text", &record.text)?;
    if !record.weight.is_finite() || record.weight < 0.0 {
        return Err(UserDbError::invalid_input(
            "weight",
            "value must be finite and non-negative",
        ));
    }
    if record.status == TermStatus::Deleted {
        return Err(UserDbError::invalid_input(
            "status",
            "dictionary import does not accept deleted terms",
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

fn has_deleted_tombstone_on(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<bool> {
    let count: i64 = connection.query_row(
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

fn fetch_term_status_on(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<TermStatus>> {
    connection
        .query_row(
            "SELECT status
             FROM user_terms
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
            |row| {
                let status: String = row.get(0)?;
                TermStatus::from_str(&status).map_err(to_sqlite_conversion_failure)
            },
        )
        .optional()
        .map_err(Into::into)
}

fn escape_tsv_field(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn split_tsv_line(line: &str, line_number: usize) -> UserDbResult<Vec<String>> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '\t' => {
                fields.push(std::mem::take(&mut current));
            }
            '\\' => {
                let escaped = chars.next().ok_or_else(|| {
                    UserDbError::invalid_input(
                        "import_file",
                        format!("line {line_number} ends with an incomplete escape"),
                    )
                })?;
                match escaped {
                    '\\' => current.push('\\'),
                    't' => current.push('\t'),
                    'n' => current.push('\n'),
                    'r' => current.push('\r'),
                    _ => {
                        return Err(UserDbError::invalid_input(
                            "import_file",
                            format!("line {line_number} contains unknown escape \\{escaped}"),
                        ));
                    }
                }
            }
            _ => current.push(ch),
        }
    }

    fields.push(current);
    Ok(fields)
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
mod tests;
