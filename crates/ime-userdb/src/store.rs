use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use std::collections::BTreeSet;

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    DeletedTermTombstone, DictionaryImportBatch, DictionaryImportSummary, DictionaryTermRecord,
    DictionaryTermsDocument, DictionaryTermsFormat, LearningCaseDeletedTombstoneInspection,
    LearningCaseIdentity, LearningCaseInspection, LearningCaseRankerWeightInspection,
    LearningCaseTermInspection, LearningStatusSummary, SyncPreflightSummary, TermSource,
    TermStatus, UserDbSyncPlaintextPayload, UserTerm, LEARNING_CASE_INSPECTION_VERSION,
};

mod connection;
mod identity;
mod learning;
mod ranking;
mod snapshot;
mod sync_apply;
mod sync_payload;
mod sync_repository;
mod trusted_lifecycle;
mod wrapped_epoch_material;

use identity::has_deleted_tombstone_on;
use learning::now_ms;
pub use ranking::{DeletedTermIdentity, RankingCandidateIdentity, RankingSignals};
pub use sync_apply::UserDbSyncApplySummary;

const DICTIONARY_EXPORT_HEADER: &str = "input_code\ttext\treading\tsource\tweight\tstatus";

#[derive(Debug, Clone, PartialEq)]
pub struct RankerWeight {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
    pub frequency: i64,
    pub last_used_at_ms: Option<i64>,
    pub negative_score: f64,
    pub context_kind: String,
}

#[derive(Debug)]
pub struct UserDb {
    pub(super) connection: Connection,
}

