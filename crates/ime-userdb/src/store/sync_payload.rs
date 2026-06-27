use rusqlite::{params, OptionalExtension, Row};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{
    TermSource, TermStatus, UserDbSyncPayloadObjectType, UserDbSyncPlaintextPayload,
    USERDB_SYNC_PAYLOAD_SCHEMA_VERSION,
};

use super::{stable_hash_hex, to_sqlite_conversion_failure, UserDb};

const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";

#[derive(Debug, Clone, PartialEq)]
struct SyncUserTermPayloadRecord {
    input_code: String,
    text: String,
    reading: String,
    source: TermSource,
    weight: f64,
    status: TermStatus,
    created_at_ms: i64,
    updated_at_ms: i64,
    last_used_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyncDeletedTermPayloadRecord {
    input_code: String,
    text: String,
    reading: String,
    deleted_at_ms: i64,
    reason: String,
}

pub(super) fn collect_p2_plaintext_payloads(
    db: &UserDb,
) -> UserDbResult<Vec<UserDbSyncPlaintextPayload>> {
    let mut payloads = Vec::new();

    let user_terms = sync_user_term_payload_records(db)?;
    if !user_terms.is_empty() {
        payloads.push(UserDbSyncPlaintextPayload::new(
            UserDbSyncPayloadObjectType::DictionaryUserTerms,
            user_terms.len(),
            encode_user_terms_sync_payload(&user_terms).into_bytes(),
        )?);
    }

    let deleted_terms = sync_deleted_term_payload_records(db)?;
    if !deleted_terms.is_empty() {
        payloads.push(UserDbSyncPlaintextPayload::new(
            UserDbSyncPayloadObjectType::DictionaryDeletedTerms,
            deleted_terms.len(),
            encode_deleted_terms_sync_payload(&deleted_terms).into_bytes(),
        )?);
    }

    Ok(payloads)
}

fn sync_user_term_payload_records(db: &UserDb) -> UserDbResult<Vec<SyncUserTermPayloadRecord>> {
    let mut statement = db.connection.prepare(
        "SELECT input_code, text, reading, source, weight, status,
                created_at_ms, updated_at_ms, last_used_at_ms
         FROM user_terms
         WHERE status IN ('active', 'suppressed')
         ORDER BY input_code, text, reading",
    )?;
    let records = statement
        .query_map([], sync_user_term_payload_record_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(records)
}

fn sync_deleted_term_payload_records(
    db: &UserDb,
) -> UserDbResult<Vec<SyncDeletedTermPayloadRecord>> {
    let mut statement = db.connection.prepare(
        "SELECT input_code, text, reading, updated_at_ms
         FROM user_terms
         WHERE status = 'deleted'
         ORDER BY input_code, text, reading",
    )?;
    let deleted_terms = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut records = Vec::with_capacity(deleted_terms.len());
    for (input_code, text, reading, updated_at_ms) in deleted_terms {
        let tombstone = latest_deleted_tombstone(db, &input_code, &text, &reading)?;
        let (deleted_at_ms, reason) =
            tombstone.unwrap_or_else(|| (updated_at_ms, "manual_delete".to_owned()));
        records.push(SyncDeletedTermPayloadRecord {
            input_code,
            text,
            reading,
            deleted_at_ms,
            reason,
        });
    }

    Ok(records)
}

fn latest_deleted_tombstone(
    db: &UserDb,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<(i64, String)>> {
    db.connection
        .query_row(
            "SELECT deleted_at_ms, reason
             FROM deleted_terms
             WHERE input_code_hash = ?1 AND text_hash = ?2 AND reading_hash = ?3
             ORDER BY deleted_at_ms DESC, id DESC
             LIMIT 1",
            params![
                stable_hash_hex(input_code),
                stable_hash_hex(text),
                stable_hash_hex(reading)
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
}

fn sync_user_term_payload_record_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<SyncUserTermPayloadRecord> {
    let source: String = row.get(3)?;
    let status: String = row.get(5)?;

    let source = TermSource::from_str(&source).map_err(to_sqlite_conversion_failure)?;
    let status = TermStatus::from_str(&status).map_err(to_sqlite_conversion_failure)?;

    let weight: f64 = row.get(4)?;
    if !weight.is_finite() || weight < 0.0 {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Real,
            Box::new(UserDbError::invalid_input(
                "weight",
                "value must be finite and non-negative",
            )),
        ));
    }

    Ok(SyncUserTermPayloadRecord {
        input_code: row.get(0)?,
        text: row.get(1)?,
        reading: row.get(2)?,
        source,
        weight,
        status,
        created_at_ms: row.get(6)?,
        updated_at_ms: row.get(7)?,
        last_used_at_ms: row.get(8)?,
    })
}

fn encode_user_terms_sync_payload(records: &[SyncUserTermPayloadRecord]) -> String {
    let mut output = String::new();
    output.push_str("{\"payload_schema_version\":");
    output.push_str(&USERDB_SYNC_PAYLOAD_SCHEMA_VERSION.to_string());
    output.push_str(",\"object_type\":\"");
    output.push_str(UserDbSyncPayloadObjectType::DictionaryUserTerms.as_str());
    output.push_str("\",\"terms\":[");

    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('{');
        push_json_string_field(&mut output, "input_code", &record.input_code);
        output.push(',');
        push_json_string_field(&mut output, "text", &record.text);
        output.push(',');
        push_json_string_field(&mut output, "reading", &record.reading);
        output.push(',');
        push_json_string_field(&mut output, "source", record.source.as_str());
        output.push(',');
        push_json_number_field(&mut output, "weight", record.weight);
        output.push(',');
        push_json_string_field(&mut output, "status", record.status.as_str());
        output.push(',');
        push_json_i64_field(&mut output, "created_at_ms", record.created_at_ms);
        output.push(',');
        push_json_i64_field(&mut output, "updated_at_ms", record.updated_at_ms);
        output.push(',');
        push_json_optional_i64_field(&mut output, "last_used_at_ms", record.last_used_at_ms);
        output.push('}');
    }

    output.push_str("]}");
    output
}

fn encode_deleted_terms_sync_payload(records: &[SyncDeletedTermPayloadRecord]) -> String {
    let mut output = String::new();
    output.push_str("{\"payload_schema_version\":");
    output.push_str(&USERDB_SYNC_PAYLOAD_SCHEMA_VERSION.to_string());
    output.push_str(",\"object_type\":\"");
    output.push_str(UserDbSyncPayloadObjectType::DictionaryDeletedTerms.as_str());
    output.push_str("\",\"tombstones\":[");

    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('{');
        push_json_string_field(&mut output, "input_code", &record.input_code);
        output.push(',');
        push_json_string_field(&mut output, "text", &record.text);
        output.push(',');
        push_json_string_field(&mut output, "reading", &record.reading);
        output.push(',');
        push_json_i64_field(&mut output, "deleted_at_ms", record.deleted_at_ms);
        output.push(',');
        push_json_string_field(&mut output, "reason", &record.reason);
        output.push('}');
    }

    output.push_str("]}");
    output
}

fn push_json_string_field(output: &mut String, name: &str, value: &str) {
    push_json_string(output, name);
    output.push(':');
    push_json_string(output, value);
}

fn push_json_number_field(output: &mut String, name: &str, value: f64) {
    push_json_string(output, name);
    output.push(':');
    output.push_str(&value.to_string());
}

fn push_json_i64_field(output: &mut String, name: &str, value: i64) {
    push_json_string(output, name);
    output.push(':');
    output.push_str(&value.to_string());
}

fn push_json_optional_i64_field(output: &mut String, name: &str, value: Option<i64>) {
    push_json_string(output, name);
    output.push(':');
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("null"),
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch <= '\u{1f}' => {
                output.push_str("\\u00");
                let value = ch as u8;
                output.push(HEX_LOWER[(value >> 4) as usize] as char);
                output.push(HEX_LOWER[(value & 0x0f) as usize] as char);
            }
            _ => output.push(ch),
        }
    }
    output.push('"');
}
