//! Cancellation-only preservation. The active cancellation request is never
//! removed here; compatibility restoration does not grant startup permission.
use super::*;
use crate::PreparationFileIdentity;

const ARCHIVE: &str = "cancellation-archive.json";
const TEMP: &str = "cancellation-archive.json.tmp";
const REQUEST: &str = "preparation-cancellation.json";
const SOURCE: &str = "cancellation-source.json";
const OUTER_STATE: &str = ".radishlex-install-v1";
const OUTER_HISTORY: &str = ".radishlex-install-history-v1";
const PREVIOUS_OUTER: &str = "receipt.json";
const NEW_OUTER: &str = "cancelled-outer.json";
const COMPATIBILITY: &str = "compatibility-outer.json";

#[path = "cancellation_archive_record.rs"]
mod record;
pub use record::{CancellationArchiveReceipt, CancellationArchiveSlot, CancellationOuterBinding};
use CancellationArchiveSlot as ArchiveSlot;
#[path = "cancellation_archive_evidence.rs"]
mod evidence;
#[path = "cancellation_archive_initialization.rs"]
mod initialization;
#[path = "cancellation_archive_persistence.rs"]
mod persistence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationArchiveCheckpoint {
    Begin,
    DirectoryCreated,
    DirectorySynced,
    ParentSynced,
    Record(PreparationCancellationCheckpoint),
    IntentRecorded,
    BeforeMove(ArchiveSlot),
    FileSynced(ArchiveSlot),
    Moved(ArchiveSlot),
    TargetSynced(ArchiveSlot),
    SourceSynced(ArchiveSlot),
    SlotRecorded(ArchiveSlot),
    BeforeCompatibilityCreate,
    CompatibilityCreated,
    CompatibilityWritten,
    CompatibilityFileSynced,
    CompatibilityDirectorySynced,
    CompatibilityBound,
    BeforeCompatibilityPublish,
    CompatibilityPublished,
    CompatibilityTargetSynced,
    CompatibilitySourceSynced,
    Preserved,
}
use CancellationArchiveCheckpoint as Point;

/// Must hold the outer install guard BEFORE the supplied upgrade guard and
/// revalidate it at EVERY callback. Validate previous/new canonical receipts
/// with the install core, including terminal/source identity, root, exact IDs,
/// artifact-free Prepared new outer, and fresh installed programs/quiescence.
/// Verify the callback's original bound receipts, not an assumed active slot:
/// that slot can legitimately be absent or hold the compatibility projection.
/// Success is authority for this bounded step only, never for marker release.
pub trait CancellationArchivePort {
    fn confirm_authority_and_quiescence(
        &mut self,
        record: &CancellationArchiveReceipt,
        checkpoint: CancellationArchiveCheckpoint,
    ) -> bool;
}

pub struct CancellationArchiveStore {
    journal: PreparationJournalStore,
}

impl CancellationArchiveStore {
    pub fn open_existing(root: VerifiedDataRoot) -> Result<Self> {
        let value = Self {
            journal: PreparationJournalStore {
                store: UpgradeReceiptStore::open_existing_directory(root)?,
            },
        };
        value.validate_entries(None)?;
        Ok(value)
    }

    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard> {
        self.validate_entries(None)?;
        Ok(self.journal.store.acquire_directory_guard()?)
    }

