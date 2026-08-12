use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::debian::validate_radishlex_package_release;
use crate::model::LinuxArtifactIdentity;

use super::super::read_only_state::same_file_identity;
use super::super::types::{
    LinuxStartupComponent, LinuxStartupPaths, LinuxStartupPortError, LinuxStartupPortErrorCode,
};
use super::{port_io_error, read_owned_regular_file};

const MAX_PRODUCT_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PRODUCT_COMPONENT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProductManifest {
    directories: Vec<ManifestDirectoryRecord>,
    ffi_equivalence: ManifestFfiEquivalence,
    files: Vec<ManifestFileRecord>,
    format_version: u32,
    inventory_scope: String,
    layout_id: String,
    manifest_path: String,
    owner_model: String,
    product: ManifestProductIdentity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestDirectoryRecord {
    gid: u32,
    mode: String,
    path: String,
    uid: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestFfiEquivalence {
    paths: Vec<String>,
    relationship: String,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestFileRecord {
    component_id: String,
    gid: u32,
    mode: String,
    path: String,
    sha256: String,
    size: u64,
    uid: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestProductIdentity {
    build_number: String,
    data_layout: String,
    data_removal: String,
    debian_architecture: String,
    debian_codename: String,
    debian_release: String,
    default_removal: String,
    distribution_identity: String,
    ffi_abi_version: u32,
    hard_dependencies: Vec<String>,
    install_layout_format_version: u32,
    manager_application_id: String,
    multiarch_tuple: String,
    package_name: String,
    package_version: String,
    privacy_format_version: u32,
    product_id: String,
    product_manifest_format_version: u32,
    product_version: String,
    rime_data_lock_sha256: String,
    rime_data_manifest_version: u32,
    rime_schema_id: String,
    runtime_layout: String,
    settings_format_version: u32,
    userdb_schema_version: u32,
}

pub(super) fn validate_system_component(
    paths: &LinuxStartupPaths,
    component_path: &Path,
    runtime_ffi_abi_version: u32,
    component: LinuxStartupComponent,
    artifact: &LinuxArtifactIdentity,
) -> Result<(), LinuxStartupPortError> {
    let expected_component_path = match component {
        LinuxStartupComponent::Manager => paths.manager_component_path.as_path(),
        LinuxStartupComponent::FcitxAddon => paths.fcitx_component_path.as_path(),
    };
    if component_path != expected_component_path {
        return Err(component_identity_error());
    }
    let manifest = read_and_validate_manifest(paths, runtime_ffi_abi_version, artifact)?;
    validate_ffi_equivalence(paths, &manifest)?;
    match component {
        LinuxStartupComponent::Manager => validate_manager_inventory(paths, &manifest),
        LinuxStartupComponent::FcitxAddon => validate_fcitx_inventory(paths, &manifest),
    }
}

pub(crate) fn validate_system_product(
    paths: &LinuxStartupPaths,
    runtime_ffi_abi_version: u32,
    artifact: &LinuxArtifactIdentity,
) -> Result<(), LinuxStartupPortError> {
    let manifest = read_and_validate_manifest(paths, runtime_ffi_abi_version, artifact)?;
    validate_ffi_equivalence(paths, &manifest)?;
    validate_manager_inventory(paths, &manifest)?;
    validate_fcitx_inventory(paths, &manifest)?;
    for record in &manifest.directories {
        validate_manifest_directory_record(record, paths)?;
    }
    for record in &manifest.files {
        let mode = match record.mode.as_str() {
            "0644" => 0o644,
            "0755" => 0o755,
            _ => return Err(component_identity_error()),
        };
        validate_manifest_file_record(record, Path::new(&record.path), mode, paths)?;
    }
    Ok(())
}

fn read_and_validate_manifest(
    paths: &LinuxStartupPaths,
    runtime_ffi_abi_version: u32,
    artifact: &LinuxArtifactIdentity,
) -> Result<ProductManifest, LinuxStartupPortError> {
    let manifest_bytes = read_owned_regular_file(
        &paths.product_manifest_path,
        paths.expected_owner_id,
        paths.expected_group_id,
        &[0o644],
        1,
        MAX_PRODUCT_MANIFEST_BYTES,
        LinuxStartupPortErrorCode::PackageIdentityChanged,
    )?
    .ok_or_else(package_identity_error)?;
    if sha256_bytes(&manifest_bytes) != artifact.product_manifest_sha256() {
        return Err(package_identity_error());
    }
    let manifest: ProductManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| package_identity_error())?;
    let mut canonical = serde_json::to_string_pretty(&manifest)
        .map_err(|_| package_identity_error())?
        .into_bytes();
    canonical.push(b'\n');
    if canonical != manifest_bytes {
        return Err(package_identity_error());
    }
    validate_manifest_identity(paths, &manifest, artifact, runtime_ffi_abi_version)?;
    validate_manifest_record_order(&manifest)?;
    Ok(manifest)
}

fn validate_manifest_identity(
    paths: &LinuxStartupPaths,
    manifest: &ProductManifest,
    artifact: &LinuxArtifactIdentity,
    runtime_ffi_abi_version: u32,
) -> Result<(), LinuxStartupPortError> {
    if manifest.format_version != 1
        || manifest.inventory_scope != "rootfs-payload-excluding-manifest-self-v1"
        || manifest.layout_id != artifact.data_contract().runtime_layout()
        || Some(manifest.manifest_path.as_str()) != paths.product_manifest_path.to_str()
        || manifest.owner_model != "package-install-root-v1"
    {
        return Err(package_identity_error());
    }
    let product = &manifest.product;
    let data_contract = artifact.data_contract();
    let normalized_strings = [
        product.build_number.as_str(),
        product.data_layout.as_str(),
        product.data_removal.as_str(),
        product.debian_architecture.as_str(),
        product.debian_codename.as_str(),
        product.debian_release.as_str(),
        product.default_removal.as_str(),
        product.distribution_identity.as_str(),
        product.manager_application_id.as_str(),
        product.multiarch_tuple.as_str(),
        product.package_name.as_str(),
        product.package_version.as_str(),
        product.product_id.as_str(),
        product.product_version.as_str(),
        product.rime_data_lock_sha256.as_str(),
        product.rime_schema_id.as_str(),
        product.runtime_layout.as_str(),
    ];
    let expected_dependencies = [
        "${shlibs:Depends}",
        "${misc:Depends}",
        "fcitx5 (>= 5.1.9)",
        "librime1t64 (>= 1.13.1)",
        "fonts-dejavu-core",
        "fonts-noto-cjk",
    ];
    if normalized_strings
        .iter()
        .any(|value| value.is_empty() || value.trim() != *value)
        || !is_positive_decimal(&product.build_number)
        || product.product_id != "radishlex-linux"
        || product.distribution_identity != "debian-local-deb-v1"
        || product.debian_release != "13"
        || product.debian_codename != "trixie"
        || product.default_removal != "programs-only"
        || product.data_removal != "separate-authorized-flow"
        || product.manager_application_id != "dev.radishlex.radishlexManager"
        || product.multiarch_tuple != "aarch64-linux-gnu"
        || product.package_name != artifact.package_name()
        || product.package_version != artifact.package_version()
        || validate_radishlex_package_release(
            &product.package_version,
            &product.product_version,
            &product.build_number,
        )
        .is_err()
        || product.debian_architecture != artifact.architecture()
        || product.runtime_layout != data_contract.runtime_layout()
        || product.data_layout != data_contract.data_layout()
        || product.rime_schema_id != data_contract.rime_schema_id()
        || product.rime_data_lock_sha256 != data_contract.rime_data_lock_sha256()
        || product.userdb_schema_version != data_contract.userdb_schema_version()
        || product.settings_format_version != data_contract.settings_format_version()
        || product.privacy_format_version != data_contract.privacy_format_version()
        || product.product_manifest_format_version != 1
        || product.product_manifest_format_version != manifest.format_version
        || product.install_layout_format_version != 1
        || product.rime_data_manifest_version != 1
        || product.hard_dependencies.len() != expected_dependencies.len()
        || product
            .hard_dependencies
            .iter()
            .map(String::as_str)
            .ne(expected_dependencies)
    {
        return Err(package_identity_error());
    }
    if product.ffi_abi_version != runtime_ffi_abi_version
        || runtime_ffi_abi_version != data_contract.ffi_abi_version()
    {
        return Err(component_identity_error());
    }
    Ok(())
}

fn is_positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.bytes().any(|byte| byte != b'0')
}

fn validate_manifest_record_order(manifest: &ProductManifest) -> Result<(), LinuxStartupPortError> {
    if manifest
        .directories
        .windows(2)
        .any(|records| records[0].path >= records[1].path)
        || manifest
            .files
            .windows(2)
            .any(|records| records[0].path >= records[1].path)
        || manifest
            .directories
            .iter()
            .any(|record| !is_normalized_absolute_path(&record.path))
        || manifest
            .files
            .iter()
            .any(|record| !is_normalized_absolute_path(&record.path))
    {
        return Err(package_identity_error());
    }
    let directories: BTreeSet<_> = manifest
        .directories
        .iter()
        .map(|record| record.path.as_str())
        .collect();
    if manifest
        .files
        .iter()
        .any(|record| directories.contains(record.path.as_str()))
    {
        return Err(package_identity_error());
    }
    Ok(())
}

fn is_normalized_absolute_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        && value != "/"
        && !value.ends_with('/')
        && !value.contains("//")
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
}

fn validate_ffi_equivalence(
    paths: &LinuxStartupPaths,
    manifest: &ProductManifest,
) -> Result<(), LinuxStartupPortError> {
    let expected_paths = [
        path_text(&paths.manager_ffi_path)?,
        path_text(&paths.fcitx_ffi_path)?,
    ];
    if manifest.ffi_equivalence.relationship != "same-content-distinct-inode"
        || manifest.ffi_equivalence.paths != expected_paths
    {
        return Err(component_identity_error());
    }
    let manager_record = exact_file_record(manifest, &paths.manager_ffi_path, "manager-bundle")?;
    let fcitx_record = exact_file_record(manifest, &paths.fcitx_ffi_path, "fcitx-ffi")?;
    if manager_record.sha256 != manifest.ffi_equivalence.sha256
        || fcitx_record.sha256 != manifest.ffi_equivalence.sha256
    {
        return Err(component_identity_error());
    }
    let manager_metadata =
        validate_manifest_file_record(manager_record, &paths.manager_ffi_path, 0o644, paths)?;
    let fcitx_metadata =
        validate_manifest_file_record(fcitx_record, &paths.fcitx_ffi_path, 0o644, paths)?;
    if manager_metadata.dev() == fcitx_metadata.dev()
        && manager_metadata.ino() == fcitx_metadata.ino()
    {
        return Err(component_identity_error());
    }
    Ok(())
}

fn validate_manager_inventory(
    paths: &LinuxStartupPaths,
    manifest: &ProductManifest,
) -> Result<(), LinuxStartupPortError> {
    let manager_root = paths
        .manager_component_path
        .parent()
        .ok_or_else(component_identity_error)?;
    let scoped_files: Vec<_> = manifest
        .files
        .iter()
        .filter(|record| Path::new(&record.path).starts_with(manager_root))
        .collect();
    if scoped_files.is_empty()
        || manifest.files.iter().any(|record| {
            record.component_id == "manager-bundle"
                && !Path::new(&record.path).starts_with(manager_root)
        })
        || scoped_files
            .iter()
            .any(|record| record.component_id != "manager-bundle")
    {
        return Err(component_identity_error());
    }
    exact_file_record(manifest, &paths.manager_component_path, "manager-bundle")?;
    let scoped_directories: Vec<_> = manifest
        .directories
        .iter()
        .filter(|record| Path::new(&record.path).starts_with(manager_root))
        .collect();
    exact_directory_record(manifest, manager_root)?;

    let before = collect_tree_inventory(manager_root)?;
    require_exact_tree_inventory(manager_root, &before, &scoped_directories, &scoped_files)?;
    for record in scoped_directories {
        validate_manifest_directory_record(record, paths)?;
    }
    for record in scoped_files {
        let expected_mode = if Path::new(&record.path) == paths.manager_component_path {
            0o755
        } else {
            0o644
        };
        validate_manifest_file_record(record, Path::new(&record.path), expected_mode, paths)?;
    }
    let after = collect_tree_inventory(manager_root)?;
    if before != after {
        return Err(component_identity_error());
    }
    Ok(())
}

fn validate_fcitx_inventory(
    paths: &LinuxStartupPaths,
    manifest: &ProductManifest,
) -> Result<(), LinuxStartupPortError> {
    let fixed_files = [
        (paths.fcitx_component_path.as_path(), "fcitx-addon"),
        (paths.fcitx_ffi_path.as_path(), "fcitx-ffi"),
        (
            paths.fcitx_addon_metadata_path.as_path(),
            "fcitx-addon-metadata",
        ),
        (
            paths.fcitx_input_method_metadata_path.as_path(),
            "fcitx-input-method-metadata",
        ),
    ];
    for (expected_path, component_id) in fixed_files {
        let record = exact_file_record(manifest, expected_path, component_id)?;
        validate_manifest_file_record(record, expected_path, 0o644, paths)?;
        validate_manifest_directory_record(
            exact_directory_record(
                manifest,
                expected_path
                    .parent()
                    .ok_or_else(component_identity_error)?,
            )?,
            paths,
        )?;
    }
    if manifest
        .files
        .iter()
        .any(|record| match record.component_id.as_str() {
            "fcitx-addon" => Path::new(&record.path) != paths.fcitx_component_path,
            "fcitx-ffi" => Path::new(&record.path) != paths.fcitx_ffi_path,
            "fcitx-addon-metadata" => Path::new(&record.path) != paths.fcitx_addon_metadata_path,
            "fcitx-input-method-metadata" => {
                Path::new(&record.path) != paths.fcitx_input_method_metadata_path
            }
            "rime-data" => !Path::new(&record.path).starts_with(&paths.rime_data_root),
            _ => Path::new(&record.path).starts_with(&paths.rime_data_root),
        })
    {
        return Err(component_identity_error());
    }

    let rime_files: Vec<_> = manifest
        .files
        .iter()
        .filter(|record| record.component_id == "rime-data")
        .collect();
    let rime_directories: Vec<_> = manifest
        .directories
        .iter()
        .filter(|record| Path::new(&record.path).starts_with(&paths.rime_data_root))
        .collect();
    if rime_files.is_empty() {
        return Err(component_identity_error());
    }
    exact_directory_record(manifest, &paths.rime_data_root)?;
    let before = collect_tree_inventory(&paths.rime_data_root)?;
    require_exact_tree_inventory(
        &paths.rime_data_root,
        &before,
        &rime_directories,
        &rime_files,
    )?;
    for record in rime_directories {
        validate_manifest_directory_record(record, paths)?;
    }
    for record in rime_files {
        validate_manifest_file_record(record, Path::new(&record.path), 0o644, paths)?;
    }
    let after = collect_tree_inventory(&paths.rime_data_root)?;
    if before != after {
        return Err(component_identity_error());
    }
    Ok(())
}

fn exact_file_record<'a>(
    manifest: &'a ProductManifest,
    path: &Path,
    component_id: &str,
) -> Result<&'a ManifestFileRecord, LinuxStartupPortError> {
    let path = path_text(path)?;
    let record = manifest
        .files
        .iter()
        .filter(|record| record.path == path)
        .exactly_one()
        .map_err(|()| component_identity_error())?;
    if record.component_id != component_id {
        return Err(component_identity_error());
    }
    Ok(record)
}

