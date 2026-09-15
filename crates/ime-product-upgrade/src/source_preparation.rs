//! Guard-bound SQLite preparation. Product authority is supplied by the
//! outer coordinator; paths, physical evidence and durable order are enforced here.
use super::*;
use crate::PreparationFamily;
use radishlex_ime_userdb::{UserDb, UserDbMaintenanceError};

#[path = "preparation_files.rs"]
mod files;
use files::{no_sidecars, readonly_family, same_object};

/// A trusted platform implementation of SHA-256 over the complete stream.
/// Implementations must propagate read failures and never return a partial hash.
pub trait PreparationHasher {
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreparationCheckpoint {
    Begin,
    SnapshotEstimated,
    SnapshotCreated,
    SnapshotCopied,
    SnapshotSynced,
    SnapshotRenamed,
    SnapshotDirectorySynced,
    SnapshotRecorded,
    MaintenanceIntentRecorded,
    BeforeMaintenance,
    SourceMaintained,
    SourceFileSynced,
    SourceSynced,
    SourceRecorded,
}

/// The product implementation must own and revalidate the outer install guard,
/// exact matching prepared outer operation, sealed source/target products and
/// previous receipt/inventory at EVERY checkpoint, then freshly prove quiescence.
/// No UI-provided observations or cached authorization may implement this port.
pub trait SourcePreparationPort {
    fn confirm_authority_and_quiescence(
        &mut self,
        receipt: &PreparationReceipt,
        checkpoint: SourcePreparationCheckpoint,
    ) -> bool;
    /// Fresh capacity for the fixed authoritative data root, never UI input.
    fn available_bytes(&mut self) -> Option<u64>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreparationError {
    Journal(PreparationJournalError),
    Maintenance(UserDbMaintenanceError),
    InvalidPhase,
    EvidenceChanged,
    AuthorityNotProven,
    InsufficientSpace,
    SnapshotFailed,
    Io,
}
impl fmt::Display for SourcePreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Journal(_) => "source preparation journal rejected",
            Self::Maintenance(_) => "source preparation maintenance rejected",
            Self::InvalidPhase => "source preparation phase rejected",
            Self::EvidenceChanged => "source preparation evidence changed",
            Self::AuthorityNotProven => "source preparation authority or quiescence not proven",
            Self::InsufficientSpace => "source preparation space not proven",
            Self::SnapshotFailed => "source preparation snapshot failed",
            Self::Io => "source preparation filesystem operation failed",
        })
    }
}
impl std::error::Error for SourcePreparationError {}
impl From<PreparationJournalError> for SourcePreparationError {
    fn from(value: PreparationJournalError) -> Self {
        Self::Journal(value)
    }
}
impl From<UpgradeFilesystemError> for SourcePreparationError {
    fn from(value: UpgradeFilesystemError) -> Self {
        Self::Journal(value.into())
    }
}
impl From<UserDbMaintenanceError> for SourcePreparationError {
    fn from(value: UserDbMaintenanceError) -> Self {
        Self::Maintenance(value)
    }
}
use SourcePreparationError as Error;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePreparationSpaceBudget {
    pub required_available_bytes: u64,
}
impl SourcePreparationSpaceBudget {
    /// Three existing logical working copies, one protection snapshot, one
    /// source-growth allowance, one journal allowance, settings and 64 MiB.
    /// Use max(logical size, physical main size), so a large main is not hidden
    /// by a smaller logical estimate. Existing archive renames cost no copy.
    pub fn evaluate(
        database_bytes: u64,
        settings_bytes: u64,
        available_bytes: u64,
    ) -> Result<Self> {
        let required = database_bytes
            .checked_mul(6)
            .and_then(|bytes| bytes.checked_add(settings_bytes))
            .and_then(|bytes| bytes.checked_add(64 * 1024 * 1024))
            .ok_or(Error::InsufficientSpace)?;
        if available_bytes < required {
            return Err(Error::InsufficientSpace);
        }
        Ok(Self {
            required_available_bytes: required,
        })
    }
}

