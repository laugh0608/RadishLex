use std::collections::BTreeSet;

use super::evidence::{parse_control_dependency_field, DirectDependency};
use super::version::parse_debian_version;
use super::{
    is_architecture, is_package_name, parse_debian_paragraph, validate_input_size,
    DebianParagraphMode, DebianRelationshipError, DebianRelationshipErrorCode,
    VerifiedArtifactRelationship, FONT_PACKAGES, MAX_DPKG_RECORDS, MAX_DPKG_STATUS_BYTES,
    NATIVE_ARCHITECTURE, PACKAGE_NAME,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgDesiredState {
    Unknown,
    Install,
    Hold,
    Deinstall,
    Purge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgErrorState {
    Ok,
    Reinstreq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgCurrentState {
    NotInstalled,
    ConfigFiles,
    HalfInstalled,
    Unpacked,
    HalfConfigured,
    TriggersAwaited,
    TriggersPending,
    Installed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgMultiArch {
    No,
    Same,
    Foreign,
    Allowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgPackageRecord {
    package: String,
    architecture: Option<String>,
    version: Option<String>,
    desired: DpkgDesiredState,
    error: DpkgErrorState,
    current: DpkgCurrentState,
    raw_dependencies: Option<String>,
    product_dependencies: Option<Vec<DirectDependency>>,
    multi_arch: Option<DpkgMultiArch>,
}

impl DpkgPackageRecord {
    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn architecture(&self) -> Option<&str> {
        self.architecture.as_deref()
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub const fn desired(&self) -> DpkgDesiredState {
        self.desired
    }

    pub const fn error(&self) -> DpkgErrorState {
        self.error
    }

    pub const fn current(&self) -> DpkgCurrentState {
        self.current
    }

    pub fn raw_dependencies(&self) -> Option<&str> {
        self.raw_dependencies.as_deref()
    }

    /// Returns parsed dependencies only for the RadishLex package record.
    ///
    /// Unrelated packages retain their raw field because the full Debian dependency grammar is
    /// intentionally outside this relationship domain.
    pub fn product_dependencies(&self) -> Option<&[DirectDependency]> {
        self.product_dependencies.as_deref()
    }

    pub const fn multi_arch(&self) -> Option<DpkgMultiArch> {
        self.multi_arch
    }

    fn is_fully_installed(&self) -> bool {
        matches!(
            self.desired,
            DpkgDesiredState::Install | DpkgDesiredState::Hold
        ) && self.error == DpkgErrorState::Ok
            && self.current == DpkgCurrentState::Installed
            && self.version.is_some()
            && self.architecture.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgStatusSnapshot {
    records: Vec<DpkgPackageRecord>,
}

impl DpkgStatusSnapshot {
    pub fn parse(value: &[u8]) -> Result<Self, DebianRelationshipError> {
        validate_input_size(
            value,
            MAX_DPKG_STATUS_BYTES,
            DebianRelationshipErrorCode::DpkgStatusInvalid,
            "dpkg status snapshot exceeds the size limit",
        )?;
        let text = std::str::from_utf8(value).map_err(|_| dpkg_status_error())?;
        if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
            return Err(dpkg_status_error());
        }
        let normalized = text.trim_end_matches('\n');
        if normalized.is_empty() {
            return Err(dpkg_status_error());
        }
        let mut records = Vec::new();
        let mut identities = BTreeSet::new();
        for paragraph in normalized.split("\n\n") {
            if paragraph.is_empty() || records.len() >= MAX_DPKG_RECORDS {
                return Err(dpkg_status_error());
            }
            let mut paragraph_owned = paragraph.to_owned();
            paragraph_owned.push('\n');
            let fields = parse_debian_paragraph(&paragraph_owned, DebianParagraphMode::DpkgStatus)
                .map_err(|_| dpkg_status_error())?;
            let record = parse_dpkg_record(&fields)?;
            let identity = (
                record.package.clone(),
                record.architecture.clone().unwrap_or_default(),
            );
            if !identities.insert(identity) {
                return Err(dpkg_status_error());
            }
            records.push(record);
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[DpkgPackageRecord] {
        &self.records
    }

    pub fn validate_installed_relationship(
        &self,
        artifact: &VerifiedArtifactRelationship,
    ) -> Result<(), DebianRelationshipError> {
        let product_records: Vec<_> = self
            .records
            .iter()
            .filter(|record| record.package == PACKAGE_NAME)
            .collect();
        if product_records.len() != 1 {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::PackageStateInvalid,
                "installed RadishLex package identity is missing or ambiguous",
            ));
        }
        let product = product_records[0];
        if product.package() != artifact.package_name()
            || product.architecture() != Some(artifact.architecture())
            || product.version() != Some(artifact.package_version())
        {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::PackageIdentityMismatch,
                "installed RadishLex package identity differs from the artifact",
            ));
        }
        if !product.is_fully_installed() {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::PackageStateInvalid,
                "installed RadishLex package is not fully configured",
            ));
        }
        if product.product_dependencies() != Some(artifact.dependencies()) {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::PackageIdentityMismatch,
                "installed RadishLex Depends differs from the artifact",
            ));
        }
        self.validate_dependencies(artifact)
    }

    /// Validates target package dependencies before any dpkg mutation.
    ///
    /// Unlike `validate_installed_relationship`, this does not require a RadishLex package record
    /// and is therefore suitable for install and staged target preflight.
    pub fn validate_dependencies(
        &self,
        artifact: &VerifiedArtifactRelationship,
    ) -> Result<(), DebianRelationshipError> {
        for dependency in artifact.dependencies() {
            self.validate_dependency(dependency)?;
        }
        Ok(())
    }

    fn validate_dependency(
        &self,
        dependency: &DirectDependency,
    ) -> Result<(), DebianRelationshipError> {
        let package_records: Vec<_> = self
            .records
            .iter()
            .filter(|record| record.package == dependency.package())
            .collect();
        if package_records.is_empty() {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::DependencyUnavailable,
                "a required Debian package is unavailable",
            ));
        }
        let required_architecture = if FONT_PACKAGES.contains(&dependency.package()) {
            "all"
        } else {
            dependency.architecture().unwrap_or(NATIVE_ARCHITECTURE)
        };
        let matching: Vec<_> = package_records
            .iter()
            .copied()
            .filter(|record| record.architecture() == Some(required_architecture))
            .collect();
        if matching.is_empty() {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::DependencyArchitectureMismatch,
                "a required Debian package has the wrong architecture",
            ));
        }
        if matching.len() != 1 {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::DpkgStatusInvalid,
                "a required Debian package identity is ambiguous",
            ));
        }
        let record = matching[0];
        if !record.is_fully_installed() {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::DependencyUnavailable,
                "a required Debian package is not fully configured",
            ));
        }
        if let Some(constraint) = dependency.constraint() {
            let version = record.version().ok_or_else(|| {
                DebianRelationshipError::new(
                    DebianRelationshipErrorCode::DpkgStatusInvalid,
                    "an installed Debian package has no version",
                )
            })?;
            if !constraint.is_satisfied_by(version)? {
                return Err(DebianRelationshipError::new(
                    DebianRelationshipErrorCode::DependencyVersionUnsatisfied,
                    "a required Debian package version is not satisfied",
                ));
            }
        }
        Ok(())
    }
}

