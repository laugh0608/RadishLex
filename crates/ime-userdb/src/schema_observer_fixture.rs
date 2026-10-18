use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use rusqlite::{config::DbConfig, Connection, OpenFlags};

use super::{read_manifest, ObserverError, SyntheticFixture};
use crate::UserDb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureKind {
    MainOnly,
    EmptyWal,
    HeaderOnlyWal,
    CommittedWal,
    MultipleCommits,
    OldMainNewWal,
    UncommittedTail,
    PartialCheckpoint,
    WithoutShm,
    WithShm,
    SaltMismatch,
    ChecksumMismatch,
    TruncatedWal,
    InvalidLayout,
    UnsupportedVersion,
    MissingDb,
    EmptyDb,
    MissingWalForWalMode,
    LargeDatabase,
    MaxWal,
    MaxWalSmallPages,
    BadHeaderMagic,
    BadHeaderChecksum,
    PageSizeMismatch,
    ZeroFramePage,
    UnsupportedNegativeVersion,
    HotJournal,
    ResetSaltTail,
}

fn fixture_error<T>(result: Result<T, impl std::fmt::Debug>) -> Result<T, ObserverError> {
    result.map_err(|_| ObserverError::Fixture)
}

/// Generates only new synthetic bytes in an existing empty source directory.
/// Never opens an existing input DB and never retains a source SQLite connection.
/// The host owns source-directory identity, permissions and teardown.
pub fn create_synthetic_fixture(
    output_source_dir: &Path,
    kind: FixtureKind,
) -> Result<SyntheticFixture, ObserverError> {
    // The profile is intentionally a single reviewed schema, not an alias for
    // whatever a future UserDb build happens to support.
    if UserDb::supported_schema_version() != 9 {
        return Err(ObserverError::UnsupportedSchema);
    }
    let metadata = fixture_error(fs::symlink_metadata(output_source_dir))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || fixture_error(fs::read_dir(output_source_dir))?
            .next()
            .is_some()
    {
        return Err(ObserverError::Fixture);
    }
    let builder = output_source_dir.join("fixture-builder.sqlite3");
    drop(new_file(&builder)?);
    if kind == FixtureKind::MaxWalSmallPages {
        configure_small_page_builder(&builder)?;
    }
    let database = fixture_error(UserDb::open(&builder))?;
    let connection = database.connection;
    let result = generate(&connection, &builder, output_source_dir, kind);
    // These are resources created here, not paths selected by the observer host.
    // Failure is retained for host inspection; never recursively remove source.
    let close = connection.close();
    if let Err((connection, _)) = close {
        drop(connection);
        return Err(ObserverError::CloseUnconfirmed);
    }
    let (trusted_manifest, reader) = result?;
    if let Some(reader) = reader {
        fixture_error(reader.execute_batch("ROLLBACK"))?;
        if let Err((connection, _)) = reader.close() {
            drop(connection);
            return Err(ObserverError::CloseUnconfirmed);
        }
    }
    for name in [
        "fixture-builder.sqlite3",
        "fixture-builder.sqlite3-wal",
        "fixture-builder.sqlite3-shm",
    ] {
        let path = output_source_dir.join(name);
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ObserverError::Fixture),
        }
    }
    let reference_version = if kind == FixtureKind::OldMainNewWal {
        Some(reference_version(output_source_dir)?)
    } else {
        None
    };
    Ok(SyntheticFixture {
        source: output_source_dir.to_path_buf(),
        trusted_manifest,
        reference_version,
    })
}

/// Materialize a fresh empty 512-byte-page database before UserDb schema
/// construction. Merely setting page_size on an empty file is connection-local.
fn configure_small_page_builder(builder: &Path) -> Result<(), ObserverError> {
    let connection = fixture_error(Connection::open_with_flags(
        builder,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    ))?;
    let result: Result<(), ObserverError> =
        (|| {
            fixture_error(connection.busy_timeout(std::time::Duration::from_millis(100)))?;
            // Two real page-one commits persist the requested geometry while
            // returning the empty database to user_version=0 for UserDb creation.
            fixture_error(connection.execute_batch(
                "PRAGMA page_size=512; PRAGMA user_version=1; PRAGMA user_version=0;",
            ))?;
            let page_size: u32 =
                fixture_error(connection.query_row("PRAGMA page_size", [], |row| row.get(0)))?;
            let version: i64 =
                fixture_error(connection.query_row("PRAGMA user_version", [], |row| row.get(0)))?;
            if page_size != 512 || version != 0 {
                return Err(ObserverError::Fixture);
            }
            Ok(())
        })();
    if let Err((connection, _)) = connection.close() {
        drop(connection);
        return Err(ObserverError::CloseUnconfirmed);
    }
    result
}

