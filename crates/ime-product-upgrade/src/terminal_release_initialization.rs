//! Bind all physical inputs before moving any active v1 slot.
use super::*;

impl TerminalReleaseStore {
    pub(super) fn initialize(
        &self,
        guard: &UpgradeProcessGuard,
        binding: &TerminalReleaseBinding,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<TerminalReleaseReceipt> {
        if !valid_id(&binding.operation_id) {
            return Err(Error::EvidenceChanged);
        }
        let receipt = self
            .journal
            .store
            .load_guarded(guard)?
            .ok_or(Error::InvalidPhase)?;
        if !receipt.state().is_terminal()
            || receipt.manual_recovery_required()
            || receipt.operation_id() != binding.operation_id
        {
            return Err(Error::InvalidPhase);
        }
        let mut files: [Option<PreparationFileIdentity>; 5] = std::array::from_fn(|_| None);
        for (i, slot) in SLOTS.iter().copied().enumerate() {
            files[i] = self
                .history()
                .optional_file(&self.active(slot_name(slot)), hasher)?;
        }
        let root = self.journal.store.root.path.join(HISTORY);
        let operation = root.join(&binding.operation_id);
        if path_exists(&root)? {
            self.journal.history_directory(&root)?;
        }
        if path_exists(&operation)? {
            self.journal.history_directory(&operation)?;
        }
        let preparation = [
            self.history()
                .optional_file(&operation.join(PREPARATION[0]), hasher)?,
            self.history()
                .optional_file(&operation.join(PREPARATION[1]), hasher)?,
        ];
        if path_exists(&operation)?
            && (preparation[0].is_none()
                || preparation[1].is_none()
                || path_exists(&operation.join("data"))?
                || path_exists(&operation.join(RELEASE))?)
        {
            return Err(Error::EvidenceChanged);
        }
        let previous_index = self.history().load_index(hasher)?;
        if let Some(previous) = &previous_index {
            self.history().resolve_index(&previous.index, hasher)?;
        }
        let settings = self.history().optional_file(
            &self.journal.store.root.path.join("manager-settings.json"),
            hasher,
        )?;
        let rime_path = self.journal.store.root.path.join("Rime");
        let rime = if path_exists(&rime_path)? {
            Some(self.journal.history_directory(&rime_path)?)
        } else {
            None
        };
        let mut record = TerminalReleaseReceipt {
            format: FORMAT.to_owned(),
            phase: TerminalReleasePhase::Reserved,
            binding: binding.clone(),
            data_root: self.journal.data_root_identity(),
            state_directory: self.journal.state_directory_identity(),
            directories: Vec::new(),
            receipt,
            files,
            preparation,
            database: self.journal.evidence(&self.journal.source_path(), hasher)?,
            settings,
            rime,
            previous_index,
            archived_slots: Vec::new(),
        };
        record.validate_inputs()?;
        let paths = self.paths(&record);
        let mut initial_directories = [None, None, None];
        for (path, expected) in paths.iter().zip(&mut initial_directories) {
            if path_exists(path)? {
                *expected = Some(self.journal.history_directory(path)?);
            }
        }
        self.initialization_checkpoint(
            guard,
            port,
            hasher,
            &record,
            Point::Begin,
            &initial_directories,
        )?;
        let kinds = [
            TerminalReleaseDirectory::HistoryRoot,
            TerminalReleaseDirectory::Operation,
            TerminalReleaseDirectory::Data,
        ];
        for (path, kind) in paths.iter().zip(kinds) {
            if !path_exists(path)? {
                DirBuilder::new()
                    .mode(0o700)
                    .create(path)
                    .map_err(|_| Error::Io)?;
            } else if kind == TerminalReleaseDirectory::Data {
                return Err(Error::EvidenceChanged);
            }
            record
                .directories
                .push(self.journal.history_directory(path)?);
            self.initialization_checkpoint(
                guard,
                port,
                hasher,
                &record,
                Point::DirectoryCreated(kind),
                &initial_directories,
            )?;
            sync_directory(path)?;
            self.initialization_checkpoint(
                guard,
                port,
                hasher,
                &record,
                Point::DirectorySynced(kind),
                &initial_directories,
            )?;
            sync_directory(path.parent().ok_or(Error::EvidenceChanged)?)?;
            self.initialization_checkpoint(
                guard,
                port,
                hasher,
                &record,
                Point::ParentSynced(kind),
                &initial_directories,
            )?;
        }
        self.persist_record(guard, None, &record, port, hasher)?;
        Ok(record)
    }

    fn initialization_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
        record: &TerminalReleaseReceipt,
        point: Point,
        initial_directories: &[Option<PreparationDirectoryIdentity>; 3],
    ) -> Result<()> {
        let verify = || -> Result<()> {
            self.journal.verify_guard(guard)?;
            self.validate_active_entries()?;
            if path_exists(&self.active(RELEASE))?
                || self.journal.store.load_guarded(guard)?.as_ref() != Some(&record.receipt)
            {
                return Err(Error::EvidenceChanged);
            }
            for (i, path) in self.paths(record).iter().enumerate() {
                let actual = if path_exists(path)? {
                    Some(self.journal.history_directory(path)?)
                } else {
                    None
                };
                let expected = record
                    .directories
                    .get(i)
                    .or(initial_directories[i].as_ref());
                if actual.as_ref() != expected {
                    return Err(Error::EvidenceChanged);
                }
            }
            self.history().verify_directories(record, true)?;
            self.history().verify_files(record, hasher, false)?;
            self.history().verify_runtime(record, hasher)?;
            self.history().verify_preparation(record, hasher)?;
            if self.history().load_index(hasher)? != record.previous_index {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        };
        verify()?;
        if !port.confirm_authority_and_quiescence(
            record,
            point,
            TerminalReleaseOuterRequirement::Nonterminal,
        ) {
            return Err(Error::AuthorityNotProven);
        }
        verify()
    }
}
