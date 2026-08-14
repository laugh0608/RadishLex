use std::collections::BTreeSet;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use super::evidence::BinaryControlSnapshot;
use super::version::validate_radishlex_package_release;
use super::{
    is_sha256, DebianRelationshipError, DebianRelationshipErrorCode, ProductDataContract,
    DISTRIBUTION_IDENTITY, PRODUCT_MANIFEST_PATH,
};

const MAX_PRODUCT_COMPONENT_BYTES: u64 = 512 * 1024 * 1024;

const MANAGER_ROOT: &str = "/usr/lib/aarch64-linux-gnu/radishlex/manager";
const MANAGER_EXECUTABLE: &str = "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager";
const MANAGER_FFI: &str =
    "/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/libradishlex_ime_ffi.so";
const FCITX_ADDON: &str = "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so";
const FCITX_FFI: &str = "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so";
const RIME_DATA_ROOT: &str = "/usr/share/radishlex/rime";
const FCITX_ADDON_METADATA: &str = "/usr/share/fcitx5/addon/radishlex.conf";
const FCITX_INPUT_METHOD_METADATA: &str = "/usr/share/fcitx5/inputmethod/radishlex.conf";
const MANAGER_DESKTOP_ENTRY: &str =
    "/usr/share/applications/dev.radishlex.radishlexManager.desktop";
const MANAGER_ICON: &str =
    "/usr/share/icons/hicolor/scalable/apps/dev.radishlex.radishlexManager.svg";