    /// Capture is read-only. The prior outer must already be sealed at the
    /// fixed install-history slot. No history scan or inferred predecessor.
    pub fn observe_outer(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationOuterBinding> {
        self.journal.verify_guard(guard)?;
        if path_exists(&self.active(ARCHIVE))? {
            return Err(Error::InvalidPhase);
        }
        let request =
            PreparationCancellationRequest::decode(&self.read_bytes(&self.active(REQUEST))?)?;
        let previous = request
            .preparation()
            .binding()
            .previous_install_operation_id
            .as_deref()
            .ok_or(Error::EvidenceChanged)?;
        let root = &self.journal.store.root.path;
        let history = root.join(OUTER_HISTORY);
        let directory = history.join(previous);
        let path = directory.join(PREVIOUS_OUTER);
        let original = self.journal.evidence(&path, hasher)?;
        let previous_bytes =
            String::from_utf8(self.read_bytes(&path)?).map_err(|_| Error::EvidenceChanged)?;
        if self.journal.evidence(&path, hasher)? != original {
            return Err(Error::EvidenceChanged);
        }
        let active_path = root.join(OUTER_STATE).join(RECEIPT_FILE_NAME);
        let active_identity = self.journal.evidence(&active_path, hasher)?;
        let active_bytes = String::from_utf8(self.read_bytes(&active_path)?)
            .map_err(|_| Error::EvidenceChanged)?;
        if self.journal.evidence(&active_path, hasher)? != active_identity {
            return Err(Error::EvidenceChanged);
        }
        let result = CancellationOuterBinding {
            state_directory: self.journal.history_directory(&root.join(OUTER_STATE))?,
            history_root: self.journal.history_directory(&history)?,
            previous_directory: self.journal.history_directory(&directory)?,
            previous_identity: original,
            previous_bytes,
            active_identity,
            new_bytes: None,
        };
        let result = if active_bytes != result.previous_bytes {
            CancellationOuterBinding {
                new_bytes: Some(active_bytes),
                ..result
            }
        } else {
            result
        };
        self.verify_outer(&request, &result, None, false, false, hasher)?;
        self.journal.verify_guard(guard)?;
        Ok(result)
    }

    pub fn load_guarded(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<CancellationArchiveReceipt>> {
        self.journal.verify_guard(guard)?;
        self.validate_entries(None)?;
        let Some((record, identity)) = self.read_record(hasher)? else {
            return Ok(None);
        };
        self.verify(guard, &record, hasher, None)?;
        if self.journal.evidence(&self.active(ARCHIVE), hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some(record))
    }

    /// Preserve material and restore the old outer projection. Keeps REQUEST
    /// and ARCHIVE active; v2 lifecycle publication/release is a later protocol.
    pub fn preserve_cancellation(
        &self,
        guard: &UpgradeProcessGuard,
        outer: &CancellationOuterBinding,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationArchiveReceipt> {
        self.journal.verify_guard(guard)?;
        self.validate_entries(None)?;
        let mut record = match self.read_record(hasher)? {
            Some((record, _)) => record,
            None => self.initialize(guard, outer, port, hasher)?,
        };
        if &record.outer != outer {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(guard, &record, port, hasher, Point::IntentRecorded)?;
        self.verify_source_equivalence(&record, hasher)?;
        self.persist(guard, Some(&record), &record, port, hasher)?;
        for slot in record.expected_slots() {
            if record.moved.contains(&slot) {
                continue;
            }
            self.checkpoint(guard, &record, port, hasher, Point::BeforeMove(slot))?;
            let (source, target, identity) = self.slot(&record, slot)?;
            let location = self.unique(&source, &target)?;
            self.sync_file(&location, identity, hasher)?;
            self.checkpoint(guard, &record, port, hasher, Point::FileSynced(slot))?;
            if location == source {
                if path_exists(&target)? {
                    return Err(Error::EvidenceChanged);
                }
                fs::rename(&source, &target).map_err(|_| Error::Io)?;
            }
            self.checkpoint(guard, &record, port, hasher, Point::Moved(slot))?;
            sync_directory(target.parent().ok_or(Error::EvidenceChanged)?)?;
            self.checkpoint(guard, &record, port, hasher, Point::TargetSynced(slot))?;
            sync_directory(source.parent().ok_or(Error::EvidenceChanged)?)?;
            self.checkpoint(guard, &record, port, hasher, Point::SourceSynced(slot))?;
            let mut next = record.clone();
            next.moved.push(slot);
            self.persist(guard, Some(&record), &next, port, hasher)?;
            record = next;
            self.checkpoint(guard, &record, port, hasher, Point::SlotRecorded(slot))?;
        }
        self.restore_compatibility(guard, &mut record, port, hasher)?;
        if !record.preserved {
            let mut next = record.clone();
            next.preserved = true;
            self.persist(guard, Some(&record), &next, port, hasher)?;
            record = next;
        }
        self.checkpoint(guard, &record, port, hasher, Point::Preserved)?;
        Ok(record)
    }

    fn checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        record: &CancellationArchiveReceipt,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
        point: Point,
    ) -> Result<()> {
        self.verify(guard, record, hasher, None)?;
        let stored = self.read_record(hasher)?.ok_or(Error::EvidenceChanged)?;
        if &stored.0 != record {
            return Err(Error::EvidenceChanged);
        }
        if !port.confirm_authority_and_quiescence(record, point) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify(guard, record, hasher, None)?;
        if self.read_record(hasher)?.as_ref() != Some(&stored) {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    fn active(&self, name: &str) -> PathBuf {
        self.journal.path(name)
    }
    fn directory(&self, record: &CancellationArchiveReceipt) -> PathBuf {
        self.journal
            .store
            .root
            .path
            .join(HISTORY)
            .join(&record.request.preparation().binding().operation_id)
    }
    fn unique(&self, source: &Path, target: &Path) -> Result<PathBuf> {
        match (path_exists(source)?, path_exists(target)?) {
            (true, false) => Ok(source.to_owned()),
            (false, true) => Ok(target.to_owned()),
            _ => Err(Error::EvidenceChanged),
        }
    }
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        if self.journal.private_metadata(path)?.len() > MAX_PREPARATION_RECEIPT_BYTES as u64 {
            return Err(Error::EvidenceChanged);
        }
        self.journal.read_history_bytes(path)
    }
    fn sync_file(
        &self,
        path: &Path,
        identity: &PreparationFileIdentity,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self.journal.evidence(path, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        let file = File::open(path).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != identity.device_id || metadata.ino() != identity.inode {
            return Err(Error::EvidenceChanged);
        }
        file.sync_all().map_err(|_| Error::Io)?;
        if self.journal.evidence(path, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
