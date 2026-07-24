use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeSwitchDisposition {
    Switched,
    AlreadySwitched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeSwitchSummary {
    disposition: UpgradeSwitchDisposition,
    active_identity: UpgradeArtifactIdentity,
    backup_identity: UpgradeArtifactIdentity,
}

impl UpgradeSwitchSummary {
    pub const fn disposition(&self) -> UpgradeSwitchDisposition {
        self.disposition
    }

    pub fn active_identity(&self) -> &UpgradeArtifactIdentity {
        &self.active_identity
    }

    pub fn backup_identity(&self) -> &UpgradeArtifactIdentity {
        &self.backup_identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchFaultPoint {
    BackupIdentityPersisted,
    SwitchPreparedPersisted,
    BackupRenamed,
    BeforeBackupDestinationSync,
    BackupDestinationSynced,
    BeforeBackupSourceSync,
    BackupSourceSynced,
    CandidateRenamed,
    BeforeCandidateDestinationSync,
    CandidateDestinationSynced,
    BeforeCandidateSourceSync,
    CandidateSourceSynced,
    SwitchedReceiptPersisted,
}

trait SwitchFaultInjector {
    fn checkpoint(&self, _point: SwitchFaultPoint) -> Result<(), UpgradeFilesystemError> {
        Ok(())
    }
}

struct NoSwitchFaults;

impl SwitchFaultInjector for NoSwitchFaults {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchScene {
    Unmoved,
    BackupMoved,
    CandidateMoved,
}

impl UpgradeReceiptStore {
    pub fn switch_userdb_candidate(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
    ) -> Result<UpgradeSwitchSummary, UpgradeFilesystemError> {
        self.switch_userdb_candidate_with_faults(guard, receipt, &NoSwitchFaults)
    }

    fn switch_userdb_candidate_with_faults<F: SwitchFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        faults: &F,
    ) -> Result<UpgradeSwitchSummary, UpgradeFilesystemError> {
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
        validate_switch_state(self, Some(receipt))?;

        if receipt.state() == UpgradeState::Switched {
            return switch_summary(receipt, UpgradeSwitchDisposition::AlreadySwitched);
        }
        if !matches!(
            receipt.state(),
            UpgradeState::CandidateVerified | UpgradeState::SwitchPrepared
        ) {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }

        if receipt.state() == UpgradeState::CandidateVerified {
            self.prepare_switch(guard, receipt, faults)?;
        }

        let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
        let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
        let backup_identity = artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
        validate_distinct_database_objects(source_identity, candidate_identity)?;
        validate_same_filesystem(self, source_identity, candidate_identity, backup_identity)?;

        match classify_switch_scene(self, source_identity, candidate_identity, backup_identity)? {
            SwitchScene::Unmoved => {
                self.revalidate()?;
                guard.revalidate()?;
                fs::rename(self.active_userdb_path(), self.source_backup_path())
                    .map_err(|_| error(UpgradeFilesystemErrorCode::SwitchFailed))?;
                faults.checkpoint(SwitchFaultPoint::BackupRenamed)?;
                sync_backup_rename(self, faults)?;
                require_scene(
                    self,
                    source_identity,
                    candidate_identity,
                    backup_identity,
                    SwitchScene::BackupMoved,
                )?;
            }
            SwitchScene::BackupMoved => {
                sync_backup_rename(self, faults)?;
            }
            SwitchScene::CandidateMoved => {}
        }

        if classify_switch_scene(self, source_identity, candidate_identity, backup_identity)?
            == SwitchScene::BackupMoved
        {
            self.revalidate()?;
            guard.revalidate()?;
            fs::rename(self.candidate_path(), self.active_userdb_path())
                .map_err(|_| error(UpgradeFilesystemErrorCode::SwitchFailed))?;
            faults.checkpoint(SwitchFaultPoint::CandidateRenamed)?;
            sync_candidate_rename(self, faults)?;
        } else {
            sync_candidate_rename(self, faults)?;
        }

        require_scene(
            self,
            source_identity,
            candidate_identity,
            backup_identity,
            SwitchScene::CandidateMoved,
        )?;
        self.revalidate()?;
        guard.revalidate()?;

        let mut next_receipt = receipt.clone();
        next_receipt
            .advance(UpgradeState::Switched)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;
        faults.checkpoint(SwitchFaultPoint::SwitchedReceiptPersisted)?;

        switch_summary(receipt, UpgradeSwitchDisposition::Switched)
    }

    fn prepare_switch<F: SwitchFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        faults: &F,
    ) -> Result<(), UpgradeFilesystemError> {
        let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
        let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
        validate_distinct_database_objects(source_identity, candidate_identity)?;

        if let Some(existing_backup) =
            optional_artifact(receipt, UpgradeArtifactSlot::BackupDatabase)
        {
            if !same_database_object(source_identity, existing_backup) {
                return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
            }
        } else {
            let source_metadata =
                required_private_metadata(&self.active_userdb_path(), self.root.expected_owner_id)?;
            if !snapshot::file_artifact_matches_metadata(source_identity, &source_metadata) {
                return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
            }
            validate_same_filesystem_without_backup(self, source_identity, candidate_identity)?;
            ensure_no_database_sidecars(&self.active_userdb_path())?;
            ensure_no_database_sidecars(&self.candidate_path())?;
            ensure_no_database_sidecars(&self.source_backup_path())?;
            if optional_private_metadata(&self.source_backup_path(), self.root.expected_owner_id)?
                .is_some()
            {
                return Err(error(UpgradeFilesystemErrorCode::InterruptedSwitch));
            }

            let backup_identity =
                identity_with_slot(UpgradeArtifactSlot::BackupDatabase, source_identity)?;
            let mut next_receipt = receipt.clone();
            next_receipt
                .record_artifact(backup_identity)
                .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
            self.persist(guard, &next_receipt)?;
            *receipt = next_receipt;
            faults.checkpoint(SwitchFaultPoint::BackupIdentityPersisted)?;
        }

        validate_switch_state(self, Some(receipt))?;
        let mut next_receipt = receipt.clone();
        next_receipt
            .advance(UpgradeState::SwitchPrepared)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSwitchState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;
        faults.checkpoint(SwitchFaultPoint::SwitchPreparedPersisted)
    }

    pub(super) fn active_userdb_path(&self) -> PathBuf {
        self.root.path.join(snapshot::USERDB_FILE_NAME)
    }

    pub(super) fn source_backup_path(&self) -> PathBuf {
        self.state_directory.join(SOURCE_BACKUP_FILE_NAME)
    }
}

pub(super) fn validate_switch_state(
    store: &UpgradeReceiptStore,
    receipt: Option<&UpgradeReceipt>,
) -> Result<(), UpgradeFilesystemError> {
    let backup_exists =
        optional_private_metadata(&store.source_backup_path(), store.root.expected_owner_id)?
            .is_some();
    let Some(receipt) = receipt else {
        return if backup_exists {
            Err(error(UpgradeFilesystemErrorCode::InterruptedSwitch))
        } else {
            Ok(())
        };
    };

    match receipt.state() {
        UpgradeState::CandidateVerified => validate_candidate_verified_scene(store, receipt),
        UpgradeState::SwitchPrepared => {
            validate_switch_sidecars(store)?;
            let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
            let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
            let backup_identity = artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
            validate_distinct_database_objects(source_identity, candidate_identity)?;
            validate_same_filesystem(store, source_identity, candidate_identity, backup_identity)?;
            classify_switch_scene(store, source_identity, candidate_identity, backup_identity)
                .map(|_| ())
        }
        UpgradeState::Switched | UpgradeState::PostSwitchVerified | UpgradeState::Completed => {
            validate_switch_sidecars(store)?;
            let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
            let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
            let backup_identity = artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
            validate_same_filesystem(store, source_identity, candidate_identity, backup_identity)?;
            require_scene(
                store,
                source_identity,
                candidate_identity,
                backup_identity,
                SwitchScene::CandidateMoved,
            )
        }
        UpgradeState::RollbackRequired | UpgradeState::RolledBack => {
            rollback::validate_rollback_state(store, receipt)
        }
        _ => {
            if backup_exists {
                Err(error(UpgradeFilesystemErrorCode::InterruptedSwitch))
            } else {
                Ok(())
            }
        }
    }
}

pub(super) fn validate_exact_switched_scene(
    store: &UpgradeReceiptStore,
    receipt: &UpgradeReceipt,
) -> Result<(), UpgradeFilesystemError> {
    validate_switch_sidecars(store)?;
    let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
    let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
    let backup_identity = artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?;
    validate_same_filesystem(store, source_identity, candidate_identity, backup_identity)?;
    require_scene(
        store,
        source_identity,
        candidate_identity,
        backup_identity,
        SwitchScene::CandidateMoved,
    )
}

pub(super) fn validate_switch_sidecars(
    store: &UpgradeReceiptStore,
) -> Result<(), UpgradeFilesystemError> {
    for path in [
        store.active_userdb_path(),
        store.candidate_path(),
        store.source_backup_path(),
    ] {
        ensure_no_database_sidecars(&path)?;
    }
    Ok(())
}

fn validate_candidate_verified_scene(
    store: &UpgradeReceiptStore,
    receipt: &UpgradeReceipt,
) -> Result<(), UpgradeFilesystemError> {
    let source_identity = artifact(receipt, UpgradeArtifactSlot::SourceDatabase)?;
    let candidate_identity = artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?;
    validate_distinct_database_objects(source_identity, candidate_identity)?;
    validate_same_filesystem_without_backup(store, source_identity, candidate_identity)?;
    let source_metadata =
        required_private_metadata(&store.active_userdb_path(), store.root.expected_owner_id)?;
    let candidate_metadata =
        required_private_metadata(&store.candidate_path(), store.root.expected_owner_id)?;
    if !snapshot::file_artifact_matches_metadata(source_identity, &source_metadata)
        || !snapshot::file_artifact_matches_metadata(candidate_identity, &candidate_metadata)
        || optional_private_metadata(&store.source_backup_path(), store.root.expected_owner_id)?
            .is_some()
    {
        return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
    }
    if let Some(backup_identity) = optional_artifact(receipt, UpgradeArtifactSlot::BackupDatabase) {
        if !same_database_object(source_identity, backup_identity) {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState));
        }
    }
    Ok(())
}