fn parse_dpkg_record(
    fields: &[(String, String)],
) -> Result<DpkgPackageRecord, DebianRelationshipError> {
    let field = |name: &str| {
        fields
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    };
    let package = field("Package").ok_or_else(dpkg_status_error)?;
    let status = field("Status").ok_or_else(dpkg_status_error)?;
    if !is_package_name(package) {
        return Err(dpkg_status_error());
    }
    let status_parts: Vec<_> = status.split_ascii_whitespace().collect();
    if status_parts.len() != 3 || status_parts.join(" ") != status {
        return Err(dpkg_status_error());
    }
    let desired = match status_parts[0] {
        "unknown" => DpkgDesiredState::Unknown,
        "install" => DpkgDesiredState::Install,
        "hold" => DpkgDesiredState::Hold,
        "deinstall" => DpkgDesiredState::Deinstall,
        "purge" => DpkgDesiredState::Purge,
        _ => return Err(dpkg_status_error()),
    };
    let error = match status_parts[1] {
        "ok" => DpkgErrorState::Ok,
        "reinstreq" => DpkgErrorState::Reinstreq,
        _ => return Err(dpkg_status_error()),
    };
    let current = match status_parts[2] {
        "not-installed" => DpkgCurrentState::NotInstalled,
        "config-files" => DpkgCurrentState::ConfigFiles,
        "half-installed" => DpkgCurrentState::HalfInstalled,
        "unpacked" => DpkgCurrentState::Unpacked,
        "half-configured" => DpkgCurrentState::HalfConfigured,
        "triggers-awaited" => DpkgCurrentState::TriggersAwaited,
        "triggers-pending" => DpkgCurrentState::TriggersPending,
        "installed" => DpkgCurrentState::Installed,
        _ => return Err(dpkg_status_error()),
    };
    let architecture = field("Architecture").map(str::to_owned);
    if architecture
        .as_deref()
        .is_some_and(|value| value != "all" && !is_architecture(value))
    {
        return Err(dpkg_status_error());
    }
    let version = field("Version").map(str::to_owned);
    if let Some(version) = &version {
        parse_debian_version(version).map_err(|_| dpkg_status_error())?;
    }
    let carries_identity = architecture.is_some() || version.is_some();
    if architecture.is_some() != version.is_some()
        || (current != DpkgCurrentState::NotInstalled && !carries_identity)
    {
        return Err(dpkg_status_error());
    }
    let raw_dependencies = field("Depends").map(str::to_owned);
    let product_dependencies = if package == PACKAGE_NAME {
        raw_dependencies
            .as_deref()
            .map(parse_control_dependency_field)
            .transpose()
            .map_err(|_| dpkg_status_error())?
    } else {
        None
    };
    let multi_arch = field("Multi-Arch")
        .map(|value| match value {
            "no" => Ok(DpkgMultiArch::No),
            "same" => Ok(DpkgMultiArch::Same),
            "foreign" => Ok(DpkgMultiArch::Foreign),
            "allowed" => Ok(DpkgMultiArch::Allowed),
            _ => Err(dpkg_status_error()),
        })
        .transpose()?;
    Ok(DpkgPackageRecord {
        package: package.to_owned(),
        architecture,
        version,
        desired,
        error,
        current,
        raw_dependencies,
        product_dependencies,
        multi_arch,
    })
}

fn dpkg_status_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::DpkgStatusInvalid,
        "dpkg status snapshot is malformed or ambiguous",
    )
}
