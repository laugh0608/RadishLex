use std::path::Path;

use crate::model::{ArtifactSlot, LinuxInstallReceipt, LinuxInstallState};

use super::filesystem::{
    artifact_file_identity, path_entry_exists, staged_paths, staged_temporary_path,
};
use super::{LinuxInstallStoreError, LinuxInstallStoreErrorCode};

pub(super) fn validate_current_operation_slots(
    operation_directory: &Path,
    receipt: &LinuxInstallReceipt,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    for slot in [ArtifactSlot::Source, ArtifactSlot::Target] {
        let paths = staged_paths(operation_directory, slot);
        let package_exists = path_entry_exists(paths.package_path())?;
        let evidence_exists = path_entry_exists(paths.evidence_path())?;
        let package_temporary = path_entry_exists(&staged_temporary_path(paths.package_path())?)?;
        let evidence_temporary = path_entry_exists(&staged_temporary_path(paths.evidence_path())?)?;
        match receipt.staged_artifact(slot) {
            Some(proof) => {
                if !package_exists || !evidence_exists || package_temporary || evidence_temporary {
                    return Err(LinuxInstallStoreError::new(
                        LinuxInstallStoreErrorCode::ArtifactInvalid,
                        "current operation files differ from its staged slot inventory",
                    ));
                }
                let package = artifact_file_identity(paths.package_path(), owner_id, group_id)?;
                let evidence = artifact_file_identity(paths.evidence_path(), owner_id, group_id)?;
                if &package != proof.package_file() || &evidence != proof.evidence_file() {
                    return Err(LinuxInstallStoreError::new(
                        LinuxInstallStoreErrorCode::ArtifactInvalid,
                        "current operation file identity differs from its receipt proof",
                    ));
                }
            }
            None if !receipt.required_slots().contains(&slot) => {
                if !(package_exists || evidence_exists || package_temporary || evidence_temporary) {
                    continue;
                }
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ArtifactInvalid,
                    "current operation contains a non-required artifact slot",
                ));
            }
            None if receipt.state() == LinuxInstallState::Prepared => {
                let artifact = receipt.artifact_for_slot(slot).expect("required slot");
                if package_exists {
                    let package = artifact_file_identity(paths.package_path(), owner_id, group_id)?;
                    if package.size() != artifact.package_size()
                        || package.sha256() != artifact.package_sha256()
                    {
                        return Err(LinuxInstallStoreError::new(
                            LinuxInstallStoreErrorCode::ArtifactInvalid,
                            "unrecorded package file differs from its artifact identity",
                        ));
                    }
                }
                if evidence_exists {
                    let evidence =
                        artifact_file_identity(paths.evidence_path(), owner_id, group_id)?;
                    if evidence.size() != artifact.evidence_size()
                        || evidence.sha256() != artifact.evidence_sha256()
                    {
                        return Err(LinuxInstallStoreError::new(
                            LinuxInstallStoreErrorCode::ArtifactInvalid,
                            "unrecorded evidence file differs from its artifact identity",
                        ));
                    }
                }
            }
            None => {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ArtifactInvalid,
                    "current operation is missing a required staged artifact proof",
                ));
            }
        }
    }
    Ok(())
}
