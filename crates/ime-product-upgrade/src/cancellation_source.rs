//! Finishes only the admitted preparation's source work. Immutable admission
//! evidence and the original preparation marker are never replaced.
use super::*;

pub(super) const SOURCE_PROGRESS: &str = "cancellation-source.json";
pub(super) const STAGED_SOURCE_PROGRESS: &str = "cancellation-source.json.tmp";

#[path = "cancellation_source_record.rs"]
mod source_record;
pub use source_record::{CancellationSourcePhase, CancellationSourceReceipt};
#[path = "cancellation_source_persistence.rs"]
mod persistence;
#[path = "cancellation_source_recovery.rs"]
mod recovery;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationSourceCheckpoint {
    Begin,
    BeforeMaintenance,
    BeforeJournalRecovery,
    JournalRecoveryIntentRecorded,
    JournalRecovered,
    SourceMaintained,
    BeforeSourceSync,
    SourceFileSynced,
    SourceDirectorySynced,
    BeforeVerify,
    SourceVerified,
    Write(CancellationSourcePhase, PreparationCancellationCheckpoint),
    SourceReady,
}
use CancellationSourceCheckpoint as SourcePoint;

/// Same fresh outer/product/quiescence authority as cancellation admission,
/// rechecked for every source checkpoint, including persistence. Must keep both
/// guards and unchanged source programs. Capacity is for the bound data root.
/// Neither this port nor a SourceReady receipt permits startup or outer release.
pub trait CancellationSourcePort {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        progress: &CancellationSourceReceipt,
        checkpoint: CancellationSourceCheckpoint,
    ) -> bool;
    fn available_bytes(&mut self) -> Option<u64>;
}

impl PreparationCancellationStore {
    /// Read-only physical reload. Logical equivalence is freshly rechecked by
    /// finish_source; no SQLite connection or product authority is implied here.
    pub fn load_source_guarded(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<CancellationSourceReceipt>> {
        self.journal.verify_guard(guard)?;
        self.validate_source_entries()?;
        self.reject_source_staged()?;
        let (request, request_identity) = self.read_request(hasher)?.ok_or(Error::InvalidPhase)?;
        let Some((progress, identity)) = self.read_source_progress(hasher)? else {
            self.verify_request(guard, &request, hasher)?;
            return Ok(None);
        };
        if progress.request_identity() != &request_identity {
            return Err(Error::EvidenceChanged);
        }
        self.verify_source_progress(guard, &request, &progress, hasher)?;
        if self
            .journal
            .evidence(&self.journal.path(SOURCE_PROGRESS), hasher)?
            != identity
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some(progress))
    }

