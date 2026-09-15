use super::*;

impl PreparationJournalStore {
    pub(super) fn recover_source_journal(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
    ) -> Result<PreparationReceipt> {
        if record.phase() != PreparationPhase::MaintenanceIntent {
            return Err(Error::InvalidPhase);
        }
        self.checkpoint(
            guard,
            port,
            record,
            SourcePreparationCheckpoint::BeforeJournalRecovery,
        )?;
        self.verify_snapshot(record, hasher)?;
        let family = self.read_family(hasher)?;
        if family.wal.is_some()
            || family.shm.is_some()
            || family.journal.is_none()
            || !same_object(&family.database, &record.initial_source().database)
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(recovery) = record.journal_recovery() {
            if family.journal != recovery.journal
                || !same_object(&family.database, &recovery.database)
            {
                return Err(Error::EvidenceChanged);
            }
        }
        let source = self.source_path();
        let snapshot = self.path(PREPARATION_SNAPSHOT);
        if Some(UserDb::qualify_maintenance_journal(&source, &snapshot)?)
            != record.snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
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
            SourcePreparationCheckpoint::BeforeJournalRecovery,
        )?;
        self.verify_snapshot(record, hasher)?;
        if self.read_family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        let mut next = record.clone();
        if next.journal_recovery().is_none() {
            next.record_journal_recovery(family.clone())
                .map_err(|_| Error::EvidenceChanged)?;
            self.persist(guard, Some(record), &next)?;
        }
        self.checkpoint(
            guard,
            port,
            &next,
            SourcePreparationCheckpoint::JournalRecoveryIntentRecorded,
        )?;
        self.verify_snapshot(&next, hasher)?;
        if self.read_family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        if Some(UserDb::recover_maintenance_journal(&source, &snapshot)?)
            != next.snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
        }
        let restored = self.family(hasher)?;
        no_sidecars(&source)?;
        if !same_object(&restored.database, &family.database) {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            port,
            &next,
            SourcePreparationCheckpoint::JournalRecovered,
        )?;
        self.verify_snapshot(&next, hasher)?;
        if self.family(hasher)? != restored {
            return Err(Error::EvidenceChanged);
        }
        // Normal preparation will repeat DELETE/content verification and fsync
        // the data root before recording source_prepared. Keep the recovery
        // identity permanently, including when SQLite has removed the journal.
        Ok(next)
    }
}
