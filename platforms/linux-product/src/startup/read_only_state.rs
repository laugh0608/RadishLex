use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::filesystem_policy::parent_permissions_are_safe;
use crate::model::{
    LinuxInstallReceipt, LinuxInstallRootIdentity, LinuxInstallState,
    MAX_LINUX_INSTALL_RECEIPT_BYTES,
};

use super::types::{
    LinuxStartupDecision, LinuxStartupOutcome, LinuxStartupPaths, LinuxStartupReason,
};

pub(super) const RECEIPT_FILENAME: &str = "receipt.json";
pub(super) const RECEIPT_TMP_FILENAME: &str = "receipt.json.tmp";
pub(super) const OPERATIONS_DIRECTORY: &str = "operations";

#[derive(Debug)]
pub(super) struct StartupInspectionError {
    reason: LinuxStartupReason,
}

impl StartupInspectionError {
    const fn new(reason: LinuxStartupReason) -> Self {
        Self { reason }
    }

    pub(super) const fn outcome(
        self,
        receipt_state: Option<LinuxInstallState>,
    ) -> LinuxStartupOutcome {
        LinuxStartupOutcome::new(
            LinuxStartupDecision::FailedClosed,
            self.reason,
            receipt_state,
        )
    }

    pub(super) fn port_error(self) -> super::LinuxStartupPortError {
        let code = match self.reason {
            LinuxStartupReason::PermissionDenied => {
                super::LinuxStartupPortErrorCode::PermissionDenied
            }
            LinuxStartupReason::Io => super::LinuxStartupPortErrorCode::Io,
            _ => super::LinuxStartupPortErrorCode::PackageIdentityChanged,
        };
        super::LinuxStartupPortError::new(code, "Linux startup state inspection failed")
    }

    fn from_io(error: &io::Error) -> Self {
        Self::new(if error.kind() == io::ErrorKind::PermissionDenied {
            LinuxStartupReason::PermissionDenied
        } else {
            LinuxStartupReason::Io
        })
    }
}

#[derive(Debug)]
pub(super) enum ReadOnlyStateView {
    Absent,
    Present {
        root_identity: LinuxInstallRootIdentity,
    },
}

impl ReadOnlyStateView {
    pub(super) const fn is_present(&self) -> bool {
        matches!(self, Self::Present { .. })
    }

