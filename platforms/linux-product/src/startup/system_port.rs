use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::debian::{DebianRelationshipError, DebianRelationshipErrorCode, DpkgStatusSnapshot};
use crate::model::{
    ArtifactSlot, DpkgPackageState, LinuxArtifactIdentity, LinuxInstallReceipt, LinuxInstallState,
};
use crate::system::{
    verify_owned_artifact, LinuxSystemObservationError, LinuxSystemObservationErrorCode,
};

use super::read_only_state::same_file_identity;

mod manifest;

use super::types::{
    LinuxPackageObservation, LinuxStartupComponent, LinuxStartupPaths, LinuxStartupPort,
    LinuxStartupPortError, LinuxStartupPortErrorCode,
};
use manifest::validate_system_component;
pub(crate) use manifest::validate_system_product;

const MAX_DPKG_STATUS_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct LinuxSystemStartupPort {
    paths: LinuxStartupPaths,
    component_path: PathBuf,
    runtime_ffi_abi_version: u32,
}

impl LinuxSystemStartupPort {
    pub fn new(
        paths: LinuxStartupPaths,
        component_path: PathBuf,
        runtime_ffi_abi_version: u32,
    ) -> Self {
        Self {
            paths,
            component_path,
            runtime_ffi_abi_version,
        }
    }
}

impl LinuxStartupPort for LinuxSystemStartupPort {
    fn inspect_package(&self) -> Result<LinuxPackageObservation, LinuxStartupPortError> {
        inspect_dpkg_status(&self.paths)
    }

    fn validate_package_relationship(
        &self,
        receipt: &LinuxInstallReceipt,
        artifact: &LinuxArtifactIdentity,
    ) -> Result<(), LinuxStartupPortError> {
        let preferred_slot = match receipt.state() {
            LinuxInstallState::Completed => ArtifactSlot::Target,
            LinuxInstallState::AbortedPreserved | LinuxInstallState::RolledBack => {
                ArtifactSlot::Source
            }
            _ => {
                return Err(LinuxStartupPortError::new(
                    LinuxStartupPortErrorCode::PackageIdentityChanged,
                    "startup package relationship requires a terminal receipt",
                ))
            }
        };
        let slot = [preferred_slot, alternate_slot(preferred_slot)]
            .into_iter()
            .find(|slot| {
                receipt
                    .staged_artifact(*slot)
                    .is_some_and(|staged| staged.artifact() == artifact)
            })
            .ok_or_else(|| {
                LinuxStartupPortError::new(
                    LinuxStartupPortErrorCode::PackageIdentityChanged,
                    "startup package relationship lacks its installed staged proof",
                )
            })?;
        let operation = self
            .paths
            .state_root
            .join("operations")
            .join(receipt.operation_id());
        let (package_name, evidence_name) = match slot {
            ArtifactSlot::Source => ("source.deb", "source.evidence.json"),
            ArtifactSlot::Target => ("target.deb", "target.evidence.json"),
        };
        let relationship = verify_owned_artifact(
            &operation.join(package_name),
            &operation.join(evidence_name),
            artifact,
            &[0o600],
            self.paths.expected_owner_id,
            self.paths.expected_group_id,
        )
        .map_err(startup_observation_error)?;
        let status = read_owned_regular_file(
            &self.paths.dpkg_status_path,
            self.paths.expected_owner_id,
            self.paths.expected_group_id,
            &[0o644],
            1,
            MAX_DPKG_STATUS_BYTES,
            LinuxStartupPortErrorCode::PackageStateUnknown,
        )?
        .ok_or_else(|| {
            LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnavailable,
                "dpkg status database is unavailable",
            )
        })?;
        let status = DpkgStatusSnapshot::parse(&status).map_err(startup_relationship_error)?;
        status
            .validate_installed_relationship(&relationship)
            .map_err(startup_relationship_error)
    }

    fn validate_component(
        &self,
        component: LinuxStartupComponent,
        artifact: &LinuxArtifactIdentity,
    ) -> Result<(), LinuxStartupPortError> {
        validate_system_component(
            &self.paths,
            &self.component_path,
            self.runtime_ffi_abi_version,
            component,
            artifact,
        )
    }
}

pub(super) fn inspect_dpkg_status(
    paths: &LinuxStartupPaths,
) -> Result<LinuxPackageObservation, LinuxStartupPortError> {
    let value = match read_owned_regular_file(
        &paths.dpkg_status_path,
        paths.expected_owner_id,
        paths.expected_group_id,
        &[0o644],
        1,
        MAX_DPKG_STATUS_BYTES,
        LinuxStartupPortErrorCode::PackageStateUnknown,
    ) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnavailable,
                "dpkg status database is unavailable",
            ))
        }
        Err(error) => return Err(error),
    };
    let text = std::str::from_utf8(&value).map_err(|_| {
        LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "dpkg status is not UTF-8",
        )
    })?;
    let mut radishlex = None;
    for paragraph in text.split("\n\n") {
        let fields = parse_debian_fields(paragraph)?;
        if fields.get("Package").map(String::as_str) != Some("radishlex") {
            continue;
        }
        if radishlex.replace(fields).is_some() {
            return Err(LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnknown,
                "dpkg status contains duplicate RadishLex paragraphs",
            ));
        }
    }
    let Some(fields) = radishlex else {
        return Ok(LinuxPackageObservation::not_installed());
    };
    let status = fields.get("Status").ok_or_else(|| {
        LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "RadishLex dpkg status is missing Status",
        )
    })?;
    let status_fields: Vec<_> = status.split_ascii_whitespace().collect();
    if status_fields.len() != 3
        || !matches!(
            status_fields[0],
            "unknown" | "install" | "hold" | "deinstall" | "purge"
        )
        || !matches!(status_fields[1], "ok" | "reinstreq")
    {
        return Err(LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "RadishLex dpkg status tuple is unknown",
        ));
    }
    let state_name = status_fields[2];
    let state = match state_name {
        "not-installed" => DpkgPackageState::NotInstalled,
        "config-files" => DpkgPackageState::ConfigFiles,
        "installed" => DpkgPackageState::Installed,
        "unpacked" => DpkgPackageState::Unpacked,
        "half-configured" => DpkgPackageState::HalfConfigured,
        "half-installed" => DpkgPackageState::HalfInstalled,
        "triggers-awaited" => DpkgPackageState::TriggersAwaited,
        "triggers-pending" => DpkgPackageState::TriggersPending,
        _ => DpkgPackageState::Unknown,
    };
    if state == DpkgPackageState::Unknown {
        return Err(LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "RadishLex dpkg state is unknown",
        ));
    }
    if state == DpkgPackageState::Installed
        && (!matches!(status_fields[0], "install" | "hold") || status_fields[1] != "ok")
    {
        return Err(LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "RadishLex installed dpkg tuple is inconsistent",
        ));
    }
    let (version, architecture) = if state.is_recoverable() {
        (
            Some(required_debian_field(&fields, "Version")?.to_owned()),
            Some(required_debian_field(&fields, "Architecture")?.to_owned()),
        )
    } else {
        (None, None)
    };
    LinuxPackageObservation::new(state, version, architecture)
}

