use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use radishlex_ime_userdb::{UserDb, UserDbSnapshotEstimate};

use super::*;

const SNAPSHOT_WORKING_COPY_MULTIPLIER: u64 = 3;
const MINIMUM_FREE_SPACE_RESERVE_BYTES: u64 = 64 * 1024 * 1024;
const USERDB_FILE_NAME: &str = "userdb.sqlite3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeSnapshotSpaceBudget {
    logical_snapshot_bytes: u64,
    required_available_bytes: u64,
    reported_available_bytes: u64,
}

impl UpgradeSnapshotSpaceBudget {
    pub fn evaluate(
        logical_snapshot_bytes: u64,
        reported_available_bytes: u64,
    ) -> Result<Self, UpgradeFilesystemError> {
        let working_bytes = logical_snapshot_bytes
            .checked_mul(SNAPSHOT_WORKING_COPY_MULTIPLIER)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InsufficientSpace))?;
        let required_available_bytes = working_bytes
            .checked_add(MINIMUM_FREE_SPACE_RESERVE_BYTES)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InsufficientSpace))?;
        if reported_available_bytes < required_available_bytes {
            return Err(error(UpgradeFilesystemErrorCode::InsufficientSpace));
        }
        Ok(Self {
            logical_snapshot_bytes,
            required_available_bytes,
            reported_available_bytes,
        })
    }

    pub const fn logical_snapshot_bytes(self) -> u64 {
        self.logical_snapshot_bytes
    }

    pub const fn required_available_bytes(self) -> u64 {
        self.required_available_bytes
    }

    pub const fn reported_available_bytes(self) -> u64 {
        self.reported_available_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeSnapshotSummary {
    space_budget: UpgradeSnapshotSpaceBudget,
    snapshot_identity: UpgradeArtifactIdentity,
    source_schema_version: i64,
    snapshot_schema_version: i64,
    page_size_bytes: u64,
    page_count: u64,
}

impl UpgradeSnapshotSummary {
    pub const fn space_budget(&self) -> UpgradeSnapshotSpaceBudget {
        self.space_budget
    }

    pub fn snapshot_identity(&self) -> &UpgradeArtifactIdentity {
        &self.snapshot_identity
    }

    pub const fn source_schema_version(&self) -> i64 {
        self.source_schema_version
    }

    pub const fn snapshot_schema_version(&self) -> i64 {
        self.snapshot_schema_version
    }

    pub const fn page_size_bytes(&self) -> u64 {
        self.page_size_bytes
    }

    pub const fn page_count(&self) -> u64 {
        self.page_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotFaultPoint {
    StagedFileCreated,
    SqliteBackupCompleted,
    SnapshotRenamed,
    ReceiptEvidencePersisted,
}

trait SnapshotFaultInjector {
    fn checkpoint(&self, _point: SnapshotFaultPoint) -> Result<(), UpgradeFilesystemError> {
        Ok(())
    }
}

struct NoSnapshotFaults;

impl SnapshotFaultInjector for NoSnapshotFaults {}

impl UpgradeReceiptStore {
    pub fn create_userdb_snapshot(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        reported_available_bytes: u64,
    ) -> Result<UpgradeSnapshotSummary, UpgradeFilesystemError> {
        self.create_userdb_snapshot_with_faults(
            guard,
            receipt,
            reported_available_bytes,
            &NoSnapshotFaults,
        )
    }

    fn create_userdb_snapshot_with_faults<F: SnapshotFaultInjector>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        reported_available_bytes: u64,
        faults: &F,
    ) -> Result<UpgradeSnapshotSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if receipt.state() != UpgradeState::Quiesced {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSnapshotState));
        }
        let current_receipt = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSnapshotState))?;
        if current_receipt != *receipt
            || receipt
                .artifacts()
                .iter()
                .any(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase)
        {
            return Err(error(UpgradeFilesystemErrorCode::InvalidSnapshotState));
        }
        if path_exists(&self.staged_snapshot_path())? || path_exists(&self.snapshot_path())? {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedSnapshot));
        }

        let source_path = self.root.path.join(USERDB_FILE_NAME);
        let source_metadata = private_data_file_metadata(
            &source_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        let source_identity = FileIdentity::from_metadata(&source_metadata);
        let expected_source = receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceDatabase)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidSnapshotState))?;
        if !file_artifact_matches_metadata(expected_source, &source_metadata)
            || receipt.source_schema_version().is_none()
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }

        let estimate = UserDb::estimate_snapshot(&source_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::SnapshotFailed))?;
        validate_estimate(receipt, estimate)?;
        let space_budget = UpgradeSnapshotSpaceBudget::evaluate(
            estimate.logical_size_bytes,
            reported_available_bytes,
        )?;

        let staged_path = self.staged_snapshot_path();
        let staged_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::AlreadyExists {
                    error(UpgradeFilesystemErrorCode::InterruptedSnapshot)
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
        drop(staged_file);
        faults.checkpoint(SnapshotFaultPoint::StagedFileCreated)?;

        let snapshot = UserDb::create_consistent_snapshot(&source_path, &staged_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::SnapshotFailed))?;
        faults.checkpoint(SnapshotFaultPoint::SqliteBackupCompleted)?;
        File::open(&staged_path)
            .and_then(|file| file.sync_all())
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        let staged_metadata = private_data_file_metadata(
            &staged_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if staged_metadata.dev() != staged_identity.device_id
            || staged_metadata.ino() != staged_identity.inode
            || staged_metadata.len() != snapshot.snapshot_file_bytes
            || snapshot.logical_size_bytes != estimate.logical_size_bytes
            || snapshot.source_schema_version != estimate.schema_version
            || snapshot.snapshot_schema_version != estimate.schema_version
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        self.revalidate()?;
        guard.revalidate()?;
        let source_after = private_data_file_metadata(
            &source_path,
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        if FileIdentity::from_metadata(&source_after) != source_identity {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }

        fs::rename(&staged_path, self.snapshot_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        sync_directory(&self.state_directory)?;
        faults.checkpoint(SnapshotFaultPoint::SnapshotRenamed)?;
        let snapshot_metadata = private_data_file_metadata(
            &self.snapshot_path(),
            self.root.expected_owner_id,
            UpgradeFilesystemErrorCode::IdentityChanged,
        )?;
        let snapshot_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SnapshotDatabase,
            snapshot_metadata.dev(),
            snapshot_metadata.ino(),
            snapshot_metadata.uid(),
            snapshot_metadata.len(),
        )
        .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;

        let mut next_receipt = receipt.clone();
        next_receipt
            .record_artifact(snapshot_identity.clone())
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSnapshotState))?;
        self.persist(guard, &next_receipt)?;
        faults.checkpoint(SnapshotFaultPoint::ReceiptEvidencePersisted)?;
        next_receipt
            .advance(UpgradeState::SnapshotReady)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidSnapshotState))?;
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;

        Ok(UpgradeSnapshotSummary {
            space_budget,
            snapshot_identity,
            source_schema_version: snapshot.source_schema_version,
            snapshot_schema_version: snapshot.snapshot_schema_version,
            page_size_bytes: snapshot.page_size_bytes,
            page_count: snapshot.page_count,
        })
    }

    fn snapshot_path(&self) -> PathBuf {
        self.state_directory.join(SNAPSHOT_FILE_NAME)
    }

    fn staged_snapshot_path(&self) -> PathBuf {
        self.state_directory.join(STAGED_SNAPSHOT_FILE_NAME)
    }
}

