//! Explicit SQLite maintenance; authorization and durable intent belong to the coordinator.

use std::fmt;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OpenFlags};

use super::connection::{verify_integrity, BUSY_TIMEOUT};
use super::{maintenance_compare, UserDb};

/// No paths, SQL, row contents or driver messages can escape this API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDbMaintenanceError {
    UnsafeFile,
    IdentityChanged,
    UnsupportedSchema,
    UnsupportedJournalMode,
    ContentChanged,
    DatabaseBusy,
    CheckpointIncomplete,
    CloseFailed,
    SidecarRemaining,
    DatabaseInvalid,
    Io,
}

impl fmt::Display for UserDbMaintenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsafeFile => "unsafe_database_file",
            Self::IdentityChanged => "database_identity_changed",
            Self::UnsupportedSchema => "unsupported_database_schema",
            Self::UnsupportedJournalMode => "unsupported_journal_mode",
            Self::ContentChanged => "database_content_changed",
            Self::DatabaseBusy => "database_busy",
            Self::CheckpointIncomplete => "checkpoint_incomplete",
            Self::CloseFailed => "database_close_failed",
            Self::SidecarRemaining => "database_sidecar_remaining",
            Self::DatabaseInvalid => "database_invalid",
            Self::Io => "database_maintenance_io",
        })
    }
}
impl std::error::Error for UserDbMaintenanceError {}
type Error = UserDbMaintenanceError;