fn exact_directory_record<'a>(
    manifest: &'a ProductManifest,
    path: &Path,
) -> Result<&'a ManifestDirectoryRecord, LinuxStartupPortError> {
    let path = path_text(path)?;
    manifest
        .directories
        .iter()
        .filter(|record| record.path == path)
        .exactly_one()
        .map_err(|()| component_identity_error())
}

fn path_text(path: &Path) -> Result<&str, LinuxStartupPortError> {
    path.to_str().ok_or_else(component_identity_error)
}

fn validate_manifest_file_record(
    record: &ManifestFileRecord,
    path: &Path,
    mode: u32,
    paths: &LinuxStartupPaths,
) -> Result<fs::Metadata, LinuxStartupPortError> {
    if record.path != path_text(path)?
        || record.uid != paths.expected_owner_id
        || record.gid != paths.expected_group_id
        || record.mode != format!("{mode:04o}")
        || record.size > MAX_PRODUCT_COMPONENT_BYTES
    {
        return Err(component_identity_error());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| component_identity_error())?;
    if fs::canonicalize(path).map_err(|_| component_identity_error())? != path
        || !metadata.file_type().is_file()
        || metadata.uid() != paths.expected_owner_id
        || metadata.gid() != paths.expected_group_id
        || metadata.mode() & 0o7777 != mode
        || metadata.nlink() != 1
        || metadata.len() > MAX_PRODUCT_COMPONENT_BYTES
        || record.size != metadata.len()
    {
        return Err(component_identity_error());
    }
    let actual_hash = sha256_stable_file(path, &metadata)?;
    if record.sha256 != actual_hash {
        return Err(component_identity_error());
    }
    Ok(metadata)
}

