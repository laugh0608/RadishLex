use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::{ArtifactFileIdentity, ArtifactSlot, MAX_LINUX_INSTALL_RECEIPT_BYTES};

use super::{
    LinuxInstallStoreError, LinuxInstallStoreErrorCode, OPERATIONS_DIRECTORY, RECEIPT_FILENAME,
    RECEIPT_TMP_FILENAME, STAGED_TEMP_SUFFIX,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedArtifactPaths {
    pub(super) package_path: PathBuf,
    pub(super) evidence_path: PathBuf,
}

impl StagedArtifactPaths {
    pub fn package_path(&self) -> &Path {
        &self.package_path
    }

    pub fn evidence_path(&self) -> &Path {
        &self.evidence_path
    }
}

pub(super) fn ensure_directory(path: &Path, mode: u32) -> Result<(), LinuxInstallStoreError> {
    let mut builder = fs::DirBuilder::new();
    builder.mode(mode);
    match builder.create(path) {
        Ok(()) => {
            let created = fs::symlink_metadata(path)
                .map_err(|error| LinuxInstallStoreError::io("inspect created directory", error))?;
            if !created.file_type().is_dir() {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::IdentityChanged,
                    "created directory path is not a directory",
                ));
            }
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
                LinuxInstallStoreError::io("set exact created directory mode", error)
            })?;
            let exact = fs::symlink_metadata(path).map_err(|error| {
                LinuxInstallStoreError::io("reinspect created directory", error)
            })?;
            if !exact.file_type().is_dir()
                || exact.dev() != created.dev()
                || exact.ino() != created.ino()
                || exact.mode() & 0o7777 != mode
            {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::IdentityChanged,
                    "created directory identity or exact mode changed",
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(LinuxInstallStoreError::io("create directory", error)),
    }
}

pub(super) fn ensure_state_parent(
    state_parent: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    let state_parent_parent = state_parent.parent().ok_or_else(|| {
        LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            "state parent has no parent",
        )
    })?;
    validate_secure_parent(state_parent_parent, owner_id, group_id, false)?;
    ensure_directory(state_parent, 0o755)?;
    validate_directory(state_parent, 0o755, owner_id, group_id)?;
    sync_directory(state_parent_parent)
}

pub(super) fn validate_absolute_leaf(
    path: &Path,
    label: &'static str,
) -> Result<(), LinuxInstallStoreError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            format!("{label} must be an absolute leaf path"),
        ));
    }
    Ok(())
}

pub(super) fn validate_secure_parent(
    path: &Path,
    owner_id: u32,
    group_id: u32,
    allow_group_write: bool,
) -> Result<(), LinuxInstallStoreError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| LinuxInstallStoreError::io("canonicalize parent directory", error))?;
    if canonical != path {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            "parent directory path contains a symlink or lexical alias",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect parent directory", error))?;
    let mode = metadata.mode() & 0o7777;
    let writable = if allow_group_write {
        mode & 0o002
    } else {
        mode & 0o022
    };
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || writable != 0
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PermissionDenied,
            "parent directory ownership or write permissions are unsafe",
        ));
    }
    Ok(())
}

pub(super) fn validate_directory(
    path: &Path,
    mode: u32,
    owner_id: u32,
    group_id: u32,
) -> Result<fs::Metadata, LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect directory", error))?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != mode
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "directory identity, ownership, or mode is invalid",
        ));
    }
    Ok(metadata)
}

pub(super) fn validate_root_entries(
    state_root: &Path,
    owner_id: u32,
    group_id: u32,
    allow_interrupted_receipt: bool,
) -> Result<(), LinuxInstallStoreError> {
    for entry in fs::read_dir(state_root)
        .map_err(|error| LinuxInstallStoreError::io("read Linux install state root", error))?
    {
        let entry = entry.map_err(|error| LinuxInstallStoreError::io("read root entry", error))?;
        let name = entry.file_name();
        let name = name.to_str();
        let allowed = matches!(name, Some(OPERATIONS_DIRECTORY | RECEIPT_FILENAME))
            || (allow_interrupted_receipt && name == Some(RECEIPT_TMP_FILENAME));
        if !allowed {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "Linux install state root contains an unexpected entry",
            ));
        }
        if name == Some(RECEIPT_TMP_FILENAME) {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                LinuxInstallStoreError::io("inspect interrupted receipt", error)
            })?;
            validate_regular_metadata_modes(
                &metadata,
                &[0o000, 0o200, 0o400, 0o600, 0o644],
                owner_id,
                group_id,
                MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
                true,
            )?;
        }
    }
    Ok(())
}