fn classify_switch_scene(
    store: &UpgradeReceiptStore,
    source_identity: &UpgradeArtifactIdentity,
    candidate_identity: &UpgradeArtifactIdentity,
    backup_identity: &UpgradeArtifactIdentity,
) -> Result<SwitchScene, UpgradeFilesystemError> {
    let active =
        optional_private_metadata(&store.active_userdb_path(), store.root.expected_owner_id)?;
    let candidate =
        optional_private_metadata(&store.candidate_path(), store.root.expected_owner_id)?;
    let backup =
        optional_private_metadata(&store.source_backup_path(), store.root.expected_owner_id)?;

    match (active.as_ref(), candidate.as_ref(), backup.as_ref()) {
        (Some(active), Some(candidate), None)
            if snapshot::file_artifact_matches_metadata(source_identity, active)
                && snapshot::file_artifact_matches_metadata(candidate_identity, candidate) =>
        {
            Ok(SwitchScene::Unmoved)
        }
        (None, Some(candidate), Some(backup))
            if snapshot::file_artifact_matches_metadata(candidate_identity, candidate)
                && snapshot::file_artifact_matches_metadata(backup_identity, backup) =>
        {
            Ok(SwitchScene::BackupMoved)
        }
        (Some(active), None, Some(backup))
            if snapshot::file_artifact_matches_metadata(candidate_identity, active)
                && snapshot::file_artifact_matches_metadata(backup_identity, backup) =>
        {
            Ok(SwitchScene::CandidateMoved)
        }
        _ => Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState)),
    }
}

