use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use radishlex_ime_product_install::{InstallOperationKind, InstallReceipt};
use radishlex_ime_product_upgrade::{
    ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot, UpgradeFilesystemErrorCode,
    UpgradeReceipt, UpgradeReceiptStore, VerifiedDataRoot,
};
use radishlex_ime_userdb::{UserDb, UserDbSchemaCompatibility};

const USERDB_FILE_NAME: &str = "userdb.sqlite3";
const SETTINGS_FILE_NAME: &str = "manager-settings.json";
const RIME_DIRECTORY_NAME: &str = "Rime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeBootstrapErrorCode {
    InvalidInstallReceipt,
    DataRootIdentityChanged,
    UnsafeDataObject,
    UnsupportedUserDatabase,
    ReceiptBindingChanged,
    UpgradeFilesystem(UpgradeFilesystemErrorCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeBootstrapError {
    code: UpgradeBootstrapErrorCode,
}

impl UpgradeBootstrapError {
    const fn new(code: UpgradeBootstrapErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> UpgradeBootstrapErrorCode {
        self.code
    }

    pub const fn stable_code(self) -> &'static str {
        match self.code {
            UpgradeBootstrapErrorCode::InvalidInstallReceipt => "invalid_install_receipt",
            UpgradeBootstrapErrorCode::DataRootIdentityChanged => "data_root_identity_changed",
            UpgradeBootstrapErrorCode::UnsafeDataObject => "unsafe_data_object",
            UpgradeBootstrapErrorCode::UnsupportedUserDatabase => "unsupported_user_database",
            UpgradeBootstrapErrorCode::ReceiptBindingChanged => "receipt_binding_changed",
            UpgradeBootstrapErrorCode::UpgradeFilesystem(_) => "upgrade_filesystem_failure",
        }
    }
}

impl fmt::Display for UpgradeBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for UpgradeBootstrapError {}

pub struct BootstrappedUpgradeReceipt {
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl BootstrappedUpgradeReceipt {
    pub fn store(&self) -> &UpgradeReceiptStore {
        &self.store
    }

    pub fn receipt(&self) -> &UpgradeReceipt {
        &self.receipt
    }

    pub fn receipt_mut(&mut self) -> &mut UpgradeReceipt {
        &mut self.receipt
    }

    pub fn into_parts(self) -> (UpgradeReceiptStore, UpgradeReceipt) {
        (self.store, self.receipt)
    }
}

pub fn bootstrap_upgrade_receipt(
    install_receipt: &InstallReceipt,
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
) -> Result<BootstrappedUpgradeReceipt, UpgradeBootstrapError> {
    if install_receipt.operation_kind() != InstallOperationKind::Upgrade
        || install_receipt.state().is_terminal()
    {
        return Err(error(UpgradeBootstrapErrorCode::InvalidInstallReceipt));
    }
    let source_product = install_receipt
        .source_product()
        .ok_or_else(|| error(UpgradeBootstrapErrorCode::InvalidInstallReceipt))?;
    let target_product = install_receipt
        .target_product()
        .ok_or_else(|| error(UpgradeBootstrapErrorCode::InvalidInstallReceipt))?;
    let verified_root =
        VerifiedDataRoot::verify(data_root.as_ref(), expected_owner_id).map_err(map_filesystem)?;
    let store = UpgradeReceiptStore::open(verified_root).map_err(map_filesystem)?;
    verify_outer_root_binding(install_receipt, store.data_root_identity())?;

    let source_release = upgrade_release(source_product.release())?;
    let target_release = upgrade_release(target_product.release())?;
    let guard = store.acquire_guard().map_err(map_filesystem)?;
    let receipt = match store.load_guarded(&guard).map_err(map_filesystem)? {
        Some(current)
            if persisted_receipt_binding_matches(
                &current,
                install_receipt,
                store.data_root_identity(),
                &source_release,
                &target_release,
            ) =>
        {
            current
        }
        Some(_) => return Err(error(UpgradeBootstrapErrorCode::ReceiptBindingChanged)),
        None => {
            let expected = new_upgrade_receipt(
                install_receipt,
                data_root.as_ref(),
                expected_owner_id,
                &store,
                source_release,
                target_release,
            )?;
            store.persist(&guard, &expected).map_err(map_filesystem)?;
            expected
        }
    };
    drop(guard);
    Ok(BootstrappedUpgradeReceipt { store, receipt })
}

fn new_upgrade_receipt(
    install_receipt: &InstallReceipt,
    data_root: &Path,
    expected_owner_id: u32,
    store: &UpgradeReceiptStore,
    source_release: ProductRelease,
    target_release: ProductRelease,
) -> Result<UpgradeReceipt, UpgradeBootstrapError> {
    let database_path = data_root.join(USERDB_FILE_NAME);
    let database_identity = private_file_identity(
        &database_path,
        expected_owner_id,
        UpgradeArtifactSlot::SourceDatabase,
    )?;
    let inspection = UserDb::inspect_file(&database_path)
        .map_err(|_| error(UpgradeBootstrapErrorCode::UnsupportedUserDatabase))?;
    if inspection.compatibility == UserDbSchemaCompatibility::Future {
        return Err(error(UpgradeBootstrapErrorCode::UnsupportedUserDatabase));
    }
    let mut artifacts = vec![store.data_root_identity().clone(), database_identity];
    if let Some(settings) = optional_private_file_identity(
        data_root.join(SETTINGS_FILE_NAME),
        expected_owner_id,
        UpgradeArtifactSlot::SourceSettings,
    )? {
        artifacts.push(settings);
    }
    if let Some(rime_root) = optional_private_directory_identity(
        data_root.join(RIME_DIRECTORY_NAME),
        expected_owner_id,
        UpgradeArtifactSlot::RimeRoot,
    )? {
        artifacts.push(rime_root);
    }
    UpgradeReceipt::new(
        install_receipt.operation_id(),
        None,
        source_release,
        target_release,
        Some(inspection.schema_version),
        inspection.supported_schema_version,
        artifacts,
    )
    .map_err(|_| error(UpgradeBootstrapErrorCode::InvalidInstallReceipt))
}

fn persisted_receipt_binding_matches(
    current: &UpgradeReceipt,
    install_receipt: &InstallReceipt,
    data_root: &UpgradeArtifactIdentity,
    source_release: &ProductRelease,
    target_release: &ProductRelease,
) -> bool {
    current.operation_id() == install_receipt.operation_id()
        && current.previous_operation_id().is_none()
        && current.source_release() == source_release
        && current.target_release() == target_release
        && current.target_schema_version() == UserDb::supported_schema_version()
        && current
            .artifacts()
            .iter()
            .any(|artifact| artifact == data_root)
}

fn verify_outer_root_binding(
    install_receipt: &InstallReceipt,
    data_root: &UpgradeArtifactIdentity,
) -> Result<(), UpgradeBootstrapError> {
    let outer = install_receipt.root_identity();
    if data_root.slot() != UpgradeArtifactSlot::DataRoot
        || outer.device_id() != data_root.device_id()
        || outer.inode() != data_root.inode()
        || outer.owner_id() != data_root.owner_id()
        || outer.mode() != data_root.mode()
    {
        return Err(error(UpgradeBootstrapErrorCode::DataRootIdentityChanged));
    }
    Ok(())
}

fn upgrade_release(
    release: &radishlex_ime_product_install::ProductRelease,
) -> Result<ProductRelease, UpgradeBootstrapError> {
    ProductRelease::new(release.product_version(), release.build_number())
        .map_err(|_| error(UpgradeBootstrapErrorCode::InvalidInstallReceipt))
}

fn optional_private_file_identity(
    path: PathBuf,
    expected_owner_id: u32,
    slot: UpgradeArtifactSlot,
) -> Result<Option<UpgradeArtifactIdentity>, UpgradeBootstrapError> {
    match fs::symlink_metadata(&path) {
        Ok(_) => private_file_identity(&path, expected_owner_id, slot).map(Some),
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(error(UpgradeBootstrapErrorCode::UnsafeDataObject)),
    }
}

fn private_file_identity(
    path: &Path,
    expected_owner_id: u32,
    slot: UpgradeArtifactSlot,
) -> Result<UpgradeArtifactIdentity, UpgradeBootstrapError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| error(UpgradeBootstrapErrorCode::UnsafeDataObject))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(error(UpgradeBootstrapErrorCode::UnsafeDataObject));
    }
    UpgradeArtifactIdentity::private_file(
        slot,
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.len(),
    )
    .map_err(|_| error(UpgradeBootstrapErrorCode::UnsafeDataObject))
}

fn optional_private_directory_identity(
    path: PathBuf,
    expected_owner_id: u32,
    slot: UpgradeArtifactSlot,
) -> Result<Option<UpgradeArtifactIdentity>, UpgradeBootstrapError> {
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(error(UpgradeBootstrapErrorCode::UnsafeDataObject)),
    };
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(error(UpgradeBootstrapErrorCode::UnsafeDataObject));
    }
    UpgradeArtifactIdentity::private_directory(
        slot,
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.nlink(),
    )
    .map(Some)
    .map_err(|_| error(UpgradeBootstrapErrorCode::UnsafeDataObject))
}

const fn error(code: UpgradeBootstrapErrorCode) -> UpgradeBootstrapError {
    UpgradeBootstrapError::new(code)
}

fn map_filesystem(
    error: radishlex_ime_product_upgrade::UpgradeFilesystemError,
) -> UpgradeBootstrapError {
    UpgradeBootstrapError::new(UpgradeBootstrapErrorCode::UpgradeFilesystem(error.code()))
}
