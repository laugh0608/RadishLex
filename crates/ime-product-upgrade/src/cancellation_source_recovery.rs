//! Only the existing qualified, single rollback journal recovery is admitted.
use super::*;

impl PreparationCancellationStore {
    pub(super) fn recover_cancelled_source(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        progress: CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationSourceReceipt> {
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeJournalRecovery,
        )?;
        let family = self.journal.read_family(hasher)?;
        let source = self.journal.source_path();
        let snapshot = self.journal.path(PREPARATION_SNAPSHOT);
        if family.journal.is_none() || family.wal.is_some() || family.shm.is_some() {
            return Err(Error::EvidenceChanged);
        }
        if Some(UserDb::qualify_maintenance_journal(&source, &snapshot)?)
            != progress.preparation().snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
        }
        self.source_budget(&progress, port)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeJournalRecovery,
        )?;
        if self.journal.read_family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        let progress = if progress.preparation().journal_recovery().is_none() {
            let next = progress.with_recovery(family.clone())?;
            self.persist_source(guard, request, Some(&progress), &next, port, hasher)?;
            next
        } else {
            progress
        };
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::JournalRecoveryIntentRecorded,
        )?;
        if self.journal.read_family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        if Some(UserDb::recover_maintenance_journal(&source, &snapshot)?)
            != progress.preparation().snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
        }
        let restored = self.journal.family(hasher)?;
        no_sidecars(&source)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::JournalRecovered,
        )?;
        if self.journal.family(hasher)? != restored
            || !same_object(&family.database, &restored.database)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(progress)
    }
}
