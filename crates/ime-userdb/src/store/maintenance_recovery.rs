//! Qualification and SQLite-owned replay of a single-database rollback journal.
use std::io::{Read, Seek, SeekFrom};

use super::*;

const MAGIC: [u8; 8] = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];

impl UserDb {
    /// Read-only qualification, before the coordinator records journal identity.
    /// Does not open the source with SQLite. A positive result is NOT authority
    /// or proof that the journal belongs to this operation.
    pub fn qualify_maintenance_journal(
        source: impl AsRef<Path>,
        snapshot: impl AsRef<Path>,
    ) -> Result<i64, Error> {
        qualify(source.as_ref(), snapshot.as_ref())
    }

    /// Only after a durable maintenance intent AND journal recovery identity,
    /// fresh authority/quiescence, protected snapshot hash and family checks.
    /// SQLite may change the main file and remove its journal before an error.
    /// No snapshot replacement, ATTACH, migration or user SQL is performed.
    pub fn recover_maintenance_journal(
        source: impl AsRef<Path>,
        snapshot: impl AsRef<Path>,
    ) -> Result<i64, Error> {
        recover(source.as_ref(), snapshot.as_ref(), &mut |_| Ok(()))
    }
}

fn qualify(source: &Path, snapshot: &Path) -> Result<i64, Error> {
    let source_before = file_metadata(source)?;
    let snapshot_before = file_metadata(snapshot)?;
    if fs::canonicalize(source).map_err(|_| Error::Io)?
        == fs::canonicalize(snapshot).map_err(|_| Error::Io)?
    {
        return Err(Error::UnsafeFile);
    }
    require_no_sidecars(snapshot)?;
    for suffix in ["-wal", "-shm"] {
        require_absent(&sidecar(source, suffix))?;
    }
    let journal = sidecar(source, "-journal");
    let journal_before = file_metadata(&journal)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for metadata in [&snapshot_before, &journal_before] {
            if metadata.dev() != source_before.dev()
                || metadata.uid() != source_before.uid()
                || metadata.mode() & 0o7777 != 0o600
                || metadata.ino() == source_before.ino()
            {
                return Err(Error::UnsafeFile);
            }
        }
        if source_before.mode() & 0o7777 != 0o600 || snapshot_before.ino() == journal_before.ino() {
            return Err(Error::UnsafeFile);
        }
    }
    let mut file = fs::File::open(&journal).map_err(|_| Error::Io)?;
    let opened = file.metadata().map_err(|_| Error::Io)?;
    require_same_file(&journal, &opened, false)?;
    require_same_file(&journal, &journal_before, false)?;
    let size = journal_before.len();
    let mut header = [0; 28];
    file.read_exact(&mut header)
        .map_err(|_| Error::DatabaseInvalid)?;
    if header[..8] != MAGIC || size <= 512 {
        return Err(Error::DatabaseInvalid);
    }
    let word = |offset: usize| u32::from_be_bytes(header[offset..offset + 4].try_into().unwrap());
    let (records, original_pages, sector, page) = (word(8), word(16), word(20), word(24));
    if records == 0
        || records == u32::MAX
        || original_pages == 0
        || !(512..=65536).contains(&sector)
        || !sector.is_power_of_two()
        || !(512..=65536).contains(&page)
        || !page.is_power_of_two()
        || u64::from(original_pages) * u64::from(page) != snapshot_before.len()
        || u64::from(sector) + u64::from(records) * (u64::from(page) + 8) > size
    {
        return Err(Error::DatabaseInvalid);
    }
    // SQLite readSuperJournal checks the final magic before following a path.
    // Reject that trailer outright, even with a bad length/checksum, so no
    // recovery here may inspect or delete another database's super-journal.
    file.seek(SeekFrom::End(-8)).map_err(|_| Error::Io)?;
    let mut tail = [0; 8];
    file.read_exact(&mut tail).map_err(|_| Error::Io)?;
    if tail == MAGIC {
        return Err(Error::UnsafeFile);
    }
    let connection = open(snapshot, false)?;
    let result = (|| {
        if journal_mode(&connection)? != "delete" {
            return Err(Error::UnsupportedJournalMode);
        }
        validate_integrity(&connection)?;
        maintenance_compare::validate_schema(&connection)
    })();
    close(connection)?;
    require_same_file(source, &source_before, false)?;
    require_same_file(snapshot, &snapshot_before, false)?;
    require_same_file(&journal, &journal_before, false)?;
    require_same_file(&journal, &opened, false)?;
    require_no_sidecars(snapshot)?;
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryPoint {
    Opened,
    Replayed,
    Compared,
    Closed,
}

fn recover(
    source: &Path,
    snapshot: &Path,
    checkpoint: &mut impl FnMut(RecoveryPoint) -> Result<(), Error>,
) -> Result<i64, Error> {
    let schema = qualify(source, snapshot)?;
    let source_before = file_metadata(source)?;
    let snapshot_before = file_metadata(snapshot)?;
    let connection = open(source, true)?;
    let result = (|| {
        checkpoint(RecoveryPoint::Opened)?;
        // Reading the schema triggers SQLite pager hot-journal recovery before
        // any data is exposed. SQLite retains responsibility for lock/busy,
        // page checksum handling, replay ordering and journal deletion.
        if maintenance_compare::validate_schema(&connection)? != schema {
            return Err(Error::ContentChanged);
        }
        checkpoint(RecoveryPoint::Replayed)?;
        let mode = journal_mode(&connection)?;
        if mode != "delete" && mode != "wal" {
            return Err(Error::UnsupportedJournalMode);
        }
        validate_integrity(&connection)?;
        let protected = open(snapshot, false)?;
        let comparison = (|| {
            connection
                .execute_batch("BEGIN")
                .map_err(Error::from_sqlite)?;
            protected
                .execute_batch("BEGIN")
                .map_err(Error::from_sqlite)?;
            maintenance_compare::compare(&connection, &protected)
        })();
        close(protected)?;
        if comparison? != schema {
            return Err(Error::ContentChanged);
        }
        checkpoint(RecoveryPoint::Compared)?;
        Ok(schema)
    })();
    close(connection)?;
    let schema = result?;
    checkpoint(RecoveryPoint::Closed)?;
    require_same_file(source, &source_before, true)?;
    require_same_file(snapshot, &snapshot_before, false)?;
    require_absent(&sidecar(source, "-journal"))?;
    fs::File::open(source)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Io)?;
    // Page-one recovery may restore the pre-transition WAL header. Complete
    // the already-authorized preparation through the normal strict primitive,
    // rather than treating that restored WAL mode as a terminal result.
    if UserDb::prepare_source_for_upgrade(source, snapshot)?.schema_version != schema {
        return Err(Error::ContentChanged);
    }
    Ok(schema)
}

#[cfg(test)]
#[path = "maintenance_recovery_tests.rs"]
mod tests;
