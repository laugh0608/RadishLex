use rusqlite::{params, Connection, OptionalExtension};

use crate::error::UserDbResult;

pub(super) fn has_deleted_tombstone_on(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<bool> {
    Ok(latest_deleted_tombstone_time(connection, input_code, text, reading)?.is_some())
}

pub(super) fn latest_deleted_tombstone_time(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<Option<i64>> {
    connection
        .query_row(
            "SELECT deleted_at_ms
             FROM deleted_terms
             WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
            params![input_code, text, reading],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn clear_deleted_tombstones(
    connection: &Connection,
    input_code: &str,
    text: &str,
    reading: &str,
) -> UserDbResult<()> {
    connection.execute(
        "DELETE FROM deleted_terms
         WHERE input_code = ?1 AND text = ?2 AND reading = ?3",
        params![input_code, text, reading],
    )?;
    Ok(())
}

pub(super) fn legacy_stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
