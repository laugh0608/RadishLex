use std::io::Read;

mod ar;
mod tar;

use ar::{ArchiveMemberIdentity, PackageReader};
use tar::{parse_control_tar, parse_data_tar};

use super::evidence::ArtifactEvidenceV1;
use super::manifest::ProductManifestV1;
use super::{
    DebianRelationshipError, DebianRelationshipErrorCode, PackageContentIdentity,
    MAX_ARTIFACT_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadDirectory {
    pub(super) path: String,
    pub(super) mode: u32,
    pub(super) uid: u32,
    pub(super) gid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadFile {
    pub(super) path: String,
    pub(super) mode: u32,
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) size: u64,
    pub(super) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadInventory {
    pub(super) directories: Vec<PayloadDirectory>,
    pub(super) files: Vec<PayloadFile>,
}

pub(super) struct ParsedPackage {
    pub(super) package_content: PackageContentIdentity,
    pub(super) control: Vec<u8>,
    pub(super) product_manifest: Vec<u8>,
    pub(super) inventory: PayloadInventory,
}

pub(super) fn read_verified_package_stream<R: Read>(
    reader: R,
    evidence: &ArtifactEvidenceV1,
) -> Result<ParsedPackage, DebianRelationshipError> {
    if evidence.package.size == 0 || evidence.package.size > MAX_ARTIFACT_BYTES {
        return Err(content_mismatch());
    }

    let mut package = PackageReader::new(reader);
    package.read_magic()?;
    let mut identities = Vec::with_capacity(3);

    let mut debian_binary = package.begin_member("debian-binary")?;
    if debian_binary.size() != 4 {
        return Err(content_mismatch());
    }
    let mut format_marker = [0_u8; 4];
    ar::read_exact(&mut debian_binary, &mut format_marker)?;
    if &format_marker != b"2.0\n" {
        return Err(content_mismatch());
    }
    let identity = debian_binary.finish()?;
    package.read_member_padding(identity.size)?;
    identities.push(identity);

    let mut control_member = package.begin_member("control.tar")?;
    let control_member_size = control_member.size();
    let control_archive = parse_control_tar(&mut control_member, control_member_size)?;
    let identity = control_member.finish()?;
    package.read_member_padding(identity.size)?;
    identities.push(identity);

    let mut data_member = package.begin_member("data.tar")?;
    let data_member_size = data_member.size();
    let data_archive = parse_data_tar(&mut data_member, data_member_size)?;
    let identity = data_member.finish()?;
    package.read_member_padding(identity.size)?;
    identities.push(identity);

    control_archive.validate_payload_md5sums(&data_archive.md5sums)?;
    if data_archive.installed_size_kib != evidence.control.installed_size_kib {
        return Err(content_mismatch());
    }

    let package_content = package.finish()?;
    validate_evidence_relationship(
        evidence,
        &package_content,
        &identities,
        &control_archive.control_sha256,
        &control_archive.md5sums_sha256,
        &data_archive.product_manifest_sha256,
    )?;

    Ok(ParsedPackage {
        package_content,
        control: control_archive.control,
        product_manifest: data_archive.product_manifest,
        inventory: data_archive.inventory,
    })
}

impl PayloadInventory {
    pub(super) fn validate_manifest(
        &self,
        manifest: &ProductManifestV1,
    ) -> Result<(), DebianRelationshipError> {
        if self.directories.len() != manifest.directories.len()
            || self.files.len() != manifest.files.len()
        {
            return Err(payload_mismatch());
        }
        for (actual, declared) in self.directories.iter().zip(&manifest.directories) {
            if actual.path != declared.path
                || format!("{:04o}", actual.mode) != declared.mode
                || actual.uid != declared.uid
                || actual.gid != declared.gid
            {
                return Err(payload_mismatch());
            }
        }
        for (actual, declared) in self.files.iter().zip(&manifest.files) {
            if actual.path != declared.path
                || format!("{:04o}", actual.mode) != declared.mode
                || actual.uid != declared.uid
                || actual.gid != declared.gid
                || actual.size != declared.size
                || actual.sha256 != declared.sha256
            {
                return Err(payload_mismatch());
            }
        }
        Ok(())
    }
}

fn validate_evidence_relationship(
    evidence: &ArtifactEvidenceV1,
    package: &PackageContentIdentity,
    members: &[ArchiveMemberIdentity],
    control_sha256: &str,
    md5sums_sha256: &str,
    product_manifest_sha256: &str,
) -> Result<(), DebianRelationshipError> {
    if package.size() != evidence.package.size
        || package.sha256() != evidence.package.sha256
        || members.len() != evidence.archive.members.len()
        || members
            .iter()
            .zip(&evidence.archive.members)
            .any(|(actual, declared)| {
                actual.name != declared.name
                    || actual.size != declared.size
                    || actual.sha256 != declared.sha256
            })
        || control_sha256 != evidence.control.sha256
        || md5sums_sha256 != evidence.control.md5sums_sha256
        || product_manifest_sha256 != evidence.product_manifest.sha256
    {
        return Err(content_mismatch());
    }
    Ok(())
}

fn content_mismatch() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::ArtifactContentMismatch,
        "Debian package bytes differ from their sidecar evidence",
    )
}

fn payload_mismatch() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::ArtifactContentMismatch,
        "Debian data archive differs from its product manifest",
    )
}

#[cfg(test)]
mod tests;
