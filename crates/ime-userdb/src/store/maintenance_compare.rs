//! Exact, streaming comparison for the supported on-disk userdb schemas.

use rusqlite::{types::ValueRef, Connection};

use super::connection::{read_schema_version, SCHEMA_VERSION};
use super::maintenance::UserDbMaintenanceError as Error;

const TABLES: &[&str] = &[
    "user_terms",
    "selection_events",
    "negative_feedback",
    "deleted_terms",
    "ranker_weights",
    "import_batches",
    "sqlite_sequence",
    "sync_domain_state",
    "sync_remote_objects",
    "sync_local_objects",
    "sync_prepared_outbox",
    "sync_cycle_journal",
    "sync_trusted_domains",
    "sync_trusted_devices",
    "sync_trusted_lifecycle_events",
    "sync_wrapped_epoch_materials",
];
const INDEXES: &[&str] = &[
    "idx_user_terms_identity",
    "idx_deleted_terms_identity",
    "idx_ranker_weights_identity",
    "idx_sync_remote_change_sequence",
];

#[derive(Debug, PartialEq, Eq)]
struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: Option<String>,
}

pub(super) fn validate_schema(connection: &Connection) -> Result<i64, Error> {
    let version = read_schema_version(connection).map_err(Error::from_sqlite)?;
    if !(0..=SCHEMA_VERSION).contains(&version) {
        return Err(Error::UnsupportedSchema);
    }
    let objects = schema_objects(connection)?;
    for object in &objects {
        match object.kind.as_str() {
            "table" if TABLES.contains(&object.name.as_str()) => {
                // All supported tables have an unshadowed rowid. Comparing it
                // preserves duplicates and sequence/legacy identity semantics.
                let (kind, without_rowid): (String, i64) = connection
                    .query_row(
                        "SELECT type, wr FROM pragma_table_list WHERE schema='main' AND name=?1",
                        [&object.name],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(Error::from_sqlite)?;
                if kind != "table" || without_rowid != 0 {
                    return Err(Error::UnsupportedSchema);
                }
                let unsupported_columns: i64 = connection.query_row(
                    "SELECT count(*) FROM pragma_table_xinfo(?1) WHERE hidden != 0 OR lower(name) IN ('rowid','_rowid_','oid')",
                    [&object.name], |row| row.get(0),
                ).map_err(Error::from_sqlite)?;
                if unsupported_columns != 0 {
                    return Err(Error::UnsupportedSchema);
                }
            }
            "index" if TABLES.contains(&object.table.as_str()) => {
                let automatic = object.sql.is_none()
                    && object
                        .name
                        .starts_with(&format!("sqlite_autoindex_{}_", object.table));
                if !automatic && !INDEXES.contains(&object.name.as_str()) {
                    return Err(Error::UnsupportedSchema);
                }
            }
            _ => return Err(Error::UnsupportedSchema),
        }
    }
    // Current databases must also satisfy the existing product schema checks.
    // Earlier versions are compared without migration; candidate migration
    // remains the authority for accepting their historical layout.
    if version == SCHEMA_VERSION {
        super::connection::validate_current_schema_on(connection)
            .map_err(|_| Error::UnsupportedSchema)?;
    }
    Ok(version)
}

pub(super) fn compare(left: &Connection, right: &Connection) -> Result<i64, Error> {
    let version = validate_schema(left)?;
    if validate_schema(right)? != version || schema_objects(left)? != schema_objects(right)? {
        return Err(Error::ContentChanged);
    }
    for pragma in [
        "application_id",
        "user_version",
        "encoding",
        "auto_vacuum",
        "page_size",
    ] {
        compare_query(left, right, &format!("PRAGMA main.{pragma}"))?;
    }
    for object in schema_objects(left)? {
        if object.kind == "table" {
            // Names were matched to the fixed table allowlist, never supplied
            // as SQL by the Installer or taken from arbitrary schema objects.
            compare_query(
                left,
                right,
                &format!(
                    "SELECT _rowid_, * FROM main.\"{}\" ORDER BY _rowid_",
                    object.name
                ),
            )?;
        }
    }
    Ok(version)
}

fn schema_objects(connection: &Connection) -> Result<Vec<SchemaObject>, Error> {
    let mut statement = connection.prepare(
        "SELECT type,name,tbl_name,sql FROM main.sqlite_schema ORDER BY type COLLATE BINARY,name COLLATE BINARY LIMIT 65",
    ).map_err(Error::from_sqlite)?;
    let objects = statement
        .query_map([], |row| {
            Ok(SchemaObject {
                kind: row.get(0)?,
                name: row.get(1)?,
                table: row.get(2)?,
                sql: row.get(3)?,
            })
        })
        .map_err(Error::from_sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from_sqlite)?;
    if objects.len() > 64 {
        return Err(Error::UnsupportedSchema);
    }
    Ok(objects)
}

fn compare_query(left: &Connection, right: &Connection, sql: &str) -> Result<(), Error> {
    let mut a = left.prepare(sql).map_err(Error::from_sqlite)?;
    let mut b = right.prepare(sql).map_err(Error::from_sqlite)?;
    let columns = a.column_count();
    if columns != b.column_count() {
        return Err(Error::ContentChanged);
    }
    let mut a = a.query([]).map_err(Error::from_sqlite)?;
    let mut b = b.query([]).map_err(Error::from_sqlite)?;
    loop {
        match (
            a.next().map_err(Error::from_sqlite)?,
            b.next().map_err(Error::from_sqlite)?,
        ) {
            (None, None) => return Ok(()),
            (Some(a), Some(b)) => {
                for column in 0..columns {
                    if !same_value(
                        a.get_ref(column).map_err(Error::from_sqlite)?,
                        b.get_ref(column).map_err(Error::from_sqlite)?,
                    ) {
                        return Err(Error::ContentChanged);
                    }
                }
            }
            _ => return Err(Error::ContentChanged),
        }
    }
}

fn same_value(a: ValueRef<'_>, b: ValueRef<'_>) -> bool {
    match (a, b) {
        (ValueRef::Null, ValueRef::Null) => true,
        (ValueRef::Integer(a), ValueRef::Integer(b)) => a == b,
        (ValueRef::Real(a), ValueRef::Real(b)) => a.to_bits() == b.to_bits(),
        (ValueRef::Text(a), ValueRef::Text(b)) | (ValueRef::Blob(a), ValueRef::Blob(b)) => a == b,
        _ => false,
    }
}
