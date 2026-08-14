use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::coordinator::DpkgStagedPackage;
use crate::debian::{
    validate_dpkg_configuration, DpkgCurrentState, DpkgStatusSnapshot, VerifiedArtifactRelationship,
};
use crate::model::LinuxArtifactIdentity;
use crate::startup::{
    validate_system_product, LinuxStartupPaths, LinuxStartupPortErrorCode, SYSTEM_DPKG_STATUS_PATH,
};

use super::process::prove_system_processes_quiescent;

const DPKG_MAIN_CONFIG: &str = "/etc/dpkg/dpkg.cfg";
const DPKG_CONFIG_DIRECTORY: &str = "/etc/dpkg/dpkg.cfg.d";
const MAX_DPKG_STATUS_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DPKG_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const RUNTIME_FFI_ABI_VERSION: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxSystemObservationErrorCode {
    EnvironmentUnsupported,
    ArtifactInvalid,
    DependencyUnavailable,
    ProgramsRunning,
    ProcessDisappeared,
    ProcessInspectionUnavailable,
    PackageStateUnavailable,
    PackageStateUnknown,
    ProductIdentityChanged,
    PermissionDenied,
    Io,
}

#[derive(Debug)]
pub struct LinuxSystemObservationError {
    code: LinuxSystemObservationErrorCode,
    message: &'static str,
    source: Option<io::Error>,
}

impl LinuxSystemObservationError {
    pub fn new(code: LinuxSystemObservationErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            source: None,
        }
    }

    fn io(message: &'static str, source: io::Error) -> Self {
        let code = if source.kind() == io::ErrorKind::PermissionDenied {
            LinuxSystemObservationErrorCode::PermissionDenied
        } else {
            LinuxSystemObservationErrorCode::Io
        };
        Self {
            code,
            message,
            source: Some(source),
        }
    }

    pub const fn code(&self) -> LinuxSystemObservationErrorCode {
        self.code
    }
}

impl fmt::Display for LinuxSystemObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Some(source) => write!(formatter, "{}: {}", self.message, source),
            None => formatter.write_str(self.message),
        }
    }
}

impl std::error::Error for LinuxSystemObservationError {}

pub trait DpkgSystemObserver {
    type QuiescencePermit;

    fn validate_environment(&mut self) -> Result<(), LinuxSystemObservationError>;

    fn inspect_status(&mut self) -> Result<DpkgStatusSnapshot, LinuxSystemObservationError>;

    fn verify_staged_package(
        &mut self,
        package: DpkgStagedPackage<'_>,
    ) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError>;

    fn prove_quiescent(&mut self) -> Result<Self::QuiescencePermit, LinuxSystemObservationError>;

