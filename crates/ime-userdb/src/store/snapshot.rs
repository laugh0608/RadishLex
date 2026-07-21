use std::fs;
use std::path::Path;

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use crate::error::{UserDbError, UserDbResult};
use crate::model::{UserDbSnapshotEstimate, UserDbSnapshotSummary};

use super::connection::{
    preserved_database_error, read_schema_version, verify_integrity, BUSY_TIMEOUT,
};
use super::UserDb;

impl UserDb {
    /// Estimates the logical bytes required for a SQLite-consistent snapshot.
    ///
    /// This reads page metadata inside one read transaction and does not
    /// configure WAL, migrate the source, or create a destination file.
    pub fn estimate_snapshot(path: impl AsRef<Path>) -> UserDbResult<UserDbSnapshotEstimate> {
        let path = path.as_ref();
        let connection = open_snapshot_source(path)?;
        let estimate = snapshot_estimate_on(&connection)
            .map_err(|error| preserved_database_error(path, "estimate snapshot", error))?;
        verify_integrity(&connection)
            .map_err(|error| preserved_database_error(path, "estimate snapshot", error))?;
        Ok(estimate)
    }

    /// Copies one existing source database into an existing empty candidate
    /// through SQLite's online backup API and validates the copied result.
    ///
    /// Product callers must create and identity-check `destination` inside an
    /// isolated private directory. This function never migrates either file.
    pub fn create_consistent_snapshot(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> UserDbResult<UserDbSnapshotSummary> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        validate_snapshot_paths(source, destination)?;

        let source_connection = open_snapshot_source(source)?;
        let source_estimate = snapshot_estimate_on(&source_connection)
            .map_err(|error| preserved_database_error(source, "prepare snapshot", error))?;
        verify_integrity(&source_connection)
            .map_err(|error| preserved_database_error(source, "prepare snapshot", error))?;
        let mut destination_connection = Connection::open_with_flags(
            destination,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| preserved_database_error(destination, "open snapshot", error))?;
        destination_connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| preserved_database_error(destination, "configure snapshot", error))?;

        copy_snapshot(destination, &source_connection, &mut destination_connection)?;
        finalize_standalone_snapshot(destination, &destination_connection)?;
        verify_integrity(&destination_connection)
            .map_err(|error| preserved_database_error(destination, "validate snapshot", error))?;
        let snapshot_estimate = snapshot_estimate_on(&destination_connection)
            .map_err(|error| preserved_database_error(destination, "validate snapshot", error))?;
        if source_estimate != snapshot_estimate {
            return Err(UserDbError::invalid_input(
                "snapshot",
                "snapshot page metadata does not match the source transaction",
            ));
        }
        drop(destination_connection);
        let snapshot_file_bytes = fs::metadata(destination)
            .map_err(|error| UserDbError::Io {
                path: destination.to_path_buf(),
                source: error,
            })?
            .len();

        Ok(UserDbSnapshotSummary {
            source_schema_version: source_estimate.schema_version,
            snapshot_schema_version: snapshot_estimate.schema_version,
            page_size_bytes: snapshot_estimate.page_size_bytes,
            page_count: snapshot_estimate.page_count,
            logical_size_bytes: snapshot_estimate.logical_size_bytes,
            snapshot_file_bytes,
        })
    }
}

fn validate_snapshot_paths(source: &Path, destination: &Path) -> UserDbResult<()> {
    let source_metadata = fs::symlink_metadata(source).map_err(|error| UserDbError::Io {
        path: source.to_path_buf(),
        source: error,
    })?;
    let destination_metadata =
        fs::symlink_metadata(destination).map_err(|error| UserDbError::Io {
            path: destination.to_path_buf(),
            source: error,
        })?;
    let source_canonical = fs::canonicalize(source).map_err(|error| UserDbError::Io {
        path: source.to_path_buf(),
        source: error,
    })?;
    let destination_canonical = fs::canonicalize(destination).map_err(|error| UserDbError::Io {
        path: destination.to_path_buf(),
        source: error,
    })?;
    if !source_metadata.file_type().is_file()
        || !destination_metadata.file_type().is_file()
        || destination_metadata.len() != 0
        || source_canonical == destination_canonical
    {
        return Err(UserDbError::invalid_input(
            "snapshot_destination",
            "snapshot requires a distinct existing empty regular file",
        ));
    }
    Ok(())
}

fn open_snapshot_source(path: &Path) -> UserDbResult<Connection> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| preserved_database_error(path, "open snapshot source", error))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| preserved_database_error(path, "configure snapshot source", error))?;
    connection
        .execute_batch("BEGIN")
        .map_err(|error| preserved_database_error(path, "begin snapshot read", error))?;
    Ok(connection)
}

fn copy_snapshot(
    destination_path: &Path,
    source: &Connection,
    destination: &mut Connection,
) -> UserDbResult<()> {
    let backup = Backup::new(source, destination)
        .map_err(|error| preserved_database_error(destination_path, "start snapshot", error))?;
    match backup
        .step(-1)
        .map_err(|error| preserved_database_error(destination_path, "copy snapshot", error))?
    {
        StepResult::Done => Ok(()),
        StepResult::Busy | StepResult::Locked | StepResult::More => {
            Err(UserDbError::invalid_input(
                "snapshot_source",
                "source database was not quiescent for a complete snapshot",
            ))
        }
        _ => Err(UserDbError::invalid_input(
            "snapshot_source",
            "source database returned an unsupported backup result",
        )),
    }
}

fn finalize_standalone_snapshot(path: &Path, connection: &Connection) -> UserDbResult<()> {
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(|error| preserved_database_error(path, "finalize snapshot journal", error))?;
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|error| preserved_database_error(path, "verify snapshot journal", error))?;
    if !journal_mode.eq_ignore_ascii_case("delete") {
        return Err(UserDbError::invalid_input(
            "snapshot_destination",
            "snapshot must finish as one standalone SQLite file",
        ));
    }
    Ok(())
}

fn snapshot_estimate_on(connection: &Connection) -> rusqlite::Result<UserDbSnapshotEstimate> {
    let schema_version = read_schema_version(connection)?;
    if schema_version < 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let page_size: i64 = connection.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let page_count: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    if page_size <= 0 || page_count < 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let page_size_bytes = u64::try_from(page_size).map_err(|_| rusqlite::Error::InvalidQuery)?;
    // Backing up a zero-byte schema-0 database materializes one SQLite header
    // page in the destination, so the estimate must reserve that page.
    let page_count = u64::try_from(page_count)
        .map_err(|_| rusqlite::Error::InvalidQuery)?
        .max(1);
    let logical_size_bytes = page_size_bytes
        .checked_mul(page_count)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    Ok(UserDbSnapshotEstimate {
        schema_version,
        page_size_bytes,
        page_count,
        logical_size_bytes,
    })
}
