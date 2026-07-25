use std::collections::BTreeSet;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use radishlex_ime_product_install::{
    ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
    INSTALL_PRODUCT_ID,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    error, MacOsCodeSignatureVerifier, MacOsInstallAdapterError, MacOsInstallAdapterErrorCode,
};

const COMMITTED_LAYOUT: &[u8] = include_bytes!("../../../../packaging/macos/install-layout.json");
const PAYLOAD_MANIFEST_NAME: &str = "InstallPayloadManifest.json";
const UPGRADE_SOURCES_DIRECTORY: &str = "UpgradeSources";
const PRODUCT_MANIFEST_NAME: &str = "ProductManifest.json";
const LICENSE_NAME: &str = "LICENSE";
const MANAGER_PRODUCT_PATH: &str = "Components/radishlex_manager.app";
const INPUT_METHOD_PRODUCT_PATH: &str = "Components/RadishLexInputMethod.app";
const MANAGER_TARGET_PATH: &str = "Applications/RadishLex Manager.app";
const INPUT_METHOD_TARGET_PATH: &str = "Library/Input Methods/RadishLexInputMethod.app";
const DATA_ROOT_PATH: &str = "Library/Application Support/RadishLex";
const INSTALL_STATE_PATH: &str = "Library/Application Support/RadishLex/.radishlex-install-v1";
const INSTALLER_BUNDLE_ID: &str = "org.radishlex.installer.macos";
const MANAGER_BUNDLE_ID: &str = "dev.radishlex.radishlexManager";
const INPUT_METHOD_BUNDLE_ID: &str = "org.radishlex.inputmethod.macos";
const DISTRIBUTION_IDENTITY: &str = "community-adhoc-v1";
const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_BUNDLE_RECORDS: usize = 20_000;
const MAX_UPGRADE_SOURCES: usize = 64;

#[derive(Debug)]
pub(crate) struct VerifiedInstallPayload {
    root: PathBuf,
    root_identity: DirectoryIdentity,
    manager_bundle: PathBuf,
    input_method_bundle: PathBuf,
    target_product: ProductArtifactIdentity,
    upgrade_sources: Vec<VerifiedUpgradeSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedUpgradeSource {
    release: ProductRelease,
    product_root: PathBuf,
    product: ProductArtifactIdentity,
}

impl VerifiedInstallPayload {
    pub(crate) fn load(
        root: &Path,
        verifier: &dyn MacOsCodeSignatureVerifier,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let snapshot = PayloadSnapshot::load(root, verifier)?;
        Ok(Self {
            root: root.to_path_buf(),
            root_identity: snapshot.root_identity,
            manager_bundle: snapshot.manager_bundle,
            input_method_bundle: snapshot.input_method_bundle,
            target_product: snapshot.target_product,
            upgrade_sources: snapshot.upgrade_sources,
        })
    }

    pub(crate) fn target_product(&self) -> &ProductArtifactIdentity {
        &self.target_product
    }

    pub(crate) fn bundle_path(&self, component: ProgramComponent) -> &Path {
        match component {
            ProgramComponent::Manager => &self.manager_bundle,
            ProgramComponent::InputMethod => &self.input_method_bundle,
        }
    }

    pub(crate) fn upgrade_source_product_root(&self, release: &ProductRelease) -> Option<&Path> {
        self.upgrade_sources
            .iter()
            .find(|source| &source.release == release)
            .map(|source| source.product_root.as_path())
    }