type Generated = (Vec<super::SchemaObject>, Option<Connection>);
fn generate(
    connection: &Connection,
    builder: &Path,
    output: &Path,
    kind: FixtureKind,
) -> Result<Generated, ObserverError> {
    let control: super::ObserverControl = Arc::new(|| Ok(()));
    let trusted_manifest = read_manifest(connection, &mut 0, &control)?;
    fixture_error(connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true))?;
    fixture_error(
        connection.execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA wal_checkpoint(TRUNCATE);"),
    )?;
    if kind == FixtureKind::MainOnly {
        fixture_error(connection.execute_batch("PRAGMA journal_mode=DELETE"))?;
    }
    if kind == FixtureKind::OldMainNewWal {
        fixture_error(
            connection.execute_batch("PRAGMA user_version=8; PRAGMA wal_checkpoint(TRUNCATE);"),
        )?;
    }
    if kind == FixtureKind::LargeDatabase {
        fill_large_database(connection)?;
    }
    if kind == FixtureKind::MaxWalSmallPages {
        let page_size: u32 =
            fixture_error(connection.query_row("PRAGMA page_size", [], |row| row.get(0)))?;
        if page_size != 512 {
            return Err(ObserverError::Fixture);
        }
    }
    if matches!(kind, FixtureKind::MaxWal | FixtureKind::MaxWalSmallPages) {
        fill_max_wal(connection, output)?;
    }
    let mut reader = None;
    if kind == FixtureKind::PartialCheckpoint {
        fixture_error(connection.execute_batch("PRAGMA user_version=9"))?;
        let pinned = fixture_error(Connection::open_with_flags(
            builder,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        ))?;
        fixture_error(pinned.execute_batch("BEGIN; SELECT count(*) FROM sqlite_schema;"))?;
        reader = Some(pinned);
    }
    if !matches!(
        kind,
        FixtureKind::MainOnly
            | FixtureKind::EmptyWal
            | FixtureKind::MissingDb
            | FixtureKind::EmptyDb
            | FixtureKind::LargeDatabase
            | FixtureKind::MaxWal
            | FixtureKind::MaxWalSmallPages
    ) {
        fixture_error(connection.execute_batch("PRAGMA user_version=9"))?;
    }
    if matches!(
        kind,
        FixtureKind::MultipleCommits | FixtureKind::PartialCheckpoint
    ) {
        fixture_error(connection.execute_batch(
            "INSERT INTO import_batches(source_name,term_count,created_at_ms,notes)
             VALUES('synthetic',0,1,'synthetic'); PRAGMA user_version=9;",
        ))?;
    }
    if kind == FixtureKind::PartialCheckpoint {
        let (busy, logged, checkpointed): (i64, i64, i64) = fixture_error(connection.query_row(
            "PRAGMA wal_checkpoint(PASSIVE)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ))?;
        if busy != 0 || checkpointed <= 0 || checkpointed >= logged {
            return Err(ObserverError::Fixture);
        }
    }
    if kind == FixtureKind::InvalidLayout {
        fixture_error(connection.execute_batch("DROP INDEX idx_user_terms_identity"))?;
    }
    if kind == FixtureKind::UnsupportedNegativeVersion {
        fixture_error(connection.execute_batch("PRAGMA user_version=-1"))?;
    }
    if kind == FixtureKind::UnsupportedVersion {
        fixture_error(connection.execute_batch("PRAGMA user_version=8"))?;
    }
    if kind == FixtureKind::UncommittedTail {
        // A real SQLite writer spills valid uncommitted frames with cache=1.
        fixture_error(connection.execute_batch(
            "PRAGMA cache_size=1; PRAGMA cache_spill=ON; BEGIN IMMEDIATE;
             PRAGMA user_version=99;
             WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<256)
             INSERT INTO import_batches(source_name,term_count,created_at_ms,notes)
             SELECT 'synthetic',0,v,printf('%04096d',v) FROM n;",
        ))?;
    }
    let reset_generation = if kind == FixtureKind::ResetSaltTail {
        Some(generate_reset_salt_tail(connection, output)?)
    } else {
        None
    };
    let db = output.join("userdb.sqlite3");
    match kind {
        FixtureKind::MissingDb => {}
        FixtureKind::EmptyDb => {
            drop(new_file(&db)?);
        }
        _ => copy_new(builder, &db, None)?,
    }
    let wal_source = output.join("fixture-builder.sqlite3-wal");
    let wal_output = output.join("userdb.sqlite3-wal");
    match kind {
        FixtureKind::MainOnly
        | FixtureKind::MissingDb
        | FixtureKind::EmptyDb
        | FixtureKind::LargeDatabase
        | FixtureKind::MissingWalForWalMode => {}
        FixtureKind::EmptyWal => {
            drop(new_file(&wal_output)?);
        }
        FixtureKind::HeaderOnlyWal => copy_new(&wal_source, &wal_output, Some(32))?,
        _ => copy_new(&wal_source, &wal_output, None)?,
    }
    if let Some((old_salt, old_length)) = reset_generation {
        verify_reset_salt_tail(&wal_output, old_salt, old_length)?;
    }
    if kind == FixtureKind::UncommittedTail {
        fixture_error(connection.execute_batch("ROLLBACK"))?;
    }
    if kind == FixtureKind::WithShm {
        copy_new(
            &output.join("fixture-builder.sqlite3-shm"),
            &output.join("userdb.sqlite3-shm"),
            None,
        )?;
    }
    match kind {
        FixtureKind::BadHeaderMagic => flip_byte(&wal_output, 0)?,
        FixtureKind::BadHeaderChecksum => flip_byte(&wal_output, 24)?,
        FixtureKind::PageSizeMismatch => write_be32(&wal_output, 8, 8192)?,
        FixtureKind::ZeroFramePage => write_be32(&wal_output, 32, 0)?,
        FixtureKind::HotJournal => {
            // Presence alone must be rejected by capture before any SQLite open.
            // This is synthetic journal-shaped evidence, not a recovery attempt.
            let mut journal = new_file(&output.join("userdb.sqlite3-journal"))?;
            fixture_error(journal.write_all(b"synthetic-observer-journal-blocked-v1\n"))?;
            fixture_error(journal.sync_all())?;
        }
        FixtureKind::SaltMismatch => flip_byte(&wal_output, 40)?,
        FixtureKind::ChecksumMismatch => flip_byte(&wal_output, 48)?,
        FixtureKind::TruncatedWal => {
            let file = fixture_error(OpenOptions::new().write(true).open(&wal_output))?;
            let length = fixture_error(file.metadata())?.len();
            fixture_error(file.set_len(length.checked_sub(1).ok_or(ObserverError::Fixture)?))?;
        }
        _ => {}
    }
    Ok((trusted_manifest, reader))
}

