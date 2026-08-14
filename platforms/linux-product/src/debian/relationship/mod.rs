use std::fmt;
use std::io::Read;

use crate::model::{DataContractIdentity, LinuxArtifactIdentity};

mod archive;
mod evidence;
mod manifest;
mod status;
mod version;

pub use evidence::{
    BinaryControlSnapshot, DebianVersionConstraint, DebianVersionOperator, DirectDependency,
};
pub use status::{
    DpkgCurrentState, DpkgDesiredState, DpkgErrorState, DpkgMultiArch, DpkgPackageRecord,
    DpkgStatusSnapshot,
};
pub(crate) use version::validate_radishlex_package_release;
pub use version::{compare_debian_versions, validate_operation_relation};

const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CONTROL_BYTES: usize = 64 * 1024;
const MAX_EVIDENCE_BYTES: usize = 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_DPKG_STATUS_BYTES: usize = 64 * 1024 * 1024;
const MAX_DPKG_RECORDS: usize = 200_000;

const DISTRIBUTION_IDENTITY: &str = "debian-local-deb-v1";
const DEPENDENCY_ANALYSIS_PROFILE: &str = "dpkg-shlibdeps-debian13-arm64-v1";
const PACKAGE_NAME: &str = "radishlex";
const NATIVE_ARCHITECTURE: &str = "arm64";
const PRODUCT_MANIFEST_PATH: &str = "/usr/share/radishlex/product-manifest.json";

const FIXED_DEPENDENCIES: [&str; 4] = [
    "fcitx5 (>= 5.1.9)",
    "librime1t64 (>= 1.13.1)",
    "fonts-dejavu-core",
    "fonts-noto-cjk",
];