    fn validate_installed_product(
        &mut self,
        artifact: &LinuxArtifactIdentity,
        relationship: &VerifiedArtifactRelationship,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError>;

    fn validate_absent_product(
        &mut self,
        relationship: Option<&VerifiedArtifactRelationship>,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError>;
}

#[derive(Debug)]
pub struct LinuxSystemQuiescencePermit {
    _private: (),
}

#[derive(Debug, Default)]
pub struct LinuxSystemObserver;

impl DpkgSystemObserver for LinuxSystemObserver {
    type QuiescencePermit = LinuxSystemQuiescencePermit;

    fn validate_environment(&mut self) -> Result<(), LinuxSystemObservationError> {
        validate_effective_root()?;
        validate_dpkg_configuration_files()
    }

    fn inspect_status(&mut self) -> Result<DpkgStatusSnapshot, LinuxSystemObservationError> {
        let value = read_owned_regular_file(
            Path::new(SYSTEM_DPKG_STATUS_PATH),
            &[0o644],
            1,
            MAX_DPKG_STATUS_BYTES,
        )?;
        DpkgStatusSnapshot::parse(&value).map_err(|_| {
            LinuxSystemObservationError::new(
                LinuxSystemObservationErrorCode::PackageStateUnknown,
                "dpkg status snapshot is malformed or ambiguous",
            )
        })
    }

    fn verify_staged_package(
        &mut self,
        package: DpkgStagedPackage<'_>,
    ) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
        verify_root_owned_artifact(
            package.package_path(),
            package.evidence_path(),
            package.artifact(),
            &[0o600],
        )
    }

    fn prove_quiescent(&mut self) -> Result<Self::QuiescencePermit, LinuxSystemObservationError> {
        prove_system_processes_quiescent()?;
        Ok(LinuxSystemQuiescencePermit { _private: () })
    }

    fn validate_installed_product(
        &mut self,
        artifact: &LinuxArtifactIdentity,
        relationship: &VerifiedArtifactRelationship,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError> {
        status
            .validate_installed_relationship(relationship)
            .map_err(relationship_observation_error)?;
        validate_system_product(
            &LinuxStartupPaths::system(),
            RUNTIME_FFI_ABI_VERSION,
            artifact,
        )
        .map_err(|error| {
            LinuxSystemObservationError::new(
                match error.code() {
                    LinuxStartupPortErrorCode::DependencyUnavailable => {
                        LinuxSystemObservationErrorCode::DependencyUnavailable
                    }
                    LinuxStartupPortErrorCode::PermissionDenied => {
                        LinuxSystemObservationErrorCode::PermissionDenied
                    }
                    LinuxStartupPortErrorCode::Io => LinuxSystemObservationErrorCode::Io,
                    _ => LinuxSystemObservationErrorCode::ProductIdentityChanged,
                },
                "installed Linux product inventory is invalid",
            )
        })
    }

    fn validate_absent_product(
        &mut self,
        relationship: Option<&VerifiedArtifactRelationship>,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError> {
        let product_records = status
            .records()
            .iter()
            .filter(|record| record.package() == "radishlex")
            .collect::<Vec<_>>();
        if product_records.len() > 1
            || product_records.first().is_some_and(|record| {
                !matches!(
                    record.current(),
                    DpkgCurrentState::NotInstalled | DpkgCurrentState::ConfigFiles
                )
            })
        {
            return Err(LinuxSystemObservationError::new(
                LinuxSystemObservationErrorCode::PackageStateUnknown,
                "RadishLex package is not absent after removal",
            ));
        }
        if let Some(relationship) = relationship {
            for path in relationship.installed_file_paths() {
                match fs::symlink_metadata(path) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(LinuxSystemObservationError::io(
                            "inspect removed product path",
                            error,
                        ))
                    }
                    Ok(_) => {
                        return Err(LinuxSystemObservationError::new(
                            LinuxSystemObservationErrorCode::ProductIdentityChanged,
                            "a removed RadishLex product file is still present",
                        ))
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn verify_root_owned_artifact(
    package_path: &Path,
    evidence_path: &Path,
    expected: &LinuxArtifactIdentity,
    modes: &[u32],
) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
    verify_owned_artifact(package_path, evidence_path, expected, modes, 0, 0)
}

pub(crate) fn verify_root_owned_input_artifact(
    package_path: &Path,
    evidence_path: &Path,
) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
    let package_filename = regular_leaf_name(package_path)?;
    let evidence_filename = regular_leaf_name(evidence_path)?;
    verify_owned_artifact_named(
        package_path,
        evidence_path,
        package_filename,
        evidence_filename,
        &[0o600, 0o644],
        0,
        0,
    )
}

pub(crate) fn verify_owned_artifact(
    package_path: &Path,
    evidence_path: &Path,
    expected: &LinuxArtifactIdentity,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
    let relationship = verify_owned_artifact_named(
        package_path,
        evidence_path,
        expected.package_filename(),
        expected.evidence_filename(),
        modes,
        owner_id,
        group_id,
    )?;
    if !relationship
        .matches_linux_artifact_identity(expected)
        .map_err(|_| artifact_identity_error())?
    {
        return Err(artifact_identity_error());
    }
    Ok(relationship)
}

#[allow(clippy::too_many_arguments)]
fn verify_owned_artifact_named(
    package_path: &Path,
    evidence_path: &Path,
    package_filename: &str,
    evidence_filename: &str,
    modes: &[u32],
    owner_id: u32,
    group_id: u32,
) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
    let evidence = read_regular_file_with_identity(
        evidence_path,
        modes,
        1,
        MAX_EVIDENCE_BYTES,
        owner_id,
        group_id,
    )?;
    let package_before = validate_regular_file_identity(
        package_path,
        modes,
        1,
        MAX_PACKAGE_BYTES,
        owner_id,
        group_id,
    )?;
    let mut package = File::open(package_path)
        .map_err(|error| LinuxSystemObservationError::io("open staged Debian package", error))?;
    let opened = package
        .metadata()
        .map_err(|error| LinuxSystemObservationError::io("inspect opened Debian package", error))?;
    if !same_file_identity(&package_before, &opened) {
        return Err(artifact_identity_error());
    }
    let relationship = VerifiedArtifactRelationship::verify_package(
        package_filename,
        evidence_filename,
        &mut package,
        &evidence,
    )
    .map_err(|_| artifact_identity_error())?;
    let package_after = fs::symlink_metadata(package_path)
        .map_err(|error| LinuxSystemObservationError::io("reinspect Debian package", error))?;
    if !same_file_identity(&package_before, &package_after) {
        return Err(artifact_identity_error());
    }
    Ok(relationship)
}

fn regular_leaf_name(path: &Path) -> Result<&str, LinuxSystemObservationError> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(artifact_identity_error());
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(artifact_identity_error)
}

pub(crate) fn validate_effective_root() -> Result<(), LinuxSystemObservationError> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status").map_err(|error| {
            LinuxSystemObservationError::io("read privileged process identity", error)
        })?;
        for field in ["Uid:", "Gid:"] {
            let values = status
                .lines()
                .find_map(|line| line.strip_prefix(field))
                .ok_or_else(|| {
                    LinuxSystemObservationError::new(
                        LinuxSystemObservationErrorCode::EnvironmentUnsupported,
                        "privileged process identity is unavailable",
                    )
                })?
                .split_ascii_whitespace()
                .collect::<Vec<_>>();
            if values.len() != 4 || values[1] != "0" {
                return Err(LinuxSystemObservationError::new(
                    LinuxSystemObservationErrorCode::PermissionDenied,
                    "Linux maintenance host requires effective root identity",
                ));
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(LinuxSystemObservationError::new(
            LinuxSystemObservationErrorCode::EnvironmentUnsupported,
            "Linux maintenance host is unavailable on this platform",
        ))
    }
}

fn validate_dpkg_configuration_files() -> Result<(), LinuxSystemObservationError> {
    let main = read_owned_regular_file(
        Path::new(DPKG_MAIN_CONFIG),
        &[0o644],
        0,
        MAX_DPKG_CONFIG_BYTES,
    )?;
    validate_configuration_bytes(&main)?;
    let directory = Path::new(DPKG_CONFIG_DIRECTORY);
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        LinuxSystemObservationError::io("inspect dpkg configuration directory", error)
    })?;
    if fs::canonicalize(directory).map_err(|error| {
        LinuxSystemObservationError::io("canonicalize dpkg configuration directory", error)
    })? != directory
        || !metadata.file_type().is_dir()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.mode() & 0o7777 != 0o755
    {
        return Err(environment_identity_error());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| LinuxSystemObservationError::io("read dpkg configuration", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| LinuxSystemObservationError::io("read dpkg configuration entry", error))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(environment_identity_error)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(environment_identity_error());
        }
        let value = read_owned_regular_file(&entry.path(), &[0o644], 0, MAX_DPKG_CONFIG_BYTES)?;
        validate_configuration_bytes(&value)?;
    }
    Ok(())
}

