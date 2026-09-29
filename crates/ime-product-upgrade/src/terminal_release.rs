//! Guard-bound terminal preservation. Product finalization is a separate step.
use super::*;
use crate::PreparationFileIdentity;

const FORMAT: &str = "radishlex-terminal-release-v1";
const INDEX_FORMAT: &str = "radishlex-latest-release-v1";
const RELEASE: &str = "terminal-release.json";
const RELEASE_TEMP: &str = "terminal-release.json.tmp";
const INDEX: &str = "latest-release.json";
const INDEX_TEMP: &str = "latest-release.json.tmp";
const PREPARATION: [&str; 2] = ["preparation.json", PREPARATION_SNAPSHOT];

#[path = "terminal_release_record.rs"]
mod record;
use record::{decode, encode, valid_id, PreviousIndex, ReleaseIndex};
pub use record::{TerminalReleaseBinding, TerminalReleasePhase, TerminalReleaseReceipt};
#[path = "terminal_release_evidence.rs"]
mod evidence;
#[path = "terminal_release_history.rs"]
mod history_reader;
use history_reader::ReleaseHistory;
#[path = "terminal_release_initialization.rs"]
mod initialization;
#[path = "terminal_release_persistence.rs"]
mod persistence;
#[path = "preparation_released_predecessor.rs"]
mod released_predecessor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalReleaseDirectory {
    HistoryRoot,
    Operation,
    Data,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalReleaseCheckpoint {
    Begin,
    DirectoryCreated(TerminalReleaseDirectory),
    DirectorySynced(TerminalReleaseDirectory),
    ParentSynced(TerminalReleaseDirectory),
    IntentRecorded,
    RecordCreated,
    RecordWritten,
    RecordFileSynced,
    RecordRenamed,
    RecordDirectorySynced,
    BeforeMove(Slot),
    FileSynced(Slot),
    Moved(Slot),
    TargetSynced(Slot),
    SourceSynced(Slot),
    SlotRecorded(Slot),
    ReadyRecorded,
    BeforeIndexWrite,
    IndexCreated,
    IndexWritten,
    IndexFileSynced,
    IndexRenamed,
    IndexDirectorySynced,
    IndexRecorded,
    BeforeMarkerMove,
    MarkerMoved,
    MarkerTargetSynced,
    MarkerSourceSynced,
    Released,
}
use TerminalReleaseCheckpoint as Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalReleaseOuterRequirement {
    Nonterminal,
    MatchingTerminal,
}

/// The implementation must hold the outer guard and freshly prove the exact
/// installed products, data terminal result and quiescence at EVERY callback.
/// Nonterminal reserves finalization; MatchingTerminal proves outer finalization
/// against this release proof. A caller-supplied flag is not that proof.
/// Initialization callbacks carry an observed draft; only ReleaseReady supplies
/// the final canonical proof to bind in the durable outer result.
pub trait TerminalReleasePort {
    fn confirm_authority_and_quiescence(
        &mut self,
        release: &TerminalReleaseReceipt,
        checkpoint: TerminalReleaseCheckpoint,
        requirement: TerminalReleaseOuterRequirement,
    ) -> bool;
}

/// Recognizes only release slots, never ignores preparation or unknown objects.
pub struct TerminalReleaseStore {
    journal: PreparationJournalStore,
}

impl TerminalReleaseStore {
    fn history(&self) -> ReleaseHistory<'_> {
        ReleaseHistory {
            journal: &self.journal,
        }
    }
    pub fn open_existing(root: VerifiedDataRoot) -> Result<Self> {
        let value = Self {
            journal: PreparationJournalStore {
                store: UpgradeReceiptStore::open_existing_directory(root)?,
            },
        };
        value.validate_active_entries()?;
        Ok(value)
    }
    pub fn attach(store: UpgradeReceiptStore, guard: &UpgradeProcessGuard) -> Result<Self> {
        let value = Self {
            journal: PreparationJournalStore { store },
        };
        value.journal.verify_guard(guard)?;
        value.validate_active_entries()?;
        Ok(value)
    }
    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard> {
        self.validate_active_entries()?;
        Ok(self.journal.store.acquire_directory_guard()?)
    }

