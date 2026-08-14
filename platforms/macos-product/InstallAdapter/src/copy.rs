use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{error, MacOsInstallAdapterError, MacOsInstallAdapterErrorCode};

const DITTO_PATH: &str = "/usr/bin/ditto";
const XATTR_PATH: &str = "/usr/bin/xattr";
const QUARANTINE_ATTRIBUTE: &str = "com.apple.quarantine";

pub(crate) trait BundleCopier: Send + Sync {
    fn copy_bundle(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), MacOsInstallAdapterError>;

    fn verify_staged_bundle_metadata(&self, staged: &Path) -> Result<(), MacOsInstallAdapterError>;
}

#[derive(Debug)]
pub(crate) struct DittoBundleCopier;

impl BundleCopier for DittoBundleCopier {
    fn copy_bundle(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), MacOsInstallAdapterError> {
        if !source.is_absolute() || !destination.is_absolute() {
            return Err(error(MacOsInstallAdapterErrorCode::CopyFailed));
        }
        match fs::symlink_metadata(destination) {
            Err(io_error) if io_error.kind() == ErrorKind::NotFound => {}
            Ok(_) => return Err(error(MacOsInstallAdapterErrorCode::StagingConflict)),
            Err(_) => return Err(error(MacOsInstallAdapterErrorCode::Io)),
        }
        let status = Command::new(DITTO_PATH)
            .env_clear()
            .args([
                "-X",
                "--rsrc",
                "--extattr",
                "--noqtn",
                "--acl",
                "--preserveHFSCompression",
                "--nocache",
            ])
            .arg(source)
            .arg(destination)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        if !status.success() {
            return Err(error(MacOsInstallAdapterErrorCode::CopyFailed));
        }
        Ok(())
    }

    fn verify_staged_bundle_metadata(&self, staged: &Path) -> Result<(), MacOsInstallAdapterError> {
        if !staged.is_absolute() {
            return Err(error(
                MacOsInstallAdapterErrorCode::StagedQuarantineRejected,
            ));
        }
        let metadata = fs::symlink_metadata(staged)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::StagedQuarantineRejected))?;
        if !metadata.file_type().is_dir() {
            return Err(error(
                MacOsInstallAdapterErrorCode::StagedQuarantineRejected,
            ));
        }
        verify_quarantine_absent(staged)
    }
}

fn verify_quarantine_absent(path: &Path) -> Result<(), MacOsInstallAdapterError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::StagedQuarantineRejected))?;
    let output = Command::new(XATTR_PATH)
        .env_clear()
        .arg("-s")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::StagedQuarantineRejected))?;
    if !output.status.success()
        || output
            .stdout
            .split(|byte| *byte == b'\n')
            .any(|attribute| attribute == QUARANTINE_ATTRIBUTE.as_bytes())
    {
        return Err(error(
            MacOsInstallAdapterErrorCode::StagedQuarantineRejected,
        ));
    }
    if metadata.file_type().is_symlink() || metadata.file_type().is_file() {
        return Ok(());
    }
    if !metadata.file_type().is_dir() {
        return Err(error(
            MacOsInstallAdapterErrorCode::StagedQuarantineRejected,
        ));
    }
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::StagedQuarantineRejected))?
        .collect::<Result<_, _>>()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::StagedQuarantineRejected))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        verify_quarantine_absent(&entry.path())?;
    }
    Ok(())
}

pub(crate) fn sync_bundle_tree(root: &Path) -> Result<(), MacOsInstallAdapterError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
    if !metadata.file_type().is_dir() {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    sync_directory_tree(root)
}

fn sync_directory_tree(directory: &Path) -> Result<(), MacOsInstallAdapterError> {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?
        .collect::<Result<_, _>>()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.file_type().is_dir() {
            sync_directory_tree(&path)?;
        } else if metadata.file_type().is_file() {
            File::open(&path)
                .and_then(|file| file.sync_all())
                .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?;
        } else {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
    }
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))
}