    /// Completes source preservation, not the full cancellation. Pre-intent
    /// paths do not call SQLite. Post-intent paths finish the original bounded
    /// maintenance and compare all persisted content against its sealed snapshot.
    pub fn finish_source(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationSourceReceipt> {
        self.journal.verify_guard(guard)?;
        self.validate_source_entries()?;
        self.reject_source_staged()?;
        let (request, identity) = self.read_request(hasher)?.ok_or(Error::InvalidPhase)?;
        let mut progress = if let Some((progress, _)) = self.read_source_progress(hasher)? {
            if progress.request_identity() != &identity {
                return Err(Error::EvidenceChanged);
            }
            self.persist_source(guard, &request, Some(&progress), &progress, port, hasher)?;
            progress
        } else {
            self.verify_request(guard, &request, hasher)?;
            let next = CancellationSourceReceipt::new(&request, identity);
            self.persist_source(guard, &request, None, &next, port, hasher)?;
            next
        };
        self.source_checkpoint(guard, &request, &progress, port, hasher, SourcePoint::Begin)?;
        if progress.phase() == CancellationSourcePhase::SourceReady {
            self.verify_source_content(guard, &request, &progress, port, hasher)?;
            self.source_checkpoint(
                guard,
                &request,
                &progress,
                port,
                hasher,
                SourcePoint::SourceReady,
            )?;
            return Ok(progress);
        }
        if progress.preparation().phase() == PreparationPhase::MaintenanceIntent {
            progress = self.finish_source_maintenance(guard, &request, progress, port, hasher)?;
        }
        let family = self.journal.family(hasher)?;
        self.verify_source_content(guard, &request, &progress, port, hasher)?;
        if self.journal.family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        let next = progress.ready(family)?;
        self.persist_source(guard, &request, Some(&progress), &next, port, hasher)?;
        self.source_checkpoint(
            guard,
            &request,
            &next,
            port,
            hasher,
            SourcePoint::SourceReady,
        )?;
        Ok(next)
    }

    fn finish_source_maintenance(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        mut progress: CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationSourceReceipt> {
        if self.journal.read_family(hasher)?.journal.is_some() {
            progress = self.recover_cancelled_source(guard, request, progress, port, hasher)?;
        }
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeMaintenance,
        )?;
        let family = self.journal.family(hasher)?;
        self.source_budget(&progress, port)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeMaintenance,
        )?;
        if self.journal.family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        let source = self.journal.source_path();
        let snapshot = self.journal.path(PREPARATION_SNAPSHOT);
        if Some(UserDb::verify_maintenance_snapshot(&source, &snapshot)?)
            != progress.preparation().snapshot_schema_version()
        {
            return Err(Error::EvidenceChanged);
        }
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeMaintenance,
        )?;
        readonly_family(&family, &self.journal.family(hasher)?)?;
        let summary = UserDb::prepare_source_for_upgrade(&source, &snapshot)?;
        if Some(summary.schema_version) != progress.preparation().snapshot_schema_version() {
            return Err(Error::EvidenceChanged);
        }
        let prepared = self.journal.family(hasher)?;
        no_sidecars(&source)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::SourceMaintained,
        )?;
        self.verify_source_content(guard, request, &progress, port, hasher)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::BeforeSourceSync,
        )?;
        if self.journal.family(hasher)? != prepared {
            return Err(Error::EvidenceChanged);
        }
        let file = File::open(&source).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != prepared.database.device_id
            || metadata.ino() != prepared.database.inode
        {
            return Err(Error::EvidenceChanged);
        }
        file.sync_all().map_err(|_| Error::Io)?;
        drop(file);
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::SourceFileSynced,
        )?;
        sync_directory(&self.journal.store.root.path)?;
        self.source_checkpoint(
            guard,
            request,
            &progress,
            port,
            hasher,
            SourcePoint::SourceDirectorySynced,
        )?;
        if self.journal.family(hasher)? != prepared {
            return Err(Error::EvidenceChanged);
        }
        Ok(progress)
    }

    fn verify_source_content(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        progress: &CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.source_checkpoint(
            guard,
            request,
            progress,
            port,
            hasher,
            SourcePoint::BeforeVerify,
        )?;
        let before = self.journal.family(hasher)?;
        if request.preparation().phase() >= PreparationPhase::MaintenanceIntent {
            no_sidecars(&self.journal.source_path())?;
            if Some(UserDb::verify_prepared_source(
                self.journal.source_path(),
                self.journal.path(PREPARATION_SNAPSHOT),
            )?) != progress.preparation().snapshot_schema_version()
            {
                return Err(Error::EvidenceChanged);
            }
        }
        self.source_checkpoint(
            guard,
            request,
            progress,
            port,
            hasher,
            SourcePoint::SourceVerified,
        )?;
        if self.journal.family(hasher)? != before {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    fn source_budget(
        &self,
        progress: &CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
    ) -> Result<()> {
        let logical = progress
            .preparation()
            .snapshot_identity()
            .ok_or(Error::EvidenceChanged)?
            .byte_len;
        let settings = self.journal.store.root.path.join("manager-settings.json");
        let settings_bytes = if path_exists(&settings)? {
            self.journal.private_metadata(&settings)?.len()
        } else {
            0
        };
        let source_bytes = self
            .journal
            .private_metadata(&self.journal.source_path())?
            .len();
        SourcePreparationSpaceBudget::evaluate(
            logical.max(source_bytes),
            settings_bytes,
            port.available_bytes().ok_or(Error::InsufficientSpace)?,
        )?;
        Ok(())
    }

    fn source_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        progress: &CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
        point: SourcePoint,
    ) -> Result<()> {
        self.reject_source_staged()?;
        let stored = self
            .read_source_progress(hasher)?
            .ok_or(Error::EvidenceChanged)?;
        if &stored.0 != progress {
            return Err(Error::EvidenceChanged);
        }
        self.verify_source_progress(guard, request, progress, hasher)?;
        let family = self.journal.read_family(hasher)?;
        if !port.confirm_authority_and_quiescence(request, progress, point) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_source_progress(guard, request, progress, hasher)?;
        self.reject_source_staged()?;
        if self.read_source_progress(hasher)?.as_ref() != Some(&stored)
            || self.journal.read_family(hasher)? != family
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    fn verify_source_progress(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        progress: &CancellationSourceReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify_bound_materials(guard, request, hasher)?;
        progress.validate_request(request)?;
        self.verify_slot(Some((REQUEST, progress.request_identity())), hasher)?;
        let family = self.journal.read_family(hasher)?;
        // read_family already checks owner/mode/device/link and inode uniqueness.
        // The generic family contract rejects every journal; only the bounded
        // branch below may pass one to the existing qualification routine.
        if let Some(ready) = progress.source_ready() {
            if &family != ready {
                return Err(Error::EvidenceChanged);
            }
        } else if request.preparation().phase() != PreparationPhase::MaintenanceIntent {
            if &family != request.observed_source() {
                return Err(Error::EvidenceChanged);
            }
        } else if family.journal.is_some() {
            if family.wal.is_some()
                || family.shm.is_some()
                || !same_object(&family.database, &request.observed_source().database)
            {
                return Err(Error::EvidenceChanged);
            }
            if let Some(recovery) = progress.preparation().journal_recovery() {
                if family.journal != recovery.journal
                    || !same_object(&family.database, &recovery.database)
                {
                    return Err(Error::EvidenceChanged);
                }
            }
        } else {
            record::validate_source(progress.preparation(), &family)?;
        }
        Ok(())
    }

    pub(super) fn validate_source_entries(&self) -> Result<()> {
        self.journal.store.revalidate()?;
        self.journal.validate_entries_with_extra(&[
            REQUEST,
            STAGED_REQUEST,
            SOURCE_PROGRESS,
            STAGED_SOURCE_PROGRESS,
        ])?;
        Ok(())
    }
    fn reject_source_staged(&self) -> Result<()> {
        self.reject_staged()?;
        if path_exists(&self.journal.path(STAGED_SOURCE_PROGRESS))? {
            return Err(Error::Journal(PreparationJournalError::InterruptedWrite));
        }
        Ok(())
    }
}
