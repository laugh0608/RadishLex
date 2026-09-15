//! Durable metadata only. The coordinator must independently prove outer
//! authority, quiescence, source/snapshot hashes and archive relationships.
//! Journal read/write primitives never open SQLite, move an artifact or remove
//! a marker. The child source_preparation module enforces SQLite orchestration.

use crate::{
    PreparationDirectoryIdentity, PreparationPhase, PreparationReceipt,
    MAX_PREPARATION_RECEIPT_BYTES,
};

use super::*;

const JOURNAL: &str = "source-preparation.json";
const STAGED_JOURNAL: &str = "source-preparation.json.tmp";
const PREPARATION_SNAPSHOT: &str = "preparation-snapshot.sqlite3";
const STAGED_PREPARATION_SNAPSHOT: &str = "preparation-snapshot.sqlite3.tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationJournalError {
    Filesystem(UpgradeFilesystemErrorCode),
    UnexpectedObject,
    InterruptedWrite,
    InvalidRecord,
    InvalidReplacement,
    IdentityChanged,
    Io,
}

impl fmt::Display for PreparationJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Filesystem(_) => "preparation journal filesystem verification failed",
            Self::UnexpectedObject => "preparation journal has an unexpected object",
            Self::InterruptedWrite => "preparation journal write requires recovery",
            Self::InvalidRecord => "preparation journal record is invalid",
            Self::InvalidReplacement => "preparation journal replacement is invalid",
            Self::IdentityChanged => "preparation journal identity changed",
            Self::Io => "preparation journal persistence failed",
        })
    }
}

impl std::error::Error for PreparationJournalError {}

impl From<UpgradeFilesystemError> for PreparationJournalError {
    fn from(value: UpgradeFilesystemError) -> Self {
        Self::Filesystem(value.code())
    }
}

type Result<T> = std::result::Result<T, PreparationJournalError>;
use PreparationJournalError as Error;

/// A preparation-only journal over the unchanged v1 state directory and guard.
/// Record validity does not certify the physical artifacts described by it.
pub struct PreparationJournalStore {
    store: UpgradeReceiptStore,
}

impl PreparationJournalStore {
    /// Reuses a freshly verified v1 store without dropping its process guard.
    pub fn attach(store: UpgradeReceiptStore, guard: &UpgradeProcessGuard) -> Result<Self> {
        let journal = Self { store };
        journal.verify_guard(guard)?;
        journal.validate_entries()?;
        Ok(journal)
    }

    /// Read-only construction for recovery. Unlike the ordinary v1 reader,
    /// this recognizes preparation slots, but never ignores unknown objects.
    pub fn open_existing(root: VerifiedDataRoot) -> Result<Self> {
        let journal = Self {
            store: UpgradeReceiptStore::open_existing_directory(root)?,
        };
        journal.validate_entries()?;
        Ok(journal)
    }

    /// The product caller must already hold its outer install guard.
    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard> {
        self.store.revalidate()?;
        self.validate_entries()?;
        Ok(self.store.acquire_directory_guard()?)
    }

    pub fn data_root_identity(&self) -> PreparationDirectoryIdentity {
        let root = &self.store.root.identity;
        PreparationDirectoryIdentity {
            device_id: root.device_id(),
            inode: root.inode(),
            owner_id: root.owner_id(),
            mode: root.mode(),
        }
    }

    pub fn state_directory_identity(&self) -> PreparationDirectoryIdentity {
        let state = self.store.state_directory_identity;
        PreparationDirectoryIdentity {
            device_id: state.device_id,
            inode: state.inode,
            owner_id: state.owner_id,
            mode: state.mode,
        }
    }

    pub fn load_guarded(&self, guard: &UpgradeProcessGuard) -> Result<Option<PreparationReceipt>> {
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        let stored = self.read_record()?;
        self.verify_guard(guard)?;
        Ok(stored.map(|stored| stored.receipt))
    }

    /// Persists exactly one transition from the caller's observed predecessor.
    /// Same-byte replay also fsyncs the file and directory: a previous call may
    /// have stopped after rename, before durability was acknowledged.
    pub fn persist(
        &self,
        guard: &UpgradeProcessGuard,
        previous: Option<&PreparationReceipt>,
        next: &PreparationReceipt,
    ) -> Result<()> {
        self.persist_with_faults(guard, previous, next, &NoFaults)
    }

