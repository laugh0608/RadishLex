//! Preserve old private slots without moving the active state directory or data.
use super::*;
use crate::{PreparationArchiveSlot as Slot, PreparationDirectoryIdentity};

#[path = "preparation_inventory.rs"]
mod inventory;
pub use inventory::PreviousInventory;
use inventory::{expected_private, matches_artifact, slot_name, HISTORY, INVENTORY, SLOTS};

#[path = "preparation_handoff.rs"]
mod handoff;

#[path = "terminal_release.rs"]
mod terminal_release;
pub use terminal_release::{
    TerminalReleaseBinding, TerminalReleaseCheckpoint, TerminalReleaseDirectory,
    TerminalReleaseOuterRequirement, TerminalReleasePhase, TerminalReleasePort,
    TerminalReleaseReceipt, TerminalReleaseStore,
};

impl PreparationJournalStore {
    /// Exact, guard-bound continuation from source_prepared. The preparation
    /// marker remains active; this neither creates v1 state nor allows startup.
    pub fn archive_previous_upgrade(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationReceipt> {
        let mut record = self.load_guarded(guard)?.ok_or(Error::InvalidPhase)?;
        if !matches!(
            record.phase(),
            PreparationPhase::SourcePrepared | PreparationPhase::PreviousArchived
        ) {
            return Err(Error::InvalidPhase);
        }
        let inventory = self.archive_checkpoint(
            guard,
            port,
            hasher,
            &record,
            SourcePreparationCheckpoint::ArchiveBegin,
        )?;
        self.persist(guard, Some(&record), &record)?;
        if record.phase() == PreparationPhase::PreviousArchived {
            return Ok(record);
        }
        if inventory.receipt.is_some() {
            let paths = inventory.paths(self)?;
            for (index, slot) in SLOTS.iter().copied().enumerate() {
                let Some(identity) = &inventory.files[index] else {
                    continue;
                };
                if record.archived_slots().contains(&slot) {
                    continue;
                }
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveBeforeFileSync(slot),
                )?;
                let source = self.path(slot_name(slot));
                let target = paths[2].join(slot_name(slot));
                let location = if path_exists(&source)? {
                    &source
                } else {
                    &target
                };
                let file = File::open(location).map_err(|_| Error::Io)?;
                let metadata = file.metadata().map_err(|_| Error::Io)?;
                if metadata.dev() != identity.device_id
                    || metadata.ino() != identity.inode
                    || self.evidence(location, hasher)? != *identity
                {
                    return Err(Error::EvidenceChanged);
                }
                file.sync_all().map_err(|_| Error::Io)?;
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveFileSynced(slot),
                )?;
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveBeforeRename(slot),
                )?;
                // Both guards exclude cooperating writers. Recheck the exact
                // two slots after authority callbacks and never replace a target.
                if path_exists(&source)? {
                    if path_exists(&target)? || self.evidence(&source, hasher)? != *identity {
                        return Err(Error::EvidenceChanged);
                    }
                    fs::rename(&source, &target).map_err(|_| Error::Io)?;
                }
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveRenamed(slot),
                )?;
                sync_directory(&paths[2])?;
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveTargetSynced(slot),
                )?;
                sync_directory(&self.store.state_directory)?;
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveSourceSynced(slot),
                )?;
                let mut next = record.clone();
                next.record_archived_slot(slot)
                    .map_err(|_| Error::EvidenceChanged)?;
                self.persist(guard, Some(&record), &next)?;
                record = next;
                self.archive_checkpoint(
                    guard,
                    port,
                    hasher,
                    &record,
                    SourcePreparationCheckpoint::ArchiveSlotRecorded(slot),
                )?;
            }
        }
        let mut next = record.clone();
        let digest = match &record.binding().previous_inventory_sha256 {
            Some(digest) => digest.clone(),
            None => {
                let bytes = inventory.encode()?;
                let digest = hasher
                    .sha256(&mut bytes.as_slice())
                    .map_err(|_| Error::Io)?;
                digest.iter().map(|byte| format!("{byte:02x}")).collect()
            }
        };
        next.record_previous_archive(digest)
            .map_err(|_| Error::EvidenceChanged)?;
        self.persist(guard, Some(&record), &next)?;
        self.archive_checkpoint(
            guard,
            port,
            hasher,
            &next,
            SourcePreparationCheckpoint::ArchiveCompleted,
        )?;
        Ok(next)
    }

    fn archive_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
        checkpoint: SourcePreparationCheckpoint,
    ) -> Result<PreviousInventory> {
        self.checkpoint(guard, port, hasher, record, checkpoint)?;
        self.verify_prepared(record, hasher)?;
        let inventory = self.load_previous_inventory(record, hasher)?;
        self.verify_inventory_files(&inventory, hasher, Some(record))?;
        self.verify_guard(guard)?;
        if self.load_guarded(guard)?.as_ref() != Some(record) {
            return Err(Error::EvidenceChanged);
        }
        Ok(inventory)
    }

    pub(super) fn load_previous_inventory(
        &self,
        record: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<PreviousInventory> {
        let binding = record.binding();
        let Some(previous) = &binding.previous_data_operation_id else {
            if path_exists(&self.store.root.path.join(HISTORY))?
                || record.previous_inventory_identity().is_some()
            {
                return Err(Error::EvidenceChanged);
            }
            return PreviousInventory::empty(
                self,
                &binding.operation_id,
                binding.previous_install_operation_id.as_deref(),
            );
        };
        let root = self.store.root.path.join(HISTORY);
        let operation = root.join(previous);
        self.history_directory(&root)?;
        self.history_directory(&operation)?;
        let path = operation.join(INVENTORY);
        let expected = record
            .previous_inventory_identity()
            .ok_or(Error::EvidenceChanged)?;
        if self.evidence(&path, hasher)? != *expected {
            return Err(Error::EvidenceChanged);
        }
        let inventory = PreviousInventory::decode(&self.read_history_bytes(&path)?)?;
        if self.evidence(&path, hasher)? != *expected {
            return Err(Error::EvidenceChanged);
        }
        inventory.verify_context(self, record)?;
        self.verify_history_directories(&inventory, true)?;
        Ok(inventory)
    }

    pub(super) fn history_directory(&self, path: &Path) -> Result<PreparationDirectoryIdentity> {
        let metadata = fs::symlink_metadata(path).map_err(|_| Error::EvidenceChanged)?;
        if fs::canonicalize(path).map_err(|_| Error::EvidenceChanged)? != path
            || !metadata.is_dir()
            || metadata.uid() != self.store.root.expected_owner_id
            || metadata.dev() != self.store.root.identity.device_id()
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(PreparationDirectoryIdentity {
            device_id: metadata.dev(),
            inode: metadata.ino(),
            owner_id: metadata.uid(),
            mode: metadata.mode() & 0o7777,
        })
    }

    pub(super) fn verify_history_directories(
        &self,
        inventory: &PreviousInventory,
        sealed: bool,
    ) -> Result<()> {
        let paths = inventory.paths(self)?;
        let expected = inventory
            .directories
            .as_ref()
            .ok_or(Error::EvidenceChanged)?;
        for (path, identity) in paths.iter().zip(expected) {
            if self.history_directory(path)? != *identity {
                return Err(Error::EvidenceChanged);
            }
        }
        for entry in fs::read_dir(&paths[1]).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name();
            if name != "data" && name != INVENTORY && (sealed || name != "inventory.json.tmp") {
                return Err(Error::EvidenceChanged);
            }
            if name != "data" {
                self.private_metadata(&entry.path())?;
            }
        }
        for entry in fs::read_dir(&paths[2]).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if !SLOTS
                .iter()
                .any(|slot| entry.file_name() == slot_name(*slot))
            {
                return Err(Error::EvidenceChanged);
            }
            self.private_metadata(&entry.path())?;
        }
        Ok(())
    }

    pub(super) fn verify_inventory_files(
        &self,
        inventory: &PreviousInventory,
        hasher: &impl PreparationHasher,
        record: Option<&PreparationReceipt>,
    ) -> Result<()> {
        self.validate_entries()?;
        let history = if inventory.directories.is_some() {
            self.verify_history_directories(inventory, record.is_some())?;
            Some(inventory.paths(self)?[2].clone())
        } else {
            None
        };
        let expected_slots: Vec<_> = SLOTS
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| inventory.files[index].as_ref().map(|_| *slot))
            .collect();
        let recorded = record.map_or(&[][..], PreparationReceipt::archived_slots);
        if !expected_slots.starts_with(recorded) {
            return Err(Error::EvidenceChanged);
        }
        let mut moved = Vec::new();
        let mut remaining_seen = false;
        for (index, slot) in SLOTS.iter().copied().enumerate() {
            let source = self.path(slot_name(slot));
            let target = history.as_ref().map(|path| path.join(slot_name(slot)));
            let source_exists = path_exists(&source)?;
            let target_exists = target
                .as_ref()
                .map(|path| path_exists(path))
                .transpose()?
                .unwrap_or(false);
            let Some(expected) = &inventory.files[index] else {
                if source_exists || target_exists {
                    return Err(Error::EvidenceChanged);
                }
                if slot != Slot::Receipt
                    && inventory
                        .receipt
                        .as_ref()
                        .and_then(|receipt| expected_private(receipt, slot))
                        .is_some()
                {
                    return Err(Error::EvidenceChanged);
                }
                continue;
            };
            if source_exists == target_exists {
                return Err(Error::EvidenceChanged);
            }
            let path = if source_exists {
                remaining_seen = true;
                &source
            } else {
                if remaining_seen || record.is_none() {
                    return Err(Error::EvidenceChanged);
                }
                moved.push(slot);
                target.as_ref().ok_or(Error::EvidenceChanged)?
            };
            if self.evidence(path, hasher)? != *expected {
                return Err(Error::EvidenceChanged);
            }
            let receipt = inventory.receipt.as_ref().ok_or(Error::EvidenceChanged)?;
            if slot == Slot::Receipt {
                if self.read_history_bytes(path)?
                    != receipt.encode().map_err(|_| Error::EvidenceChanged)?
                    || self.evidence(path, hasher)? != *expected
                {
                    return Err(Error::EvidenceChanged);
                }
            } else if !expected_private(receipt, slot)
                .is_some_and(|artifact| matches_artifact(expected, artifact))
            {
                return Err(Error::EvidenceChanged);
            }
        }
        if !moved.starts_with(recorded) || moved.len() > recorded.len() + 1 {
            return Err(Error::EvidenceChanged);
        }
        if record.is_some_and(|record| record.phase() == PreparationPhase::PreviousArchived)
            && (moved != expected_slots || recorded != expected_slots)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