    pub(super) fn inspect(paths: &LinuxStartupPaths) -> Result<Self, StartupInspectionError> {
        validate_absolute_leaf(&paths.state_root)?;
        match fs::symlink_metadata(&paths.state_root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::Absent),
            Err(error) => Err(StartupInspectionError::from_io(&error)),
            Ok(metadata) => {
                let parent = paths.state_root.parent().ok_or_else(|| {
                    StartupInspectionError::new(LinuxStartupReason::UnexpectedStateObject)
                })?;
                validate_parent(
                    parent,
                    paths.expected_owner_id,
                    paths.expected_group_id,
                    false,
                )?;
                let parent_meta = fs::symlink_metadata(parent)
                    .map_err(|e| StartupInspectionError::from_io(&e))?;
                validate_directory_metadata(
                    &parent_meta,
                    0o755,
                    paths.expected_owner_id,
                    paths.expected_group_id,
                )?;
                validate_directory_metadata(
                    &metadata,
                    0o755,
                    paths.expected_owner_id,
                    paths.expected_group_id,
                )?;
                let operations = paths.state_root.join(OPERATIONS_DIRECTORY);
                let operations_metadata = fs::symlink_metadata(&operations)
                    .map_err(|error| StartupInspectionError::from_io(&error))?;
                validate_directory_metadata(
                    &operations_metadata,
                    0o755,
                    paths.expected_owner_id,
                    paths.expected_group_id,
                )?;
                validate_state_entries(&paths.state_root)?;
                let root_identity = LinuxInstallRootIdentity::new(
                    metadata.dev(),
                    metadata.ino(),
                    metadata.uid(),
                    metadata.gid(),
                    metadata.mode() & 0o7777,
                )
                .map_err(|_| {
                    StartupInspectionError::new(LinuxStartupReason::RootIdentityChanged)
                })?;
                Ok(Self::Present { root_identity })
            }
        }
    }

    pub(super) fn inspect_temporary_receipt(
        &self,
        paths: &LinuxStartupPaths,
    ) -> Option<LinuxStartupOutcome> {
        let Self::Present { .. } = self else {
            return None;
        };
        let temporary = paths.state_root.join(RECEIPT_TMP_FILENAME);
        match fs::symlink_metadata(&temporary) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => Some(StartupInspectionError::from_io(&error).outcome(None)),
            Ok(metadata) => {
                let valid = metadata.file_type().is_file()
                    && metadata.uid() == paths.expected_owner_id
                    && metadata.gid() == paths.expected_group_id
                    && matches!(
                        metadata.mode() & 0o7777,
                        0o000 | 0o200 | 0o400 | 0o600 | 0o644
                    )
                    && metadata.nlink() == 1
                    && metadata.len() <= MAX_LINUX_INSTALL_RECEIPT_BYTES as u64;
                Some(LinuxStartupOutcome::new(
                    if valid {
                        LinuxStartupDecision::MaintenanceRequired
                    } else {
                        LinuxStartupDecision::FailedClosed
                    },
                    if valid {
                        LinuxStartupReason::InterruptedReceipt
                    } else {
                        LinuxStartupReason::InterruptedReceiptInvalid
                    },
                    None,
                ))
            }
        }
    }

    pub(super) fn read_receipt(
        &self,
        paths: &LinuxStartupPaths,
    ) -> Result<Option<LinuxInstallReceipt>, StartupInspectionError> {
        let Self::Present { root_identity } = self else {
            return Ok(None);
        };
        let receipt_path = paths.state_root.join(RECEIPT_FILENAME);
        let before = match fs::symlink_metadata(&receipt_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut operations = fs::read_dir(paths.state_root.join(OPERATIONS_DIRECTORY))
                    .map_err(|error| StartupInspectionError::from_io(&error))?;
                if operations.next().is_some() {
                    return Err(StartupInspectionError::new(
                        LinuxStartupReason::ReceiptInvalid,
                    ));
                }
                return Ok(None);
            }
            Err(error) => return Err(StartupInspectionError::from_io(&error)),
        };
        validate_regular_metadata(
            &before,
            &[0o644],
            paths.expected_owner_id,
            paths.expected_group_id,
            1,
            MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
            LinuxStartupReason::ReceiptInvalid,
        )?;
        let mut file =
            File::open(&receipt_path).map_err(|error| StartupInspectionError::from_io(&error))?;
        let opened = file
            .metadata()
            .map_err(|error| StartupInspectionError::from_io(&error))?;
        if !same_file_identity(&before, &opened) {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::RootIdentityChanged,
            ));
        }
        let mut value = Vec::with_capacity(before.len() as usize);
        file.read_to_end(&mut value)
            .map_err(|error| StartupInspectionError::from_io(&error))?;
        let after = fs::symlink_metadata(&receipt_path)
            .map_err(|error| StartupInspectionError::from_io(&error))?;
        if !same_file_identity(&before, &after) || value.len() as u64 != before.len() {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::RootIdentityChanged,
            ));
        }
        let receipt = LinuxInstallReceipt::decode(&value)
            .map_err(|_| StartupInspectionError::new(LinuxStartupReason::ReceiptInvalid))?;
        if receipt.root_identity() != root_identity {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::RootIdentityChanged,
            ));
        }
        validate_operation_entries(paths, &receipt)?;
        Ok(Some(receipt))
    }
}

pub(super) fn inspect_guard(paths: &LinuxStartupPaths) -> Option<LinuxStartupOutcome> {
    if validate_absolute_leaf(&paths.guard_path).is_err() {
        return Some(StartupInspectionError::new(LinuxStartupReason::GuardInvalid).outcome(None));
    }
    let metadata = match fs::symlink_metadata(&paths.guard_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => return Some(StartupInspectionError::from_io(&error).outcome(None)),
    };
    let parent = match paths.guard_path.parent() {
        Some(parent) => parent,
        None => {
            return Some(
                StartupInspectionError::new(LinuxStartupReason::GuardInvalid).outcome(None),
            )
        }
    };
    let valid = validate_parent(
        parent,
        paths.expected_owner_id,
        paths.expected_group_id,
        true,
    )
    .is_ok()
        && metadata.file_type().is_file()
        && metadata.uid() == paths.expected_owner_id
        && metadata.gid() == paths.expected_group_id
        && matches!(metadata.mode() & 0o7777, 0o000 | 0o200 | 0o400 | 0o600)
        && metadata.nlink() == 1
        && metadata.len() == 0;
    Some(LinuxStartupOutcome::new(
        if valid {
            LinuxStartupDecision::MaintenanceRequired
        } else {
            LinuxStartupDecision::FailedClosed
        },
        if valid {
            LinuxStartupReason::ActiveGuard
        } else {
            LinuxStartupReason::GuardInvalid
        },
        None,
    ))
}