const FONT_PACKAGES: [&str; 2] = ["fonts-dejavu-core", "fonts-noto-cjk"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianRelationshipErrorCode {
    ArtifactEvidenceInvalid,
    BinaryControlInvalid,
    ProductManifestInvalid,
    ArtifactContentMismatch,
    PackageIdentityMismatch,
    DependencyDeclarationInvalid,
    DpkgStatusInvalid,
    PackageStateInvalid,
    DependencyUnavailable,
    DependencyVersionUnsatisfied,
    DependencyArchitectureMismatch,
    VersionInvalid,
    VersionRelationInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebianRelationshipError {
    code: DebianRelationshipErrorCode,
    message: &'static str,
}

impl DebianRelationshipError {
    const fn new(code: DebianRelationshipErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }

    pub const fn code(&self) -> DebianRelationshipErrorCode {
        self.code
    }
}

impl fmt::Display for DebianRelationshipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for DebianRelationshipError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageContentIdentity {
    size: u64,
    sha256: String,
}

impl PackageContentIdentity {
    #[cfg(test)]
    fn from_stable_digest(
        size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, DebianRelationshipError> {
        Self::from_stream_digest(size, sha256)
    }

    fn from_stream_digest(
        size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, DebianRelationshipError> {
        let identity = Self {
            size,
            sha256: sha256.into(),
        };
        if identity.size == 0 || identity.size > MAX_ARTIFACT_BYTES || !is_sha256(&identity.sha256)
        {
            return Err(DebianRelationshipError::new(
                DebianRelationshipErrorCode::ArtifactContentMismatch,
                "stable Debian package content identity is invalid",
            ));
        }
        Ok(identity)
    }

    const fn size(&self) -> u64 {
        self.size
    }

    fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Debug, Clone, Copy)]
struct ArtifactRelationshipInput<'a> {
    package_filename: &'a str,
    evidence_filename: &'a str,
    package_content: &'a PackageContentIdentity,
    binary_control_bytes: &'a [u8],
    product_manifest_bytes: &'a [u8],
    evidence_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArtifactRelationship {
    package_filename: String,
    evidence_filename: String,
    package_name: String,
    package_version: String,
    architecture: String,
    package_size: u64,
    package_sha256: String,
    evidence_size: u64,
    evidence_sha256: String,
    product_manifest_sha256: String,
    data_contract: ProductDataContract,
    dependencies: Vec<DirectDependency>,
    installed_file_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDataContract {
    ffi_abi_version: u32,
    userdb_schema_version: u32,
    runtime_layout: String,
    data_layout: String,
    settings_format_version: u32,
    privacy_format_version: u32,
    product_manifest_format_version: u32,
    install_layout_format_version: u32,
    rime_data_manifest_version: u32,
    rime_schema_id: String,
    rime_data_lock_sha256: String,
}

impl ProductDataContract {
    pub const fn ffi_abi_version(&self) -> u32 {
        self.ffi_abi_version
    }

    pub const fn userdb_schema_version(&self) -> u32 {
        self.userdb_schema_version
    }

    pub fn runtime_layout(&self) -> &str {
        &self.runtime_layout
    }

    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }

    pub const fn settings_format_version(&self) -> u32 {
        self.settings_format_version
    }

    pub const fn privacy_format_version(&self) -> u32 {
        self.privacy_format_version
    }

    pub const fn product_manifest_format_version(&self) -> u32 {
        self.product_manifest_format_version
    }

    pub const fn install_layout_format_version(&self) -> u32 {
        self.install_layout_format_version
    }

    pub const fn rime_data_manifest_version(&self) -> u32 {
        self.rime_data_manifest_version
    }

    pub fn rime_schema_id(&self) -> &str {
        &self.rime_schema_id
    }

    pub fn rime_data_lock_sha256(&self) -> &str {
        &self.rime_data_lock_sha256
    }
}

impl VerifiedArtifactRelationship {
    /// Verifies one staged Debian package stream against its canonical sidecar evidence.
    ///
    /// The stream is consumed exactly once and must end immediately after the third ar member.
    pub fn verify_package<R: Read>(
        package_filename: &str,
        evidence_filename: &str,
        package_reader: &mut R,
        evidence_bytes: &[u8],
    ) -> Result<Self, DebianRelationshipError> {
        validate_input_size(
            evidence_bytes,
            MAX_EVIDENCE_BYTES,
            DebianRelationshipErrorCode::ArtifactEvidenceInvalid,
            "Debian artifact evidence exceeds the size limit",
        )?;
        let evidence = evidence::ArtifactEvidenceV1::parse(evidence_bytes)?;
        evidence.validate_profile()?;
        let package = archive::read_verified_package_stream(package_reader, &evidence)?;
        let verified = evidence::verify_artifact_relationship(ArtifactRelationshipInput {
            package_filename,
            evidence_filename,
            package_content: &package.package_content,
            binary_control_bytes: &package.control,
            product_manifest_bytes: &package.product_manifest,
            evidence_bytes,
        })?;
        let manifest = manifest::ProductManifestV1::parse(&package.product_manifest)?;
        package.inventory.validate_manifest(&manifest)?;
        Ok(verified)
    }

    #[cfg(test)]
    fn verify(input: ArtifactRelationshipInput<'_>) -> Result<Self, DebianRelationshipError> {
        evidence::verify_artifact_relationship(input)
    }

    pub fn package_filename(&self) -> &str {
        &self.package_filename
    }

    pub fn evidence_filename(&self) -> &str {
        &self.evidence_filename
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    pub const fn package_size(&self) -> u64 {
        self.package_size
    }

    pub fn package_sha256(&self) -> &str {
        &self.package_sha256
    }

    pub const fn evidence_size(&self) -> u64 {
        self.evidence_size
    }

    pub fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }

    pub fn product_manifest_sha256(&self) -> &str {
        &self.product_manifest_sha256
    }

    pub fn data_contract(&self) -> &ProductDataContract {
        &self.data_contract
    }

    pub fn dependencies(&self) -> &[DirectDependency] {
        &self.dependencies
    }

    pub fn installed_file_paths(&self) -> &[String] {
        &self.installed_file_paths
    }

    pub fn to_linux_artifact_identity(
        &self,
    ) -> Result<LinuxArtifactIdentity, DebianRelationshipError> {
        let data_contract = DataContractIdentity::new(
            self.data_contract.ffi_abi_version,
            self.data_contract.userdb_schema_version,
            &self.data_contract.runtime_layout,
            &self.data_contract.data_layout,
            self.data_contract.settings_format_version,
            self.data_contract.privacy_format_version,
            &self.data_contract.rime_schema_id,
            &self.data_contract.rime_data_lock_sha256,
        )
        .map_err(|_| linux_artifact_identity_error())?;
        LinuxArtifactIdentity::new(
            &self.package_version,
            self.package_size,
            self.evidence_size,
            &self.package_sha256,
            &self.evidence_sha256,
            &self.product_manifest_sha256,
            data_contract,
        )
        .map_err(|_| linux_artifact_identity_error())
    }

    pub fn matches_linux_artifact_identity(
        &self,
        identity: &LinuxArtifactIdentity,
    ) -> Result<bool, DebianRelationshipError> {
        Ok(self.to_linux_artifact_identity()? == *identity)
    }
}

fn linux_artifact_identity_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::PackageIdentityMismatch,
        "verified Debian relationship cannot form the Linux artifact identity",
    )
}

fn validate_input_size(
    value: &[u8],
    maximum: usize,
    code: DebianRelationshipErrorCode,
    message: &'static str,
) -> Result<(), DebianRelationshipError> {
    if value.is_empty() || value.len() > maximum {
        return Err(DebianRelationshipError::new(code, message));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebianParagraphMode {
    BinaryControl,
    DpkgStatus,
}

fn parse_debian_paragraph(
    text: &str,
    mode: DebianParagraphMode,
) -> Result<Vec<(String, String)>, ()> {
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for line in text.lines() {
        if let Some(continuation) = line.strip_prefix(' ') {
            let Some((name, value)) = fields.last_mut() else {
                return Err(());
            };
            if continuation.is_empty()
                || (mode == DebianParagraphMode::BinaryControl && name != "Description")
            {
                return Err(());
            }
            if mode == DebianParagraphMode::BinaryControl {
                value.push('\n');
            } else if !value.is_empty() {
                value.push(' ');
            }
            value.push_str(continuation);
            continue;
        }
        if line.starts_with('\t') {
            return Err(());
        }
        let (name, value) = if let Some((name, value)) = line.split_once(": ") {
            if value.is_empty() {
                return Err(());
            }
            (name, value)
        } else if mode == DebianParagraphMode::DpkgStatus {
            let Some(name) = line.strip_suffix(':') else {
                return Err(());
            };
            (name, "")
        } else {
            return Err(());
        };
        if !is_field_name(name) || !names.insert(name.to_owned()) {
            return Err(());
        }
        fields.push((name.to_owned(), value.to_owned()));
    }
    if fields.is_empty() {
        return Err(());
    }
    Ok(fields)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_package_name(value: &str) -> bool {
    (2..=128).contains(&value.len())
        && (value.as_bytes()[0].is_ascii_lowercase() || value.as_bytes()[0].is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'+' | b'.' | b'-')
        })
}

fn is_architecture(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_field_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn is_canonical_positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_digit()
        && value.as_bytes()[0] != b'0'
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
use evidence::{
    sha256_bytes, ArchiveEvidence, ArchiveMemberEvidence, ArtifactEvidenceV1, ControlEvidence,
    DependencyAnalysisEvidence, PackageEvidence, ProductManifestEvidence,
};

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
pub(crate) use archive::tests::fixture::{
    ArchiveFixture, EVIDENCE_FILENAME as ARCHIVE_EVIDENCE_FILENAME,
    PACKAGE_FILENAME as ARCHIVE_PACKAGE_FILENAME,
};