fn require_scene(
    store: &UpgradeReceiptStore,
    source_identity: &UpgradeArtifactIdentity,
    candidate_identity: &UpgradeArtifactIdentity,
    backup_identity: &UpgradeArtifactIdentity,
    expected: SwitchScene,
) -> Result<(), UpgradeFilesystemError> {
    if classify_switch_scene(store, source_identity, candidate_identity, backup_identity)?
        == expected
    {
        Ok(())
    } else {
        Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState))
    }
}

fn sync_backup_rename<F: SwitchFaultInjector>(
    store: &UpgradeReceiptStore,
    faults: &F,
) -> Result<(), UpgradeFilesystemError> {
    faults.checkpoint(SwitchFaultPoint::BeforeBackupDestinationSync)?;
    sync_directory(&store.state_directory)?;
    faults.checkpoint(SwitchFaultPoint::BackupDestinationSynced)?;
    faults.checkpoint(SwitchFaultPoint::BeforeBackupSourceSync)?;
    sync_directory(&store.root.path)?;
    faults.checkpoint(SwitchFaultPoint::BackupSourceSynced)
}

fn sync_candidate_rename<F: SwitchFaultInjector>(
    store: &UpgradeReceiptStore,
    faults: &F,
) -> Result<(), UpgradeFilesystemError> {
    faults.checkpoint(SwitchFaultPoint::BeforeCandidateDestinationSync)?;
    sync_directory(&store.root.path)?;
    faults.checkpoint(SwitchFaultPoint::CandidateDestinationSynced)?;
    faults.checkpoint(SwitchFaultPoint::BeforeCandidateSourceSync)?;
    sync_directory(&store.state_directory)?;
    faults.checkpoint(SwitchFaultPoint::CandidateSourceSynced)
}

