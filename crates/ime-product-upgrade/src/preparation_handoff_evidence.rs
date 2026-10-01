//! Exact active/history relationships at each handoff checkpoint.
use super::*;

impl PreparationJournalStore {
    pub(super) fn read_handoff_proof(
        &self,
        operation: &str,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationReceipt> {
        let root = self.store.root.path.join(HISTORY);
        let directory = root.join(operation);
        self.history_directory(&root)?;
        self.history_directory(&directory)?;
        let path = directory.join(PROOF);
        let identity = self.evidence(&path, hasher)?;
        let record = PreparationReceipt::decode(&self.read_history_bytes(&path)?)
            .map_err(|_| Error::EvidenceChanged)?;
        if record.phase() != PreparationPhase::HandoffReady
            || record.binding().operation_id != operation
            || self.evidence(&path, hasher)? != identity
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(record)
    }

    pub(super) fn verify_handoff(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
    ) -> Result<()> {
        self.verify_guard(guard)?;
        self.verify_binding(record)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        let intent = record.handoff_intent().ok_or(Error::EvidenceChanged)?;
        let directory = self.handoff_directory(record);
        let root = self.store.root.path.join(HISTORY);
        if self.history_directory(&root)? != intent.directories[0]
            || self.history_directory(&directory)? != intent.directories[1]
        {
            return Err(Error::EvidenceChanged);
        }
        self.verify_handoff_history(record, hasher)?;
        for entry in fs::read_dir(&directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if ![PROOF, PREPARATION_SNAPSHOT, RECEIPT_FILE_NAME]
                .iter()
                .any(|name| entry.file_name() == *name)
            {
                return Err(Error::EvidenceChanged);
            }
            self.private_metadata(&entry.path())?;
        }
        let marker = self.unique_location(&self.path(JOURNAL), &directory.join(PROOF))?;
        if self.read_history_bytes(&marker)?
            != record.encode().map_err(|_| Error::EvidenceChanged)?
        {
            return Err(Error::EvidenceChanged);
        }
        let receipt = self.unique_location(
            &directory.join(RECEIPT_FILE_NAME),
            &self.path(RECEIPT_FILE_NAME),
        )?;
        let snapshot = self.unique_location(
            &self.path(PREPARATION_SNAPSHOT),
            &directory.join(PREPARATION_SNAPSHOT),
        )?;
        if (record.phase() == PreparationPhase::HandoffReady
            && receipt != self.path(RECEIPT_FILE_NAME))
            || (snapshot != self.path(PREPARATION_SNAPSHOT)
                && record.phase() != PreparationPhase::HandoffReady)
            || (marker != self.path(JOURNAL)
                && (record.phase() != PreparationPhase::HandoffReady
                    || snapshot == self.path(PREPARATION_SNAPSHOT)))
        {
            return Err(Error::EvidenceChanged);
        }
        let expected_snapshot = record.snapshot_identity().ok_or(Error::EvidenceChanged)?;
        if self.evidence(&receipt, hasher)? != intent.receipt_identity
            || self.read_history_bytes(&receipt)?
                != intent
                    .receipt
                    .encode()
                    .map_err(|_| Error::EvidenceChanged)?
            || self.evidence(&receipt, hasher)? != intent.receipt_identity
            || self.evidence(&snapshot, hasher)? != *expected_snapshot
        {
            return Err(Error::EvidenceChanged);
        }
        no_sidecars(&snapshot)?;
        no_sidecars(&self.source_path())?;
        if record.prepared_source() != Some(&self.family(hasher)?.database)
            || Some(UserDb::verify_prepared_source(
                self.source_path(),
                &snapshot,
            )?) != record.snapshot_schema_version()
            || self.evidence(&snapshot, hasher)? != *expected_snapshot
            || record.prepared_source() != Some(&self.family(hasher)?.database)
        {
            return Err(Error::EvidenceChanged);
        }
        let (artifacts, settings) = self.handoff_artifacts(hasher)?;
        // Directory link count can change when fixed history directories are
        // created. Stable root identity was checked above and in the contract.
        let non_root = |items: &[UpgradeArtifactIdentity]| {
            items
                .iter()
                .filter(|item| item.slot() != UpgradeArtifactSlot::DataRoot)
                .cloned()
                .collect::<Vec<_>>()
        };
        if non_root(&artifacts) != non_root(intent.receipt.artifacts())
            || settings != intent.settings
        {
            return Err(Error::EvidenceChanged);
        }
        self.verify_guard(guard)?;
        Ok(())
    }

    /// Historical slots are sealed, never considered candidates for new active
    /// files. Only the separately proved new receipt may occupy an active slot.
    pub(super) fn verify_handoff_history(
        &self,
        record: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        for slot in SLOTS.iter().filter(|slot| **slot != Slot::Receipt) {
            if path_exists(&self.path(slot_name(*slot)))? {
                return Err(Error::EvidenceChanged);
            }
        }
        let previous = &record.binding().previous_data_operation_id;
        if previous.is_some() {
            let inventory = self.load_previous_inventory(record, hasher)?;
            let directory = inventory.paths(self)?[2].clone();
            let expected_slots: Vec<_> = SLOTS
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| inventory.files[index].as_ref().map(|_| *slot))
                .collect();
            if (inventory.released.is_some() && !record.archived_slots().is_empty())
                || (inventory.released.is_none() && record.archived_slots() != expected_slots)
            {
                return Err(Error::EvidenceChanged);
            }
            for (index, slot) in SLOTS.iter().copied().enumerate() {
                let path = directory.join(slot_name(slot));
                match &inventory.files[index] {
                    Some(identity) => {
                        if self.evidence(&path, hasher)? != *identity {
                            return Err(Error::EvidenceChanged);
                        }
                        let receipt = inventory.receipt.as_ref().ok_or(Error::EvidenceChanged)?;
                        if slot == Slot::Receipt {
                            if self.read_history_bytes(&path)?
                                != receipt.encode().map_err(|_| Error::EvidenceChanged)?
                                || self.evidence(&path, hasher)? != *identity
                            {
                                return Err(Error::EvidenceChanged);
                            }
                        } else if !expected_private(receipt, slot)
                            .is_some_and(|item| matches_artifact(identity, item))
                        {
                            return Err(Error::EvidenceChanged);
                        }
                    }
                    None if path_exists(&path)? => return Err(Error::EvidenceChanged),
                    None => {}
                }
            }
            if inventory.released.is_some() {
                // The shared release reader checked the root, sealed proof and
                // every old private file. Older retained operations are allowed;
                // the new operation still has its own exclusive handoff contract.
                return Ok(());
            }
        } else {
            if record.previous_inventory_identity().is_some() || !record.archived_slots().is_empty()
            {
                return Err(Error::EvidenceChanged);
            }
            let empty = PreviousInventory::empty(
                self,
                &record.binding().operation_id,
                record.binding().previous_install_operation_id.as_deref(),
            )?
            .encode()?;
            let hash: String = hasher
                .sha256(&mut empty.as_slice())
                .map_err(|_| Error::Io)?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if record.archived_inventory_sha256() != Some(&hash) {
                return Err(Error::EvidenceChanged);
            }
        }
        let root = self.store.root.path.join(HISTORY);
        if path_exists(&root)? {
            self.history_directory(&root)?;
            for entry in fs::read_dir(root).map_err(|_| Error::Io)? {
                let name = entry.map_err(|_| Error::Io)?.file_name();
                if name != record.binding().operation_id.as_str()
                    && previous.as_ref().map_or(true, |id| name != id.as_str())
                {
                    return Err(Error::EvidenceChanged);
                }
            }
        }
        Ok(())
    }