pub(super) fn validate_operation_files(
    path: &Path,
    owner_id: u32,
    group_id: u32,
    allow_temporary: bool,
    require_complete_pairs: bool,
) -> Result<(), LinuxInstallStoreError> {
    let mut names = std::collections::BTreeSet::new();
    for entry in fs::read_dir(path)
        .map_err(|error| LinuxInstallStoreError::io("read operation directory", error))?
    {
        let entry = entry
            .map_err(|error| LinuxInstallStoreError::io("read staged artifact entry", error))?;
        let name = entry.file_name();
        let name = name.to_str();
        if let Some(name) = name {
            names.insert(name.to_owned());
        }
        let final_name = matches!(
            name,
            Some("source.deb" | "source.evidence.json" | "target.deb" | "target.evidence.json")
        );
        let temporary_name = allow_temporary
            && matches!(
                name,
                Some(
                    "source.deb.tmp"
                        | "source.evidence.json.tmp"
                        | "target.deb.tmp"
                        | "target.evidence.json.tmp"
                )
            );
        let valid = final_name || temporary_name;
        if !valid {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "operation directory contains an unexpected entry",
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| LinuxInstallStoreError::io("inspect staged operation file", error))?;
        let modes: &[u32] = if temporary_name {
            &[0o000, 0o200, 0o400, 0o600]
        } else {
            &[0o600]
        };
        validate_regular_metadata_modes(
            &metadata,
            modes,
            owner_id,
            group_id,
            512 * 1024 * 1024,
            temporary_name,
        )?;
    }
    if require_complete_pairs {
        let source_package = names.contains("source.deb");
        let source_evidence = names.contains("source.evidence.json");
        let target_package = names.contains("target.deb");
        let target_evidence = names.contains("target.evidence.json");
        if source_package != source_evidence
            || target_package != target_evidence
            || !(source_package || target_package)
        {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "operation staging does not contain complete artifact pairs",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_regular_metadata_modes(
    metadata: &fs::Metadata,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
    maximum_size: u64,
    allow_empty: bool,
) -> Result<(), LinuxInstallStoreError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || !modes.contains(&(metadata.mode() & 0o7777))
        || metadata.nlink() != 1
        || (!allow_empty && metadata.len() == 0)
        || metadata.len() > maximum_size
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity, ownership, mode, links, or size is invalid",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UnixFileIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    group_id: u32,
    pub(super) mode: u32,
    hardlink_count: u64,
    size: u64,
}

impl UnixFileIdentity {
    pub(super) fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device_id: metadata.dev(),
            inode: metadata.ino(),
            owner_id: metadata.uid(),
            group_id: metadata.gid(),
            mode: metadata.mode() & 0o7777,
            hardlink_count: metadata.nlink(),
            size: metadata.len(),
        }
    }
}

pub(super) struct StableRegularFile {
    pub(super) value: Vec<u8>,
    pub(super) identity: UnixFileIdentity,
}

struct StableRegularDigest {
    sha256: String,
    identity: UnixFileIdentity,
}

pub(super) fn read_stable_regular_file(
    path: &Path,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
    maximum_size: u64,
    allow_empty: bool,
) -> Result<StableRegularFile, LinuxInstallStoreError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect regular file", error))?;
    validate_regular_metadata_modes(
        &before,
        modes,
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let mut file =
        File::open(path).map_err(|error| LinuxInstallStoreError::io("open regular file", error))?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened regular file", error))?;
    validate_regular_metadata_modes(
        &opened,
        modes,
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let identity = UnixFileIdentity::from_metadata(&before);
    if UnixFileIdentity::from_metadata(&opened) != identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity changed while opening",
        ));
    }
    let mut value = Vec::with_capacity(before.len() as usize);
    file.read_to_end(&mut value)
        .map_err(|error| LinuxInstallStoreError::io("read regular file", error))?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("reinspect regular file", error))?;
    if UnixFileIdentity::from_metadata(&after) != identity || value.len() as u64 != identity.size {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity changed while reading",
        ));
    }
    Ok(StableRegularFile { value, identity })
}