impl UserDb {
    pub fn list_active_terms(&self) -> UserDbResult<Vec<UserTerm>> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms, restored_at_ms, import_batch_id
             FROM user_terms
             WHERE status != 'deleted'
             ORDER BY input_code, text, reading",
        )?;
        let terms = statement
            .query_map([], user_term_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(terms)
    }

    pub fn list_deleted_term_tombstones(&self) -> UserDbResult<Vec<DeletedTermTombstone>> {
        let mut statement = self.connection.prepare(
            "SELECT input_code, text, reading, deleted_at_ms, reason
             FROM deleted_terms
             ORDER BY deleted_at_ms DESC, input_code, text, reading",
        )?;
        let tombstones = statement
            .query_map([], |row| {
                let reading: String = row.get(2)?;
                Ok(DeletedTermTombstone {
                    input_code: row.get(0)?,
                    text: row.get(1)?,
                    reading: optional_from_storage(&reading),
                    deleted_at_ms: row.get(3)?,
                    reason: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tombstones)
    }

    pub fn export_dictionary_records(&self) -> UserDbResult<Vec<DictionaryTermRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms, restored_at_ms, import_batch_id
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
        validate_import_source_name(&source_name)?;
        let now = now_ms()?;

        let transaction = self.connection.transaction()?;
        let (mut summary, actions) = prepare_dictionary_import(&transaction, records)?;

        transaction.execute(
            "INSERT INTO import_batches (
                source_name, term_count, total_count, inserted_count, updated_count,
                skipped_deleted_count, skipped_duplicate_count, created_at_ms, notes
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '')",
            params![
                &source_name,
                summary.imported_terms as i64,
                summary.total_records as i64,
                summary.inserted_terms as i64,
                summary.updated_terms as i64,
                summary.skipped_deleted_terms as i64,
                summary.skipped_duplicate_terms as i64,
                now
            ],
        )?;
        let import_batch_id = transaction.last_insert_rowid();
        summary.import_batch_id = Some(import_batch_id);

        for action in actions {
            transaction.execute(
                "INSERT INTO user_terms (
                    text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms, restored_at_ms, import_batch_id
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, NULL, NULL, ?8)
                 ON CONFLICT(input_code, text, reading) DO UPDATE SET
                    source = CASE
                        WHEN user_terms.status = 'suppressed' THEN user_terms.source
                        ELSE excluded.source
                    END,
                    weight = excluded.weight,
                    status = CASE
                        WHEN user_terms.status = 'suppressed' THEN 'suppressed'
                        ELSE excluded.status
                    END,
                    updated_at_ms = excluded.updated_at_ms,
                    import_batch_id = excluded.import_batch_id",
                params![
                    action.text,
                    action.reading,
                    action.input_code,
                    action.source.as_str(),
                    action.weight,
                    action.status.as_str(),
                    now,
                    import_batch_id
                ],
            )?;
        }
        transaction.commit()?;

        Ok(summary)
    }

    pub fn preview_dictionary_import(
        &self,
        records: &[DictionaryTermRecord],
        source_name: impl AsRef<str>,
    ) -> UserDbResult<DictionaryImportSummary> {
        let source_name = normalized_required("source_name", source_name.as_ref())?;
        validate_import_source_name(&source_name)?;
        let (summary, _) = prepare_dictionary_import(&self.connection, records)?;
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
                "SELECT id, text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms, restored_at_ms, import_batch_id
                 FROM user_terms
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
                params![input_code, text, reading],
                user_term_from_row,
            )
            .optional()
            .map_err(Into::into)
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

    pub fn list_import_batches(&self) -> UserDbResult<Vec<DictionaryImportBatch>> {
        let mut statement = self.connection.prepare(
            "SELECT id, source_name, total_count, term_count, inserted_count, updated_count,
                    skipped_deleted_count, skipped_duplicate_count, created_at_ms, notes
             FROM import_batches
             ORDER BY id",
        )?;
        let batches = statement
            .query_map([], import_batch_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(batches)
    }

    pub fn sync_preflight_summary(&self) -> UserDbResult<SyncPreflightSummary> {
        Ok(SyncPreflightSummary {
            schema_version: self.schema_version()?,
            syncable_user_terms: self.count_user_terms_for_sync()?,
            syncable_ranker_weights: count_rows_usize(&self.connection, "ranker_weights")?,
            syncable_deleted_terms: count_rows_usize(&self.connection, "deleted_terms")?,
            local_selection_events: count_rows_usize(&self.connection, "selection_events")?,
            local_negative_feedback: count_rows_usize(&self.connection, "negative_feedback")?,
            local_import_batches: count_rows_usize(&self.connection, "import_batches")?,
        })
    }

    pub fn p2_plaintext_payloads(
        &self,
    ) -> UserDbResult<impl Iterator<Item = UserDbSyncPlaintextPayload>> {
        Ok(sync_payload::collect_p2_plaintext_payloads(self)?.into_iter())
    }

    pub fn apply_decoded_sync_payload_batch(
        &mut self,
        batch: &crate::sync_decode::UserDbDecodedSyncPayloadBatch,
    ) -> UserDbResult<UserDbSyncApplySummary> {
        sync_apply::apply_decoded_sync_payload_batch(self, batch)
    }

    pub fn learning_status_summary(&self) -> UserDbResult<LearningStatusSummary> {
        learning_status_summary_on(&self.connection)
    }

    pub fn inspect_learning_case(
        &mut self,
        input_code: &str,
        text: &str,
        reading: Option<&str>,
        context_kind: &str,
    ) -> UserDbResult<LearningCaseInspection> {
        let input_code = normalized_required("input_code", input_code)?;
        let text = normalized_required("text", text)?;
        let reading = normalized_optional(reading);
        let context_kind = normalized_required("context_kind", context_kind)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;

        let aggregate = learning_status_summary_on(&transaction)?;
        let term = transaction
            .query_row(
                "SELECT source, status, weight, updated_at_ms, last_used_at_ms, restored_at_ms
                 FROM user_terms
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
                params![input_code, text, reading],
                |row| {
                    let source: String = row.get(0)?;
                    let status: String = row.get(1)?;
                    Ok(LearningCaseTermInspection {
                        source: source
                            .parse::<TermSource>()
                            .map_err(to_sqlite_conversion_failure)?,
                        status: status
                            .parse::<TermStatus>()
                            .map_err(to_sqlite_conversion_failure)?,
                        weight: row.get(2)?,
                        version_ms: row.get(3)?,
                        last_used_at_ms: row.get(4)?,
                        restored_at_ms: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let ranker_weight = transaction
            .query_row(
                "SELECT frequency, last_used_at_ms, negative_score, updated_at_ms
                 FROM ranker_weights
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3 AND context_kind = ?4",
                params![input_code, text, reading, context_kind],
                |row| {
                    Ok(LearningCaseRankerWeightInspection {
                        frequency: row.get(0)?,
                        last_used_at_ms: row.get(1)?,
                        negative_score: row.get(2)?,
                        updated_at_ms: row.get(3)?,
                    })
                },
            )
            .optional()?;
        let deleted_tombstone = transaction
            .query_row(
                "SELECT deleted_at_ms
                 FROM deleted_terms
                 WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
                params![input_code, text, reading],
                |row| {
                    Ok(LearningCaseDeletedTombstoneInspection {
                        deleted_at_ms: row.get(0)?,
                    })
                },
            )
            .optional()?;

        transaction.commit()?;
        Ok(LearningCaseInspection {
            inspection_version: LEARNING_CASE_INSPECTION_VERSION,
            identity: LearningCaseIdentity {
                input_code,
                text,
                reading: optional_from_storage(&reading),
                context_kind,
            },
            aggregate,
            term,
            ranker_weight,
            deleted_tombstone,
        })
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
                "SELECT input_code, text, reading, frequency, last_used_at_ms, negative_score, context_kind
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
                        last_used_at_ms: row.get(4)?,
                        negative_score: row.get(5)?,
                        context_kind: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn count_user_terms_for_sync(&self) -> UserDbResult<usize> {
        self.connection
            .query_row(
                "SELECT COUNT(*)
                 FROM user_terms
                 WHERE status IN ('active', 'suppressed')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .and_then(|count| non_negative_usize(count, "user_terms"))
            .map_err(Into::into)
    }
}

fn user_term_from_row(row: &Row<'_>) -> rusqlite::Result<UserTerm> {
    let reading: String = row.get(2)?;
    let source: String = row.get(4)?;
    let status: String = row.get(6)?;

    let source = source
        .parse::<TermSource>()
        .map_err(to_sqlite_conversion_failure)?;
    let status = status
        .parse::<TermStatus>()
        .map_err(to_sqlite_conversion_failure)?;

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
        restored_at_ms: row.get(10)?,
        import_batch_id: row.get(11)?,
    })
}

fn import_batch_from_row(row: &Row<'_>) -> rusqlite::Result<DictionaryImportBatch> {
    Ok(DictionaryImportBatch {
        id: row.get(0)?,
        source_name: row.get(1)?,
        total_records: import_batch_count_from_row(row, 2, "total_count")?,
        imported_terms: import_batch_count_from_row(row, 3, "term_count")?,
        inserted_terms: import_batch_count_from_row(row, 4, "inserted_count")?,
        updated_terms: import_batch_count_from_row(row, 5, "updated_count")?,
        skipped_deleted_terms: import_batch_count_from_row(row, 6, "skipped_deleted_count")?,
        skipped_duplicate_terms: import_batch_count_from_row(row, 7, "skipped_duplicate_count")?,
        created_at_ms: row.get(8)?,
        notes: row.get(9)?,
    })
}

fn import_batch_count_from_row(
    row: &Row<'_>,
    index: usize,
    field: &'static str,
) -> rusqlite::Result<usize> {
    let value: i64 = row.get(index)?;
    usize::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(UserDbError::invalid_input(
                field,
                format!("expected non-negative integer, got {value}"),
            )),
        )
    })
}

fn to_sqlite_conversion_failure(error: UserDbError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

pub fn encode_dictionary_terms_tsv(records: &[DictionaryTermRecord]) -> String {
    let mut output = String::new();
    output.push_str(DictionaryTermsFormat::V1.version_line());
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
    Ok(decode_dictionary_terms_tsv_document(input)?.records)
}

pub fn decode_dictionary_terms_tsv_document(input: &str) -> UserDbResult<DictionaryTermsDocument> {
    let mut lines = input.lines();
    let version = lines
        .next()
        .ok_or_else(|| UserDbError::invalid_input("import_file", "file is empty"))?;
    let format = DictionaryTermsFormat::from_version_line(version)?;

    let header = lines.next().ok_or_else(|| {
        UserDbError::invalid_input("import_file", "missing dictionary field header")
    })?;

    match format {
        DictionaryTermsFormat::V1 => validate_dictionary_v1_header(header)?,
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

        let source = fields[3].parse::<TermSource>()?;
        let weight = fields[4].parse::<f64>().map_err(|_| {
            UserDbError::invalid_input(
                "weight",
                format!("line {line_number} has invalid weight {}", fields[4]),
            )
        })?;
        let status = fields[5].parse::<TermStatus>()?;

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

    Ok(DictionaryTermsDocument { format, records })
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedDictionaryImport {
    input_code: String,
    text: String,
    reading: String,
    source: TermSource,
    weight: f64,
    status: TermStatus,
}

fn prepare_dictionary_import(
    connection: &Connection,
    records: &[DictionaryTermRecord],
) -> UserDbResult<(DictionaryImportSummary, Vec<PreparedDictionaryImport>)> {
    let mut summary = DictionaryImportSummary::empty(records.len());
    let mut actions = Vec::new();
    let mut seen = BTreeSet::new();

    for record in records {
        validate_dictionary_record(record)?;
        let input_code = normalized_required("input_code", &record.input_code)?;
        let text = normalized_required("text", &record.text)?;
        let reading = normalized_optional(record.reading.as_deref());
        let identity = (input_code.clone(), text.clone(), reading.clone());

        if !seen.insert(identity) {
            summary.skipped_duplicate_terms += 1;
            continue;
        }

        let current_status = fetch_term_status_on(connection, &input_code, &text, &reading)?;
        if has_deleted_tombstone_on(connection, &input_code, &text, &reading)?
            || current_status == Some(TermStatus::Deleted)
        {
            summary.skipped_deleted_terms += 1;
            continue;
        }

        if current_status.is_some() {
            summary.updated_terms += 1;
        } else {
            summary.inserted_terms += 1;
        }
        summary.imported_terms += 1;
        actions.push(PreparedDictionaryImport {
            input_code,
            text,
            reading,
            source: record.source,
            weight: record.weight,
            status: record.status,
        });
    }

    Ok((summary, actions))
}

fn count_rows(connection: &Connection, table: &str) -> UserDbResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(Into::into)
}

fn count_rows_usize(connection: &Connection, table: &'static str) -> UserDbResult<usize> {
    let count = count_rows(connection, table)?;
    non_negative_usize(count, table).map_err(Into::into)
}

fn count_user_terms_with_status_on(
    connection: &Connection,
    status: TermStatus,
) -> UserDbResult<usize> {
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM user_terms
             WHERE status = ?1",
            params![status.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .and_then(|count| non_negative_usize(count, "user_terms"))
        .map_err(Into::into)
}

fn learning_status_summary_on(connection: &Connection) -> UserDbResult<LearningStatusSummary> {
    let latest_user_term_updated_at_ms = max_i64_column(connection, "user_terms", "updated_at_ms")?;
    let latest_selection_event_at_ms =
        max_i64_column(connection, "selection_events", "created_at_ms")?;
    let latest_negative_feedback_at_ms =
        max_i64_column(connection, "negative_feedback", "created_at_ms")?;
    let latest_deleted_term_at_ms = max_i64_column(connection, "deleted_terms", "deleted_at_ms")?;
    let latest_import_batch_at_ms = max_i64_column(connection, "import_batches", "created_at_ms")?;

    Ok(LearningStatusSummary {
        schema_version: connection.query_row("PRAGMA user_version", [], |row| row.get(0))?,
        active_user_terms: count_user_terms_with_status_on(connection, TermStatus::Active)?,
        suppressed_user_terms: count_user_terms_with_status_on(connection, TermStatus::Suppressed)?,
        ranker_weights: count_rows_usize(connection, "ranker_weights")?,
        deleted_term_tombstones: count_rows_usize(connection, "deleted_terms")?,
        selection_events: count_rows_usize(connection, "selection_events")?,
        negative_feedback: count_rows_usize(connection, "negative_feedback")?,
        import_batches: count_rows_usize(connection, "import_batches")?,
        latest_user_term_updated_at_ms,
        latest_selection_event_at_ms,
        latest_negative_feedback_at_ms,
        latest_deleted_term_at_ms,
        latest_import_batch_at_ms,
        latest_activity_at_ms: latest_ms([
            latest_user_term_updated_at_ms,
            latest_selection_event_at_ms,
            latest_negative_feedback_at_ms,
            latest_deleted_term_at_ms,
            latest_import_batch_at_ms,
        ]),
    })
}

fn max_i64_column(
    connection: &Connection,
    table: &'static str,
    column: &'static str,
) -> UserDbResult<Option<i64>> {
    let sql = format!("SELECT MAX({column}) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(Into::into)
}

fn latest_ms(values: impl IntoIterator<Item = Option<i64>>) -> Option<i64> {
    values.into_iter().flatten().max()
}

fn non_negative_usize(value: i64, field: &'static str) -> rusqlite::Result<usize> {
    usize::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(UserDbError::invalid_input(
                field,
                format!("expected non-negative integer, got {value}"),
            )),
        )
    })
}

fn validate_dictionary_v1_header(header: &str) -> UserDbResult<()> {
    if header != DICTIONARY_EXPORT_HEADER {
        return Err(UserDbError::invalid_input(
            "import_file",
            format!("expected v1 header {DICTIONARY_EXPORT_HEADER}"),
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

fn validate_import_source_name(value: &str) -> UserDbResult<()> {
    if value.len() > 64 {
        return Err(UserDbError::invalid_input(
            "source_name",
            "value must be 64 bytes or fewer",
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(UserDbError::invalid_input(
            "source_name",
            "value must contain only ASCII letters, digits, dot, underscore, or dash",
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
                status
                    .parse::<TermStatus>()
                    .map_err(to_sqlite_conversion_failure)
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

#[cfg(test)]
mod tests;
