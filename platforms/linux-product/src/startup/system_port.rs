use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::read_only_state::same_file_identity;
use super::record::StartupRecord;
use crate::debian::{
    DebianRelationshipError, DebianRelationshipErrorCode, DirectDependency, DpkgCurrentState,
    DpkgStatusSnapshot,
};
use crate::model::{DpkgPackageState, LinuxArtifactIdentity, LinuxInstallReceipt};

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
        if receipt.installed_artifact() != Some(artifact) {
            return Err(super::record::invalid_record());
        }
        let record = StartupRecord::read(&self.paths)?.ok_or_else(super::record::invalid_record)?;
        let deps = record.dependencies_for(receipt)?;
        read_status(&self.paths)?
            .validate_installed_dependencies(
                artifact.package_name(),
                artifact.package_version(),
                artifact.architecture(),
                &deps,
            )
            .map_err(startup_relationship_error)
    }

    fn inspect_installed_package(
        &self,
        receipt: &LinuxInstallReceipt,
        artifact: &LinuxArtifactIdentity,
        dependencies: &[DirectDependency],
    ) -> Result<LinuxPackageObservation, LinuxStartupPortError> {
        if receipt.installed_artifact() != Some(artifact) {
            return Err(super::record::invalid_record());
        }
        let status = read_status(&self.paths)?;
        let observation = package_observation(&status)?;
        if observation.matches(artifact) {
            status
                .validate_installed_dependencies(
                    artifact.package_name(),
                    artifact.package_version(),
                    artifact.architecture(),
                    dependencies,
                )
                .map_err(startup_relationship_error)?;
        }
        Ok(observation)
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
    package_observation(&read_status(paths)?)
}

fn read_status(paths: &LinuxStartupPaths) -> Result<DpkgStatusSnapshot, LinuxStartupPortError> {
    let bytes = read_owned_regular_file(
        &paths.dpkg_status_path,
        paths.expected_owner_id,
        paths.expected_group_id,
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
    DpkgStatusSnapshot::parse(&bytes).map_err(startup_relationship_error)
}

fn package_observation(
    status: &DpkgStatusSnapshot,
) -> Result<LinuxPackageObservation, LinuxStartupPortError> {
    let mut products = status
        .records()
        .iter()
        .filter(|r| r.package() == "radishlex");
    let Some(record) = products.next() else {
        return Ok(LinuxPackageObservation::not_installed());
    };
    if products.next().is_some()
        || (record.current() == DpkgCurrentState::Installed && !record.is_fully_installed())
    {
        return Err(LinuxStartupPortError::new(
            LinuxStartupPortErrorCode::PackageStateUnknown,
            "ambiguous or inconsistent product status",
        ));
    }
    let state = DpkgPackageState::from(record.current());
    if state.is_absent() {
        return LinuxPackageObservation::new(state, None, None);
    }
    LinuxPackageObservation::new(
        state,
        record.version().map(str::to_owned),
        record.architecture().map(str::to_owned),
    )
}

pub(super) fn read_owned_regular_file(
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
    (&mut file)
        .take(maximum_size + 1)
        .read_to_end(&mut value)
        .map_err(port_io_error)?;
    let after = fs::symlink_metadata(path).map_err(port_io_error)?;
    if !same_file_identity(&before, &after) || value.len() as u64 != before.len() {
        return Err(LinuxStartupPortError::new(
            identity_error_code,
            "Linux startup source identity changed",
        ));
    }
    Ok(Some(value))
}

pub(super) fn port_io_error(error: io::Error) -> LinuxStartupPortError {
    LinuxStartupPortError::new(
        if error.kind() == io::ErrorKind::PermissionDenied {
            LinuxStartupPortErrorCode::PermissionDenied
        } else {
            LinuxStartupPortErrorCode::Io
        },
        "Linux startup inspection failed",
    )
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
