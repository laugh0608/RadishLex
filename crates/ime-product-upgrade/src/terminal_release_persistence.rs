//! Canonical atomic writes and retained interruption evidence.
use super::*;

impl TerminalReleaseStore {
    pub(super) fn sync_file(
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
    pub(super) fn persist_record(
        &self,
        guard: &UpgradeProcessGuard,
        previous: Option<&TerminalReleaseReceipt>,
        next: &TerminalReleaseReceipt,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        next.validate()?;
        if let Some(previous) = previous {
            if !next.can_replace(previous) {
                return Err(Error::EvidenceChanged);
            }
        } else if next.phase != TerminalReleasePhase::Reserved || !next.archived_slots.is_empty() {
            return Err(Error::InvalidPhase);
        }
        let path = self.active(RELEASE);
        let current = self.history().optional_file(&path, hasher)?;
        let bytes = next.encode()?;
        if let Some(identity) = &current {
            let stored = self.history().read_release(&path, hasher)?;
            if stored == *next {
                self.sync_file(&path, identity, hasher)?;
                sync_directory(&self.journal.store.state_directory)?;
                self.journal.verify_guard(guard)?;
                return Ok(());
            }
            if Some(&stored) != previous {
                return Err(Error::EvidenceChanged);
            }
        } else if previous.is_some() {
            return Err(Error::EvidenceChanged);
        }
        let staged = self.active(RELEASE_TEMP);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        let created = self.journal.evidence(&staged, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != created.device_id || metadata.ino() != created.inode {
            return Err(Error::EvidenceChanged);
        }
        self.record_write_checkpoint(
            guard,
            port,
            hasher,
            next,
            Point::RecordCreated,
            current.as_ref(),
            Some(&created),
        )?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let written = self.journal.evidence(&staged, hasher)?;
        if !same_object(&created, &written) {
            return Err(Error::EvidenceChanged);
        }
        self.record_write_checkpoint(
            guard,
            port,
            hasher,
            next,
            Point::RecordWritten,
            current.as_ref(),
            Some(&written),
        )?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.record_write_checkpoint(
            guard,
            port,
            hasher,
            next,
            Point::RecordFileSynced,
            current.as_ref(),
            Some(&written),
        )?;
        let identity = self.journal.evidence(&staged, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if identity.device_id != metadata.dev()
            || identity.inode != metadata.ino()
            || self.journal.read_history_bytes(&staged)? != bytes
            || self.history().optional_file(&path, hasher)? != current
        {
            return Err(Error::EvidenceChanged);
        }
        self.journal.verify_guard(guard)?;
        if self.journal.evidence(&staged, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        fs::rename(&staged, &path).map_err(|_| Error::Io)?;
        self.record_write_checkpoint(
            guard,
            port,
            hasher,
            next,
            Point::RecordRenamed,
            Some(&identity),
            None,
        )?;
        sync_directory(&self.journal.store.state_directory)?;
        self.record_write_checkpoint(
            guard,
            port,
            hasher,
            next,
            Point::RecordDirectorySynced,
            Some(&identity),
            None,
        )?;
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        if self.journal.evidence(&path, hasher)? != identity
            || self.history().read_release(&path, hasher)? != *next
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn record_write_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
        next: &TerminalReleaseReceipt,
        point: Point,
        current: Option<&PreparationFileIdentity>,
        staged: Option<&PreparationFileIdentity>,
    ) -> Result<()> {
        let verify = || -> Result<()> {
            self.journal.verify_guard(guard)?;
            for entry in fs::read_dir(&self.journal.store.state_directory).map_err(|_| Error::Io)? {
                let entry = entry.map_err(|_| Error::Io)?;
                let name = entry.file_name();
                if name != RELEASE
                    && !(name == RELEASE_TEMP && staged.is_some())
                    && !SLOTS.iter().any(|slot| name == slot_name(*slot))
                {
                    return Err(Error::EvidenceChanged);
                }
                self.journal.private_metadata(&entry.path())?;
            }
            if self
                .history()
                .optional_file(&self.active(RELEASE), hasher)?
                .as_ref()
                != current
                || self
                    .history()
                    .optional_file(&self.active(RELEASE_TEMP), hasher)?
                    .as_ref()
                    != staged
            {
                return Err(Error::EvidenceChanged);
            }
            self.history().verify_directories(next, false)?;
            self.history().verify_files(next, hasher, false)?;
            self.history().verify_runtime(next, hasher)?;
            self.history().verify_preparation(next, hasher)?;
            let index = self.history().load_index(hasher)?;
            if index != next.previous_index
                && !(next.phase == TerminalReleasePhase::ReleaseReady
                    && index.as_ref().map(|v| &v.index)
                        == Some(&self.history().index_for(next, hasher)?))
            {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        };
        verify()?;
        if !port.confirm_authority_and_quiescence(
            next,
            point,
            TerminalReleaseOuterRequirement::Nonterminal,
        ) {
            return Err(Error::AuthorityNotProven);
        }
        verify()
    }
    pub(super) fn publish_index(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
        record: &TerminalReleaseReceipt,
    ) -> Result<()> {
        self.checkpoint(
            guard,
            port,
            hasher,
            record,
            Point::BeforeIndexWrite,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        let expected = self.history().index_for(record, hasher)?;
        let root = &self.paths(record)[0];
        let path = root.join(INDEX);
        if let Some(current) = self.history().load_index(hasher)? {
            if current.index == expected {
                self.sync_file(&path, &current.identity, hasher)?;
                sync_directory(root)?;
                return self.history().require_published_index(record, hasher);
            }
        }
        let staged = root.join(INDEX_TEMP);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        let initial = self.journal.evidence(&staged, hasher)?;
        if file.metadata().map_err(|_| Error::Io)?.ino() != initial.inode {
            return Err(Error::EvidenceChanged);
        }
        self.index_checkpoint(guard, port, hasher, record, Point::IndexCreated, &initial)?;
        let bytes = encode(&expected)?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let written = self.journal.evidence(&staged, hasher)?;
        if !same_object(&initial, &written) {
            return Err(Error::EvidenceChanged);
        }
        self.index_checkpoint(guard, port, hasher, record, Point::IndexWritten, &written)?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.index_checkpoint(
            guard,
            port,
            hasher,
            record,
            Point::IndexFileSynced,
            &written,
        )?;
        if self.journal.read_history_bytes(&staged)? != bytes
            || self.journal.evidence(&staged, hasher)? != written
            || self
                .history()
                .load_index_with_temp(hasher, Some(&written))?
                != record.previous_index
        {
            return Err(Error::EvidenceChanged);
        }
        fs::rename(&staged, &path).map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            port,
            hasher,
            record,
            Point::IndexRenamed,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        sync_directory(root)?;
        self.checkpoint(
            guard,
            port,
            hasher,
            record,
            Point::IndexDirectorySynced,
            TerminalReleaseOuterRequirement::Nonterminal,
        )?;
        self.history().require_published_index(record, hasher)
    }
    fn index_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl TerminalReleasePort,
        hasher: &impl PreparationHasher,
        record: &TerminalReleaseReceipt,
        point: Point,
        temporary: &PreparationFileIdentity,
    ) -> Result<()> {
        self.verify_live_with_index_temp(guard, record, hasher, Some(temporary))?;
        let marker = self.journal.evidence(&self.active(RELEASE), hasher)?;
        if !port.confirm_authority_and_quiescence(
            record,
            point,
            TerminalReleaseOuterRequirement::Nonterminal,
        ) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_live_with_index_temp(guard, record, hasher, Some(temporary))?;
        if self.journal.evidence(&self.active(RELEASE), hasher)? != marker {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