impl PreparationJournalStore {
    /// Read and hash only the fixed source family for a new reservation. The
    /// outer caller must already prove source authority/quiescence and persist
    /// its previous-operation inventory before creating the reserved journal.
    pub fn observe_source_family(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationFamily> {
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        let family = self.family(hasher)?;
        self.verify_guard(guard)?;
        Ok(family)
    }

    /// Advances a durable reservation through source_prepared, preserving the
    /// preparation marker and every existing v1 slot. This is NOT v1 handoff.
    /// Any error may leave SQLite physical changes or private interrupted files.
    pub fn prepare_userdb_source(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationReceipt> {
        let mut record = self.load_guarded(guard)?.ok_or(Error::InvalidPhase)?;
        if record.phase() > PreparationPhase::SourcePrepared
            || record.binding().target_schema_version != UserDb::supported_schema_version()
        {
            return Err(Error::InvalidPhase);
        }
        self.checkpoint(guard, port, &record, SourcePreparationCheckpoint::Begin)?;
        // A readable marker following an interrupted rename is not sufficient:
        // finish its durability before any SQLite call can change the source.
        self.persist(guard, Some(&record), &record)?;
        if record.phase() == PreparationPhase::Reserved {
            record = self.protect_source(guard, port, hasher, &record)?;
        }
        if record.phase() == PreparationPhase::SnapshotReady {
            self.verify_snapshot(&record, hasher)?;
            if record.maintenance_source() != Some(&self.family(hasher)?) {
                return Err(Error::EvidenceChanged);
            }
            let mut next = record.clone();
            next.begin_maintenance().map_err(|_| Error::InvalidPhase)?;
            self.persist(guard, Some(&record), &next)?;
            record = next;
            self.checkpoint(
                guard,
                port,
                &record,
                SourcePreparationCheckpoint::MaintenanceIntentRecorded,
            )?;
        }
        if record.phase() == PreparationPhase::MaintenanceIntent {
            record = self.maintain_source(guard, port, hasher, &record)?;
        }
        self.verify_prepared(&record, hasher)?;
        self.checkpoint(
            guard,
            port,
            &record,
            SourcePreparationCheckpoint::SourceRecorded,
        )?;
        self.verify_prepared(&record, hasher)?;
        if self.load_guarded(guard)?.as_ref() != Some(&record) {
            return Err(Error::EvidenceChanged);
        }
        Ok(record)
    }

    fn checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        record: &PreparationReceipt,
        checkpoint: SourcePreparationCheckpoint,
    ) -> Result<()> {
        self.verify_guard(guard)?;
        self.validate_entries()?;
        // During backup our exclusively created snapshot temp is expected;
        // all other interrupted writes remain unconditionally blocking.
        for name in [
            STAGED_JOURNAL,
            STAGED_RECEIPT_FILE_NAME,
            STAGED_SNAPSHOT_FILE_NAME,
            STAGED_CANDIDATE_FILE_NAME,
            STAGED_SETTINGS_BACKUP_FILE_NAME,
        ] {
            if path_exists(&self.path(name))? {
                return Err(Error::Journal(PreparationJournalError::InterruptedWrite));
            }
        }
        if self.read_record()?.as_ref().map(|value| &value.receipt) != Some(record) {
            return Err(Error::EvidenceChanged);
        }
        if !port.confirm_authority_and_quiescence(record, checkpoint) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_guard(guard)?;
        if self.read_record()?.as_ref().map(|value| &value.receipt) != Some(record) {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    fn protect_source(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
    ) -> Result<PreparationReceipt> {
        let source = self.source_path();
        let staged = self.path(STAGED_PREPARATION_SNAPSHOT);
        let snapshot = self.path(PREPARATION_SNAPSHOT);
        if path_exists(&staged)? || path_exists(&snapshot)? {
            return Err(Error::EvidenceChanged);
        }
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        let estimate = UserDb::estimate_snapshot(&source).map_err(|_| Error::SnapshotFailed)?;
        if estimate.schema_version < 0
            || estimate.schema_version > record.binding().target_schema_version
        {
            return Err(Error::SnapshotFailed);
        }
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotEstimated,
        )?;
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        self.budget(estimate.logical_size_bytes, port)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotEstimated,
        )?;
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        let initial = self.evidence(&staged, hasher)?;
        let opened = file.metadata().map_err(|_| Error::Io)?;
        if opened.dev() != initial.device_id || opened.ino() != initial.inode {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotCreated,
        )?;
        if self.evidence(&staged, hasher)? != initial {
            return Err(Error::EvidenceChanged);
        }
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        let summary = UserDb::create_consistent_snapshot(&source, &staged)
            .map_err(|_| Error::SnapshotFailed)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotCopied,
        )?;
        let schema = UserDb::verify_maintenance_snapshot(&source, &staged)?;
        if schema != summary.source_schema_version || schema != estimate.schema_version {
            return Err(Error::EvidenceChanged);
        }
        no_sidecars(&staged)?;
        let copied = self.evidence(&staged, hasher)?;
        if !same_object(&copied, &initial) {
            return Err(Error::EvidenceChanged);
        }
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotSynced,
        )?;
        if self.evidence(&staged, hasher)? != copied || path_exists(&snapshot)? {
            return Err(Error::EvidenceChanged);
        }
        readonly_family(record.initial_source(), &self.family(hasher)?)?;
        fs::rename(&staged, &snapshot).map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotRenamed,
        )?;
        sync_directory(&self.store.state_directory)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SnapshotDirectorySynced,
        )?;
        if self.evidence(&snapshot, hasher)? != copied {
            return Err(Error::EvidenceChanged);
        }
        let family = self.family(hasher)?;
        readonly_family(record.initial_source(), &family)?;
        let mut next = record.clone();
        next.record_snapshot(copied, family, schema)
            .map_err(|_| Error::EvidenceChanged)?;
        self.persist(guard, Some(record), &next)?;
        self.checkpoint(
            guard,
            port,
            &next,
            SourcePreparationCheckpoint::SnapshotRecorded,
        )?;
        Ok(next)
    }

    fn maintain_source(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
    ) -> Result<PreparationReceipt> {
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::BeforeMaintenance,
        )?;
        self.verify_snapshot(record, hasher)?;
        let family = self.family(hasher)?;
        if !same_object(&family.database, &record.initial_source().database) {
            return Err(Error::EvidenceChanged);
        }
        // An intent permits SQLite page/WAL changes, never replacement of the
        // main inode or adoption of a nonempty WAL belonging to another inode.
        if let Some(wal) = &family.wal {
            let original = record
                .maintenance_source()
                .and_then(|value| value.wal.as_ref());
            if wal.byte_len != 0 && original.map_or(true, |value| !same_object(value, wal)) {
                return Err(Error::EvidenceChanged);
            }
        }
        self.budget(
            record
                .snapshot_identity()
                .ok_or(Error::EvidenceChanged)?
                .byte_len,
            port,
        )?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::BeforeMaintenance,
        )?;
        self.verify_snapshot(record, hasher)?;
        if self.family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        let source = self.source_path();
        let snapshot = self.path(PREPARATION_SNAPSHOT);
        if Some(UserDb::verify_maintenance_snapshot(&source, &snapshot)?)
            != record.snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::BeforeMaintenance,
        )?;
        self.verify_snapshot(record, hasher)?;
        readonly_family(&family, &self.family(hasher)?)?;
        let summary = UserDb::prepare_source_for_upgrade(&source, &snapshot)?;
        if Some(summary.schema_version) != record.snapshot_schema_version() {
            return Err(Error::EvidenceChanged);
        }
        let prepared = self.family(hasher)?;
        no_sidecars(&source)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SourceMaintained,
        )?;
        if self.family(hasher)? != prepared || !same_object(&prepared.database, &family.database) {
            return Err(Error::EvidenceChanged);
        }
        self.verify_snapshot(record, hasher)?;
        if UserDb::verify_prepared_source(&source, &snapshot)? != summary.schema_version {
            return Err(Error::EvidenceChanged);
        }
        File::open(&source)
            .and_then(|file| file.sync_all())
            .map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SourceFileSynced,
        )?;
        sync_directory(&self.store.root.path)?;
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::SourceSynced,
        )?;
        self.verify_snapshot(record, hasher)?;
        if self.family(hasher)? != prepared {
            return Err(Error::EvidenceChanged);
        }
        let mut next = record.clone();
        next.record_prepared_source(prepared.database)
            .map_err(|_| Error::EvidenceChanged)?;
        self.persist(guard, Some(record), &next)?;
        Ok(next)
    }

    fn verify_prepared(
        &self,
        record: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify_snapshot(record, hasher)?;
        let family = self.family(hasher)?;
        no_sidecars(&self.source_path())?;
        if record.prepared_source() != Some(&family.database) {
            return Err(Error::EvidenceChanged);
        }
        let schema =
            UserDb::verify_prepared_source(self.source_path(), self.path(PREPARATION_SNAPSHOT))?;
        if Some(schema) != record.snapshot_schema_version() {
            return Err(Error::EvidenceChanged);
        }
        self.verify_snapshot(record, hasher)?;
        if self.family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