    pub(crate) fn revalidate(
        &self,
        verifier: &dyn MacOsCodeSignatureVerifier,
    ) -> Result<ProductArtifactIdentity, MacOsInstallAdapterError> {
        let snapshot = PayloadSnapshot::load(&self.root, verifier)?;
        if snapshot.root_identity != self.root_identity
            || snapshot.manager_bundle != self.manager_bundle
            || snapshot.input_method_bundle != self.input_method_bundle
            || snapshot.target_product != self.target_product
            || snapshot.upgrade_sources != self.upgrade_sources
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        Ok(snapshot.target_product)
    }
}

struct PayloadSnapshot {
    root_identity: DirectoryIdentity,
    manager_bundle: PathBuf,
    input_method_bundle: PathBuf,
    target_product: ProductArtifactIdentity,
    upgrade_sources: Vec<VerifiedUpgradeSource>,
}

impl PayloadSnapshot {
    fn load(
        root: &Path,
        verifier: &dyn MacOsCodeSignatureVerifier,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let root_identity = verify_payload_root(root)?;
        require_exact_entries(
            root,
            &[
                "InstallLayout.json",
                PAYLOAD_MANIFEST_NAME,
                "Product",
                UPGRADE_SOURCES_DIRECTORY,
            ],
            MacOsInstallAdapterErrorCode::UnsafePayload,
        )?;

        let layout_path = root.join("InstallLayout.json");
        let layout_bytes = read_regular_file(
            &layout_path,
            MacOsInstallAdapterErrorCode::InvalidPayloadManifest,
        )?;
        if layout_bytes != COMMITTED_LAYOUT {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        let payload_manifest_path = root.join(PAYLOAD_MANIFEST_NAME);
        let payload_bytes = read_regular_file(
            &payload_manifest_path,
            MacOsInstallAdapterErrorCode::InvalidPayloadManifest,
        )?;
        let payload: PayloadManifest = serde_json::from_slice(&payload_bytes)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest))?;
        payload.validate(&layout_path, &layout_bytes)?;

        let target = ProductAssemblySnapshot::load(
            &root.join("Product"),
            &payload.product_manifest,
            "Product/ProductManifest.json",
            verifier,
        )?;
        payload.validate_product(&target.manifest)?;
        let upgrade_sources_root = root.join(UPGRADE_SOURCES_DIRECTORY);
        require_real_directory(
            &upgrade_sources_root,
            MacOsInstallAdapterErrorCode::UnsafePayload,
        )?;
        let expected_source_names: BTreeSet<_> = payload
            .upgrade_sources
            .iter()
            .map(|source| source.directory_name())
            .collect::<Result<_, _>>()?;
        require_exact_entry_set(
            &upgrade_sources_root,
            &expected_source_names,
            MacOsInstallAdapterErrorCode::InvalidPayloadManifest,
        )?;
        let mut upgrade_sources = Vec::with_capacity(payload.upgrade_sources.len());
        let mut previous_build = 0;
        for source in &payload.upgrade_sources {
            source.validate(previous_build, target.product.release().build_number())?;
            let product_root = root.join(&source.product_path);
            let assembly = ProductAssemblySnapshot::load(
                &product_root,
                &source.product_manifest,
                &format!("{}/ProductManifest.json", source.product_path),
                verifier,
            )?;
            if assembly.product.release().product_version() != source.product_version
                || assembly.product.release().build_number().to_string() != source.build_number
            {
                return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
            }
            previous_build = assembly.product.release().build_number();
            upgrade_sources.push(VerifiedUpgradeSource {
                release: assembly.product.release().clone(),
                product_root,
                product: assembly.product,
            });
        }

        Ok(Self {
            root_identity,
            manager_bundle: target.manager_bundle,
            input_method_bundle: target.input_method_bundle,
            target_product: target.product,
            upgrade_sources,
        })
    }
}

struct ProductAssemblySnapshot {
    manager_bundle: PathBuf,
    input_method_bundle: PathBuf,
    manifest: ProductManifest,
    product: ProductArtifactIdentity,
}