pub(super) fn artifact(
    receipt: &UpgradeReceipt,
    slot: UpgradeArtifactSlot,
) -> Result<&UpgradeArtifactIdentity, UpgradeFilesystemError> {
    optional_artifact(receipt, slot)
        .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSwitchState))
}

fn optional_artifact(
    receipt: &UpgradeReceipt,
    slot: UpgradeArtifactSlot,
) -> Option<&UpgradeArtifactIdentity> {
    receipt
        .artifacts()
        .iter()
        .find(|artifact| artifact.slot() == slot)
}

fn identity_with_slot(
    slot: UpgradeArtifactSlot,
    source: &UpgradeArtifactIdentity,
) -> Result<UpgradeArtifactIdentity, UpgradeFilesystemError> {
    UpgradeArtifactIdentity::new(
        slot,
        source.device_id(),
        source.inode(),
        source.owner_id(),
        source.mode(),
        source.link_count(),
        source.byte_len(),
    )
    .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSwitchState))
}

fn validate_distinct_database_objects(
    source: &UpgradeArtifactIdentity,
    candidate: &UpgradeArtifactIdentity,
) -> Result<(), UpgradeFilesystemError> {
    if source.device_id() == candidate.device_id() && source.inode() == candidate.inode() {
        Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState))
    } else {
        Ok(())
    }
}

fn validate_same_filesystem_without_backup(
    store: &UpgradeReceiptStore,
    source: &UpgradeArtifactIdentity,
    candidate: &UpgradeArtifactIdentity,
) -> Result<(), UpgradeFilesystemError> {
    let expected = store.root.identity.device_id();
    if store.state_directory_identity.device_id == expected
        && source.device_id() == expected
        && candidate.device_id() == expected
    {
        Ok(())
    } else {
        Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState))
    }
}

pub(super) fn validate_same_filesystem(
    store: &UpgradeReceiptStore,
    source: &UpgradeArtifactIdentity,
    candidate: &UpgradeArtifactIdentity,
    backup: &UpgradeArtifactIdentity,
) -> Result<(), UpgradeFilesystemError> {
    validate_same_filesystem_without_backup(store, source, candidate)?;
    if backup.device_id() == store.root.identity.device_id() && same_database_object(source, backup)
    {
        Ok(())
    } else {
        Err(error(UpgradeFilesystemErrorCode::InvalidSwitchState))
    }
}

fn same_database_object(left: &UpgradeArtifactIdentity, right: &UpgradeArtifactIdentity) -> bool {
    left.device_id() == right.device_id()
        && left.inode() == right.inode()
        && left.owner_id() == right.owner_id()
        && left.mode() == right.mode()
        && left.link_count() == right.link_count()
        && left.byte_len() == right.byte_len()
}

pub(super) fn optional_private_metadata(
    path: &Path,
    expected_owner_id: u32,
) -> Result<Option<Metadata>, UpgradeFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(_) => required_private_metadata(path, expected_owner_id).map(Some),
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(error(UpgradeFilesystemErrorCode::SwitchFailed)),
    }
}

fn required_private_metadata(
    path: &Path,
    expected_owner_id: u32,
) -> Result<Metadata, UpgradeFilesystemError> {
    snapshot::private_data_file_metadata(
        path,
        expected_owner_id,
        UpgradeFilesystemErrorCode::IdentityChanged,
    )
}

pub(super) fn ensure_no_database_sidecars(path: &Path) -> Result<(), UpgradeFilesystemError> {
    let path = path.as_os_str().to_string_lossy();
    for sidecar in [
        PathBuf::from(format!("{path}-wal")),
        PathBuf::from(format!("{path}-shm")),
        PathBuf::from(format!("{path}-journal")),
    ] {
        if path_exists(&sidecar)? {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedSwitch));
        }
    }
    Ok(())
}

fn switch_summary(
    receipt: &UpgradeReceipt,
    disposition: UpgradeSwitchDisposition,
) -> Result<UpgradeSwitchSummary, UpgradeFilesystemError> {
    Ok(UpgradeSwitchSummary {
        disposition,
        active_identity: artifact(receipt, UpgradeArtifactSlot::CandidateDatabase)?.clone(),
        backup_identity: artifact(receipt, UpgradeArtifactSlot::BackupDatabase)?.clone(),
    })
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