fn parse_debian_fields(paragraph: &str) -> Result<BTreeMap<String, String>, LinuxStartupPortError> {
    let mut fields = BTreeMap::new();
    for line in paragraph.lines() {
        if line.is_empty() || line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnknown,
                "dpkg status contains a malformed field",
            ));
        };
        if name.is_empty() || fields.contains_key(name) {
            return Err(LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnknown,
                "dpkg status contains an invalid field",
            ));
        }
        fields.insert(name.to_owned(), value.trim().to_owned());
    }
    Ok(fields)
}

fn required_debian_field<'a>(
    fields: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, LinuxStartupPortError> {
    fields.get(name).map(String::as_str).ok_or_else(|| {
        LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "RadishLex dpkg identity field is missing",
        )
    })
}

fn read_owned_regular_file(
    path: &Path,
    owner_id: u32,
    group_id: u32,
    modes: &[u32],
    minimum_size: u64,
    maximum_size: u64,
    identity_error_code: LinuxStartupPortErrorCode,
) -> Result<Option<Vec<u8>>, LinuxStartupPortError> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(port_io_error(error)),
    };
    if fs::canonicalize(path).map_err(port_io_error)? != path
        || !before.file_type().is_file()
        || before.uid() != owner_id
        || before.gid() != group_id
        || !modes.contains(&(before.permissions().mode() & 0o7777))
        || before.nlink() != 1
        || before.len() < minimum_size
        || before.len() > maximum_size
    {
        return Err(LinuxStartupPortError::new(
            identity_error_code,
            "Linux startup source identity changed",
        ));
    }
    let mut file = File::open(path).map_err(port_io_error)?;
    let opened = file.metadata().map_err(port_io_error)?;
    if !same_file_identity(&before, &opened) {
        return Err(LinuxStartupPortError::new(
            identity_error_code,
            "Linux startup source identity changed",
        ));
    }
    let mut value = Vec::with_capacity(before.len() as usize);
    file.read_to_end(&mut value).map_err(port_io_error)?;
    let after = fs::symlink_metadata(path).map_err(port_io_error)?;
    if !same_file_identity(&before, &after) || value.len() as u64 != before.len() {
        return Err(LinuxStartupPortError::new(
            identity_error_code,
            "Linux startup source identity changed",
        ));
    }
    Ok(Some(value))
}

fn port_io_error(error: io::Error) -> LinuxStartupPortError {
    LinuxStartupPortError::new(
        if error.kind() == io::ErrorKind::PermissionDenied {
            LinuxStartupPortErrorCode::PermissionDenied
        } else {
            LinuxStartupPortErrorCode::Io
        },
        "Linux startup inspection failed",
    )
}

const fn alternate_slot(slot: ArtifactSlot) -> ArtifactSlot {
    match slot {
        ArtifactSlot::Source => ArtifactSlot::Target,
        ArtifactSlot::Target => ArtifactSlot::Source,
    }
}

fn startup_relationship_error(error: DebianRelationshipError) -> LinuxStartupPortError {
    let code = match error.code() {
        DebianRelationshipErrorCode::DependencyUnavailable
        | DebianRelationshipErrorCode::DependencyVersionUnsatisfied
        | DebianRelationshipErrorCode::DependencyArchitectureMismatch => {
            LinuxStartupPortErrorCode::DependencyUnavailable
        }
        DebianRelationshipErrorCode::DpkgStatusInvalid
        | DebianRelationshipErrorCode::PackageStateInvalid => {
            LinuxStartupPortErrorCode::PackageStateUnknown
        }
        _ => LinuxStartupPortErrorCode::PackageIdentityChanged,
    };
    LinuxStartupPortError::new(code, "Linux startup package relationship is invalid")
}

fn startup_observation_error(error: LinuxSystemObservationError) -> LinuxStartupPortError {
    let code = match error.code() {
        LinuxSystemObservationErrorCode::PermissionDenied => {
            LinuxStartupPortErrorCode::PermissionDenied
        }
        LinuxSystemObservationErrorCode::Io => LinuxStartupPortErrorCode::Io,
        _ => LinuxStartupPortErrorCode::PackageIdentityChanged,
    };
    LinuxStartupPortError::new(code, "Linux startup staged package identity changed")
}