fn new_file(path: &Path) -> Result<File, ObserverError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    fixture_error(options.open(path))
}
fn copy_new(source: &Path, output: &Path, limit: Option<u64>) -> Result<(), ObserverError> {
    let mut source = fixture_error(File::open(source))?;
    let mut output = new_file(output)?;
    let length = fixture_error(source.metadata())?.len();
    let mut remaining = limit.unwrap_or(length);
    if remaining > length {
        return Err(ObserverError::Fixture);
    }
    let mut buffer = [0; 64 * 1024];
    while remaining > 0 {
        let count = remaining.min(buffer.len() as u64) as usize;
        fixture_error(source.read_exact(&mut buffer[..count]))?;
        fixture_error(output.write_all(&buffer[..count]))?;
        remaining -= count as u64;
    }
    fixture_error(output.sync_all())
}
fn flip_byte(path: &Path, offset: u64) -> Result<(), ObserverError> {
    let mut file = fixture_error(OpenOptions::new().read(true).write(true).open(path))?;
    fixture_error(file.seek(SeekFrom::Start(offset)))?;
    let mut byte = [0];
    fixture_error(file.read_exact(&mut byte))?;
    byte[0] ^= 1;
    fixture_error(file.seek(SeekFrom::Start(offset)))?;
    fixture_error(file.write_all(&byte))
}

