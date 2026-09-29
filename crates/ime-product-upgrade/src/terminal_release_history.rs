//! Shared read-only verification of immutable release history.
use super::*;

pub(super) struct ReleaseHistory<'a> {
    pub(super) journal: &'a PreparationJournalStore,
}
impl ReleaseHistory<'_> {
    fn active(&self, name: &str) -> PathBuf {
        self.journal.store.state_directory.join(name)
    }
    fn paths(&self, record: &TerminalReleaseReceipt) -> [PathBuf; 3] {
        let root = self.journal.store.root.path.join(HISTORY);
        let operation = root.join(&record.binding.operation_id);
        let data = operation.join("data");
        [root, operation, data]
    }
    pub(super) fn read_release(
        &self,
        path: &Path,
        hasher: &impl PreparationHasher,
    ) -> Result<TerminalReleaseReceipt> {
        let identity = self.journal.evidence(path, hasher)?;
        let result = TerminalReleaseReceipt::decode(&self.journal.read_history_bytes(path)?)?;
        if self.journal.evidence(path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(result)
    }
    pub(super) fn verify_directories(
        &self,
        record: &TerminalReleaseReceipt,
        initializing: bool,
    ) -> Result<()> {
        if record.data_root != self.journal.data_root_identity()
            || record.state_directory != self.journal.state_directory_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        let paths = self.paths(record);
        if path_exists(&paths[0])? {
            self.journal.history_directory(&paths[0])?;
            for entry in fs::read_dir(&paths[0]).map_err(|_| Error::Io)? {
                let entry = entry.map_err(|_| Error::Io)?;
                let name = entry.file_name();
                if name == INDEX || name == INDEX_TEMP {
                    self.journal.private_metadata(&entry.path())?;
                } else if name.to_str().is_some_and(valid_id) {
                    self.journal.history_directory(&entry.path())?;
                } else {
                    return Err(Error::EvidenceChanged);
                }
            }
        }
        for (path, expected) in paths.iter().zip(&record.directories) {
            if self.journal.history_directory(path)? != *expected {
                return Err(Error::EvidenceChanged);
            }
        }
        if initializing && record.directories.len() < 2 {
            return Ok(());
        }
        for entry in fs::read_dir(&paths[1]).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name();
            if name == "data" {
                if initializing && record.directories.len() < 3 {
                    return Err(Error::EvidenceChanged);
                }
            } else if !PREPARATION.iter().any(|allowed| name == *allowed) && name != RELEASE {
                return Err(Error::EvidenceChanged);
            } else {
                self.journal.private_metadata(&entry.path())?;
            }
        }
        if !initializing || record.directories.len() == 3 {
            for entry in fs::read_dir(&paths[2]).map_err(|_| Error::Io)? {
                let entry = entry.map_err(|_| Error::Io)?;
                if !SLOTS
                    .iter()
                    .any(|slot| entry.file_name() == slot_name(*slot))
                {
                    return Err(Error::EvidenceChanged);
                }
                self.journal.private_metadata(&entry.path())?;
            }
        }
        Ok(())
    }
    pub(super) fn verify_files(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
        historical: bool,
    ) -> Result<()> {
        let data = &self.paths(record)[2];
        let expected_slots: Vec<_> = SLOTS
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| record.files[i].as_ref().map(|_| *slot))
            .collect();
        let mut moved = Vec::new();
        let mut active_seen = false;
        for (i, slot) in SLOTS.iter().copied().enumerate() {
            let target = data.join(slot_name(slot));
            let source = self.active(slot_name(slot));
            let location = if historical {
                target.clone()
            } else {
                match (path_exists(&source)?, path_exists(&target)?) {
                    (true, false) => {
                        active_seen = true;
                        source
                    }
                    (false, true) => {
                        if active_seen {
                            return Err(Error::EvidenceChanged);
                        }
                        moved.push(slot);
                        target
                    }
                    (false, false) if record.files[i].is_none() => continue,
                    _ => return Err(Error::EvidenceChanged),
                }
            };
            match &record.files[i] {
                Some(identity) => {
                    if self.journal.evidence(&location, hasher)? != *identity {
                        return Err(Error::EvidenceChanged);
                    }
                    if slot == Slot::Receipt
                        && (self.journal.read_history_bytes(&location)?
                            != record
                                .receipt
                                .encode()
                                .map_err(|_| Error::EvidenceChanged)?
                            || self.journal.evidence(&location, hasher)? != *identity)
                    {
                        return Err(Error::EvidenceChanged);
                    }
                }
                None if path_exists(&location)? => return Err(Error::EvidenceChanged),
                None => {}
            }
        }
        if !historical
            && (!moved.starts_with(&record.archived_slots)
                || moved.len() > record.archived_slots.len() + 1
                || (record.phase == TerminalReleasePhase::ReleaseReady && moved != expected_slots))
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    pub(super) fn verify_runtime(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        no_sidecars(&self.journal.source_path())?;
        if self.journal.evidence(&self.journal.source_path(), hasher)? != record.database {
            return Err(Error::EvidenceChanged);
        }
        let settings = self.journal.store.root.path.join("manager-settings.json");
        if self.optional_file(&settings, hasher)? != record.settings {
            return Err(Error::EvidenceChanged);
        }
        let rime = self.journal.store.root.path.join("Rime");
        let actual = if path_exists(&rime)? {
            Some(self.journal.history_directory(&rime)?)
        } else {
            None
        };
        if actual != record.rime {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    pub(super) fn verify_preparation(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let operation = &self.paths(record)[1];
        for (name, expected) in PREPARATION.iter().zip(&record.preparation) {
            if self.optional_file(&operation.join(name), hasher)? != *expected {
                return Err(Error::EvidenceChanged);
            }
        }
        if record.preparation[0].is_some() {
            let proof = PreparationReceipt::decode(
                &self
                    .journal
                    .read_history_bytes(&operation.join(PREPARATION[0]))?,
            )
            .map_err(|_| Error::EvidenceChanged)?;
            let intent = proof.handoff_intent().ok_or(Error::EvidenceChanged)?;
            let original = &intent.receipt;
            if self.journal.history_directory(&self.paths(record)[0])? != intent.directories[0]
                || self.journal.history_directory(operation)? != intent.directories[1]
            {
                return Err(Error::EvidenceChanged);
            }
            if proof.phase() != PreparationPhase::HandoffReady
                || proof.binding().operation_id != record.binding.operation_id
                || proof.binding().data_root != record.data_root
                || proof.binding().state_directory != record.state_directory
                || proof.snapshot_identity() != record.preparation[1].as_ref()
                || original.operation_id() != record.receipt.operation_id()
                || original.previous_operation_id() != record.receipt.previous_operation_id()
                || original.source_release() != record.receipt.source_release()
                || original.target_release() != record.receipt.target_release()
                || original.source_schema_version() != record.receipt.source_schema_version()
                || original.target_schema_version() != record.receipt.target_schema_version()
                || original
                    .artifacts()
                    .iter()
                    .any(|item| !record.receipt.artifacts().contains(item))
            {
                return Err(Error::EvidenceChanged);
            }
            let product = if record.receipt.state() == UpgradeState::Completed {
                &proof.binding().target_product_sha256
            } else {
                &proof.binding().source_product_sha256
            };
            if product != &record.binding.installed_product_sha256 {
                return Err(Error::EvidenceChanged);
            }
            if let Some(identity) = proof.previous_release_index_identity() {
                let previous = record
                    .previous_index
                    .as_ref()
                    .ok_or(Error::EvidenceChanged)?;
                let previous_path = self
                    .journal
                    .store
                    .root
                    .path
                    .join(HISTORY)
                    .join(previous.index.operation_id())
                    .join(RELEASE);
                if identity != &previous.identity
                    || proof.previous_inventory_identity()
                        != Some(&self.journal.evidence(&previous_path, hasher)?)
                {
                    return Err(Error::EvidenceChanged);
                }
            } else if record.format == FORMAT
                && proof.binding().previous_data_operation_id.is_some()
            {
                let inventory = self.journal.load_previous_inventory(&proof, hasher)?;
                let data = &inventory.paths(self.journal)?[2];
                for (slot, expected) in SLOTS.iter().zip(&inventory.files) {
                    if self.optional_file(&data.join(slot_name(*slot)), hasher)? != *expected {
                        return Err(Error::EvidenceChanged);
                    }
                }
            }
        }
        Ok(())
    }

    /// During a new v2 release, absence of a locator is accepted only alongside
    /// the current operation and its explicitly bound unreleased inventory.
    /// An orphan release/cancellation directory is never a first lifecycle.
    /// This is a live check: a historical chain head can have later descendants.
    pub(super) fn verify_index_origin(&self, record: &TerminalReleaseReceipt) -> Result<()> {
        if record.format != FORMAT || record.previous_index.is_some() {
            return Ok(());
        }
        let root = self.journal.store.root.path.join(HISTORY);
        if !path_exists(&root)? {
            return Ok(());
        }
        self.journal.history_directory(&root)?;
        let legacy_data = if record.preparation[0].is_some() {
            record.receipt.previous_operation_id()
        } else {
            None
        };
        for entry in fs::read_dir(root).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name();
            if name == INDEX || name == INDEX_TEMP {
                // The caller independently requires the exact old/new locator
                // or its own exclusive temp, including identity and contents.
                continue;
            }
            if name != record.binding.operation_id.as_str()
                && !legacy_data.is_some_and(|id| name == id)
            {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
    pub(super) fn optional_file(
        &self,
        path: &Path,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<PreparationFileIdentity>> {
        if path_exists(path)? {
            Ok(Some(self.journal.evidence(path, hasher)?))
        } else {
            Ok(None)
        }
    }
    pub(super) fn load_index(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<PreviousIndex>> {
        self.load_index_with_temp(hasher, None)
    }
    pub(super) fn load_index_with_temp(
        &self,
        hasher: &impl PreparationHasher,
        temporary: Option<&PreparationFileIdentity>,
    ) -> Result<Option<PreviousIndex>> {
        let root = self.journal.store.root.path.join(HISTORY);
        if !path_exists(&root)? {
            return Ok(None);
        }
        self.journal.history_directory(&root)?;
        if let Some(identity) = temporary {
            if self.journal.evidence(&root.join(INDEX_TEMP), hasher)? != *identity {
                return Err(Error::EvidenceChanged);
            }
        } else if path_exists(&root.join(INDEX_TEMP))? {
            return Err(Error::EvidenceChanged);
        }
        let path = root.join(INDEX);
        let Some(identity) = self.optional_file(&path, hasher)? else {
            return Ok(None);
        };
        let index: ReleaseIndex = decode(&self.journal.read_history_bytes(&path)?)?;
        index.validate()?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some(PreviousIndex { identity, index }))
    }
    pub(super) fn index_for(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<ReleaseIndex> {
        if record.phase != TerminalReleasePhase::ReleaseReady {
            return Err(Error::InvalidPhase);
        }
        let bytes = record.encode()?;
        let digest: String = hasher
            .sha256(&mut bytes.as_slice())
            .map_err(|_| Error::Io)?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        ReleaseIndex::for_terminal(record, digest)
    }
    pub(super) fn require_published_index(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self.load_index(hasher)?.as_ref().map(|v| &v.index)
            != Some(&self.index_for(record, hasher)?)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    pub(super) fn resolve_index(
        &self,
        index: &ReleaseIndex,
        hasher: &impl PreparationHasher,
    ) -> Result<TerminalReleaseReceipt> {
        let record = self.resolve_one(index, hasher)?;
        self.verify_ancestors(&record, hasher)?;
        Ok(record)
    }

    /// Follow only bound locators. Historical runtime hashes are deliberately
    /// not checked: normal learning after release may have changed that data.
    /// Iteration avoids stack growth and rejects repeated operation identities.
    pub(super) fn verify_ancestors(
        &self,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let mut seen = std::collections::BTreeSet::from([record.binding.operation_id.clone()]);
        let mut current = record.clone();
        while let Some(previous) = &current.previous_index {
            if !seen.insert(previous.index.operation_id().to_owned())
                || current.receipt.previous_operation_id() != previous.index.data_operation_id()
                || current.receipt.source_release() != previous.index.installed_release()
            {
                return Err(Error::EvidenceChanged);
            }
            // The old index inode is no longer at latest-release.json after
            // atomic publication; verify its saved canonical bytes, not the
            // new locator's identity or an invented historical file.
            let bytes = encode(&previous.index)?;
            let digest: String = hasher
                .sha256(&mut bytes.as_slice())
                .map_err(|_| Error::Io)?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if previous.identity.byte_len != bytes.len() as u64
                || previous.identity.sha256 != digest
            {
                return Err(Error::EvidenceChanged);
            }
            current = self.resolve_one(&previous.index, hasher)?;
        }
        Ok(())
    }

    fn resolve_one(
        &self,
        index: &ReleaseIndex,
        hasher: &impl PreparationHasher,
    ) -> Result<TerminalReleaseReceipt> {
        index.validate()?;
        if path_exists(&self.active(RELEASE))?
            && self
                .read_release(&self.active(RELEASE), hasher)?
                .binding
                .operation_id
                == index.operation_id()
        {
            return Err(Error::EvidenceChanged);
        }
        let root = self.journal.store.root.path.join(HISTORY);
        self.journal.history_directory(&root)?;
        let operation = root.join(index.operation_id());
        self.journal.history_directory(&operation)?;
        let path = operation.join(RELEASE);
        let identity = self.journal.evidence(&path, hasher)?;
        let record = self.read_release(&path, hasher)?;
        if self.index_for(&record, hasher)? != *index {
            return Err(Error::EvidenceChanged);
        }
        record.validate()?;
        self.verify_directories(&record, false)?;
        self.verify_files(&record, hasher, true)?;
        self.verify_preparation(&record, hasher)?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(record)
    }
}
