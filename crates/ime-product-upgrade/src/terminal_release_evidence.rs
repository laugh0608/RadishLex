//! Physical evidence for release and subsequent read-only history lookup.
use super::*;

impl TerminalReleaseStore {
    pub(super) fn validate_active_entries(&self) -> Result<()> {
        for entry in fs::read_dir(&self.journal.store.state_directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if entry.file_name() != RELEASE
                && !SLOTS
                    .iter()
                    .any(|slot| entry.file_name() == slot_name(*slot))
            {
                // All unproven temporaries, preparation markers and unknown
                // objects block. A writer checks its own exclusive temp separately.
                return Err(Error::EvidenceChanged);
            }
            self.journal.private_metadata(&entry.path())?;
        }
        Ok(())
    }
    pub(super) fn verify_live(
        &self,
        guard: &UpgradeProcessGuard,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify_live_with_index_temp(guard, record, hasher, None)
    }
    pub(super) fn verify_live_with_index_temp(
        &self,
        guard: &UpgradeProcessGuard,
        record: &TerminalReleaseReceipt,
        hasher: &impl PreparationHasher,
        temporary: Option<&PreparationFileIdentity>,
    ) -> Result<()> {
        self.journal.verify_guard(guard)?;
        self.validate_active_entries()?;
        record.validate()?;
        self.history().verify_directories(record, false)?;
        let paths = self.paths(record);
        let marker = self.unique_location(&self.active(RELEASE), &paths[1].join(RELEASE))?;
        if self.history().read_release(&marker, hasher)? != *record
            || (marker != self.active(RELEASE)
                && record.phase != TerminalReleasePhase::ReleaseReady)
        {
            return Err(Error::EvidenceChanged);
        }
        self.history().verify_files(record, hasher, false)?;
        self.history().verify_runtime(record, hasher)?;
        self.history().verify_preparation(record, hasher)?;
        let actual = self.history().load_index_with_temp(hasher, temporary)?;
        if actual != record.previous_index
            && !(record.phase == TerminalReleasePhase::ReleaseReady
                && actual.as_ref().map(|v| &v.index)
                    == Some(&self.history().index_for(record, hasher)?))
        {
            return Err(Error::EvidenceChanged);
        }
        if marker != self.active(RELEASE) {
            self.history().require_published_index(record, hasher)?;
        }
        self.journal.verify_guard(guard)?;
        Ok(())
    }
}