fn validate_absolute_leaf(path: &Path) -> Result<(), StartupInspectionError> {
    if !path.is_absolute() || path.file_name().is_none() || normalize_path(path) != path {
        return Err(StartupInspectionError::new(
            LinuxStartupReason::UnexpectedStateObject,
        ));
    }
    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::RootDir => normalized.push(Path::new("/")),
            std::path::Component::Normal(value) => normalized.push(value),
            _ => return PathBuf::new(),
        }
    }
    normalized
}

fn validate_parent(
    path: &Path,
    owner_id: u32,
    group_id: u32,
    allow_shared_lock_parent: bool,
) -> Result<(), StartupInspectionError> {
    let canonical =
        fs::canonicalize(path).map_err(|error| StartupInspectionError::from_io(&error))?;
    if canonical != path {
        return Err(StartupInspectionError::new(
            LinuxStartupReason::UnexpectedStateObject,
        ));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| StartupInspectionError::from_io(&error))?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || !parent_permissions_are_safe(metadata.mode() & 0o7777, allow_shared_lock_parent)
    {
        return Err(StartupInspectionError::new(
            LinuxStartupReason::PermissionDenied,
        ));
    }
    Ok(())
}

fn validate_directory_metadata(
    metadata: &fs::Metadata,
    mode: u32,
    owner_id: u32,
    group_id: u32,
) -> Result<(), StartupInspectionError> {
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != mode
    {
        return Err(StartupInspectionError::new(
            LinuxStartupReason::RootIdentityChanged,
        ));
    }
    Ok(())
}

fn validate_state_entries(state_root: &Path) -> Result<(), StartupInspectionError> {
    for entry in
        fs::read_dir(state_root).map_err(|error| StartupInspectionError::from_io(&error))?
    {
        let entry = entry.map_err(|error| StartupInspectionError::from_io(&error))?;
        if !matches!(
            entry.file_name().to_str(),
            Some(OPERATIONS_DIRECTORY | RECEIPT_FILENAME | RECEIPT_TMP_FILENAME)
        ) {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::UnexpectedStateObject,
            ));
        }
    }
    Ok(())
}

fn validate_operation_entries(
    paths: &LinuxStartupPaths,
    receipt: &LinuxInstallReceipt,
) -> Result<(), StartupInspectionError> {
    let operations_root = paths.state_root.join(OPERATIONS_DIRECTORY);
    let operation_names: BTreeSet<&str> = receipt
        .operation_chain()
        .iter()
        .map(String::as_str)
        .collect();
    for entry in
        fs::read_dir(&operations_root).map_err(|error| StartupInspectionError::from_io(&error))?
    {
        let entry = entry.map_err(|error| StartupInspectionError::from_io(&error))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            StartupInspectionError::new(LinuxStartupReason::UnexpectedStateObject)
        })?;
        if !operation_names.contains(name) {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::UnexpectedStateObject,
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| StartupInspectionError::from_io(&error))?;
        validate_directory_metadata(
            &metadata,
            0o700,
            paths.expected_owner_id,
            paths.expected_group_id,
        )?;
        let after = fs::symlink_metadata(entry.path())
            .map_err(|error| StartupInspectionError::from_io(&error))?;
        if !same_file_identity(&metadata, &after) {
            return Err(StartupInspectionError::new(
                LinuxStartupReason::RootIdentityChanged,
            ));
        }
    }

    for operation_id in receipt.operation_chain() {
        let operation_path = operations_root.join(operation_id);
        match fs::symlink_metadata(&operation_path) {
            Ok(_) => {}
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && operation_id == receipt.operation_id()
                    && receipt.state() == LinuxInstallState::Prepared
                    && receipt
                        .required_slots()
                        .iter()
                        .all(|slot| receipt.staged_artifact(*slot).is_none()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StartupInspectionError::new(
                    LinuxStartupReason::RootIdentityChanged,
                ));
            }
            Err(error) => return Err(StartupInspectionError::from_io(&error)),
        }
    }
    Ok(())
}

fn validate_regular_metadata(
    metadata: &fs::Metadata,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
    minimum_size: u64,
    maximum_size: u64,
    reason: LinuxStartupReason,
) -> Result<(), StartupInspectionError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || !modes.contains(&(metadata.mode() & 0o7777))
        || metadata.nlink() != 1
        || metadata.len() < minimum_size
        || metadata.len() > maximum_size
    {
        return Err(StartupInspectionError::new(reason));
    }
    Ok(())
}

pub(super) fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}
