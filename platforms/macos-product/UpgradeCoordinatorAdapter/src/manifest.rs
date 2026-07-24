use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use radishlex_ime_product_upgrade::ProductRelease;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::MacOsUpgradeAdapterError;

const PRODUCT_ID: &str = "radishlex-macos";
const DATA_LAYOUT: &str = "application-support-v1";
const MANAGER_BUNDLE_ID: &str = "dev.radishlex.radishlexManager";
const INPUT_METHOD_BUNDLE_ID: &str = "org.radishlex.inputmethod.macos";
const MANAGER_BUNDLE: &str = "Components/radishlex_manager.app";
const INPUT_METHOD_BUNDLE: &str = "Components/RadishLexInputMethod.app";
const VALIDATION_HOST: &str = "Contents/Helpers/RadishLexUpgradeValidationHost";
const PREFLIGHT_HOST: &str = "Contents/Helpers/RadishLexUpgradePreflightHost";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductRole {
    Source,
    Target,
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedProductAssembly {
    release: ProductRelease,
    schema_version: i64,
    manager_validation: VerifiedExecutable,
    input_method_validation: VerifiedExecutable,
    preflight: Option<VerifiedExecutable>,
}

impl VerifiedProductAssembly {
    pub(crate) fn load(root: &Path, role: ProductRole) -> Result<Self, MacOsUpgradeAdapterError> {
        require_real_directory(root)?;
        let root = fs::canonicalize(root).map_err(|_| MacOsUpgradeAdapterError::UnsafeProduct)?;
        let manifest_path = root.join("ProductManifest.json");
        require_regular_file(&manifest_path, false)?;
        let bytes =
            fs::read(&manifest_path).map_err(|_| MacOsUpgradeAdapterError::InvalidManifest)?;
        let manifest: ProductManifest = serde_json::from_slice(&bytes)
            .map_err(|_| MacOsUpgradeAdapterError::InvalidManifest)?;
        manifest.validate()?;

        let release = ProductRelease::new(
            manifest.product_version.clone(),
            manifest
                .build_number
                .parse::<u64>()
                .map_err(|_| MacOsUpgradeAdapterError::InvalidManifest)?,
        )
        .map_err(|_| MacOsUpgradeAdapterError::InvalidManifest)?;
        let schema_version = i64::from(manifest.userdb_schema_version);
        let components = manifest.component_map()?;
        let manager = components
            .get("manager")
            .ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;
        let input_method = components
            .get("input_method")
            .ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;

        let manager_root = verified_bundle_root(&root, MANAGER_BUNDLE)?;
        let input_method_root = verified_bundle_root(&root, INPUT_METHOD_BUNDLE)?;
        let manager_validation = manager.verified_executable(&manager_root, VALIDATION_HOST)?;
        let input_method_validation =
            input_method.verified_executable(&input_method_root, VALIDATION_HOST)?;
        let preflight = match role {
            ProductRole::Source => None,
            ProductRole::Target => {
                Some(manager.verified_executable(&manager_root, PREFLIGHT_HOST)?)
            }
        };

        Ok(Self {
            release,
            schema_version,
            manager_validation,
            input_method_validation,
            preflight,
        })
    }

    pub(crate) fn matches(&self, release: &ProductRelease, schema_version: i64) -> bool {
        self.release == *release && self.schema_version == schema_version
    }

    pub(crate) fn schema_version(&self) -> i64 {
        self.schema_version
    }

    pub(crate) fn manager_validation(&self) -> &VerifiedExecutable {
        &self.manager_validation
    }

    pub(crate) fn input_method_validation(&self) -> &VerifiedExecutable {
        &self.input_method_validation
    }

    pub(crate) fn preflight(&self) -> Option<&VerifiedExecutable> {
        self.preflight.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedExecutable {
    path: PathBuf,
    size: u64,
    sha256: String,
}

impl VerifiedExecutable {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn revalidate(&self) -> Result<(), MacOsUpgradeAdapterError> {
        require_regular_file(&self.path, true)?;
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
        if metadata.len() != self.size || file_sha256(&self.path)? != self.sha256 {
            return Err(MacOsUpgradeAdapterError::ProductChanged);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductManifest {
    format_version: u32,
    product_id: String,
    product_version: String,
    build_number: String,
    minimum_macos: String,
    ffi_abi_version: u32,
    userdb_schema_version: u32,
    rime_data_manifest_version: u32,
    native_libraries_manifest_version: u32,
    data_layout: String,
    rime_schema_id: String,
    components: Vec<ComponentManifest>,
    licenses: Vec<LicenseRecord>,
}

impl ProductManifest {
    fn validate(&self) -> Result<(), MacOsUpgradeAdapterError> {
        if self.format_version != 1
            || self.product_id != PRODUCT_ID
            || self.data_layout != DATA_LAYOUT
            || !valid_version(&self.product_version, 3)
            || !valid_decimal(&self.build_number)
            || !valid_version(&self.minimum_macos, 2)
            || self.ffi_abi_version == 0
            || self.userdb_schema_version == 0
            || self.rime_data_manifest_version == 0
            || self.native_libraries_manifest_version == 0
            || self.rime_schema_id.is_empty()
            || self.licenses.is_empty()
        {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
        for license in &self.licenses {
            license.validate()?;
        }
        let components = self.component_map()?;
        if components.len() != 2 {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
        validate_component(
            components.get("manager"),
            MANAGER_BUNDLE_ID,
            &self.product_version,
            &self.build_number,
            &self.minimum_macos,
        )?;
        validate_component(
            components.get("input_method"),
            INPUT_METHOD_BUNDLE_ID,
            &self.product_version,
            &self.build_number,
            &self.minimum_macos,
        )
    }

    fn component_map(
        &self,
    ) -> Result<BTreeMap<&str, &ComponentManifest>, MacOsUpgradeAdapterError> {
        let mut components = BTreeMap::new();
        for component in &self.components {
            if components
                .insert(component.component.as_str(), component)
                .is_some()
            {
                return Err(MacOsUpgradeAdapterError::InvalidManifest);
            }
        }
        Ok(components)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentManifest {
    component: String,
    bundle_id: String,
    product_version: String,
    build_number: String,
    minimum_macos: String,
    files: Vec<FileRecord>,
}

impl ComponentManifest {
    fn verified_executable(
        &self,
        bundle_root: &Path,
        relative_path: &str,
    ) -> Result<VerifiedExecutable, MacOsUpgradeAdapterError> {
        let mut matching = self
            .files
            .iter()
            .filter(|record| record.path() == relative_path);
        let record = matching
            .next()
            .ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;
        if matching.next().is_some() {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
        let (size, sha256) = match record {
            FileRecord::File { size, sha256, .. } => (*size, sha256.clone()),
            FileRecord::Symlink { .. } => {
                return Err(MacOsUpgradeAdapterError::InvalidManifest);
            }
        };
        if !valid_sha256(&sha256) {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
        let path = bundle_root.join(relative_path);
        require_real_path_chain(bundle_root, relative_path)?;
        require_contained_path(bundle_root, &path)?;
        let executable = VerifiedExecutable { path, size, sha256 };
        executable.revalidate()?;
        Ok(executable)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FileRecord {
    File {
        path: String,
        size: u64,
        sha256: String,
    },
    Symlink {
        path: String,
        target: String,
    },
}

impl FileRecord {
    fn path(&self) -> &str {
        match self {
            Self::File { path, .. } | Self::Symlink { path, .. } => path,
        }
    }

    fn validate(&self) -> Result<(), MacOsUpgradeAdapterError> {
        validate_relative_path(self.path())?;
        match self {
            Self::File { sha256, .. } if valid_sha256(sha256) => Ok(()),
            Self::Symlink { target, .. }
                if !target.is_empty() && !Path::new(target).is_absolute() =>
            {
                Ok(())
            }
            _ => Err(MacOsUpgradeAdapterError::InvalidManifest),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LicenseRecord {
    path: String,
    size: u64,
    sha256: String,
}

impl LicenseRecord {
    fn validate(&self) -> Result<(), MacOsUpgradeAdapterError> {
        validate_relative_path(&self.path)?;
        if self.size == 0 || !valid_sha256(&self.sha256) {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
        Ok(())
    }
}

fn validate_component(
    component: Option<&&ComponentManifest>,
    bundle_id: &str,
    product_version: &str,
    build_number: &str,
    minimum_macos: &str,
) -> Result<(), MacOsUpgradeAdapterError> {
    let component = component.ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;
    if component.bundle_id != bundle_id
        || component.product_version != product_version
        || component.build_number != build_number
        || component.minimum_macos != minimum_macos
        || component.files.is_empty()
    {
        return Err(MacOsUpgradeAdapterError::InvalidManifest);
    }
    let mut paths = BTreeSet::new();
    for record in &component.files {
        record.validate()?;
        if !paths.insert(record.path()) {
            return Err(MacOsUpgradeAdapterError::InvalidManifest);
        }
    }
    Ok(())
}

fn require_real_directory(path: &Path) -> Result<(), MacOsUpgradeAdapterError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| MacOsUpgradeAdapterError::UnsafeProduct)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MacOsUpgradeAdapterError::UnsafeProduct);
    }
    Ok(())
}

fn verified_bundle_root(root: &Path, relative: &str) -> Result<PathBuf, MacOsUpgradeAdapterError> {
    validate_relative_path(relative)?;
    let path = root.join(relative);
    require_real_path_chain(root, relative)?;
    require_contained_path(root, &path)?;
    require_real_directory(&path)?;
    Ok(path)
}

fn require_regular_file(path: &Path, executable: bool) -> Result<(), MacOsUpgradeAdapterError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.nlink() != 1 {
        return Err(MacOsUpgradeAdapterError::ProductChanged);
    }
    if executable && metadata.mode() & 0o111 == 0 {
        return Err(MacOsUpgradeAdapterError::ProductChanged);
    }
    Ok(())
}

fn require_contained_path(root: &Path, path: &Path) -> Result<(), MacOsUpgradeAdapterError> {
    let canonical = fs::canonicalize(path).map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
    if !canonical.starts_with(root) {
        return Err(MacOsUpgradeAdapterError::UnsafeProduct);
    }
    Ok(())
}

fn require_real_path_chain(root: &Path, relative: &str) -> Result<(), MacOsUpgradeAdapterError> {
    let mut current = root.to_path_buf();
    let components: Vec<_> = Path::new(relative).components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
        if metadata.file_type().is_symlink() || (index + 1 < components.len() && !metadata.is_dir())
        {
            return Err(MacOsUpgradeAdapterError::ProductChanged);
        }
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), MacOsUpgradeAdapterError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MacOsUpgradeAdapterError::InvalidManifest);
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, MacOsUpgradeAdapterError> {
    let mut file = File::open(path).map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| MacOsUpgradeAdapterError::ProductChanged)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|parsed| parsed > 0)
}

fn valid_version(value: &str, component_count: usize) -> bool {
    value.split('.').count() == component_count
        && value.split('.').all(|component| {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        })
}