fn validate_manifest_directory_record(
    record: &ManifestDirectoryRecord,
    paths: &LinuxStartupPaths,
) -> Result<(), LinuxStartupPortError> {
    let path = Path::new(&record.path);
    let metadata = fs::symlink_metadata(path).map_err(|_| component_identity_error())?;
    if record.uid != paths.expected_owner_id
        || record.gid != paths.expected_group_id
        || record.mode != "0755"
        || fs::canonicalize(path).map_err(|_| component_identity_error())? != path
        || !metadata.file_type().is_dir()
        || metadata.uid() != paths.expected_owner_id
        || metadata.gid() != paths.expected_group_id
        || metadata.mode() & 0o7777 != 0o755
    {
        return Err(component_identity_error());
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct TreeInventory {
    directories: BTreeSet<PathBuf>,
    files: BTreeSet<PathBuf>,
}

fn collect_tree_inventory(root: &Path) -> Result<TreeInventory, LinuxStartupPortError> {
    const MAX_TREE_ENTRIES: usize = 100_000;

    let mut directories = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory).map_err(|_| component_identity_error())?;
        if !metadata.file_type().is_dir() || !directories.insert(directory.clone()) {
            return Err(component_identity_error());
        }
        for entry in fs::read_dir(&directory).map_err(port_io_error)? {
            let path = entry.map_err(port_io_error)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|_| component_identity_error())?;
            if metadata.file_type().is_dir() {
                pending.push(path);
            } else if metadata.file_type().is_file() {
                files.insert(path);
            } else {
                return Err(component_identity_error());
            }
            if directories.len() + files.len() + pending.len() > MAX_TREE_ENTRIES {
                return Err(component_identity_error());
            }
        }
    }
    Ok(TreeInventory { directories, files })
}