    fn persist_with_faults(
        &self,
        guard: &UpgradeProcessGuard,
        previous: Option<&PreparationReceipt>,
        next: &PreparationReceipt,
        faults: &impl FaultInjector,
    ) -> Result<()> {
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        self.verify_binding(next)?;
        let bytes = next.encode().map_err(|_| Error::InvalidRecord)?;
        match previous {
            Some(previous) if next.can_replace(previous) => self.verify_binding(previous)?,
            None if next.phase() == PreparationPhase::Reserved => {}
            _ => return Err(Error::InvalidReplacement),
        }
        let current = self.read_record()?;
        if let Some(current) = &current {
            if current.bytes == bytes {
                return self.confirm_durable(guard, current, faults);
            }
        }
        if current.as_ref().map(|stored| &stored.receipt) != previous {
            return Err(Error::InvalidReplacement);
        }

        let staged_path = self.path(STAGED_JOURNAL);
        let mut staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io| {
                if io.kind() == ErrorKind::AlreadyExists {
                    Error::InterruptedWrite
                } else {
                    Error::Io
                }
            })?;
        // A restrictive umask may remove bits. Do not chmod a path that may
        // have been replaced: set permissions on our exclusively opened file.
        staged
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        faults.checkpoint(FaultPoint::Created)?;
        staged.write_all(&bytes).map_err(|_| Error::Io)?;
        faults.checkpoint(FaultPoint::Written)?;
        faults.checkpoint(FaultPoint::BeforeFileSync)?;
        staged.sync_all().map_err(|_| Error::Io)?;
        faults.checkpoint(FaultPoint::FileSynced)?;
        let staged_identity = self.file_identity(&staged.metadata().map_err(|_| Error::Io)?)?;
        if staged_identity.file.byte_len != bytes.len() as u64 {
            return Err(Error::IdentityChanged);
        }
        self.verify_guard(guard)?;
        self.validate_entries()?;
        if self.read_record()? != current {
            return Err(Error::IdentityChanged);
        }
        self.verify_path_identity(&staged_path, staged_identity)?;
        faults.checkpoint(FaultPoint::BeforeRename)?;
        // Repeat after the checkpoint so injected races exercise the same
        // identity check as a real change between durable write and rename.
        self.verify_guard(guard)?;
        self.verify_path_identity(&staged_path, staged_identity)?;
        if self.read_record()? != current {
            return Err(Error::IdentityChanged);
        }
        fs::rename(&staged_path, self.path(JOURNAL)).map_err(|_| Error::Io)?;
        faults.checkpoint(FaultPoint::Renamed)?;
        faults.checkpoint(FaultPoint::BeforeDirectorySync)?;
        sync_directory(&self.store.state_directory)?;
        faults.checkpoint(FaultPoint::DirectorySynced)?;
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        let stored = self.read_record()?.ok_or(Error::IdentityChanged)?;
        // rename may change ctime; device/inode/owner/mode/length must survive.
        if stored.bytes != bytes || stored.identity.file != staged_identity.file {
            return Err(Error::IdentityChanged);
        }
        faults.checkpoint(FaultPoint::ReadBack)?;
        Ok(())
    }

    fn confirm_durable(
        &self,
        guard: &UpgradeProcessGuard,
        current: &StoredRecord,
        faults: &impl FaultInjector,
    ) -> Result<()> {
        let path = self.path(JOURNAL);
        let file = File::open(&path).map_err(|_| Error::Io)?;
        if self.file_identity(&file.metadata().map_err(|_| Error::Io)?)? != current.identity {
            return Err(Error::IdentityChanged);
        }
        faults.checkpoint(FaultPoint::BeforeFileSync)?;
        file.sync_all().map_err(|_| Error::Io)?;
        faults.checkpoint(FaultPoint::FileSynced)?;
        self.verify_guard(guard)?;
        self.verify_path_identity(&path, current.identity)?;
        faults.checkpoint(FaultPoint::BeforeDirectorySync)?;
        sync_directory(&self.store.state_directory)?;
        faults.checkpoint(FaultPoint::DirectorySynced)?;
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        if self.read_record()?.as_ref() != Some(current) {
            return Err(Error::IdentityChanged);
        }
        faults.checkpoint(FaultPoint::ReadBack)?;
        Ok(())
    }

    fn verify_guard(&self, guard: &UpgradeProcessGuard) -> Result<()> {
        self.store.revalidate()?;
        if !guard.belongs_to(&self.store) {
            return Err(Error::IdentityChanged);
        }
        guard.revalidate()?;
        Ok(())
    }

    fn verify_binding(&self, receipt: &PreparationReceipt) -> Result<()> {
        if receipt.binding().data_root != self.data_root_identity()
            || receipt.binding().state_directory != self.state_directory_identity()
        {
            return Err(Error::IdentityChanged);
        }
        Ok(())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.store.state_directory.join(name)
    }

    fn validate_entries(&self) -> Result<()> {
        for entry in fs::read_dir(&self.store.state_directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name();
            if ![
                RECEIPT_FILE_NAME,
                STAGED_RECEIPT_FILE_NAME,
                SNAPSHOT_FILE_NAME,
                STAGED_SNAPSHOT_FILE_NAME,
                CANDIDATE_FILE_NAME,
                STAGED_CANDIDATE_FILE_NAME,
                SOURCE_BACKUP_FILE_NAME,
                SETTINGS_BACKUP_FILE_NAME,
                STAGED_SETTINGS_BACKUP_FILE_NAME,
                JOURNAL,
                STAGED_JOURNAL,
                PREPARATION_SNAPSHOT,
                STAGED_PREPARATION_SNAPSHOT,
            ]
            .iter()
            .any(|allowed| name == *allowed)
            {
                return Err(Error::UnexpectedObject);
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| Error::Io)?;
            let identity = FileIdentity::from_metadata(&metadata);
            if !metadata.file_type().is_file()
                || identity.device_id != self.store.root.identity.device_id()
                || identity.owner_id != self.store.root.expected_owner_id
                || identity.mode != 0o600
                || identity.link_count != 1
            {
                return Err(Error::IdentityChanged);
            }
        }
        Ok(())
    }

    fn reject_staged_record(&self) -> Result<()> {
        for name in [
            STAGED_JOURNAL,
            STAGED_RECEIPT_FILE_NAME,
            STAGED_SNAPSHOT_FILE_NAME,
            STAGED_CANDIDATE_FILE_NAME,
            STAGED_SETTINGS_BACKUP_FILE_NAME,
            STAGED_PREPARATION_SNAPSHOT,
        ] {
            if path_exists(&self.path(name))? {
                return Err(Error::InterruptedWrite);
            }
        }
        Ok(())
    }

    fn read_record(&self) -> Result<Option<StoredRecord>> {
        let path = self.path(JOURNAL);
        let before = match fs::symlink_metadata(&path) {
            Ok(metadata) => self.file_identity(&metadata)?,
            Err(io) if io.kind() == ErrorKind::NotFound => {
                if path_exists(&self.path(PREPARATION_SNAPSHOT))? {
                    return Err(Error::UnexpectedObject);
                }
                return Ok(None);
            }
            Err(_) => return Err(Error::Io),
        };
        let mut file = File::open(&path).map_err(|_| Error::Io)?;
        if self.file_identity(&file.metadata().map_err(|_| Error::Io)?)? != before {
            return Err(Error::IdentityChanged);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_PREPARATION_RECEIPT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Io)?;
        if bytes.len() as u64 != before.file.byte_len
            || self.file_identity(&file.metadata().map_err(|_| Error::Io)?)? != before
        {
            return Err(Error::IdentityChanged);
        }
        self.verify_path_identity(&path, before)?;
        let receipt = PreparationReceipt::decode(&bytes).map_err(|_| Error::InvalidRecord)?;
        self.verify_binding(&receipt)?;
        Ok(Some(StoredRecord {
            receipt,
            bytes,
            identity: before,
        }))
    }

    fn file_identity(&self, metadata: &Metadata) -> Result<JournalIdentity> {
        let identity = FileIdentity::from_metadata(metadata);
        if !metadata.file_type().is_file()
            || identity.device_id != self.store.root.identity.device_id()
            || identity.owner_id != self.store.root.expected_owner_id
            || identity.mode != 0o600
            || identity.link_count != 1
        {
            return Err(Error::IdentityChanged);
        }
        if identity.byte_len > MAX_PREPARATION_RECEIPT_BYTES as u64 {
            return Err(Error::InvalidRecord);
        }
        Ok(JournalIdentity {
            file: identity,
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }

    fn verify_path_identity(&self, path: &Path, expected: JournalIdentity) -> Result<()> {
        let metadata = fs::symlink_metadata(path).map_err(|_| Error::IdentityChanged)?;
        if self.file_identity(&metadata)? != expected {
            return Err(Error::IdentityChanged);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JournalIdentity {
    file: FileIdentity,
    modified: (i64, i64),
    changed: (i64, i64),
}

#[derive(Debug, PartialEq, Eq)]
struct StoredRecord {
    receipt: PreparationReceipt,
    bytes: Vec<u8>,
    identity: JournalIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultPoint {
    Created,
    Written,
    BeforeFileSync,
    FileSynced,
    BeforeRename,
    Renamed,
    BeforeDirectorySync,
    DirectorySynced,
    ReadBack,
}

trait FaultInjector {
    fn checkpoint(&self, _point: FaultPoint) -> Result<()> {
        Ok(())
    }
}

struct NoFaults;
impl FaultInjector for NoFaults {}

#[cfg(test)]
#[path = "preparation_journal_tests.rs"]
mod tests;

#[path = "source_preparation.rs"]
mod source_preparation;
pub use source_preparation::{
    PreparationHasher, SourcePreparationCheckpoint, SourcePreparationError, SourcePreparationPort,
    SourcePreparationSpaceBudget,
};
