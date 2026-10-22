//! Synthetic-only, default-off schema observation. No production input-path API.
//!
//! A fixture capability can only be minted by the generator in this module.
//! The caller owns verified directory capture, immutable DB/WAL checks and scratch
//! disposal. A successful SQLite read is not a cancellation qualification.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{config::DbConfig, types::ValueRef, Connection, ErrorCode, OpenFlags};

#[path = "schema_observer_fixture.rs"]
mod fixture;
#[path = "schema_observer_wal.rs"]
mod wal;

pub use fixture::{create_synthetic_fixture, FixtureKind};

/// Fixed diagnostics deliberately exclude paths, SQL, SQLite messages and rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverError {
    Io,
    InputFormat,
    WalUnsupported,
    WalChecksum,
    LimitExceeded,
    UnsupportedSchema,
    SqliteBusy,
    SqliteFailure,
    Integrity,
    Layout,
    Configuration,
    Cancelled,
    Deadline,
    CloseUnconfirmed,
    Fixture,
    EvidenceChanged,
}

impl std::fmt::Display for ObserverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ObserverError {}

pub type ObserverControl = Arc<dyn Fn() -> Result<(), ObserverError> + Send + Sync>;
pub type ReadCharge<'a> = dyn FnMut(u64) -> Result<(), ObserverError> + 'a;

type SchemaObject = (String, String, String, Option<String>);
const MAX_MATERIAL: usize = 256 * 1024;
const MAX_OBJECTS: usize = 64;

/// Created from compiled, trusted schema code before any fixture mutation.
/// Fields cannot be manufactured from paths, JSON or schema integers.
pub struct SyntheticFixture {
    source: PathBuf,
    trusted_manifest: Vec<SchemaObject>,
    reference_version: Option<i64>,
}

/// An intermediate synthetic result. The host must close its capture handles,
/// verify unchanged source/copy identities and hashes, dispose scratch, then
/// bind the result to its own proof/control lifetime before exposing success.
#[derive(Debug)]
pub struct SyntheticObservation {
    schema_version: i64,
    sqlite_source_id: String,
    compile_options: Vec<String>,
}
impl SyntheticObservation {
    pub const fn schema_version(&self) -> i64 {
        self.schema_version
    }
    pub const fn profile(&self) -> &'static str {
        "synthetic-fresh-schema9-exact-v1"
    }
    pub fn sqlite_source_id(&self) -> &str {
        &self.sqlite_source_id
    }
    pub fn compile_options(&self) -> &[String] {
        &self.compile_options
    }
}

impl SyntheticFixture {
    pub fn source_dir(&self) -> &Path {
        &self.source
    }

    /// Actual scalar from an independent same-byte fixture copy, where generated.
    pub fn reference_schema_version(&self) -> Option<i64> {
        self.reference_version
    }

