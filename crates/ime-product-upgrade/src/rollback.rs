use std::fs;

use radishlex_ime_userdb::UserDb;

use super::*;

pub const UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeRollbackValidationEvidence {
    version: u32,
    source_schema_version: i64,
    source_release_opened: u32,
    source_database_checked: u32,
}

impl UpgradeRollbackValidationEvidence {
    pub const fn new(
        version: u32,
        source_schema_version: i64,
        source_release_opened: u32,
        source_database_checked: u32,
    ) -> Self {
        Self {
            version,
            source_schema_version,
            source_release_opened,
            source_database_checked,
        }
    }

    const fn is_valid_for(self, source_schema_version: i64) -> bool {
        self.version == UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION
            && self.source_schema_version == source_schema_version
            && self.source_release_opened == 1
            && self.source_database_checked == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeRollbackRestoreDisposition {
    RestoredAwaitingValidation,
    AlreadyRestoredAwaitingValidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeRollbackRestoreSummary {
    disposition: UpgradeRollbackRestoreDisposition,
}

impl UpgradeRollbackRestoreSummary {
    pub const fn disposition(self) -> UpgradeRollbackRestoreDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeRollbackValidationDisposition {
    RolledBack,
    AlreadyRolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeRollbackValidationSummary {
    disposition: UpgradeRollbackValidationDisposition,
}

impl UpgradeRollbackValidationSummary {
    pub const fn disposition(self) -> UpgradeRollbackValidationDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollbackScene {
    Switched,
    CandidateReturned,
    SourceRestored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollbackFaultPoint {
    CandidateReturned,
    BeforeCandidateReturnDestinationSync,
    CandidateReturnDestinationSynced,
    BeforeCandidateReturnSourceSync,
    CandidateReturnSourceSynced,
    SourceRestored,
    BeforeSourceRestoreDestinationSync,
    SourceRestoreDestinationSynced,
    BeforeSourceRestoreSourceSync,
    SourceRestoreSourceSynced,
    RolledBackReceiptPersisted,
}

trait RollbackFaultInjector {
    fn checkpoint(&self, _point: RollbackFaultPoint) -> Result<(), UpgradeFilesystemError> {
        Ok(())
    }
}

struct NoRollbackFaults;

impl RollbackFaultInjector for NoRollbackFaults {}

impl UpgradeReceiptStore {
    pub fn restore_userdb_backup(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &UpgradeReceipt,
    ) -> Result<UpgradeRollbackRestoreSummary, UpgradeFilesystemError> {
        self.restore_userdb_backup_with_faults(guard, receipt, &NoRollbackFaults)
    }

    fn restore_userdb_backup_with_faults<F: RollbackFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &UpgradeReceipt,
        faults: &F,
    ) -> Result<UpgradeRollbackRestoreSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        let stored = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        if stored != *receipt || receipt.state() != UpgradeState::RollbackRequired {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }
        validate_rollback_state(self, receipt)?;

        let source_identity = switch::artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
        let candidate_identity = switch::artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
        let backup_identity = switch::artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
        let mut scene =
            classify_rollback_scene(self, source_identity, candidate_identity, backup_identity)?;

        if scene == RollbackScene::Switched {
            self.revalidate()?;
            guard.revalidate()?;
            fs::rename(self.active_userdb_path(), self.candidate_path())
                .map_err(|_| error(UpgradeFilesystemErrorCode::SwitchFailed))?;
            faults.checkpoint(RollbackFaultPoint::CandidateReturned)?;
            sync_candidate_return(self, faults)?;
            scene = require_rollback_scene(
                self,
                source_identity,
                candidate_identity,
                backup_identity,
                RollbackScene::CandidateReturned,
            )?;
        } else if scene == RollbackScene::CandidateReturned {
            sync_candidate_return(self, faults)?;
        }

        if scene == RollbackScene::CandidateReturned {
            self.revalidate()?;
            guard.revalidate()?;
            fs::rename(self.source_backup_path(), self.active_userdb_path())
                .map_err(|_| error(UpgradeFilesystemErrorCode::SwitchFailed))?;
            faults.checkpoint(RollbackFaultPoint::SourceRestored)?;
            sync_source_restore(self, faults)?;
        } else {
            sync_source_restore(self, faults)?;
        }

        require_rollback_scene(
            self,
            source_identity,
            candidate_identity,
            backup_identity,
            RollbackScene::SourceRestored,
        )?;
        Ok(UpgradeRollbackRestoreSummary {
            disposition: if scene == RollbackScene::SourceRestored {
                UpgradeRollbackRestoreDisposition::AlreadyRestoredAwaitingValidation
            } else {
                UpgradeRollbackRestoreDisposition::RestoredAwaitingValidation
            },
        })
    }

    pub fn record_rollback_validation(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        evidence: UpgradeRollbackValidationEvidence,
    ) -> Result<UpgradeRollbackValidationSummary, UpgradeFilesystemError> {
        self.record_rollback_validation_with_faults(guard, receipt, evidence, &NoRollbackFaults)
    }

    fn record_rollback_validation_with_faults<F: RollbackFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        evidence: UpgradeRollbackValidationEvidence,
        faults: &F,
    ) -> Result<UpgradeRollbackValidationSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        let stored = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        if stored != *receipt {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }
        validate_rollback_state(self, receipt)?;
        if receipt.state() == UpgradeState::RolledBack {
            return Ok(UpgradeRollbackValidationSummary {
                disposition: UpgradeRollbackValidationDisposition::AlreadyRolledBack,
            });
        }
        if receipt.state() != UpgradeState::RollbackRequired {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }
        let source_schema_version = receipt
            .source_schema_version()
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        if !evidence.is_valid_for(source_schema_version) {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }
        validate_exact_restored_scene(self, receipt)?;
        let inspection = UserDb::inspect_file(self.active_userdb_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::SwitchFailed))?;
        if inspection.schema_version != source_schema_version {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }

        let mut next_receipt = receipt.clone();
        next_receipt
            .mark_rolled_back()
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;
        faults.checkpoint(RollbackFaultPoint::RolledBackReceiptPersisted)?;
        Ok(UpgradeRollbackValidationSummary {
            disposition: UpgradeRollbackValidationDisposition::RolledBack,
        })
    }
}

pub(super) fn validate_rollback_state(
    store: &UpgradeReceiptStore,
    receipt: &UpgradeReceipt,
) -> Result<(), UpgradeFilesystemError> {
    switch::validate_switch_sidecars(store)?;
    let source_identity = switch::artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
    let candidate_identity = switch::artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
    let backup_identity = switch::artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
    switch::validate_same_filesystem(store, source_identity, candidate_identity, backup_identity)?;
    match receipt.state() {
        UpgradeState::RollbackRequired => {
            classify_rollback_scene(store, source_identity, candidate_identity, backup_identity)
                .map(|_| ())
        }
        UpgradeState::RolledBack => require_rollback_scene(
            store,
            source_identity,
            candidate_identity,
            backup_identity,
            RollbackScene::SourceRestored,
        )
        .map(|_| ()),
        _ => Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState)),
    }
}