/// A real SQLite RESTART checkpoint leaves old-generation frames beyond the
/// next generation's short valid prefix. Never manufacture salt bytes here.
fn generate_reset_salt_tail(
    connection: &Connection,
    output: &Path,
) -> Result<([u8; 8], u64), ObserverError> {
    fixture_error(connection.execute_batch("PRAGMA journal_size_limit=-1"))?;
    for _ in 0..8 {
        fixture_error(connection.execute_batch("PRAGMA user_version=9"))?;
    }
    let wal_path = output.join("fixture-builder.sqlite3-wal");
    let mut wal = fixture_error(File::open(&wal_path))?;
    let old_length = fixture_error(wal.metadata())?.len();
    let mut header = [0; 32];
    fixture_error(wal.read_exact(&mut header))?;
    drop(wal);
    let mut old_salt = [0; 8];
    old_salt.copy_from_slice(&header[16..24]);
    let (busy, logged, checkpointed): (i64, i64, i64) = fixture_error(connection.query_row(
        "PRAGMA wal_checkpoint(RESTART)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ))?;
    if busy != 0
        || logged < 2
        || checkpointed != logged
        || fixture_error(fs::metadata(&wal_path))?.len() != old_length
    {
        return Err(ObserverError::Fixture);
    }
    // This first write restarts the log using fresh generation salts, without
    // truncation. One page-one commit is shorter than the previous generation.
    fixture_error(connection.execute_batch("PRAGMA user_version=9"))?;
    Ok((old_salt, old_length))
}

fn verify_reset_salt_tail(
    path: &Path,
    old_salt: [u8; 8],
    old_length: u64,
) -> Result<(), ObserverError> {
    let mut wal = fixture_error(File::open(path))?;
    let length = fixture_error(wal.metadata())?.len();
    let mut header = [0; 32];
    fixture_error(wal.read_exact(&mut header))?;
    let page_size = u64::from(super::wal::be(&header[8..]));
    if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
        return Err(ObserverError::Fixture);
    }
    let frame_size = page_size.checked_add(24).ok_or(ObserverError::Fixture)?;
    let frames_bytes = length.checked_sub(32).ok_or(ObserverError::Fixture)?;
    if length != old_length
        || frames_bytes % frame_size != 0
        || frames_bytes / frame_size < 2
        || header[16..24] == old_salt
    {
        return Err(ObserverError::Fixture);
    }
    let mut found_old_tail = false;
    for index in 0..frames_bytes / frame_size {
        let mut frame = [0; 24];
        fixture_error(wal.read_exact(&mut frame))?;
        if index == 0 {
            if frame[8..16] != header[16..24] || super::wal::be(&frame[4..]) == 0 {
                return Err(ObserverError::Fixture);
            }
        } else if frame[8..16] == old_salt && frame[8..16] != header[16..24] {
            found_old_tail = true;
        }
        fixture_error(wal.seek(SeekFrom::Current(
            i64::try_from(page_size).map_err(|_| ObserverError::Fixture)?,
        )))?;
    }
    if !found_old_tail {
        return Err(ObserverError::Fixture);
    }
    Ok(())
}

fn fill_max_wal(connection: &Connection, output: &Path) -> Result<(), ObserverError> {
    fixture_error(connection.execute_batch("PRAGMA synchronous=OFF; PRAGMA cache_size=-8192;"))?;
    let page_size: u32 =
        fixture_error(connection.query_row("PRAGMA page_size", [], |row| row.get(0)))?;
    let page_size = u64::from(page_size);
    let max_frames = (super::wal::MAX_WAL - 32) / (page_size + 24);
    for _ in 0..max_frames {
        fixture_error(connection.execute_batch("PRAGMA user_version=9"))?;
    }
    let size = fixture_error(fs::metadata(output.join("fixture-builder.sqlite3-wal")))?.len();
    if size != 32 + max_frames * (page_size + 24) {
        return Err(ObserverError::Fixture);
    }
    Ok(())
}

fn fill_large_database(connection: &Connection) -> Result<(), ObserverError> {
    fixture_error(connection.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=OFF; PRAGMA cache_size=-8192;",
    ))?;
    let page_size: u32 =
        fixture_error(connection.query_row("PRAGMA page_size", [], |row| row.get(0)))?;
    // This fixture is generated by the bundled default 4096-byte pager. Its
    // single-row overflow calculation is deliberately not a general pager.
    if page_size != 4096 {
        return Err(ObserverError::Fixture);
    }
    let page_size = u64::from(page_size);
    let max_pages = super::wal::MAX_DB / page_size;
    fixture_error(connection.pragma_update(
        None,
        "max_page_count",
        i64::try_from(max_pages).map_err(|_| ObserverError::LimitExceeded)?,
    ))?;
    loop {
        let count: u32 =
            fixture_error(connection.query_row("PRAGMA page_count", [], |row| row.get(0)))?;
        let remaining = max_pages
            .checked_sub(u64::from(count))
            .ok_or(ObserverError::Fixture)?;
        if remaining == 0 {
            return Ok(());
        }
        if remaining <= 512 {
            // Original notes=3800 has no overflow and leaves room for the few
            // extra record-header bytes in its leaf. One update appends exactly
            // N overflow payloads; no new B-tree cell or leaf split is required.
            // Repeated updates are unsuitable: SQLite allocates a replacement
            // overflow chain before freeing the previous one.
            let payload = remaining
                .checked_mul(page_size - 4)
                .ok_or(ObserverError::LimitExceeded)?;
            fixture_error(connection.execute_batch("SAVEPOINT fixture_exact_capacity"))?;
            let finish = (|| {
                let updated = fixture_error(connection.execute(
                    "UPDATE import_batches SET notes=notes || printf('%0*d',?1,0)
                     WHERE id=(SELECT min(id) FROM import_batches)",
                    [i64::try_from(payload).map_err(|_| ObserverError::LimitExceeded)?],
                ))?;
                let actual: u32 =
                    fixture_error(connection.query_row("PRAGMA page_count", [], |row| row.get(0)))?;
                if updated != 1 || u64::from(actual) != max_pages {
                    return Err(ObserverError::Fixture);
                }
                Ok(())
            })();
            if finish.is_err() {
                if !connection.is_autocommit() {
                    fixture_error(connection.execute_batch(
                        "ROLLBACK TO fixture_exact_capacity; RELEASE fixture_exact_capacity;",
                    ))?;
                }
                return finish;
            }
            fixture_error(connection.execute_batch("RELEASE fixture_exact_capacity"))?;
            return Ok(());
        }
        // Keep ample room for normal B-tree growth; the exact tail above is a
        // separate fixture construction step, not observer recovery/fallback.
        fixture_error(connection.execute(
            "WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<256)
             INSERT INTO import_batches(source_name,term_count,created_at_ms,notes)
             SELECT 'synthetic',0,v,printf('%03800d',v) FROM n",
            [],
        ))?;
    }
}

