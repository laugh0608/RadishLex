use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use super::*;

const SETTINGS_FILE_NAME: &str = "manager-settings.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeSettingsBackupSummary {
    backup_identity: UpgradeArtifactIdentity,
}

impl UpgradeSettingsBackupSummary {
    pub fn backup_identity(&self) -> &UpgradeArtifactIdentity {
        &self.backup_identity
    }

    pub const fn byte_len(&self) -> u64 {
        self.backup_identity.byte_len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsFaultPoint {
    StagedFileCreated,
    SettingsCopied,
    BackupRenamed,
    ReceiptEvidencePersisted,
}

trait SettingsFaultInjector {
    fn checkpoint(&self, _point: SettingsFaultPoint) -> Result<(), UpgradeFilesystemError> {
        Ok(())
    }
}

struct NoSettingsFaults;

impl SettingsFaultInjector for NoSettingsFaults {}

impl UpgradeReceiptStore {
    pub fn create_settings_backup(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
    ) -> Result<UpgradeSettingsBackupSummary, UpgradeFilesystemError> {
        self.create_settings_backup_with_faults(guard, receipt, &NoSettingsFaults)
    }

    fn create_settings_backup_with_faults<F: SettingsFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        faults: &F,
    ) -> Result<UpgradeSettingsBackupSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if receipt.state() != UpgradeState::Quiesced {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSettingsState));
        }
        let current_receipt = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSettingsState))?;
        if current_receipt != *receipt
            || receipt.artifacts().iter().any(|artifact| {
                matches!(
                    artifact.slot(),
                    UpgradeArtifactSlot::BackupSettings
                        | UpgradeArtifactSlot::SnapshotDatabase
                        | UpgradeArtifactSlot::CandidateDatabase
                )
            })
        {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSettingsState));
        }
        let source_identity = receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceSettings)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSettingsState))?;
        if path_exists(&self.staged_settings_backup_path())?
            || path_exists(&self.settings_backup_path())?
        {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedSettingsBackup));
        }

        let source_path = self.root.path.join(SETTINGS_FILE_NAME);
        let source_metadata = snapshot::private_data_file_metadata(
            &source_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if !snapshot::file_artifact_matches_metadata(source_identity, &source_metadata) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        let mut source_file = File::open(&source_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?;
        if FileIdentity::from_metadata(
            &source_file
                .metadata()
                .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?,
        ) != FileIdentity::from_metadata(&source_metadata)
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }

        let staged_path = self.staged_settings_backup_path();
        let mut staged_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::AlreadyExists {
                    error(UpgradeFilesystemErrorCode::InterruptedSettingsBackup)
                } else {
                    error(UpgradeFilesystemErrorCode::SettingsBackupFailed)
                }
            })?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?;
        let staged_identity = FileIdentity::from_metadata(
            &staged_file
                .metadata()
                .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?,
        );
        faults.checkpoint(SettingsFaultPoint::StagedFileCreated)?;

        let copied = io::copy(&mut source_file, &mut staged_file)
            .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?;
        if copied != source_metadata.len() {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        staged_file
            .sync_all()
            .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?;
        drop(staged_file);
        faults.checkpoint(SettingsFaultPoint::SettingsCopied)?;

        let staged_metadata = snapshot::private_data_file_metadata(
            &staged_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if staged_metadata.dev() != staged_identity.device_id
            || staged_metadata.ino() != staged_identity.inode
            || staged_metadata.len() != source_metadata.len()
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        self.revalidate()?;
        guard.revalidate()?;
        let source_after = snapshot::private_data_file_metadata(
            &source_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if FileIdentity::from_metadata(&source_after)
            != FileIdentity::from_metadata(&source_metadata)
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }

        fs::rename(&staged_path, self.settings_backup_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::SettingsBackupFailed))?;
        sync_directory(&self.state_directory)?;
        faults.checkpoint(SettingsFaultPoint::BackupRenamed)?;
        let backup_metadata = snapshot::private_data_file_metadata(
            &self.settings_backup_path(),
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        let backup_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::BackupSettings,
            backup_metadata.dev(),
            backup_metadata.ino(),
            backup_metadata.uid(),
            backup_metadata.len(),
        )
        .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;

        let mut next_receipt = receipt.clone();
        next_receipt
            .record_artifact(backup_identity.clone())
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSettingsState))?;
        self.persist(guard, &next_receipt)?;
        faults.checkpoint(SettingsFaultPoint::ReceiptEvidencePersisted)?;
        *receipt = next_receipt;

        Ok(UpgradeSettingsBackupSummary { backup_identity })
    }

    fn settings_backup_path(&self) -> PathBuf {
        self.state_directory.join(SETTINGS_BACKUP_FILE_NAME)
    }

    fn staged_settings_backup_path(&self) -> PathBuf {
        self.state_directory.join(STAGED_SETTINGS_BACKUP_FILE_NAME)
    }
}

pub(super) fn settings_backup_is_ready(receipt: &UpgradeReceipt) -> bool {
    let source_present = receipt
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceSettings);
    let backup_present = receipt
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::BackupSettings);
    source_present == backup_present
}

pub(super) fn validate_settings_backup_state(
    store: &UpgradeReceiptStore,
    receipt: Option<&UpgradeReceipt>,
) -> Result<(), UpgradeFilesystemError> {
    if path_exists(&store.staged_settings_backup_path())? {
        return Err(error(UpgradeFilesystemErrorCode::InterruptedSettingsBackup));
    }
    let backup_exists = path_exists(&store.settings_backup_path())?;
    let recorded_backup = receipt.and_then(|receipt| {
        receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::BackupSettings)
    });
    match (backup_exists, recorded_backup) {
        (false, None) => Ok(()),
        (true, Some(identity)) => {
            let metadata = snapshot::private_data_file_metadata(
                &store.settings_backup_path(),
                store.root.expected_owner_id,
                UpgradeFilesystemErrorCode::IdentityChanged,
            )?;
            if snapshot::file_artifact_matches_metadata(identity, &metadata) {
                Ok(())
            } else {
                Err(error(UpgradeFilesystemErrorCode::IdentityChanged))
            }
        }
        (true, None) => Err(error(UpgradeFilesystemErrorCode::InterruptedSettingsBackup)),
        (false, Some(_)) => Err(error(UpgradeFilesystemErrorCode::IdentityChanged)),
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