fn require_exact_tree_inventory(
    root: &Path,
    actual: &TreeInventory,
    directories: &[&ManifestDirectoryRecord],
    files: &[&ManifestFileRecord],
) -> Result<(), LinuxStartupPortError> {
    let expected_directories: BTreeSet<_> = directories
        .iter()
        .map(|record| PathBuf::from(&record.path))
        .collect();
    let expected_files: BTreeSet<_> = files
        .iter()
        .map(|record| PathBuf::from(&record.path))
        .collect();
    if !expected_directories.contains(root)
        || actual.directories != expected_directories
        || actual.files != expected_files
    {
        return Err(component_identity_error());
    }
    Ok(())
}

trait ExactlyOne: Iterator + Sized {
    fn exactly_one(mut self) -> Result<Self::Item, ()> {
        let first = self.next().ok_or(())?;
        if self.next().is_some() {
            return Err(());
        }
        Ok(first)
    }
}

impl<I: Iterator> ExactlyOne for I {}

fn sha256_stable_file(path: &Path, before: &fs::Metadata) -> Result<String, LinuxStartupPortError> {
    let mut file = File::open(path).map_err(port_io_error)?;
    let opened = file.metadata().map_err(port_io_error)?;
    if !same_file_identity(before, &opened) {
        return Err(component_identity_error());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(port_io_error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let after = fs::symlink_metadata(path).map_err(port_io_error)?;
    if !same_file_identity(before, &after) {
        return Err(component_identity_error());
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn sha256_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn package_identity_error() -> LinuxStartupPortError {
    LinuxStartupPortError::new(
        LinuxStartupPortErrorCode::PackageIdentityChanged,
        "Linux product package identity changed",
    )
}

fn component_identity_error() -> LinuxStartupPortError {
    LinuxStartupPortError::new(
        LinuxStartupPortErrorCode::ComponentIdentityChanged,
        "Linux product component identity changed",
    )
}