    /// Only the fixed sibling scratch path may be inspected. The host must
    /// establish its directory/file capability and copy the captured family first.
    pub fn inspect_copy(
        &self,
        control: ObserverControl,
        charge: &mut ReadCharge<'_>,
        after_transaction: &mut dyn FnMut() -> Result<(), ObserverError>,
    ) -> Result<SyntheticObservation, ObserverError> {
        control()?;
        let path = self
            .source
            .parent()
            .ok_or(ObserverError::Fixture)?
            .join("scratch")
            .join("userdb.sqlite3");
        let admission = wal::admit(&path, &control, charge)?;
        control()?;
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql_error)?;
        let interrupted = Arc::new(Mutex::new(None));
        let result = (|| {
            configure(&connection)?;
            install_progress(&connection, control.clone(), interrupted.clone())?;
            control()?;
            connection
                .execute_batch("BEGIN DEFERRED")
                .map_err(sql_error)?;
            control()?;
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .map_err(sql_error)?;
            control()?;
            if version != 9 {
                return Err(ObserverError::UnsupportedSchema);
            }
            if let Some(pages) = admission.committed_pages {
                let actual: u32 = connection
                    .query_row("PRAGMA page_count", [], |r| r.get(0))
                    .map_err(sql_error)?;
                if actual != pages {
                    return Err(ObserverError::InputFormat);
                }
            }
            let page_size: u32 = connection
                .query_row("PRAGMA page_size", [], |r| r.get(0))
                .map_err(sql_error)?;
            if page_size != admission.page_size {
                return Err(ObserverError::InputFormat);
            }
            control()?;
            let mut integrity = connection
                .prepare("PRAGMA integrity_check")
                .map_err(sql_error)?;
            let mut rows = integrity.query([]).map_err(sql_error)?;
            let row = rows
                .next()
                .map_err(sql_error)?
                .ok_or(ObserverError::Integrity)?;
            if row.get_ref(0).map_err(sql_error)? != ValueRef::Text(b"ok")
                || rows.next().map_err(sql_error)?.is_some()
            {
                return Err(ObserverError::Integrity);
            }
            drop(rows);
            drop(integrity);
            control()?;
            let mut material = 0;
            let manifest = read_manifest(&connection, &mut material, &control)?;
            if manifest != self.trusted_manifest {
                return Err(ObserverError::Layout);
            }
            control()?;
            let mut source_query = connection
                .prepare("SELECT sqlite_source_id()")
                .map_err(sql_error)?;
            let mut rows = source_query.query([]).map_err(sql_error)?;
            let row = rows
                .next()
                .map_err(sql_error)?
                .ok_or(ObserverError::Configuration)?;
            let source_id = text(row.get_ref(0).map_err(sql_error)?, &mut material)?;
            if rows.next().map_err(sql_error)?.is_some() {
                return Err(ObserverError::Configuration);
            }
            drop(rows);
            drop(source_query);
            let mut compile_options = Vec::new();
            let mut options = connection
                .prepare("PRAGMA compile_options")
                .map_err(sql_error)?;
            let mut rows = options.query([]).map_err(sql_error)?;
            while let Some(row) = rows.next().map_err(sql_error)? {
                control()?;
                compile_options.push(text(row.get_ref(0).map_err(sql_error)?, &mut material)?);
            }
            drop(rows);
            drop(options);
            if !compile_options
                .iter()
                .any(|v| matches!(v.as_str(), "TEMP_STORE=1" | "TEMP_STORE=2" | "TEMP_STORE=3"))
                || compile_options
                    .iter()
                    .any(|v| matches!(v.as_str(), "OMIT_WAL" | "OMIT_PROGRESS_CALLBACK"))
            {
                return Err(ObserverError::Configuration);
            }
            control()?;
            connection.execute_batch("COMMIT").map_err(sql_error)?;
            control()?;
            after_transaction()?;
            control()?;
            Ok(SyntheticObservation {
                schema_version: version,
                sqlite_source_id: source_id,
                compile_options,
            })
        })();
        // On revoke/error only resource disposal remains; no further observation.
        let reason = match interrupted.lock() {
            Ok(mut value) => value.take(),
            Err(_) => Some(ObserverError::Configuration),
        };
        let disable = connection
            .progress_handler(0, None::<fn() -> bool>)
            .map_err(sql_error);
        let rollback = if !connection.is_autocommit() {
            connection.execute_batch("ROLLBACK").map_err(sql_error)
        } else {
            Ok(())
        };
        match connection.close() {
            Ok(()) => {}
            Err((connection, _)) => {
                drop(connection);
                return Err(ObserverError::CloseUnconfirmed);
            }
        }
        disable?;
        rollback?;
        if let Some(reason) = reason {
            return Err(reason);
        }
        let observation = result?;
        control()?;
        Ok(observation)
    }
}