pub(super) fn validate_snapshot_state(
    store: &UpgradeReceiptStore,
    receipt: Option<&UpgradeReceipt>,
) -> Result<(), UpgradeFilesystemError> {
    if path_exists(&store.staged_snapshot_path())? {
        return Err(error(UpgradeFilesystemErrorCode::InterruptedSnapshot));
    }
    let snapshot_exists = path_exists(&store.snapshot_path())?;
    let recorded_snapshot = receipt.and_then(|receipt| {
        receipt
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase)
    });
    match (snapshot_exists, recorded_snapshot) {
        (false, None) => Ok(()),
        (true, Some(identity)) => {
            let metadata = private_data_file_metadata(
                &store.snapshot_path(),
                store.root.expected_owner_id,
                UpgradeFilesystemErrorCode::IdentityChanged,
            )?;
            if file_artifact_matches_metadata(identity, &metadata) {
                Ok(())
            } else {
                Err(error(UpgradeFilesystemErrorCode::IdentityChanged))
            }
        }
        (true, None) => Err(error(UpgradeFilesystemErrorCode::InterruptedSnapshot)),
        (false, Some(_)) => Err(error(UpgradeFilesystemErrorCode::IdentityChanged)),
    }
}

fn validate_estimate(
    receipt: &UpgradeReceipt,
    estimate: UserDbSnapshotEstimate,
) -> Result<(), UpgradeFilesystemError> {
    if receipt.source_schema_version() != Some(estimate.schema_version)
        || estimate.page_size_bytes == 0
    {
        return Err(error(UpgradeFilesystemErrorCode::InvalidSnapshotState));
    }
    Ok(())
}

fn private_data_file_metadata(
    path: &Path,
    expected_owner_id: u32,
    error_code: UpgradeFilesystemErrorCode,
) -> Result<Metadata, UpgradeFilesystemError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error(error_code))?;
    if !metadata.file_type().is_file()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(error(error_code));
    }
    Ok(metadata)
}

fn file_artifact_matches_metadata(identity: &UpgradeArtifactIdentity, metadata: &Metadata) -> bool {
    identity.device_id() == metadata.dev()
        && identity.inode() == metadata.ino()
        && identity.owner_id() == metadata.uid()
        && identity.mode() == metadata.mode() & 0o7777
        && identity.link_count() == metadata.nlink()
        && identity.byte_len() == metadata.len()
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
