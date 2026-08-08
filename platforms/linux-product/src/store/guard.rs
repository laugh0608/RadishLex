use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use fs2::FileExt;

use super::filesystem::{sync_directory, UnixFileIdentity};
use super::mode::set_regular_file_mode_and_sync;
use super::{LinuxInstallStoreError, LinuxInstallStoreErrorCode};

fn validate_guard_file(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<fs::Metadata, LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect guard file", error))?;
    validate_guard_metadata(&metadata, owner_id, group_id)?;
    Ok(metadata)
}

fn validate_guard_metadata(
    metadata: &fs::Metadata,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() != 0
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "guard file identity, ownership, mode, links, or size is invalid",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct GuardLock {
    file: File,
    identity: UnixFileIdentity,
}

pub(super) fn acquire_guard_lock(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<GuardLock, LinuxInstallStoreError> {
    let (file, identity) = open_guard_file(path, owner_id, group_id)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::GuardActive,
                "another Linux package transaction owns the guard",
            ));
        }
        Err(error) => {
            return Err(LinuxInstallStoreError::io(
                "acquire Linux install guard lock",
                error,
            ));
        }
    }
    let guard = GuardLock { file, identity };
    verify_guard_lock(path, &guard, owner_id, group_id)?;
    Ok(guard)
}

pub(super) fn verify_guard_lock(
    path: &Path,
    guard: &GuardLock,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    let opened = guard
        .file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened guard file", error))?;
    validate_guard_metadata(&opened, owner_id, group_id)?;
    if UnixFileIdentity::from_metadata(&opened) != guard.identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "opened Linux install guard identity changed",
        ));
    }
    let current = validate_guard_file(path, owner_id, group_id)?;
    if UnixFileIdentity::from_metadata(&current) != guard.identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "Linux install guard path identity changed",
        ));
    }
    Ok(())
}

pub(super) fn remove_locked_guard_path(path: &Path, guard: &GuardLock) {
    if guard
        .file
        .metadata()
        .ok()
        .map(|metadata| UnixFileIdentity::from_metadata(&metadata))
        != Some(guard.identity)
    {
        return;
    }
    if fs::symlink_metadata(path)
        .ok()
        .map(|metadata| UnixFileIdentity::from_metadata(&metadata))
        != Some(guard.identity)
    {
        return;
    }
    let _ = fs::remove_file(path);
}

fn open_guard_file(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<(File, UnixFileIdentity), LinuxInstallStoreError> {
    match fs::symlink_metadata(path) {
        Ok(before) => open_existing_guard_file(path, before, owner_id, group_id),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
            {
                Ok(file) => finish_created_guard_file(path, file, owner_id, group_id),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let before = fs::symlink_metadata(path).map_err(|error| {
                        LinuxInstallStoreError::io("inspect concurrently created guard file", error)
                    })?;
                    open_existing_guard_file(path, before, owner_id, group_id)
                }
                Err(error) => Err(LinuxInstallStoreError::io("create guard file", error)),
            }
        }
        Err(error) => Err(LinuxInstallStoreError::io("inspect guard file", error)),
    }
}

fn finish_created_guard_file(
    path: &Path,
    file: File,
    owner_id: u32,
    group_id: u32,
) -> Result<(File, UnixFileIdentity), LinuxInstallStoreError> {
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| LinuxInstallStoreError::io("set guard file mode", error))?;
    file.sync_all()
        .map_err(|error| LinuxInstallStoreError::io("sync guard file", error))?;
    sync_directory(path.parent().ok_or_else(|| {
        LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            "guard file has no parent",
        )
    })?)?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect created guard file", error))?;
    validate_guard_metadata(&opened, owner_id, group_id)?;
    let identity = UnixFileIdentity::from_metadata(&opened);
    let after = validate_guard_file(path, owner_id, group_id)?;
    if UnixFileIdentity::from_metadata(&after) != identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "created guard file identity changed before locking",
        ));
    }
    Ok((file, identity))
}

fn open_existing_guard_file(
    path: &Path,
    _before: fs::Metadata,
    owner_id: u32,
    group_id: u32,
) -> Result<(File, UnixFileIdentity), LinuxInstallStoreError> {
    set_regular_file_mode_and_sync(
        path,
        &[0o000, 0o200, 0o400, 0o600],
        0o600,
        owner_id,
        group_id,
        0,
        true,
    )?;
    let before = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect normalized guard file", error))?;
    validate_guard_metadata(&before, owner_id, group_id)?;
    let identity = UnixFileIdentity::from_metadata(&before);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| LinuxInstallStoreError::io("open guard file", error))?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened guard file", error))?;
    validate_guard_metadata(&opened, owner_id, group_id)?;
    let after = validate_guard_file(path, owner_id, group_id)?;
    if UnixFileIdentity::from_metadata(&opened) != identity
        || UnixFileIdentity::from_metadata(&after) != identity
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "guard file identity changed while opening",
        ));
    }
    Ok((file, identity))
}