fn validate_exact_restored_scene(
    store: &UpgradeReceiptStore,
    receipt: &UpgradeReceipt,
) -> Result<(), UpgradeFilesystemError> {
    if receipt.state() != UpgradeState::RollbackRequired {
        return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
    }
    validate_rollback_state(store, receipt)?;
    let source_identity = switch::artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
    let candidate_identity = switch::artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
    let backup_identity = switch::artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
    require_rollback_scene(
        store,
        source_identity,
        candidate_identity,
        backup_identity,
        RollbackScene::SourceRestored,
    )
    .map(|_| ())
}

fn classify_rollback_scene(
    store: &UpgradeReceiptStore,
    source_identity: &UpgradeArtifactIdentity,
    candidate_identity: &UpgradeArtifactIdentity,
    backup_identity: &UpgradeArtifactIdentity,
) -> Result<RollbackScene, UpgradeFilesystemError> {
    let active = switch::optional_private_metadata(
        &store.active_userdb_path(),
        store.root.expected_owner_id,
    )?;
    let candidate =
        switch::optional_private_metadata(&store.candidate_path(), store.root.expected_owner_id)?;
    let backup = switch::optional_private_metadata(
        &store.source_backup_path(),
        store.root.expected_owner_id,
    )?;

    match (active.as_ref(), candidate.as_ref(), backup.as_ref()) {
        (Some(active), None, Some(backup))
            if snapshot::file_artifact_matches_metadata(candidate_identity, active)
                && snapshot::file_artifact_matches_metadata(backup_identity, backup) =>
        {
            Ok(RollbackScene::Switched)
        }
        (None, Some(candidate), Some(backup))
            if snapshot::file_artifact_matches_metadata(candidate_identity, candidate)
                && snapshot::file_artifact_matches_metadata(backup_identity, backup) =>
        {
            Ok(RollbackScene::CandidateReturned)
        }
        (Some(active), Some(candidate), None)
            if snapshot::file_artifact_matches_metadata(source_identity, active)
                && snapshot::file_artifact_matches_metadata(candidate_identity, candidate) =>
        {
            Ok(RollbackScene::SourceRestored)
        }
        _ => Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState)),
    }
}

fn require_rollback_scene(
    store: &UpgradeReceiptStore,
    source_identity: &UpgradeArtifactIdentity,
    candidate_identity: &UpgradeArtifactIdentity,
    backup_identity: &UpgradeArtifactIdentity,
    expected: RollbackScene,
) -> Result<RollbackScene, UpgradeFilesystemError> {
    let actual =
        classify_rollback_scene(store, source_identity, candidate_identity, backup_identity)?;
    if actual == expected {
        Ok(actual)
    } else {
        Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState))
    }
}

fn sync_candidate_return<F: RollbackFaultInjector>(
    store: &UpgradeReceiptStore,
    faults: &F,
) -> Result<(), UpgradeFilesystemError> {
    faults.checkpoint(RollbackFaultPoint::BeforeCandidateReturnDestinationSync)?;
    sync_directory(&store.state_directory)?;
    faults.checkpoint(RollbackFaultPoint::CandidateReturnDestinationSynced)?;
    faults.checkpoint(RollbackFaultPoint::BeforeCandidateReturnSourceSync)?;
    sync_directory(&store.root.path)?;
    faults.checkpoint(RollbackFaultPoint::CandidateReturnSourceSynced)
}

fn sync_source_restore<F: RollbackFaultInjector>(
    store: &UpgradeReceiptStore,
    faults: &F,
) -> Result<(), UpgradeFilesystemError> {
    faults.checkpoint(RollbackFaultPoint::BeforeSourceRestoreDestinationSync)?;
    sync_directory(&store.root.path)?;
    faults.checkpoint(RollbackFaultPoint::SourceRestoreDestinationSynced)?;
    faults.checkpoint(RollbackFaultPoint::BeforeSourceRestoreSourceSync)?;
    sync_directory(&store.state_directory)?;
    faults.checkpoint(RollbackFaultPoint::SourceRestoreSourceSynced)
}

#[cfg(test)]
#[path = "rollback_tests.rs"]
mod tests;