fn install_progress(
    connection: &Connection,
    control: ObserverControl,
    interrupted: Arc<Mutex<Option<ObserverError>>>,
) -> Result<(), ObserverError> {
    connection
        .progress_handler(
            1000,
            Some(move || match control() {
                Ok(()) => false,
                Err(error) => {
                    if let Ok(mut slot) = interrupted.lock() {
                        *slot = Some(error);
                    }
                    true
                }
            }),
        )
        .map_err(sql_error)
}

fn sql_error(error: rusqlite::Error) -> ObserverError {
    if matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    ) {
        ObserverError::SqliteBusy
    } else {
        ObserverError::SqliteFailure
    }
}

fn configure(connection: &Connection) -> Result<(), ObserverError> {
    connection
        .busy_timeout(Duration::from_millis(100))
        .map_err(sql_error)?;
    if !connection.is_readonly("main").map_err(sql_error)? {
        return Err(ObserverError::Configuration);
    }
    for (config, value) in [
        (DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true),
        (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
        (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
    ] {
        if connection.set_db_config(config, value).map_err(sql_error)? != value
            || connection.db_config(config).map_err(sql_error)? != value
        {
            return Err(ObserverError::Configuration);
        }
    }
    connection
        .execute_batch(
            "PRAGMA query_only=ON; PRAGMA cache_size=-8192; PRAGMA mmap_size=0;
         PRAGMA temp_store=MEMORY; PRAGMA threads=0; PRAGMA trusted_schema=OFF;",
        )
        .map_err(sql_error)?;
    for (sql, expected) in [
        ("PRAGMA query_only", 1i64),
        ("PRAGMA cache_size", -8192),
        ("PRAGMA mmap_size", 0),
        ("PRAGMA temp_store", 2),
        ("PRAGMA threads", 0),
        ("PRAGMA trusted_schema", 0),
        ("PRAGMA busy_timeout", 100),
    ] {
        let actual: i64 = connection
            .query_row(sql, [], |row| row.get(0))
            .map_err(sql_error)?;
        if actual != expected {
            return Err(ObserverError::Configuration);
        }
    }
    Ok(())
}

fn account(total: &mut usize, amount: usize) -> Result<(), ObserverError> {
    *total = total
        .checked_add(amount)
        .ok_or(ObserverError::LimitExceeded)?;
    if *total > MAX_MATERIAL {
        return Err(ObserverError::LimitExceeded);
    }
    Ok(())
}
fn text(value: ValueRef<'_>, total: &mut usize) -> Result<String, ObserverError> {
    match value {
        ValueRef::Text(bytes) => {
            account(total, bytes.len())?;
            Ok(std::str::from_utf8(bytes)
                .map_err(|_| ObserverError::InputFormat)?
                .to_owned())
        }
        _ => Err(ObserverError::InputFormat),
    }
}
fn read_manifest(
    connection: &Connection,
    material: &mut usize,
    control: &ObserverControl,
) -> Result<Vec<SchemaObject>, ObserverError> {
    let count: i64 = connection
        .query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))
        .map_err(sql_error)?;
    if count < 0 || count > MAX_OBJECTS as i64 {
        return Err(ObserverError::LimitExceeded);
    }
    let mut statement = connection
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name,tbl_name")
        .map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let mut manifest = Vec::new();
    while let Some(row) = rows.next().map_err(sql_error)? {
        control()?;
        if manifest.len() == MAX_OBJECTS {
            return Err(ObserverError::LimitExceeded);
        }
        let sql = match row.get_ref(3).map_err(sql_error)? {
            ValueRef::Null => None,
            value => Some(text(value, material)?),
        };
        manifest.push((
            text(row.get_ref(0).map_err(sql_error)?, material)?,
            text(row.get_ref(1).map_err(sql_error)?, material)?,
            text(row.get_ref(2).map_err(sql_error)?, material)?,
            sql,
        ));
    }
    Ok(manifest)
}

#[cfg(test)]
#[path = "schema_observer_tests.rs"]
mod tests;