impl Error {
    pub(super) fn from_sqlite(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::DatabaseBusy,
            _ => Self::DatabaseInvalid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserDbMaintenanceSummary {
    pub schema_version: i64,
    pub checkpointed_wal: bool,
}

impl UserDb {
    /// Read-only verification of an already prepared source. WAL mode is not
    /// accepted even when its sidecars happen to be absent before opening.
    pub fn verify_prepared_source(
        source: impl AsRef<Path>,
        snapshot: impl AsRef<Path>,
    ) -> Result<i64, Error> {
        let source = source.as_ref();
        require_no_sidecars(source)?;
        validate_pair(source, snapshot.as_ref())?;
        let connection = open(source, false)?;
        let mode = journal_mode(&connection);
        close(connection)?;
        if mode? != "delete" {
            return Err(Error::UnsupportedJournalMode);
        }
        let schema = Self::verify_maintenance_snapshot(source, snapshot)?;
        require_no_sidecars(source)?;
        Ok(schema)
    }

    /// Compares all supported persistent data without migration or WAL configuration.
    /// Callers must hold their guards and prove quiescence. Reading a WAL source
    /// may create SQLite sidecars; the protected snapshot must be standalone.
    pub fn verify_maintenance_snapshot(
        source: impl AsRef<Path>,
        snapshot: impl AsRef<Path>,
    ) -> Result<i64, Error> {
        let (source, snapshot) = (source.as_ref(), snapshot.as_ref());
        let (source_before, snapshot_before) = validate_pair(source, snapshot)?;
        let a = open(source, false)?;
        let b = match open(snapshot, false) {
            Ok(connection) => connection,
            Err(error) => {
                close(a)?;
                return Err(error);
            }
        };
        let result = (|| {
            a.execute_batch("BEGIN").map_err(Error::from_sqlite)?;
            b.execute_batch("BEGIN").map_err(Error::from_sqlite)?;
            validate_integrity(&a)?;
            validate_integrity(&b)?;
            if journal_mode(&b)? != "delete" {
                return Err(Error::UnsupportedJournalMode);
            }
            maintenance_compare::compare(&a, &b)
        })();
        let close_a = close(a);
        let close_b = close(b);
        close_a?;
        close_b?;
        require_same_file(source, &source_before, false)?;
        require_same_file(snapshot, &snapshot_before, false)?;
        require_no_sidecars(snapshot)?;
        result
    }

    /// Prepares an existing source without schema migration or logical data changes.
    /// SQLite itself may rewrite pages and remove WAL/SHM. BEFORE calling this,
    /// the coordinator must durably bind the snapshot and maintenance intent,
    /// own both guards, validate file-family identity and prove quiescence.
    /// An error can occur after physical mutation: never treat it as rollback.
    pub fn prepare_source_for_upgrade(
        source: impl AsRef<Path>,
        snapshot: impl AsRef<Path>,
    ) -> Result<UserDbMaintenanceSummary, Error> {
        prepare(source.as_ref(), snapshot.as_ref(), &mut |_| Ok(()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaintenancePoint {
    BeforeCheckpoint,
    AfterCheckpoint,
    AfterJournalMode,
    BeforeClose,
    AfterClose,
}

fn prepare(
    source: &Path,
    snapshot: &Path,
    checkpoint: &mut impl FnMut(MaintenancePoint) -> Result<(), Error>,
) -> Result<UserDbMaintenanceSummary, Error> {
    let (source_before, snapshot_before) = validate_pair(source, snapshot)?;
    UserDb::verify_maintenance_snapshot(source, snapshot)?;
    let connection = open(source, true)?;
    let result = (|| {
        let schema_version = maintenance_compare::validate_schema(&connection)?;
        let mode = journal_mode(&connection)?;
        if mode != "wal" && mode != "delete" {
            return Err(Error::UnsupportedJournalMode);
        }
        let checkpointed_wal = mode == "wal";
        checkpoint(MaintenancePoint::BeforeCheckpoint)?;
        if mode == "wal" {
            let (busy, log, copied): (i64, i64, i64) = connection
                .query_row("PRAGMA main.wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(Error::from_sqlite)?;
            if busy != 0 {
                return Err(Error::DatabaseBusy);
            }
            if log != 0 || copied != 0 {
                return Err(Error::CheckpointIncomplete);
            }
        }
        checkpoint(MaintenancePoint::AfterCheckpoint)?;
        let mode: String = connection
            .query_row("PRAGMA main.journal_mode=DELETE", [], |row| row.get(0))
            .map_err(Error::from_sqlite)?;
        if mode != "delete" {
            return Err(Error::UnsupportedJournalMode);
        }
        checkpoint(MaintenancePoint::AfterJournalMode)?;
        validate_integrity(&connection)?;
        checkpoint(MaintenancePoint::BeforeClose)?;
        Ok(UserDbMaintenanceSummary {
            schema_version,
            checkpointed_wal,
        })
    })();
    // Explicit close on BOTH success and failure. An outstanding handle is
    // never reported as a successful maintenance result.
    close(connection)?;
    let summary = result?;
    checkpoint(MaintenancePoint::AfterClose)?;
    require_same_file(source, &source_before, true)?;
    require_same_file(snapshot, &snapshot_before, false)?;
    require_no_sidecars(source)?;
    fs::File::open(source)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Io)?;
    let schema_version = UserDb::verify_maintenance_snapshot(source, snapshot)?;
    require_no_sidecars(source)?;
    // The coordinator separately syncs the authoritative data-root directory
    // and binds the final identity before it records source_prepared.
    if schema_version != summary.schema_version {
        return Err(Error::ContentChanged);
    }
    Ok(summary)
}

fn open(path: &Path, writable: bool) -> Result<Connection, Error> {
    let flags = if writable {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        .map_err(Error::from_sqlite)?;
    if let Err(error) = connection.busy_timeout(BUSY_TIMEOUT) {
        close(connection)?;
        return Err(Error::from_sqlite(error));
    }
    Ok(connection)
}

fn close(connection: Connection) -> Result<(), Error> {
    connection
        .close()
        .map_err(|(_connection, _error)| Error::CloseFailed)
}

fn journal_mode(connection: &Connection) -> Result<String, Error> {
    connection
        .query_row("PRAGMA main.journal_mode", [], |row| row.get(0))
        .map_err(Error::from_sqlite)
}

fn validate_integrity(connection: &Connection) -> Result<(), Error> {
    verify_integrity(connection).map_err(|_| Error::DatabaseInvalid)
}

fn validate_pair(source: &Path, snapshot: &Path) -> Result<(Metadata, Metadata), Error> {
    let a = file_metadata(source)?;
    let b = file_metadata(snapshot)?;
    if fs::canonicalize(source).map_err(|_| Error::Io)?
        == fs::canonicalize(snapshot).map_err(|_| Error::Io)?
    {
        return Err(Error::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() == b.dev() && a.ino() == b.ino() {
            return Err(Error::UnsafeFile);
        }
    }
    require_no_sidecars(snapshot)?;
    for suffix in ["-wal", "-shm"] {
        let path = sidecar(source, suffix);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                file_metadata(&path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Io),
        }
    }
    // A journal present before entry has no durable identity at this layer.
    // This normal preparation API fails closed. The separate
    // qualify_maintenance_journal / recover_maintenance_journal APIs require
    // coordinator-owned durable recovery evidence before SQLite replay.
    require_absent(&sidecar(source, "-journal"))?;
    Ok((a, b))
}

fn file_metadata(path: &Path) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::UnsafeFile)?;
    if !metadata.file_type().is_file() {
        return Err(Error::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(Error::UnsafeFile);
        }
    }
    Ok(metadata)
}

fn require_same_file(
    path: &Path,
    before: &Metadata,
    allow_length_change: bool,
) -> Result<(), Error> {
    let after = file_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.uid() != after.uid()
            || before.mode() != after.mode()
        {
            return Err(Error::IdentityChanged);
        }
    }
    if !allow_length_change && before.len() != after.len() {
        return Err(Error::IdentityChanged);
    }
    Ok(())
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
fn require_absent(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(Error::SidecarRemaining),
        Err(_) => Err(Error::Io),
    }
}
fn require_no_sidecars(path: &Path) -> Result<(), Error> {
    for suffix in ["-wal", "-shm", "-journal"] {
        require_absent(&sidecar(path, suffix))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "maintenance_tests.rs"]
mod tests;

#[path = "maintenance_recovery.rs"]
mod recovery;