fn write_be32(path: &Path, offset: u64, value: u32) -> Result<(), ObserverError> {
    let mut file = fixture_error(OpenOptions::new().write(true).open(path))?;
    fixture_error(file.seek(SeekFrom::Start(offset)))?;
    fixture_error(file.write_all(&value.to_be_bytes()))
}

fn reference_version(source: &Path) -> Result<i64, ObserverError> {
    let reference = source.join("fixture-reference.sqlite3");
    copy_new(&source.join("userdb.sqlite3"), &reference, None)?;
    copy_new(
        &source.join("userdb.sqlite3-wal"),
        &source.join("fixture-reference.sqlite3-wal"),
        None,
    )?;
    let connection = fixture_error(Connection::open_with_flags(
        &reference,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    ))?;
    // Separate engine query on another byte copy, with no observer WAL parser,
    // schema expectation, supported-version comparison, or target input.
    let result: Result<i64, ObserverError> = (|| {
        super::configure(&connection)?;
        fixture_error(connection.execute_batch("BEGIN DEFERRED"))?;
        let version =
            fixture_error(connection.query_row("PRAGMA user_version", [], |row| row.get(0)))?;
        fixture_error(connection.execute_batch("COMMIT"))?;
        Ok(version)
    })();
    let rollback = if !connection.is_autocommit() {
        fixture_error(connection.execute_batch("ROLLBACK"))
    } else {
        Ok(())
    };
    if let Err((connection, _)) = connection.close() {
        drop(connection);
        return Err(ObserverError::CloseUnconfirmed);
    }
    rollback?;
    let version = result?;
    for name in [
        "fixture-reference.sqlite3",
        "fixture-reference.sqlite3-wal",
        "fixture-reference.sqlite3-shm",
    ] {
        match fs::remove_file(source.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ObserverError::Fixture),
        }
    }
    Ok(version)
}
