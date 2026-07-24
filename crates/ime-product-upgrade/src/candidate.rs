use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use radishlex_ime_userdb::{UserDb, UserDbSchemaCompatibility};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeCandidateSummary {
    candidate_identity: UpgradeArtifactIdentity,
    source_schema_version: i64,
    target_schema_version: i64,
    migrated: bool,
}

impl UpgradeCandidateSummary {
    pub fn candidate_identity(&self) -> &UpgradeArtifactIdentity {
        &self.candidate_identity
    }

    pub const fn source_schema_version(&self) -> i64 {
        self.source_schema_version
    }

    pub const fn target_schema_version(&self) -> i64 {
        self.target_schema_version
    }

    pub const fn migrated(&self) -> bool {
        self.migrated
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateFaultPoint {
    StagedFileCreated,
    SnapshotCopied,
    MigrationCompleted,
    CandidateRenamed,
    ReceiptEvidencePersisted,
}

trait CandidateFaultInjector {
    fn checkpoint(&self, _point: CandidateFaultPoint) -> Result<(), UpgradeFilesystemError> {
        Ok(())
    }
}

struct NoCandidateFaults;

impl CandidateFaultInjector for NoCandidateFaults {}

impl UpgradeReceiptStore {
    pub fn create_userdb_candidate(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
    ) -> Result<UpgradeCandidateSummary, UpgradeFilesystemError> {
        self.create_userdb_candidate_with_faults(guard, receipt, &NoCandidateFaults)
    }

    fn create_userdb_candidate_with_faults<F: CandidateFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        faults: &F,
    ) -> Result<UpgradeCandidateSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if receipt.state() != UpgradeState::SnapshotReady {
            return Err(error(UpgradeFilesystemErrorCode::InvalidCandidateState));
        }
        let current_receipt = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        if current_receipt != *receipt {
            return Err(error(UpgradeFilesystemErrorCode::InvalidCandidateState));
        }
        if let Some(candidate_identity) = receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::CandidateDatabase)
            .cloned()
        {
            return self.resume_recorded_candidate(guard, receipt, candidate_identity);
        }
        if path_exists(&self.staged_candidate_path())? || path_exists(&self.candidate_path())? {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedCandidate));
        }

        let snapshot_identity = receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        let snapshot_metadata = snapshot::private_data_file_metadata(
            &self.snapshot_path(),
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if !snapshot::file_artifact_matches_metadata(snapshot_identity, &snapshot_metadata) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        let snapshot_inspection = UserDb::inspect_file(self.snapshot_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::CandidateMigrationFailed))?;
        if receipt.source_schema_version() != Some(snapshot_inspection.schema_version)
            || snapshot_inspection.compatibility == UserDbSchemaCompatibility::Future
        {
            return Err(error(UpgradeFilesystemErrorCode::InvalidCandidateState));
        }

        let mut snapshot_file =
            File::open(self.snapshot_path()).map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        if FileIdentity::from_metadata(
            &snapshot_file
                .metadata()
                .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?,
        ) != FileIdentity::from_metadata(&snapshot_metadata)
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        let staged_path = self.staged_candidate_path();
        let mut staged_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::AlreadyExists {
                    error(UpgradeFilesystemErrorCode::InterruptedCandidate)
                } else {
                    error(UpgradeFilesystemErrorCode::Io)
                }
            })?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        let staged_identity = FileIdentity::from_metadata(
            &staged_file
                .metadata()
                .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?,
        );
        faults.checkpoint(CandidateFaultPoint::StagedFileCreated)?;

        let copied = io::copy(&mut snapshot_file, &mut staged_file)
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        if copied != snapshot_metadata.len() {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        staged_file
            .sync_all()
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        drop(staged_file);
        faults.checkpoint(CandidateFaultPoint::SnapshotCopied)?;

        let migration = UserDb::migrate_and_validate(&staged_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::CandidateMigrationFailed))?;
        if migration.source_schema_version != snapshot_inspection.schema_version
            || migration.target_schema_version != receipt.target_schema_version()
            || migration.target_schema_version != UserDb::supported_schema_version()
        {
            return Err(error(UpgradeFilesystemErrorCode::InvalidCandidateState));
        }
        faults.checkpoint(CandidateFaultPoint::MigrationCompleted)?;
        File::open(&staged_path)
            .and_then(|file| file.sync_all())
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        let staged_metadata = snapshot::private_data_file_metadata(
            &staged_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if !same_file_object(&staged_identity, &staged_metadata) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        ensure_no_sidecars(&staged_path)?;

        self.revalidate()?;
        guard.revalidate()?;
        let snapshot_after = snapshot::private_data_file_metadata(
            &self.snapshot_path(),
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if !snapshot::file_artifact_matches_metadata(snapshot_identity, &snapshot_after) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }

        fs::rename(&staged_path, self.candidate_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        sync_directory(&self.state_directory)?;
        faults.checkpoint(CandidateFaultPoint::CandidateRenamed)?;
        let candidate_metadata = snapshot::private_data_file_metadata(
            &self.candidate_path(),
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        let candidate_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::CandidateDatabase,
            candidate_metadata.dev(),
            candidate_metadata.ino(),
            candidate_metadata.uid(),
            candidate_metadata.len(),
        )
        .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;

        let mut next_receipt = receipt.clone();
        next_receipt
            .record_artifact(candidate_identity.clone())
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        self.persist(guard, &next_receipt)?;
        faults.checkpoint(CandidateFaultPoint::ReceiptEvidencePersisted)?;
        next_receipt
            .advance(UpgradeState::CandidateMigrated)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;

        Ok(UpgradeCandidateSummary {
            candidate_identity,
            source_schema_version: migration.source_schema_version,
            target_schema_version: migration.target_schema_version,
            migrated: migration.migrated,
        })
    }

    pub(super) fn candidate_path(&self) -> PathBuf {
        self.state_directory.join(CANDIDATE_FILE_NAME)
    }

    fn staged_candidate_path(&self) -> PathBuf {
        self.state_directory.join(STAGED_CANDIDATE_FILE_NAME)
    }

    fn resume_recorded_candidate(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        candidate_identity: UpgradeArtifactIdentity,
    ) -> Result<UpgradeCandidateSummary, UpgradeFilesystemError> {
        validate_candidate_state(self, Some(receipt))?;
        snapshot::validate_snapshot_state(self, Some(receipt))?;
        let snapshot = UserDb::inspect_file(self.snapshot_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::CandidateMigrationFailed))?;
        let candidate = UserDb::inspect_file(self.candidate_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::CandidateMigrationFailed))?;
        let source_schema_version = receipt
            .source_schema_version()
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        if snapshot.schema_version != source_schema_version
            || candidate.schema_version != receipt.target_schema_version()
            || candidate.compatibility != UserDbSchemaCompatibility::Current
        {
            return Err(error(UpgradeFilesystemErrorCode::InvalidCandidateState));
        }
        let mut next_receipt = receipt.clone();
        next_receipt
            .advance(UpgradeState::CandidateMigrated)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidCandidateState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;
        Ok(UpgradeCandidateSummary {
            candidate_identity,
            source_schema_version,
            target_schema_version: candidate.schema_version,
            migrated: source_schema_version != candidate.schema_version,
        })
    }
}