    /// Archive a real terminal v1 result and publish its locator while retaining
    /// the active marker. This does NOT finalize outer or authorize startup.
    pub fn prepare_terminal_release(
        &self,
        guard: &UpgradeProcessGuard,
        binding: &TerminalReleaseBinding,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<TerminalReleaseReceipt> {
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        let mut record = if path_exists(&self.active(RELEASE))? {
            self.history().read_release(&self.active(RELEASE), hasher)?
        } else {
            self.initialize(guard, binding, port, hasher)?
        };
        if record.binding() != binding {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::IntentRecorded,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        self.persist_record(guard, Some(&record), &record, port, hasher)?;
        if record.phase == TerminalReleasePhase::Reserved {
            for (index, slot) in SLOTS.iter().copied().enumerate() {
                let Some(identity) = &record.files[index] else {
                    continue;
                };
                if record.archived_slots.contains(&slot) {
                    continue;
                }
                let identity = identity.clone();
                let source = self.active(slot_name(slot));
                let target = self.paths(&record)[2].join(slot_name(slot));
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::BeforeMove(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
                let location = self.unique_location(&source, &target)?;
                self.sync_file(&location, &identity, hasher)?;
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::FileSynced(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
                if location == source {
                    if path_exists(&target)? || self.journal.evidence(&source, hasher)? != identity
                    {
                        return Err(Error::EvidenceChanged);
                    }
                    fs::rename(&source, &target).map_err(|_| Error::Io)?;
                }
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::Moved(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
                sync_directory(&self.paths(&record)[2])?;
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::TargetSynced(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
                sync_directory(&self.journal.store.state_directory)?;
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::SourceSynced(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
                let mut next = record.clone();
                next.archived_slots.push(slot);
                self.persist_record(guard, Some(&record), &next, port, hasher)?;
                record = next;
                self.checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    Point::SlotRecorded(slot),
                    TerminalReleaseOuterRequirement::Nonterminal,
                )?;
            }
            let mut next = record.clone();
            next.phase = TerminalReleasePhase::ReleaseReady;
            self.persist_record(guard, Some(&record), &next, port, hasher)?;
            record = next;
        }
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::ReadyRecorded,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        self.publish_index(guard, port, hasher, &record)?;
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::IndexRecorded,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        Ok(record)
    }

    /// Call ONLY after the outer coordinator durably finalized against the
    /// ready proof and exact installed products. Marker is the last moved slot.
    pub fn finish_terminal_release(
        self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<(UpgradeReceiptStore, TerminalReleaseReceipt)> {
        if !valid_id(operation) {
            return Err(Error::EvidenceChanged);
        }
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        let history = self
            .journal
            .store
            .root
            .path
            .join(HISTORY)
            .join(operation)
            .join(RELEASE);
        let location = self.unique_location(&self.active(RELEASE), &history)?;
        let record = self.history().read_release(&location, hasher)?;
        if record.phase != TerminalReleasePhase::ReleaseReady
            || record.binding.operation_id != operation
        {
            return Err(Error::InvalidPhase);
        }
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::BeforeMarkerMove,
            TerminalReleaseOuterRequirement::MatchingTerminal,
        )?;
        self.history().require_published_index(&record, hasher)?;
        let identity = self.journal.evidence(&location, hasher)?;
        self.sync_file(&location, &identity, hasher)?;
        if location != history {
            if path_exists(&history)? {
                return Err(Error::EvidenceChanged);
            }
            fs::rename(&location, &history).map_err(|_| Error::Io)?;
        }
        if self.journal.evidence(&history, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::MarkerMoved,
            TerminalReleaseOuterRequirement::MatchingTerminal,
        )?;
        sync_directory(history.parent().ok_or(Error::EvidenceChanged)?)?;
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::MarkerTargetSynced,
            TerminalReleaseOuterRequirement::MatchingTerminal,
        )?;
        sync_directory(&self.journal.store.state_directory)?;
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::MarkerSourceSynced,
            TerminalReleaseOuterRequirement::MatchingTerminal,
        )?;
        self.checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::Released,
            TerminalReleaseOuterRequirement::MatchingTerminal,
        )?;
        if self.journal.store.load_guarded(guard)?.is_some() {
            return Err(Error::EvidenceChanged);
        }
        Ok((self.journal.store, record))
    }

    /// Historical locator lookup, NOT startup permission. Current runtime data
    /// may have legitimately resumed WAL/learning. Verify frozen proof/artifacts.
    pub fn load_latest_release(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<TerminalReleaseReceipt>> {
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        let index = self.history().load_index(hasher)?;
        let result = index
            .as_ref()
            .map(|value| self.history().resolve_index(&value.index, hasher))
            .transpose()?;
        if self.history().load_index(hasher)? != index {
            return Err(Error::EvidenceChanged);
        }
        self.validate_active_entries()?;
        self.journal.verify_guard(guard)?;
        Ok(result)
    }

    fn checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
        record: &TerminalReleaseReceipt,
        point: Point,
        requirement: TerminalReleaseOuterRequirement,
    ) -> Result<()> {
        self.verify_live(guard, record, hasher)?;
        let path =
            self.unique_location(&self.active(RELEASE), &self.paths(record)[1].join(RELEASE))?;
        let identity = self.journal.evidence(&path, hasher)?;
        if !port.confirm_authority_and_quiescence(record, point, requirement) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_live(guard, record, hasher)?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    fn active(&self, name: &str) -> PathBuf {
        self.journal.store.state_directory.join(name)
    }
    fn paths(&self, record: &TerminalReleaseReceipt) -> [PathBuf; 3] {
        let root = self.journal.store.root.path.join(HISTORY);
        let operation = root.join(&record.binding.operation_id);
        let data = operation.join("data");
        [root, operation, data]
    }
    fn unique_location(&self, source: &Path, target: &Path) -> Result<PathBuf> {
        match (path_exists(source)?, path_exists(target)?) {
            (true, false) => Ok(source.to_owned()),
            (false, true) => Ok(target.to_owned()),
            _ => Err(Error::EvidenceChanged),
        }
    }
}
