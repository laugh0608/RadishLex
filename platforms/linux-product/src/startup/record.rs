use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::read_only_state::same_file_identity;
use super::system_port::{port_io_error, read_owned_regular_file};
use super::{LinuxStartupPaths, LinuxStartupPortError, LinuxStartupPortErrorCode};
use crate::debian::{parse_dependency_texts, validate_dependency_profile, DirectDependency};
use crate::model::LinuxInstallReceipt;

const FORMAT: &str = "radishlex-linux-startup-v1";
const MAX_BYTES: u64 = 128 * 1024;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StartupRecord {
    format: String,
    receipt_sha256: String,
    depends: Vec<String>,
}

impl LinuxStartupPaths {
    pub(crate) fn record_path(&self) -> PathBuf {
        self.state_root.with_file_name("startup-v1.json")
    }

    pub(crate) fn record_tmp(&self) -> PathBuf {
        self.state_root.with_file_name("startup-v1.json.tmp")
    }
}

pub(crate) fn invalid_record() -> LinuxStartupPortError {
    LinuxStartupPortError::new(
        LinuxStartupPortErrorCode::PackageIdentityChanged,
        "invalid Linux startup record",
    )
}

impl StartupRecord {
    pub(crate) fn new(
        receipt: &LinuxInstallReceipt,
        dependencies: &[DirectDependency],
    ) -> Result<Self, LinuxStartupPortError> {
        validate_dependency_profile(dependencies).map_err(|_| invalid_record())?;
        Ok(Self {
            format: FORMAT.to_owned(),
            receipt_sha256: receipt_digest(receipt)?,
            depends: dependencies
                .iter()
                .map(DirectDependency::to_control_text)
                .collect(),
        })
    }

    pub(crate) fn dependencies_for(
        &self,
        receipt: &LinuxInstallReceipt,
    ) -> Result<Vec<DirectDependency>, LinuxStartupPortError> {
        if self.receipt_sha256 != receipt_digest(receipt)? {
            return Err(invalid_record());
        }
        let deps = parse_dependency_texts(&self.depends).map_err(|_| invalid_record())?;
        validate_dependency_profile(&deps).map_err(|_| invalid_record())?;
        Ok(deps)
    }

    fn encode(&self) -> Result<Vec<u8>, LinuxStartupPortError> {
        let mut bytes = serde_json::to_vec(self).map_err(|_| invalid_record())?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid_record());
        }
        Ok(bytes)
    }

    pub(crate) fn read(paths: &LinuxStartupPaths) -> Result<Option<Self>, LinuxStartupPortError> {
        let bytes = read_owned_regular_file(
            &paths.record_path(),
            paths.expected_owner_id,
            paths.expected_group_id,
            &[0o644],
            1,
            MAX_BYTES,
            LinuxStartupPortErrorCode::PackageIdentityChanged,
        )?;
        bytes
            .map(|bytes| {
                let record: Self = serde_json::from_slice(&bytes).map_err(|_| invalid_record())?;
                if record.format != FORMAT || record.encode()? != bytes {
                    return Err(invalid_record());
                }
                Ok(record)
            })
            .transpose()
    }

    // The caller holds the maintenance guard throughout all record mutations.
    pub(crate) fn revoke(&self, paths: &LinuxStartupPaths) -> Result<(), LinuxStartupPortError> {
        if Self::read(paths)?.as_ref() != Some(self) {
            return Err(invalid_record());
        }
        fs::remove_file(paths.record_path()).map_err(port_io_error)?;
        sync_parent(paths)
    }

    pub(crate) fn publish(&self, paths: &LinuxStartupPaths) -> Result<(), LinuxStartupPortError> {
        if Self::read(paths)?.is_some() {
            return Err(invalid_record());
        }
        let bytes = self.encode()?;
        let tmp = paths.record_tmp();
        let file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)
        {
            Ok(mut file) => {
                file.write_all(&bytes).map_err(port_io_error)?;
                file.sync_all().map_err(port_io_error)?;
                file
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if !temporary_record_present(paths)? {
                    return Err(invalid_record());
                }
                File::open(&tmp).map_err(port_io_error)?
            }
            Err(e) => return Err(port_io_error(e)),
        };
        let observed = read_owned_regular_file(
            &tmp,
            paths.expected_owner_id,
            paths.expected_group_id,
            &[0o000, 0o200, 0o400, 0o600, 0o644],
            1,
            MAX_BYTES,
            LinuxStartupPortErrorCode::PackageIdentityChanged,
        )?;
        if observed.as_deref() != Some(bytes.as_slice())
            || !same_file_identity(
                &file.metadata().map_err(port_io_error)?,
                &fs::symlink_metadata(&tmp).map_err(port_io_error)?,
            )
        {
            return Err(invalid_record());
        }
        file.set_permissions(fs::Permissions::from_mode(0o644))
            .map_err(port_io_error)?;
        file.sync_all().map_err(port_io_error)?;
        if Self::read(paths)?.is_some()
            || !same_file_identity(
                &file.metadata().map_err(port_io_error)?,
                &fs::symlink_metadata(&tmp).map_err(port_io_error)?,
            )
        {
            return Err(invalid_record());
        }
        fs::rename(tmp, paths.record_path()).map_err(port_io_error)?;
        sync_parent(paths)?;
        if Self::read(paths)?.as_ref() != Some(self) {
            return Err(invalid_record());
        }
        Ok(())
    }
}

pub(crate) fn sync_parent(paths: &LinuxStartupPaths) -> Result<(), LinuxStartupPortError> {
    let parent = paths.state_root.parent().ok_or_else(invalid_record)?;
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(port_io_error)
}

pub(crate) fn temporary_record_present(
    paths: &LinuxStartupPaths,
) -> Result<bool, LinuxStartupPortError> {
    let meta = match fs::symlink_metadata(paths.record_tmp()) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(port_io_error(e)),
        Ok(meta) => meta,
    };
    if !meta.is_file()
        || meta.uid() != paths.expected_owner_id
        || meta.gid() != paths.expected_group_id
        || meta.nlink() != 1
        || meta.len() > MAX_BYTES
        || !matches!(meta.mode() & 0o7777, 0o000 | 0o200 | 0o400 | 0o600 | 0o644)
    {
        return Err(invalid_record());
    }
    Ok(true)
}

fn receipt_digest(receipt: &LinuxInstallReceipt) -> Result<String, LinuxStartupPortError> {
    if !receipt.state().is_terminal() || receipt.installed_artifact().is_none() {
        return Err(invalid_record());
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(receipt.encode().map_err(|_| invalid_record())?)
    ))
}