pub(super) fn validate_candidate_state(
    store: &UpgradeReceiptStore,
    receipt: Option<&UpgradeReceipt>,
) -> Result<(), UpgradeFilesystemError> {
    if path_exists(&store.staged_candidate_path())? {
        return Err(error(UpgradeFilesystemErrorCode::InterruptedCandidate));
    }
    let candidate_exists = path_exists(&store.candidate_path())?;
    let recorded_candidate = receipt.and_then(|receipt| {
        receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::CandidateDatabase)
    });
    match (candidate_exists, recorded_candidate) {
        (false, None) => Ok(()),
        (true, Some(identity)) => {
            let metadata = snapshot::private_data_file_metadata(
                &store.candidate_path(),
                store.root.expected_owner_id,
                UpgradeFilesystemErrorCode::IdentityChanged,
            )?;
            if snapshot::file_artifact_matches_metadata(identity, &metadata) {
                ensure_no_sidecars(&store.candidate_path())
            } else {
                Err(error(UpgradeFilesystemErrorCode::IdentityChanged))
            }
        }
        (true, None) => Err(error(UpgradeFilesystemErrorCode::InterruptedCandidate)),
        (false, Some(_))
            if receipt.is_some_and(|receipt| {
                matches!(
                    receipt.state(),
                    UpgradeState::SwitchPrepared
                        | UpgradeState::Switched
                        | UpgradeState::PostSwitchVerified
                        | UpgradeState::Completed
                        | UpgradeState::RollbackRequired
                        | UpgradeState::RolledBack
                )
            }) =>
        {
            Ok(())
        }
        (false, Some(_)) => Err(error(UpgradeFilesystemErrorCode::IdentityChanged)),
    }
}

fn same_file_object(identity: &FileIdentity, metadata: &Metadata) -> bool {
    identity.device_id == metadata.dev()
        && identity.inode == metadata.ino()
        && identity.owner_id == metadata.uid()
        && metadata.mode() & 0o7777 == 0o600
        && metadata.nlink() == 1
}

fn ensure_no_sidecars(path: &std::path::Path) -> Result<(), UpgradeFilesystemError> {
    let path = path.as_os_str().to_string_lossy();
    for sidecar in [
        PathBuf::from(format!("{path}-wal")),
        PathBuf::from(format!("{path}-shm")),
        PathBuf::from(format!("{path}-journal")),
    ] {
        if path_exists(&sidecar)? {
            return Err(error(UpgradeFilesystemErrorCode::CandidateMigrationFailed));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "candidate_tests.rs"]
mod tests;