fn digest_stable_regular_file(
    path: &Path,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
    maximum_size: u64,
    allow_empty: bool,
) -> Result<StableRegularDigest, LinuxInstallStoreError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect regular file", error))?;
    validate_regular_metadata_modes(
        &before,
        modes,
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let mut file =
        File::open(path).map_err(|error| LinuxInstallStoreError::io("open regular file", error))?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened regular file", error))?;
    validate_regular_metadata_modes(
        &opened,
        modes,
        owner_id,
        group_id,
        maximum_size,
        allow_empty,
    )?;
    let identity = UnixFileIdentity::from_metadata(&before);
    if UnixFileIdentity::from_metadata(&opened) != identity {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity changed while opening",
        ));
    }
    let mut digest = Sha256::new();
    let mut bytes_read = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| LinuxInstallStoreError::io("hash regular file", error))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        bytes_read += read as u64;
    }
    let after = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("reinspect regular file", error))?;
    if UnixFileIdentity::from_metadata(&after) != identity || bytes_read != identity.size {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity changed while hashing",
        ));
    }
    Ok(StableRegularDigest {
        sha256: sha256_digest(digest),
        identity,
    })
}

pub(super) fn ensure_exact_regular_file(
    path: &Path,
    expected: &UnixFileIdentity,
) -> Result<(), LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("reinspect exact regular file", error))?;
    if !metadata.file_type().is_file() || UnixFileIdentity::from_metadata(&metadata) != *expected {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity changed before mutation",
        ));
    }
    Ok(())
}

pub(super) fn remove_exact_regular_file(
    path: &Path,
    expected: &UnixFileIdentity,
) -> Result<(), LinuxInstallStoreError> {
    ensure_exact_regular_file(path, expected)?;
    fs::remove_file(path)
        .map_err(|error| LinuxInstallStoreError::io("remove interrupted regular file", error))
}

pub(super) fn validate_source_artifact(
    path: &Path,
    expected_name: &str,
    expected_size: u64,
    expected_sha256: &str,
    state_root: &Path,
) -> Result<(), LinuxInstallStoreError> {
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name) {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source filename differs from the committed identity",
        ));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| LinuxInstallStoreError::io("canonicalize artifact source", error))?;
    if canonical != path || canonical.starts_with(state_root) {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source cannot contain symlinks or be inside the install state root",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect artifact source", error))?;
    let mode = metadata.mode() & 0o7777;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || mode & 0o022 != 0
        || metadata.len() != expected_size
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source identity, mode, size, or digest is invalid",
        ));
    }
    let stable = digest_stable_regular_file(
        path,
        &[mode],
        metadata.uid(),
        metadata.gid(),
        expected_size,
        false,
    )?;
    if stable.identity.size != expected_size || stable.sha256 != expected_sha256 {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source identity, mode, size, or digest is invalid",
        ));
    }
    Ok(())
}