    pub(super) fn handoff_artifacts(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<(
        Vec<UpgradeArtifactIdentity>,
        Option<PreparationFileIdentity>,
    )> {
        let source = self.evidence(&self.source_path(), hasher)?;
        let file_artifact = |slot, identity: &PreparationFileIdentity| {
            UpgradeArtifactIdentity::private_file(
                slot,
                identity.device_id,
                identity.inode,
                identity.owner_id,
                identity.byte_len,
            )
            .map_err(|_| Error::EvidenceChanged)
        };
        let mut artifacts = vec![
            self.store.data_root_identity().clone(),
            file_artifact(UpgradeArtifactSlot::SourceDatabase, &source)?,
        ];
        let settings = self.store.root.path.join("manager-settings.json");
        let settings = if path_exists(&settings)? {
            let identity = self.evidence(&settings, hasher)?;
            artifacts.push(file_artifact(
                UpgradeArtifactSlot::SourceSettings,
                &identity,
            )?);
            Some(identity)
        } else {
            None
        };
        let rime = self.store.root.path.join("Rime");
        if path_exists(&rime)? {
            let identity = self.history_directory(&rime)?;
            let metadata = fs::symlink_metadata(&rime).map_err(|_| Error::Io)?;
            artifacts.push(
                UpgradeArtifactIdentity::private_directory(
                    UpgradeArtifactSlot::RimeRoot,
                    identity.device_id,
                    identity.inode,
                    identity.owner_id,
                    metadata.nlink(),
                )
                .map_err(|_| Error::EvidenceChanged)?,
            );
        }
        artifacts.sort_by_key(UpgradeArtifactIdentity::slot);
        Ok((artifacts, settings))
    }
}