const INPUT_METHOD_ICON: &str = "/usr/share/icons/hicolor/scalable/apps/fcitx-radishlex.svg";
const PRODUCT_LICENSE: &str = "/usr/share/doc/radishlex/copyright";
const MATERIAL_ICONS_FONT: &str = "/usr/lib/aarch64-linux-gnu/radishlex/manager/data/flutter_assets/fonts/MaterialIcons-Regular.otf";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProductManifestV1 {
    pub(super) directories: Vec<ManifestDirectoryRecord>,
    ffi_equivalence: ManifestFfiEquivalence,
    pub(super) files: Vec<ManifestFileRecord>,
    format_version: u32,
    inventory_scope: String,
    layout_id: String,
    manifest_path: String,
    owner_model: String,
    product: ProductManifestIdentity,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestDirectoryRecord {
    pub(super) gid: u32,
    pub(super) mode: String,
    pub(super) path: String,
    pub(super) uid: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestFfiEquivalence {
    paths: Vec<String>,
    relationship: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestFileRecord {
    pub(super) component_id: String,
    pub(super) gid: u32,
    pub(super) mode: String,
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) size: u64,
    pub(super) uid: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProductManifestIdentity {
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

impl ProductManifestV1 {
    pub(super) fn parse(value: &[u8]) -> Result<Self, DebianRelationshipError> {
        let manifest: Self = serde_json::from_slice(value).map_err(|_| product_manifest_error())?;
        let mut canonical =
            serde_json::to_vec_pretty(&manifest).map_err(|_| product_manifest_error())?;
        canonical.push(b'\n');
        if canonical != value {
            return Err(product_manifest_error());
        }
        Ok(manifest)
    }

    pub(super) fn validate_profile(
        &self,
        control: &BinaryControlSnapshot,
    ) -> Result<ProductDataContract, DebianRelationshipError> {
        self.validate_inventory()?;
        let product = &self.product;
        if self.format_version != 1
            || self.inventory_scope != "rootfs-payload-excluding-manifest-self-v1"
            || self.layout_id != "debian-system-v1"
            || self.manifest_path != PRODUCT_MANIFEST_PATH
            || self.owner_model != "package-install-root-v1"
            || product.product_manifest_format_version != self.format_version
            || product.install_layout_format_version != 1
            || product.product_id != "radishlex-linux"
            || product.distribution_identity != DISTRIBUTION_IDENTITY
            || product.package_name != control.package()
            || product.package_version != control.version()
            || product.debian_architecture != control.architecture()
            || product.debian_release != "13"
            || product.debian_codename != "trixie"
            || product.multiarch_tuple != "aarch64-linux-gnu"
            || product.runtime_layout != self.layout_id
            || product.data_layout != "xdg-v1"
            || product.settings_format_version != 1
            || product.privacy_format_version != 1
            || product.ffi_abi_version != 9
            || product.userdb_schema_version != 9
            || product.rime_data_manifest_version != 1
            || !is_sha256(&product.rime_data_lock_sha256)
            || product.rime_schema_id != "radishlex_pinyin"
            || product.manager_application_id != "dev.radishlex.radishlexManager"
            || product.default_removal != "programs-only"
            || product.data_removal != "separate-authorized-flow"
        {
            return Err(product_manifest_error());
        }
        validate_radishlex_package_release(
            &product.package_version,
            &product.product_version,
            &product.build_number,
        )
        .map_err(|_| product_manifest_error())?;
        let expected_hard_dependencies = [
            "${shlibs:Depends}".to_owned(),
            "${misc:Depends}".to_owned(),
            "fcitx5 (>= 5.1.9)".to_owned(),
            "librime1t64 (>= 1.13.1)".to_owned(),
            "fonts-dejavu-core".to_owned(),
            "fonts-noto-cjk".to_owned(),
        ];
        let control_dependencies = control
            .dependencies()
            .iter()
            .map(|dependency| dependency.to_control_text())
            .collect::<Vec<_>>();
        if product.hard_dependencies.as_slice() != expected_hard_dependencies.as_slice()
            || control_dependencies.len() < 4
            || product.hard_dependencies[2..]
                != control_dependencies[control_dependencies.len() - 4..]
        {
            return Err(product_manifest_error());
        }
        Ok(ProductDataContract {
            ffi_abi_version: product.ffi_abi_version,
            userdb_schema_version: product.userdb_schema_version,
            runtime_layout: product.runtime_layout.clone(),
            data_layout: product.data_layout.clone(),
            settings_format_version: product.settings_format_version,
            privacy_format_version: product.privacy_format_version,
            product_manifest_format_version: product.product_manifest_format_version,
            install_layout_format_version: product.install_layout_format_version,
            rime_data_manifest_version: product.rime_data_manifest_version,
            rime_schema_id: product.rime_schema_id.clone(),
            rime_data_lock_sha256: product.rime_data_lock_sha256.clone(),
        })
    }

    pub(super) fn installed_file_paths(&self) -> Vec<String> {
        let mut paths = self
            .files
            .iter()
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();
        paths.push(self.manifest_path.clone());
        paths.sort();
        paths
    }

    fn validate_inventory(&self) -> Result<(), DebianRelationshipError> {
        if self.directories.is_empty()
            || self.files.is_empty()
            || self
                .directories
                .windows(2)
                .any(|records| records[0].path >= records[1].path)
            || self
                .files
                .windows(2)
                .any(|records| records[0].path >= records[1].path)
        {
            return Err(product_manifest_error());
        }

        let mut directories = BTreeSet::new();
        for record in &self.directories {
            if record.uid != 0
                || record.gid != 0
                || record.mode != "0755"
                || !is_normalized_absolute_path(&record.path)
                || !directories.insert(record.path.as_str())
            {
                return Err(product_manifest_error());
            }
        }

        let mut files = BTreeSet::new();
        for record in &self.files {
            let expected_mode = if record.path == MANAGER_EXECUTABLE {
                "0755"
            } else {
                "0644"
            };
            if record.uid != 0
                || record.gid != 0
                || record.mode != expected_mode
                || record.size == 0
                || record.size > MAX_PRODUCT_COMPONENT_BYTES
                || !is_sha256(&record.sha256)
                || !is_normalized_absolute_path(&record.path)
                || !known_component_path(&record.component_id, &record.path)
                || directories.contains(record.path.as_str())
                || !files.insert(record.path.as_str())
            {
                return Err(product_manifest_error());
            }
            let parent = Path::new(&record.path)
                .parent()
                .and_then(Path::to_str)
                .ok_or_else(product_manifest_error)?;
            if !directories.contains(parent) {
                return Err(product_manifest_error());
            }
        }
        if !is_normalized_absolute_path(&self.manifest_path)
            || directories != required_directories(&self.files, &self.manifest_path)?
        {
            return Err(product_manifest_error());
        }
        self.validate_required_components()?;
        self.validate_ffi_equivalence()
    }

    fn validate_required_components(&self) -> Result<(), DebianRelationshipError> {
        for (path, component_id) in [
            (MANAGER_EXECUTABLE, "manager-bundle"),
            (MANAGER_FFI, "manager-bundle"),
            (MATERIAL_ICONS_FONT, "manager-bundle"),
            (FCITX_ADDON, "fcitx-addon"),
            (FCITX_FFI, "fcitx-ffi"),
            (FCITX_ADDON_METADATA, "fcitx-addon-metadata"),
            (FCITX_INPUT_METHOD_METADATA, "fcitx-input-method-metadata"),
            (MANAGER_DESKTOP_ENTRY, "manager-desktop-entry"),
            (MANAGER_ICON, "manager-icon"),
            (INPUT_METHOD_ICON, "input-method-icon"),
            (PRODUCT_LICENSE, "product-license"),
        ] {
            let record = self
                .files
                .iter()
                .find(|record| record.path == path)
                .ok_or_else(product_manifest_error)?;
            if record.component_id != component_id {
                return Err(product_manifest_error());
            }
        }
        if !self
            .files
            .iter()
            .any(|record| record.component_id == "rime-data")
        {
            return Err(product_manifest_error());
        }
        let font_records = self
            .files
            .iter()
            .filter(|record| is_font_path(&record.path));
        if font_records
            .map(|record| (record.path.as_str(), record.component_id.as_str()))
            .ne([(MATERIAL_ICONS_FONT, "manager-bundle")])
        {
            return Err(product_manifest_error());
        }
        Ok(())
    }

    fn validate_ffi_equivalence(&self) -> Result<(), DebianRelationshipError> {
        if self.ffi_equivalence.relationship != "same-content-distinct-inode"
            || self.ffi_equivalence.paths.as_slice() != [MANAGER_FFI, FCITX_FFI]
            || !is_sha256(&self.ffi_equivalence.sha256)
        {
            return Err(product_manifest_error());
        }
        let manager = self
            .files
            .iter()
            .find(|record| record.path == MANAGER_FFI)
            .ok_or_else(product_manifest_error)?;
        let fcitx = self
            .files
            .iter()
            .find(|record| record.path == FCITX_FFI)
            .ok_or_else(product_manifest_error)?;
        if manager.component_id != "manager-bundle"
            || fcitx.component_id != "fcitx-ffi"
            || manager.sha256 != self.ffi_equivalence.sha256
            || fcitx.sha256 != self.ffi_equivalence.sha256
            || manager.size != fcitx.size
        {
            return Err(product_manifest_error());
        }
        Ok(())
    }
}

fn known_component_path(component_id: &str, path: &str) -> bool {
    match component_id {
        "manager-bundle" => is_strict_descendant(path, MANAGER_ROOT),
        "fcitx-addon" => path == FCITX_ADDON,
        "fcitx-ffi" => path == FCITX_FFI,
        "rime-data" => is_strict_descendant(path, RIME_DATA_ROOT),
        "fcitx-addon-metadata" => path == FCITX_ADDON_METADATA,
        "fcitx-input-method-metadata" => path == FCITX_INPUT_METHOD_METADATA,
        "manager-desktop-entry" => path == MANAGER_DESKTOP_ENTRY,
        "manager-icon" => path == MANAGER_ICON,
        "input-method-icon" => path == INPUT_METHOD_ICON,
        "product-license" => path == PRODUCT_LICENSE,
        _ => false,
    }
}

fn is_strict_descendant(path: &str, root: &str) -> bool {
    path.strip_prefix(root)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_normalized_absolute_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        && value != "/"
        && !value.ends_with('/')
        && !value.contains("//")
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn required_directories<'a>(
    files: &'a [ManifestFileRecord],
    manifest_path: &'a str,
) -> Result<BTreeSet<&'a str>, DebianRelationshipError> {
    let mut directories = BTreeSet::new();
    for path in files
        .iter()
        .map(|record| record.path.as_str())
        .chain(std::iter::once(manifest_path))
    {
        let mut parent = Path::new(path).parent();
        while let Some(directory) = parent {
            let value = directory.to_str().ok_or_else(product_manifest_error)?;
            if value == "/" {
                break;
            }
            directories.insert(value);
            parent = directory.parent();
        }
    }
    Ok(directories)
}

fn is_font_path(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "otf" | "ttc" | "ttf"
            )
        })
}

fn product_manifest_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::ProductManifestInvalid,
        "Linux product manifest differs from canonical format v1",
    )
}