pub(super) fn stage_file(
    source: &Path,
    target: &Path,
    expected_size: u64,
    expected_sha256: &str,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    let temporary = staged_temporary_path(target)?;
    let existing = match fs::symlink_metadata(target) {
        Ok(_) => {
            let existing = digest_stable_regular_file(
                target,
                &[0o600],
                owner_id,
                group_id,
                512 * 1024 * 1024,
                false,
            )?;
            if existing.identity.size != expected_size || existing.sha256 != expected_sha256 {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ArtifactInvalid,
                    "existing staged artifact differs from committed identity",
                ));
            }
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(LinuxInstallStoreError::io(
                "inspect existing staged artifact",
                error,
            ))
        }
    };
    let interrupted = match fs::symlink_metadata(&temporary) {
        Ok(_) => {
            super::mode::set_regular_file_mode_and_sync(
                &temporary,
                &[0o000, 0o200, 0o400, 0o600],
                0o600,
                owner_id,
                group_id,
                512 * 1024 * 1024,
                true,
            )?;
            Some(digest_stable_regular_file(
                &temporary,
                &[0o600],
                owner_id,
                group_id,
                512 * 1024 * 1024,
                true,
            )?)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(LinuxInstallStoreError::io(
                "inspect staged artifact temporary",
                error,
            ))
        }
    };
    if existing {
        if let Some(interrupted) = interrupted {
            remove_exact_regular_file(&temporary, &interrupted.identity)?;
            sync_directory(target.parent().ok_or_else(|| {
                LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "staged artifact has no parent",
                )
            })?)?;
        }
        return Ok(());
    }
    if let Some(interrupted) = interrupted {
        if interrupted.identity.size == expected_size && interrupted.sha256 == expected_sha256 {
            ensure_exact_regular_file(&temporary, &interrupted.identity)?;
            fs::rename(&temporary, target).map_err(|error| {
                LinuxInstallStoreError::io("recover staged artifact temporary", error)
            })?;
            sync_directory(target.parent().ok_or_else(|| {
                LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "staged artifact has no parent",
                )
            })?)?;
            return Ok(());
        }
        remove_exact_regular_file(&temporary, &interrupted.identity)?;
        sync_directory(target.parent().ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "staged artifact has no parent",
            )
        })?)?;
    }

    let source_before = fs::symlink_metadata(source)
        .map_err(|error| LinuxInstallStoreError::io("inspect artifact source", error))?;
    if !source_before.file_type().is_file()
        || source_before.nlink() != 1
        || source_before.mode() & 0o022 != 0
        || source_before.len() != expected_size
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source identity changed before staging",
        ));
    }
    let mut input = File::open(source)
        .map_err(|error| LinuxInstallStoreError::io("open artifact source", error))?;
    let source_open = input
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened artifact source", error))?;
    if UnixFileIdentity::from_metadata(&source_before)
        != UnixFileIdentity::from_metadata(&source_open)
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source identity changed while opening",
        ));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| LinuxInstallStoreError::io("create staged artifact temporary", error))?;
    output
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| LinuxInstallStoreError::io("set staged artifact mode", error))?;
    let opened_output = output
        .metadata()
        .map_err(|error| LinuxInstallStoreError::io("inspect opened staged temporary", error))?;
    validate_regular_metadata_modes(
        &opened_output,
        &[0o600],
        owner_id,
        group_id,
        expected_size,
        true,
    )?;
    let temporary_path = fs::symlink_metadata(&temporary)
        .map_err(|error| LinuxInstallStoreError::io("inspect staged temporary path", error))?;
    if UnixFileIdentity::from_metadata(&opened_output)
        != UnixFileIdentity::from_metadata(&temporary_path)
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "staged temporary identity changed before writing",
        ));
    }
    let mut digest = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| LinuxInstallStoreError::io("read artifact source", error))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| LinuxInstallStoreError::io("write staged artifact", error))?;
        digest.update(&buffer[..read]);
        copied += read as u64;
    }
    output
        .sync_all()
        .map_err(|error| LinuxInstallStoreError::io("sync staged artifact", error))?;
    let source_after = fs::symlink_metadata(source)
        .map_err(|error| LinuxInstallStoreError::io("reinspect artifact source", error))?;
    if UnixFileIdentity::from_metadata(&source_before)
        != UnixFileIdentity::from_metadata(&source_after)
        || copied != expected_size
        || sha256_digest(digest) != expected_sha256
    {
        let temporary_identity =
            UnixFileIdentity::from_metadata(&fs::symlink_metadata(&temporary).map_err(
                |error| LinuxInstallStoreError::io("inspect invalid staged temporary", error),
            )?);
        remove_exact_regular_file(&temporary, &temporary_identity)?;
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source changed or staged digest differs from committed identity",
        ));
    }
    drop(output);
    fs::rename(&temporary, target)
        .map_err(|error| LinuxInstallStoreError::io("commit staged artifact", error))?;
    sync_directory(target.parent().ok_or_else(|| {
        LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            "staged artifact has no parent",
        )
    })?)
}

pub(super) fn artifact_file_identity(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<ArtifactFileIdentity, LinuxInstallStoreError> {
    let stable =
        digest_stable_regular_file(path, &[0o600], owner_id, group_id, 512 * 1024 * 1024, false)?;
    Ok(ArtifactFileIdentity::new(
        stable.identity.device_id,
        stable.identity.inode,
        stable.identity.owner_id,
        stable.identity.group_id,
        stable.identity.mode,
        stable.identity.hardlink_count,
        stable.identity.size,
        stable.sha256,
    )?)
}

pub(super) fn staged_paths(operation_directory: &Path, slot: ArtifactSlot) -> StagedArtifactPaths {
    let prefix = match slot {
        ArtifactSlot::Source => "source",
        ArtifactSlot::Target => "target",
    };
    StagedArtifactPaths {
        package_path: operation_directory.join(format!("{prefix}.deb")),
        evidence_path: operation_directory.join(format!("{prefix}.evidence.json")),
    }
}

pub(super) fn staged_temporary_path(target: &Path) -> Result<PathBuf, LinuxInstallStoreError> {
    let filename = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "staged artifact filename is invalid",
            )
        })?;
    Ok(target.with_file_name(format!("{filename}{STAGED_TEMP_SUFFIX}")))
}

pub(super) fn path_entry_exists(path: &Path) -> Result<bool, LinuxInstallStoreError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(LinuxInstallStoreError::io(
            "inspect staged operation entry",
            error,
        )),
    }
}

fn sha256_digest(digest: Sha256) -> String {
    let mut value = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

pub(super) fn sync_directory(path: &Path) -> Result<(), LinuxInstallStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| LinuxInstallStoreError::io("sync directory", error))
}