impl ProductAssemblySnapshot {
    fn load(
        product_root: &Path,
        manifest_record: &ManifestFileRecord,
        expected_manifest_path: &str,
        verifier: &dyn MacOsCodeSignatureVerifier,
    ) -> Result<Self, MacOsInstallAdapterError> {
        require_real_directory(product_root, MacOsInstallAdapterErrorCode::UnsafePayload)?;
        require_exact_entries(
            product_root,
            &["Components", LICENSE_NAME, PRODUCT_MANIFEST_NAME],
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        let components_root = product_root.join("Components");
        require_real_directory(
            &components_root,
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        require_exact_entries(
            &components_root,
            &["RadishLexInputMethod.app", "radishlex_manager.app"],
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        let manager_bundle = product_root.join(MANAGER_PRODUCT_PATH);
        let input_method_bundle = product_root.join(INPUT_METHOD_PRODUCT_PATH);
        require_real_directory(
            &manager_bundle,
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        require_real_directory(
            &input_method_bundle,
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        let product_manifest_path = product_root.join(PRODUCT_MANIFEST_NAME);
        let product_bytes = read_regular_file(
            &product_manifest_path,
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        manifest_record
            .verify(
                &product_manifest_path,
                &product_bytes,
                expected_manifest_path,
            )
            .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest))?;
        let manifest: ProductManifest = serde_json::from_slice(&product_bytes)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        manifest.validate()?;
        let license_path = product_root.join(LICENSE_NAME);
        let license_bytes = read_regular_file(
            &license_path,
            MacOsInstallAdapterErrorCode::InvalidProductManifest,
        )?;
        manifest.verify_license(&license_path, &license_bytes)?;
        let manager = manifest.component("manager")?;
        let input_method = manifest.component("input_method")?;
        let manager_tree = inspect_bundle_tree(&manager_bundle)?;
        manager.verify_tree(&manager_tree)?;
        let input_method_tree = inspect_bundle_tree(&input_method_bundle)?;
        input_method.verify_tree(&input_method_tree)?;
        let manager_code = verifier.verify(
            &manager_bundle,
            ProgramComponent::Manager,
            &manager.bundle_id,
        )?;
        let input_method_code = verifier.verify(
            &input_method_bundle,
            ProgramComponent::InputMethod,
            &input_method.bundle_id,
        )?;
        let confirmed_manager_tree = inspect_bundle_tree(&manager_bundle)?;
        let confirmed_input_method_tree = inspect_bundle_tree(&input_method_bundle)?;
        if confirmed_manager_tree.records != manager_tree.records
            || confirmed_input_method_tree.records != input_method_tree.records
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        if manager_code.bundle_id() != manager.bundle_id
            || input_method_code.bundle_id() != input_method.bundle_id
        {
            return Err(error(
                MacOsInstallAdapterErrorCode::SignatureIdentityChanged,
            ));
        }
        let release = ProductRelease::new(
            manifest.product_version.clone(),
            parse_positive_u64(&manifest.build_number)?,
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        let manager_identity = ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            manager.bundle_id.clone(),
            manager_tree.sha256.clone(),
            manager_code.evidence_sha256(),
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        let input_method_identity = ProgramBundleIdentity::new(
            ProgramComponent::InputMethod,
            input_method.bundle_id.clone(),
            input_method_tree.sha256.clone(),
            input_method_code.evidence_sha256(),
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        let product = ProductArtifactIdentity::new(
            manifest.product_id.clone(),
            release,
            sha256_bytes(&product_bytes),
            manager_identity,
            input_method_identity,
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        Ok(Self {
            manager_bundle,
            input_method_bundle,
            manifest,
            product,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadManifest {
    format_version: u32,
    product_id: String,
    product_version: String,
    build_number: String,
    distribution_container: String,
    installer_kind: String,
    installation_scope: String,
    installer_bundle_id: String,
    components: PayloadComponents,
    data: PayloadData,
    install_layout: ManifestFileRecord,
    product_manifest: ManifestFileRecord,
    upgrade_sources: Vec<PayloadUpgradeSource>,
}

impl PayloadManifest {
    fn validate(
        &self,
        layout_path: &Path,
        layout_bytes: &[u8],
    ) -> Result<(), MacOsInstallAdapterError> {
        if self.format_version != 2
            || self.product_id != INSTALL_PRODUCT_ID
            || !valid_version(&self.product_version, 3)
            || parse_positive_u64(&self.build_number).is_err()
            || self.distribution_container != "dmg"
            || self.installer_kind != "dedicated-user-domain-app"
            || self.installation_scope != "current-user"
            || self.installer_bundle_id != INSTALLER_BUNDLE_ID
            || self.components.manager.product_path != MANAGER_PRODUCT_PATH
            || self.components.manager.target_path != MANAGER_TARGET_PATH
            || self.components.input_method.product_path != INPUT_METHOD_PRODUCT_PATH
            || self.components.input_method.target_path != INPUT_METHOD_TARGET_PATH
            || self.data.root_path != DATA_ROOT_PATH
            || self.data.install_state_path != INSTALL_STATE_PATH
            || self.data.default_removal != "programs-only"
            || self.data.data_removal != "separate-authorized-flow"
            || self.upgrade_sources.len() > MAX_UPGRADE_SOURCES
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        self.install_layout
            .verify(layout_path, layout_bytes, "InstallLayout.json")
    }

    fn validate_product(&self, product: &ProductManifest) -> Result<(), MacOsInstallAdapterError> {
        if self.product_id != product.product_id
            || self.product_version != product.product_version
            || self.build_number != product.build_number
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadUpgradeSource {
    product_version: String,
    build_number: String,
    product_path: String,
    product_manifest: ManifestFileRecord,
}

impl PayloadUpgradeSource {
    fn directory_name(&self) -> Result<String, MacOsInstallAdapterError> {
        if !valid_version(&self.product_version, 3)
            || parse_positive_u64(&self.build_number).is_err()
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        let name = format!("{}-{}", self.product_version, self.build_number);
        if self.product_path != format!("{UPGRADE_SOURCES_DIRECTORY}/{name}") {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        Ok(name)
    }

    fn validate(
        &self,
        previous_build: u64,
        target_build: u64,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.directory_name()?;
        let build = parse_positive_u64(&self.build_number)?;
        if build <= previous_build || build >= target_build {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidPayloadManifest));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadComponents {
    manager: PayloadComponent,
    input_method: PayloadComponent,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadComponent {
    product_path: String,
    target_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadData {
    root_path: String,
    install_state_path: String,
    default_removal: String,
    data_removal: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFileRecord {
    path: String,
    size: u64,
    sha256: String,
}

impl ManifestFileRecord {
    fn verify(
        &self,
        path: &Path,
        bytes: &[u8],
        expected_path: &str,
    ) -> Result<(), MacOsInstallAdapterError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
        if self.path != expected_path
            || self.size != bytes.len() as u64
            || self.size != metadata.len()
            || !valid_sha256(&self.sha256)
            || self.sha256 != sha256_bytes(bytes)
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
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
    distribution_identity: String,
    rime_schema_id: String,
    components: Vec<ProductComponent>,
    licenses: Vec<ManifestFileRecord>,
}

impl ProductManifest {
    fn validate(&self) -> Result<(), MacOsInstallAdapterError> {
        if self.format_version != 3
            || self.product_id != INSTALL_PRODUCT_ID
            || !valid_version(&self.product_version, 3)
            || parse_positive_u64(&self.build_number).is_err()
            || !valid_version(&self.minimum_macos, 2)
            || self.ffi_abi_version == 0
            || self.userdb_schema_version == 0
            || self.rime_data_manifest_version == 0
            || self.native_libraries_manifest_version == 0
            || self.data_layout != "application-support-v1"
            || self.distribution_identity != DISTRIBUTION_IDENTITY
            || self.rime_schema_id.is_empty()
            || self.components.len() != 2
            || self.licenses.len() != 1
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
        }
        let manager = self.component("manager")?;
        let input_method = self.component("input_method")?;
        manager.validate(
            MANAGER_BUNDLE_ID,
            &self.product_version,
            &self.build_number,
            &self.minimum_macos,
        )?;
        input_method.validate(
            INPUT_METHOD_BUNDLE_ID,
            &self.product_version,
            &self.build_number,
            &self.minimum_macos,
        )
    }

    fn component(&self, name: &str) -> Result<&ProductComponent, MacOsInstallAdapterError> {
        let mut matching = self
            .components
            .iter()
            .filter(|component| component.component == name);
        let component = matching
            .next()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
        if matching.next().is_some() {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
        }
        Ok(component)
    }

    fn verify_license(&self, path: &Path, bytes: &[u8]) -> Result<(), MacOsInstallAdapterError> {
        self.licenses[0]
            .verify(path, bytes, LICENSE_NAME)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductComponent {
    component: String,
    bundle_id: String,
    product_version: String,
    build_number: String,
    minimum_macos: String,
    files: Vec<ProductFileRecord>,
}

impl ProductComponent {
    fn validate(
        &self,
        expected_bundle_id: &str,
        product_version: &str,
        build_number: &str,
        minimum_macos: &str,
    ) -> Result<(), MacOsInstallAdapterError> {
        if self.bundle_id != expected_bundle_id
            || self.product_version != product_version
            || self.build_number != build_number
            || self.minimum_macos != minimum_macos
            || self.files.is_empty()
            || self.files.len() > MAX_BUNDLE_RECORDS
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
        }
        let mut previous = None;
        let mut paths = BTreeSet::new();
        for record in &self.files {
            record.validate()?;
            if previous.is_some_and(|path: &str| path >= record.path())
                || !paths.insert(record.path())
            {
                return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
            }
            previous = Some(record.path());
        }
        Ok(())
    }

    fn verify_tree(&self, tree: &BundleTreeInspection) -> Result<(), MacOsInstallAdapterError> {
        let expected: Vec<_> = self
            .files
            .iter()
            .map(ProductFileRecord::as_tree_record)
            .collect();
        if expected != tree.records {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ProductFileRecord {
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

impl ProductFileRecord {
    fn path(&self) -> &str {
        match self {
            Self::File { path, .. } | Self::Symlink { path, .. } => path,
        }
    }

    fn validate(&self) -> Result<(), MacOsInstallAdapterError> {
        validate_relative_path(self.path())?;
        match self {
            Self::File { sha256, .. } if valid_sha256(sha256) => Ok(()),
            Self::Symlink { target, .. } if valid_relative_symlink_target(target) => Ok(()),
            _ => Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest)),
        }
    }

    fn as_tree_record(&self) -> BundleTreeRecord {
        match self {
            Self::File { path, size, sha256 } => BundleTreeRecord::File {
                path: path.clone(),
                size: *size,
                sha256: sha256.clone(),
            },
            Self::Symlink { path, target } => BundleTreeRecord::Symlink {
                path: path.clone(),
                target: target.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BundleTreeRecord {
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

impl BundleTreeRecord {
    fn path(&self) -> &str {
        match self {
            Self::File { path, .. } | Self::Symlink { path, .. } => path,
        }
    }
}

pub(crate) struct BundleTreeInspection {
    records: Vec<BundleTreeRecord>,
    sha256: String,
}

impl BundleTreeInspection {
    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

pub(crate) fn inspect_bundle_tree(
    root: &Path,
) -> Result<BundleTreeInspection, MacOsInstallAdapterError> {
    require_real_directory(root, MacOsInstallAdapterErrorCode::ProductChanged)?;
    let canonical_root =
        fs::canonicalize(root).map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
    if canonical_root != root {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    let mut records = Vec::new();
    collect_tree_records(root, root, &mut records)?;
    records.sort_by(|left, right| left.path().cmp(right.path()));
    if records.is_empty() || records.len() > MAX_BUNDLE_RECORDS {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    let mut paths = BTreeSet::new();
    if records.iter().any(|record| !paths.insert(record.path())) {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    let sha256 = tree_sha256(&records)?;
    Ok(BundleTreeInspection { records, sha256 })
}

fn collect_tree_records(
    root: &Path,
    directory: &Path,
    records: &mut Vec<BundleTreeRecord>,
) -> Result<(), MacOsInstallAdapterError> {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?
        .collect::<Result<_, _>>()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
        let relative = path_to_utf8(relative)?;
        validate_relative_path(&relative)?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)
                .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
            let target = path_to_utf8(&target)?;
            if !valid_relative_symlink_target(&target) {
                return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
            }
            let resolved = fs::canonicalize(&path)
                .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
            if !resolved.starts_with(root) {
                return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
            }
            records.push(BundleTreeRecord::Symlink {
                path: relative,
                target,
            });
        } else if metadata.file_type().is_dir() && metadata.mode() & 0o022 == 0 {
            collect_tree_records(root, &path, records)?;
        } else if metadata.file_type().is_file()
            && metadata.nlink() == 1
            && metadata.mode() & 0o022 == 0
        {
            records.push(BundleTreeRecord::File {
                path: relative,
                size: metadata.len(),
                sha256: file_sha256(&path)?,
            });
        } else {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        if records.len() > MAX_BUNDLE_RECORDS {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
    }
    Ok(())
}

fn tree_sha256(records: &[BundleTreeRecord]) -> Result<String, MacOsInstallAdapterError> {
    let mut digest = Sha256::new();
    update_field(&mut digest, b"radishlex-bundle-tree-v1");
    digest.update((records.len() as u64).to_be_bytes());
    for record in records {
        match record {
            BundleTreeRecord::File { path, size, sha256 } => {
                update_field(&mut digest, b"file");
                update_field(&mut digest, path.as_bytes());
                digest.update(size.to_be_bytes());
                digest.update(decode_sha256(sha256)?);
            }
            BundleTreeRecord::Symlink { path, target } => {
                update_field(&mut digest, b"symlink");
                update_field(&mut digest, path.as_bytes());
                update_field(&mut digest, target.as_bytes());
            }
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_payload_root(root: &Path) -> Result<DirectoryIdentity, MacOsInstallAdapterError> {
    if !root.is_absolute() {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafePayload));
    }
    let canonical =
        fs::canonicalize(root).map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafePayload))?;
    if canonical != root {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafePayload));
    }
    let metadata = fs::symlink_metadata(root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafePayload))?;
    if !metadata.file_type().is_dir() || metadata.ino() == 0 || metadata.mode() & 0o022 != 0 {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafePayload));
    }
    Ok(DirectoryIdentity::from_metadata(&metadata))
}

fn require_exact_entries(
    directory: &Path,
    expected: &[&str],
    code: MacOsInstallAdapterErrorCode,
) -> Result<(), MacOsInstallAdapterError> {
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory).map_err(|_| error(MacOsInstallAdapterErrorCode::Io))? {
        let name = entry
            .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?
            .file_name();
        let name = name.to_str().ok_or_else(|| error(code))?.to_owned();
        actual.insert(name);
    }
    let expected: BTreeSet<_> = expected.iter().map(|value| (*value).to_owned()).collect();
    if actual != expected {
        return Err(error(code));
    }
    Ok(())
}

fn require_exact_entry_set(
    directory: &Path,
    expected: &BTreeSet<String>,
    code: MacOsInstallAdapterErrorCode,
) -> Result<(), MacOsInstallAdapterError> {
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory).map_err(|_| error(MacOsInstallAdapterErrorCode::Io))? {
        let name = entry
            .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))?
            .file_name();
        actual.insert(name.to_str().ok_or_else(|| error(code))?.to_owned());
    }
    if &actual != expected {
        return Err(error(code));
    }
    Ok(())
}

fn require_real_directory(
    path: &Path,
    code: MacOsInstallAdapterErrorCode,
) -> Result<(), MacOsInstallAdapterError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error(code))?;
    if !metadata.file_type().is_dir() || metadata.ino() == 0 || metadata.mode() & 0o022 != 0 {
        return Err(error(code));
    }
    Ok(())
}

fn read_regular_file(
    path: &Path,
    code: MacOsInstallAdapterErrorCode,
) -> Result<Vec<u8>, MacOsInstallAdapterError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error(code))?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.mode() & 0o022 != 0
        || metadata.len() > MAX_MANIFEST_BYTES
    {
        return Err(error(code));
    }
    fs::read(path).map_err(|_| error(code))
}

fn file_sha256(path: &Path) -> Result<String, MacOsInstallAdapterError> {
    let mut file =
        File::open(path).map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn decode_sha256(value: &str) -> Result<[u8; 32], MacOsInstallAdapterError> {
    if !valid_sha256(value) {
        return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
    }
    let mut decoded = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_value(chunk[0]);
        let low = hex_value(chunk[1]);
        decoded[index] = (high << 4) | low;
    }
    Ok(decoded)
}

fn hex_value(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => 0,
    }
}

fn update_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn validate_relative_path(value: &str) -> Result<(), MacOsInstallAdapterError> {
    let path = Path::new(value);
    if value.is_empty()
        || value.as_bytes().contains(&b'\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
    }
    Ok(())
}

fn valid_relative_symlink_target(value: &str) -> bool {
    !value.is_empty() && !Path::new(value).is_absolute() && !value.as_bytes().contains(&b'\\')
}

fn path_to_utf8(path: &Path) -> Result<String, MacOsInstallAdapterError> {
    if path.as_os_str().as_bytes().contains(&0) {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| error(MacOsInstallAdapterErrorCode::ProductChanged))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_version(value: &str, parts: usize) -> bool {
    value.split('.').count() == parts
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'))
        })
}

fn parse_positive_u64(value: &str) -> Result<u64, MacOsInstallAdapterError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|parsed| *parsed > 0 && parsed.to_string() == value)
        .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    device_id: u64,
    inode: u64,
}

impl DirectoryIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device_id: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}
