use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::manifest::ProductManifestV1;
use super::version::{
    compare_debian_versions, parse_debian_version, validate_radishlex_package_version,
};
use super::{
    is_architecture, is_package_name, is_sha256, parse_debian_paragraph, validate_input_size,
    ArtifactRelationshipInput, DebianParagraphMode, DebianRelationshipError,
    DebianRelationshipErrorCode, VerifiedArtifactRelationship, DEPENDENCY_ANALYSIS_PROFILE,
    DISTRIBUTION_IDENTITY, FIXED_DEPENDENCIES, MAX_CONTROL_BYTES, MAX_EVIDENCE_BYTES,
    MAX_MANIFEST_BYTES, NATIVE_ARCHITECTURE, PACKAGE_NAME, PRODUCT_MANIFEST_PATH,
};

pub(super) fn verify_artifact_relationship(
    input: ArtifactRelationshipInput<'_>,
) -> Result<VerifiedArtifactRelationship, DebianRelationshipError> {
    validate_input_size(
        input.binary_control_bytes,
        MAX_CONTROL_BYTES,
        DebianRelationshipErrorCode::BinaryControlInvalid,
        "Debian binary control exceeds the size limit",
    )?;
    validate_input_size(
        input.product_manifest_bytes,
        MAX_MANIFEST_BYTES,
        DebianRelationshipErrorCode::ArtifactContentMismatch,
        "Linux product manifest exceeds the size limit",
    )?;
    validate_input_size(
        input.evidence_bytes,
        MAX_EVIDENCE_BYTES,
        DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
        "Debian artifact evidence exceeds the size limit",
    )?;

    let evidence = ArtifactEvidenceV1::parse(input.evidence_bytes)?;
    evidence.validate_profile()?;
    let evidence_size = u64::try_from(input.evidence_bytes.len()).map_err(|_| {
        DebianRelationshipError::new(
            DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
            "Debian artifact evidence size cannot be represented",
        )
    })?;
    let evidence_sha256 = sha256_bytes(input.evidence_bytes);
    let control_sha256 = sha256_bytes(input.binary_control_bytes);
    let product_manifest_sha256 = sha256_bytes(input.product_manifest_bytes);
    if evidence.package.size != input.package_content.size()
        || evidence.package.sha256 != input.package_content.sha256()
        || evidence.control.sha256 != control_sha256
        || evidence.product_manifest.sha256 != product_manifest_sha256
    {
        return Err(DebianRelationshipError::new(
            DebianRelationshipErrorCode::ArtifactContentMismatch,
            "Debian artifact content differs from its evidence",
        ));
    }

    let control = BinaryControlSnapshot::parse(input.binary_control_bytes)?;
    validate_dependency_profile(&control.dependencies)?;
    let product_manifest = ProductManifestV1::parse(input.product_manifest_bytes)?;
    let data_contract = product_manifest.validate_profile(&control)?;
    let installed_file_paths = product_manifest.installed_file_paths();

    let expected_package_filename = format!(
        "{}_{}_{}.deb",
        control.package, control.version, control.architecture
    );
    let expected_evidence_filename = format!("{expected_package_filename}.evidence.json");
    if input.package_filename != expected_package_filename
        || input.evidence_filename != expected_evidence_filename
        || evidence.package.filename != expected_package_filename
    {
        return Err(DebianRelationshipError::new(
            DebianRelationshipErrorCode::PackageIdentityMismatch,
            "Debian artifact filenames differ from the binary control identity",
        ));
    }
    if evidence.control.package != control.package
        || evidence.control.version != control.version
        || evidence.control.architecture != control.architecture
        || evidence.control.installed_size_kib != control.installed_size_kib
        || evidence.control.depends != control.dependency_texts()
    {
        return Err(DebianRelationshipError::new(
            DebianRelationshipErrorCode::ArtifactContentMismatch,
            "Debian binary control differs from its evidence",
        ));
    }
    if control.package != PACKAGE_NAME || control.architecture != NATIVE_ARCHITECTURE {
        return Err(DebianRelationshipError::new(
            DebianRelationshipErrorCode::PackageIdentityMismatch,
            "Debian package name or architecture differs from the product profile",
        ));
    }
    validate_radishlex_package_version(&control.version)?;

    Ok(VerifiedArtifactRelationship {
        package_filename: expected_package_filename,
        evidence_filename: expected_evidence_filename,
        package_name: control.package,
        package_version: control.version,
        architecture: control.architecture,
        package_size: input.package_content.size(),
        package_sha256: input.package_content.sha256().to_owned(),
        evidence_size,
        evidence_sha256,
        product_manifest_sha256,
        data_contract,
        dependencies: control.dependencies,
        installed_file_paths,
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactEvidenceV1 {
    pub(super) archive: ArchiveEvidence,
    pub(super) control: ControlEvidence,
    pub(super) dependency_analysis: DependencyAnalysisEvidence,
    pub(super) distribution_identity: String,
    pub(super) format_version: u32,
    pub(super) package: PackageEvidence,
    pub(super) product_manifest: ProductManifestEvidence,
}

impl ArtifactEvidenceV1 {
    pub(super) fn parse(value: &[u8]) -> Result<Self, DebianRelationshipError> {
        let evidence: Self = serde_json::from_slice(value).map_err(|_| {
            DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
                "Debian artifact evidence is not format v1 JSON",
            )
        })?;
        let mut canonical = serde_json::to_vec_pretty(&evidence).map_err(|_| {
            DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
                "Debian artifact evidence cannot be encoded canonically",
            )
        })?;
        canonical.push(b'\n');
        if canonical != value {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
                "Debian artifact evidence is not canonical JSON",
            ));
        }
        Ok(evidence)
    }

    pub(super) fn validate_profile(&self) -> Result<(), DebianRelationshipError> {
        let private_libraries = self
            .dependency_analysis
            .private_libraries
            .iter()
            .map(String::as_str);
        if self.format_version != 1
            || self.distribution_identity != DISTRIBUTION_IDENTITY
            || self.archive.compression != "none"
            || self.archive.format != "debian-binary-2.0-ar-v1"
            || self.archive.source_date_epoch != 0
            || self.archive.tar_format != "ustar"
            || self.dependency_analysis.loader_owner != "libc6:arm64"
            || private_libraries.ne(["libflutter_linux_gtk.so", "libradishlex_ime_ffi.so"])
            || self.dependency_analysis.profile != DEPENDENCY_ANALYSIS_PROFILE
            || self.dependency_analysis.stderr_policy
                != "exact-private-libraries-and-libc6-usrmerge-v1"
            || self.dependency_analysis.tool != "dpkg-shlibdeps"
            || self.product_manifest.format_version != 1
            || self.product_manifest.path != PRODUCT_MANIFEST_PATH
            || self.control.installed_size_kib == 0
            || self.package.size == 0
        {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
                "Debian artifact evidence differs from the fixed profile",
            ));
        }
        let expected_members = ["debian-binary", "control.tar", "data.tar"];
        if self.archive.members.len() != expected_members.len()
            || self
                .archive
                .members
                .iter()
                .zip(expected_members)
                .any(|(member, expected)| {
                    member.name != expected || member.size == 0 || !is_sha256(&member.sha256)
                })
            || self.archive.members[0].size != 4
            || self.archive.members[0].sha256 != sha256_bytes(b"2.0\n")
            || !is_sha256(&self.package.sha256)
            || !is_sha256(&self.control.sha256)
            || !is_sha256(&self.control.md5sums_sha256)
            || !is_sha256(&self.product_manifest.sha256)
        {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
                "Debian artifact evidence hashes or archive members are invalid",
            ));
        }
        let evidence_dependencies = parse_dependency_texts(&self.control.depends)?;
        validate_dependency_profile(&evidence_dependencies)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArchiveEvidence {
    pub(super) compression: String,
    pub(super) format: String,
    pub(super) members: Vec<ArchiveMemberEvidence>,
    pub(super) source_date_epoch: u64,
    pub(super) tar_format: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArchiveMemberEvidence {
    pub(super) name: String,
    pub(super) sha256: String,
    pub(super) size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlEvidence {
    pub(super) architecture: String,
    pub(super) depends: Vec<String>,
    pub(super) installed_size_kib: u64,
    pub(super) md5sums_sha256: String,
    pub(super) package: String,
    pub(super) sha256: String,
    pub(super) version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DependencyAnalysisEvidence {
    pub(super) loader_owner: String,
    pub(super) private_libraries: Vec<String>,
    pub(super) profile: String,
    pub(super) stderr_policy: String,
    pub(super) tool: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PackageEvidence {
    pub(super) filename: String,
    pub(super) sha256: String,
    pub(super) size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProductManifestEvidence {
    pub(super) format_version: u32,
    pub(super) path: String,
    pub(super) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryControlSnapshot {
    package: String,
    version: String,
    architecture: String,
    installed_size_kib: u64,
    dependencies: Vec<DirectDependency>,
}

impl BinaryControlSnapshot {
    pub fn parse(value: &[u8]) -> Result<Self, DebianRelationshipError> {
        validate_input_size(
            value,
            MAX_CONTROL_BYTES,
            DebianRelationshipErrorCode::BinaryControlInvalid,
            "Debian binary control exceeds the size limit",
        )?;
        let text = std::str::from_utf8(value).map_err(|_| binary_control_error())?;
        if !text.ends_with('\n')
            || text.contains('\r')
            || text.contains('\0')
            || text.contains("\n\n")
        {
            return Err(binary_control_error());
        }
        let fields = parse_debian_paragraph(text, DebianParagraphMode::BinaryControl)
            .map_err(|_| binary_control_error())?;
        const EXPECTED_FIELDS: [&str; 10] = [
            "Package",
            "Version",
            "Architecture",
            "Maintainer",
            "Section",
            "Priority",
            "Installed-Size",
            "Depends",
            "Homepage",
            "Description",
        ];
        if fields.len() != EXPECTED_FIELDS.len()
            || fields
                .iter()
                .map(|(name, _)| name.as_str())
                .ne(EXPECTED_FIELDS)
        {
            return Err(binary_control_error());
        }
        let field = |name: &str| {
            fields
                .iter()
                .find(|(candidate, _)| candidate == name)
                .map(|(_, value)| value.as_str())
                .ok_or_else(binary_control_error)
        };
        if field("Maintainer")? != "RadishLex <laugh0608@foxmail.com>"
            || field("Section")? != "utils"
            || field("Priority")? != "optional"
            || field("Homepage")? != "https://github.com/laugh0608/RadishLex"
            || field("Description")?
                != "Local-first Chinese input system for Fcitx5\nRadishLex combines a native Fcitx5 addon with a local management application.\nThis template describes the Debian 13 ARM64 local acceptance carrier only."
        {
            return Err(binary_control_error());
        }
        let package = field("Package")?;
        let version = field("Version")?;
        let architecture = field("Architecture")?;
        if !is_package_name(package)
            || !is_architecture(architecture)
            || parse_debian_version(version).is_err()
        {
            return Err(binary_control_error());
        }
        let installed_size_text = field("Installed-Size")?;
        let installed_size_kib = installed_size_text
            .parse::<u64>()
            .map_err(|_| binary_control_error())?;
        if installed_size_kib == 0 || installed_size_kib.to_string() != installed_size_text {
            return Err(binary_control_error());
        }
        let dependencies = parse_control_dependency_field(field("Depends")?)?;
        Ok(Self {
            package: package.to_owned(),
            version: version.to_owned(),
            architecture: architecture.to_owned(),
            installed_size_kib,
            dependencies,
        })
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    pub const fn installed_size_kib(&self) -> u64 {
        self.installed_size_kib
    }

    pub fn dependencies(&self) -> &[DirectDependency] {
        &self.dependencies
    }

    fn dependency_texts(&self) -> Vec<String> {
        self.dependencies
            .iter()
            .map(DirectDependency::to_control_text)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianVersionOperator {
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    StrictlyLess,
    StrictlyGreater,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebianVersionConstraint {
    operator: DebianVersionOperator,
    version: String,
}

impl DebianVersionConstraint {
    pub const fn operator(&self) -> DebianVersionOperator {
        self.operator
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub(super) fn is_satisfied_by(&self, actual: &str) -> Result<bool, DebianRelationshipError> {
        let ordering = compare_debian_versions(actual, &self.version)?;
        Ok(match self.operator {
            DebianVersionOperator::GreaterThanOrEqual => ordering != Ordering::Less,
            DebianVersionOperator::LessThanOrEqual => ordering != Ordering::Greater,
            DebianVersionOperator::Equal => ordering == Ordering::Equal,
            DebianVersionOperator::StrictlyLess => ordering == Ordering::Less,
            DebianVersionOperator::StrictlyGreater => ordering == Ordering::Greater,
        })
    }

    fn operator_text(&self) -> &'static str {
        match self.operator {
            DebianVersionOperator::GreaterThanOrEqual => ">=",
            DebianVersionOperator::LessThanOrEqual => "<=",
            DebianVersionOperator::Equal => "=",
            DebianVersionOperator::StrictlyLess => "<<",
            DebianVersionOperator::StrictlyGreater => ">>",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectDependency {
    package: String,
    architecture: Option<String>,
    constraint: Option<DebianVersionConstraint>,
}

impl DirectDependency {
    pub fn parse(value: &str) -> Result<Self, DebianRelationshipError> {
        if value.is_empty() || value.trim() != value || value.contains(',') {
            return Err(dependency_declaration_error());
        }
        let (package_identity, constraint) = if let Some(open) = value.find(" (") {
            if !value.ends_with(')') || value[open + 2..value.len() - 1].contains(')') {
                return Err(dependency_declaration_error());
            }
            let expression = &value[open + 2..value.len() - 1];
            let Some((operator, version)) = expression.split_once(' ') else {
                return Err(dependency_declaration_error());
            };
            if version.is_empty() || version.contains(' ') {
                return Err(dependency_declaration_error());
            }
            parse_debian_version(version).map_err(|_| dependency_declaration_error())?;
            let operator = match operator {
                ">=" => DebianVersionOperator::GreaterThanOrEqual,
                "<=" => DebianVersionOperator::LessThanOrEqual,
                "=" => DebianVersionOperator::Equal,
                "<<" => DebianVersionOperator::StrictlyLess,
                ">>" => DebianVersionOperator::StrictlyGreater,
                _ => return Err(dependency_declaration_error()),
            };
            (
                &value[..open],
                Some(DebianVersionConstraint {
                    operator,
                    version: version.to_owned(),
                }),
            )
        } else {
            if value.contains([' ', '(', ')']) {
                return Err(dependency_declaration_error());
            }
            (value, None)
        };
        let mut identity_parts = package_identity.split(':');
        let package = identity_parts.next().unwrap_or_default();
        let architecture = identity_parts.next();
        if identity_parts.next().is_some()
            || !is_package_name(package)
            || architecture.is_some_and(|value| {
                !is_architecture(value) || matches!(value, "any" | "native" | "all")
            })
        {
            return Err(dependency_declaration_error());
        }
        let dependency = Self {
            package: package.to_owned(),
            architecture: architecture.map(str::to_owned),
            constraint,
        };
        if dependency.to_control_text() != value {
            return Err(dependency_declaration_error());
        }
        Ok(dependency)
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn architecture(&self) -> Option<&str> {
        self.architecture.as_deref()
    }

    pub fn constraint(&self) -> Option<&DebianVersionConstraint> {
        self.constraint.as_ref()
    }

    pub fn to_control_text(&self) -> String {
        let mut value = self.package.clone();
        if let Some(architecture) = &self.architecture {
            value.push(':');
            value.push_str(architecture);
        }
        if let Some(constraint) = &self.constraint {
            value.push_str(" (");
            value.push_str(constraint.operator_text());
            value.push(' ');
            value.push_str(&constraint.version);
            value.push(')');
        }
        value
    }
}

pub(super) fn parse_control_dependency_field(
    value: &str,
) -> Result<Vec<DirectDependency>, DebianRelationshipError> {
    if value.is_empty() || value.trim() != value {
        return Err(dependency_declaration_error());
    }
    let texts: Vec<String> = value.split(", ").map(str::to_owned).collect();
    if texts.join(", ") != value {
        return Err(dependency_declaration_error());
    }
    parse_dependency_texts(&texts)
}

fn parse_dependency_texts(
    values: &[String],
) -> Result<Vec<DirectDependency>, DebianRelationshipError> {
    if values.is_empty() {
        return Err(dependency_declaration_error());
    }
    let mut packages = BTreeSet::new();
    let mut dependencies = Vec::with_capacity(values.len());
    for value in values {
        let dependency = DirectDependency::parse(value)?;
        if !packages.insert(dependency.package.clone()) {
            return Err(dependency_declaration_error());
        }
        dependencies.push(dependency);
    }
    Ok(dependencies)
}

fn validate_dependency_profile(
    dependencies: &[DirectDependency],
) -> Result<(), DebianRelationshipError> {
    if dependencies.len() <= FIXED_DEPENDENCIES.len() {
        return Err(dependency_declaration_error());
    }
    let fixed_start = dependencies.len() - FIXED_DEPENDENCIES.len();
    if dependencies[fixed_start..]
        .iter()
        .map(DirectDependency::to_control_text)
        .ne(FIXED_DEPENDENCIES.into_iter().map(str::to_owned))
    {
        return Err(dependency_declaration_error());
    }
    let fixed_packages: BTreeSet<&str> = FIXED_DEPENDENCIES
        .iter()
        .map(|value| value.split_once(' ').map_or(*value, |(name, _)| name))
        .collect();
    let dynamic = &dependencies[..fixed_start];
    if dynamic
        .iter()
        .any(|dependency| fixed_packages.contains(dependency.package()))
        || dynamic
            .windows(2)
            .any(|items| items[0].package >= items[1].package)
    {
        return Err(dependency_declaration_error());
    }
    Ok(())
}

fn binary_control_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::BinaryControlInvalid,
        "Debian binary control differs from format v1",
    )
}

fn dependency_declaration_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::DependencyDeclarationInvalid,
        "Debian direct dependency declaration differs from the fixed profile",
    )
}

pub(super) fn sha256_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
