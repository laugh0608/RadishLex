use std::fs::{self, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::filesystem::{validate_regular_metadata_modes, UnixFileIdentity};
use super::{LinuxInstallStoreError, LinuxInstallStoreErrorCode};

pub(super) fn set_regular_file_mode_and_sync(
    path: &Path,
    allowed_modes: &[u32],
    target_mode: u32,
    owner_id: u32,
    group_id: u32,
    maximum_size: u64,
    allow_empty: bool,
) -> Result<UnixFileIdentity, LinuxInstallStoreError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect interrupted regular file", error))?;
    validate_regular_metadata_modes(
        &before,
        allowed_modes,
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let before_identity = UnixFileIdentity::from_metadata(&before);
    if before_identity.mode & 0o600 != 0o600 {
        fs::set_permissions(path, fs::Permissions::from_mode(target_mode)).map_err(|error| {
            LinuxInstallStoreError::io("make interrupted regular file writable", error)
        })?;
        let writable = fs::symlink_metadata(path).map_err(|error| {
            LinuxInstallStoreError::io("reinspect writable interrupted regular file", error)
        })?;
        let mut expected = before_identity;
        expected.mode = target_mode;
        if UnixFileIdentity::from_metadata(&writable) != expected {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::IdentityChanged,
                "interrupted regular file identity changed while restoring owner access",
            ));
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| LinuxInstallStoreError::io("open interrupted regular file", error))?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened interrupted file", error))?;
    let expected_opened = if before_identity.mode & 0o600 != 0o600 {
        let mut identity = before_identity;
        identity.mode = target_mode;
        identity
    } else {
        before_identity
    };
    if UnixFileIdentity::from_metadata(&opened) != expected_opened {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "interrupted regular file identity changed while opening",
        ));
    }
    file.set_permissions(fs::Permissions::from_mode(target_mode))
        .map_err(|error| LinuxInstallStoreError::io("set interrupted regular file mode", error))?;
    file.sync_all().map_err(|error| {
        LinuxInstallStoreError::io("sync interrupted regular file metadata", error)
    })?;
    let committed = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect committed regular file", error))?;
    validate_regular_metadata_modes(
        &committed,
        &[target_mode],
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let identity = UnixFileIdentity::from_metadata(&committed);
    let after = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("reinspect committed regular file", error))?;
    if UnixFileIdentity::from_metadata(&after) != identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file path changed while committing metadata",
        ));
    }
    Ok(identity)
}