fn validate_configuration_bytes(value: &[u8]) -> Result<(), LinuxSystemObservationError> {
    let value = std::str::from_utf8(value).map_err(|_| environment_identity_error())?;
    validate_dpkg_configuration(value).map_err(|_| environment_identity_error())?;
    Ok(())
}

fn read_owned_regular_file(
    path: &Path,
    modes: &[u32],
    minimum_size: u64,
    maximum_size: u64,
) -> Result<Vec<u8>, LinuxSystemObservationError> {
    read_regular_file_with_identity(path, modes, minimum_size, maximum_size, 0, 0)
}

fn read_regular_file_with_identity(
    path: &Path,
    modes: &[u32],
    minimum_size: u64,
    maximum_size: u64,
    owner_id: u32,
    group_id: u32,
) -> Result<Vec<u8>, LinuxSystemObservationError> {
    let before = validate_regular_file_identity(
        path,
        modes,
        minimum_size,
        maximum_size,
        owner_id,
        group_id,
    )?;
    let mut file = File::open(path)
        .map_err(|error| LinuxSystemObservationError::io("open owned regular file", error))?;
    let opened = file
        .metadata()
        .map_err(|error| LinuxSystemObservationError::io("inspect opened regular file", error))?;
    if !same_file_identity(&before, &opened) {
        return Err(environment_identity_error());
    }
    let mut value = Vec::with_capacity(before.len() as usize);
    file.read_to_end(&mut value)
        .map_err(|error| LinuxSystemObservationError::io("read owned regular file", error))?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| LinuxSystemObservationError::io("reinspect owned regular file", error))?;
    if !same_file_identity(&before, &after) || value.len() as u64 != before.len() {
        return Err(environment_identity_error());
    }
    Ok(value)
}

