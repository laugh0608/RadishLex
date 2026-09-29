use super::*;

impl CancellationArchiveStore {
    pub(super) fn initialize(
        &self,
        guard: &UpgradeProcessGuard,
        outer: &CancellationOuterBinding,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationArchiveReceipt> {
        let root = VerifiedDataRoot::verify(
            &self.journal.store.root.path,
            self.journal.store.root.expected_owner_id,
        )?;
        let cancellation = PreparationCancellationStore::open_existing(root)?;
        let source = cancellation
            .load_source_guarded(guard, hasher)?
            .ok_or(Error::InvalidPhase)?;
        if source.phase() != CancellationSourcePhase::SourceReady {
            return Err(Error::InvalidPhase);
        }
        let request =
            PreparationCancellationRequest::decode(&self.read_bytes(&self.active(REQUEST))?)?;
        let inventory = self
            .journal
            .load_previous_inventory(request.preparation(), hasher)?;
        let moved = request
            .preparation()
            .archived_slots()
            .iter()
            .copied()
            .map(ArchiveSlot::Previous)
            .collect();
        let mut record = CancellationArchiveReceipt {
            format: "radishlex-cancellation-archive-v1".into(),
            request,
            source,
            source_identity: self.journal.evidence(&self.active(SOURCE), hasher)?,
            outer: outer.clone(),
            directories: Vec::new(),
            history_entries: Vec::new(),
            settings: self.optional_file(
                &self.journal.store.root.path.join("manager-settings.json"),
                hasher,
            )?,
            rime: self.optional_directory(&self.journal.store.root.path.join("Rime"))?,
            previous_files: inventory.files.clone(),
            previous_directories: inventory.directories.clone(),
            moved,
            compatibility: None,
            preserved: false,
        };
        let directory = self.directory(&record);
        let history = directory.parent().ok_or(Error::EvidenceChanged)?;
        if path_exists(&directory)? {
            return Err(Error::EvidenceChanged);
        }
        // Existing history roots are pinned before the first callback, including
        // the released-v1 index branch. New roots cannot be silently adopted.
        let original_root = if path_exists(history)? {
            Some(self.journal.history_directory(history)?)
        } else {
            None
        };
        record.history_entries = if original_root.is_some() {
            self.names(history)?
        } else {
            Vec::new()
        };
        record
            .history_entries
            .push(record.request.preparation().binding().operation_id.clone());
        record.history_entries.sort();
        self.initial_checkpoint(
            guard,
            &record,
            original_root.as_ref(),
            port,
            hasher,
            Point::Begin,
        )?;
        self.verify_source_equivalence(&record, hasher)?;
        for path in [history, directory.as_path()] {
            if path == directory || original_root.is_none() {
                DirBuilder::new()
                    .mode(0o700)
                    .create(path)
                    .map_err(|_| Error::Io)?;
            }
            record
                .directories
                .push(self.journal.history_directory(path)?);
            self.initial_checkpoint(
                guard,
                &record,
                original_root.as_ref(),
                port,
                hasher,
                Point::DirectoryCreated,
            )?;
            sync_directory(path)?;
            self.initial_checkpoint(
                guard,
                &record,
                original_root.as_ref(),
                port,
                hasher,
                Point::DirectorySynced,
            )?;
            sync_directory(path.parent().ok_or(Error::EvidenceChanged)?)?;
            self.initial_checkpoint(
                guard,
                &record,
                original_root.as_ref(),
                port,
                hasher,
                Point::ParentSynced,
            )?;
        }
        self.persist(guard, None, &record, port, hasher)?;
        Ok(record)
    }

    #[allow(clippy::too_many_arguments)]
    fn initial_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        record: &CancellationArchiveReceipt,
        original_root: Option<&PreparationDirectoryIdentity>,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
        point: Point,
    ) -> Result<()> {
        let verify = || -> Result<()> {
            self.journal.verify_guard(guard)?;
            self.validate_entries(None)?;
            if path_exists(&self.active(ARCHIVE))? {
                return Err(Error::EvidenceChanged);
            }
            let prep = record.request.preparation();
            record.source.validate_request(&record.request)?;
            for (name, bytes, identity) in [
                (
                    REQUEST,
                    record.request.encode()?,
                    record.source.request_identity(),
                ),
                (SOURCE, record.source.encode()?, &record.source_identity),
                (
                    JOURNAL,
                    prep.encode().map_err(|_| Error::EvidenceChanged)?,
                    record.request.preparation_identity(),
                ),
            ] {
                if self.journal.evidence(&self.active(name), hasher)? != *identity
                    || self.read_bytes(&self.active(name))? != bytes
                    || self.journal.evidence(&self.active(name), hasher)? != *identity
                {
                    return Err(Error::EvidenceChanged);
                }
            }
            if self.journal.family(hasher)?
                != *record.source.source_ready().ok_or(Error::EvidenceChanged)?
            {
                return Err(Error::EvidenceChanged);
            }
            if prep.snapshot_identity().is_some() {
                self.journal.verify_snapshot(prep, hasher)?;
            } else if path_exists(&self.active(PREPARATION_SNAPSHOT))? {
                return Err(Error::EvidenceChanged);
            }
            self.verify_previous(record, hasher)?;
            // Before intent, no additional inventory move may be admitted.
            for slot in record.expected_slots() {
                if let ArchiveSlot::Previous(_) = slot {
                    let (source, target, identity) = self.slot(record, slot)?;
                    let location = self.unique(&source, &target)?;
                    let expected_moved = record.moved.contains(&slot);
                    // An earlier archive rename may precede its progress write.
                    let index = record
                        .expected_slots()
                        .iter()
                        .position(|v| *v == slot)
                        .ok_or(Error::EvidenceChanged)?;
                    if self.journal.evidence(&location, hasher)? != *identity
                        || (location == source && expected_moved)
                        || (location == target && !expected_moved && index != record.moved.len())
                    {
                        return Err(Error::EvidenceChanged);
                    }
                }
            }
            self.verify_outer(&record.request, &record.outer, None, false, false, hasher)?;
            self.verify_runtime_materials(record, hasher)?;
            let directory = self.directory(record);
            let history = directory.parent().ok_or(Error::EvidenceChanged)?;
            for (index, path) in [history, directory.as_path()].into_iter().enumerate() {
                if let Some(identity) = record.directories.get(index) {
                    if self.journal.history_directory(path)? != *identity {
                        return Err(Error::EvidenceChanged);
                    }
                } else if index == 0 && original_root.is_some() {
                    if Some(&self.journal.history_directory(path)?) != original_root {
                        return Err(Error::EvidenceChanged);
                    }
                } else if path_exists(path)? {
                    return Err(Error::EvidenceChanged);
                }
            }
            if record.directories.len() == 2
                && fs::read_dir(&directory)
                    .map_err(|_| Error::Io)?
                    .next()
                    .is_some()
            {
                return Err(Error::EvidenceChanged);
            }
            let mut expected_entries = record.history_entries.clone();
            if record.directories.len() < 2 {
                expected_entries
                    .retain(|name| name != &record.request.preparation().binding().operation_id);
            }
            if path_exists(history)? && self.names(history)? != expected_entries {
                return Err(Error::EvidenceChanged);
            }
            if let (Some(original), Some(current)) = (original_root, record.directories.first()) {
                if original != current {
                    return Err(Error::EvidenceChanged);
                }
            }
            Ok(())
        };
        verify()?;
        if !port.confirm_authority_and_quiescence(record, point) {
            return Err(Error::AuthorityNotProven);
        }
        verify()
    }
}