fn validate_regular_file_identity(
    path: &Path,
    modes: &[u32],
    minimum_size: u64,
    maximum_size: u64,
    owner_id: u32,
    group_id: u32,
) -> Result<fs::Metadata, LinuxSystemObservationError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxSystemObservationError::io("inspect owned regular file", error))?;
    if fs::canonicalize(path).map_err(|error| {
        LinuxSystemObservationError::io("canonicalize owned regular file", error)
    })? != path
        || !metadata.file_type().is_file()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || !modes.contains(&(metadata.mode() & 0o7777))
        || metadata.nlink() != 1
        || metadata.len() < minimum_size
        || metadata.len() > maximum_size
    {
        return Err(environment_identity_error());
    }
    Ok(metadata)
}

fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
}

fn relationship_observation_error(
    error: crate::debian::DebianRelationshipError,
) -> LinuxSystemObservationError {
    use crate::debian::DebianRelationshipErrorCode as Code;
    let code = match error.code() {
        Code::DependencyUnavailable
        | Code::DependencyVersionUnsatisfied
        | Code::DependencyArchitectureMismatch => {
            LinuxSystemObservationErrorCode::DependencyUnavailable
        }
        Code::DpkgStatusInvalid | Code::PackageStateInvalid => {
            LinuxSystemObservationErrorCode::PackageStateUnknown
        }
        _ => LinuxSystemObservationErrorCode::ProductIdentityChanged,
    };
    LinuxSystemObservationError::new(code, "Linux package relationship validation failed")
}

fn artifact_identity_error() -> LinuxSystemObservationError {
    LinuxSystemObservationError::new(
        LinuxSystemObservationErrorCode::ArtifactInvalid,
        "staged Debian artifact identity is invalid",
    )
}

fn environment_identity_error() -> LinuxSystemObservationError {
    LinuxSystemObservationError::new(
        LinuxSystemObservationErrorCode::EnvironmentUnsupported,
        "Linux dpkg environment differs from the supported profile",
    )
}
