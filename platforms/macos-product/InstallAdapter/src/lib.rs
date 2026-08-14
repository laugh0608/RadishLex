//! Manifest-bound macOS adapter for RadishLex product installation.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
#[cfg(feature = "qualification-harness")]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use radishlex_ime_product_install::{
    record_program_source as record_core_program_source,
    record_staged_program as record_core_staged_program, InstallFinalizationPort,
    InstallFinalizationValidationStage, InstallOperationKind, InstallProcessGuard,
    InstallProgramValidationPort, InstallReceipt, InstallReceiptStore, InstallRootIdentity,
    InstallState, ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
    ProgramSwitchStore, RunningProgramIdentity, VerifiedInstallRoot, VerifiedProgramTarget,
};
use serde::Deserialize;

mod codesign;
mod copy;
mod manifest;

#[cfg(feature = "qualification-harness")]
use codesign::CodesignQualificationCodeSignatureVerifier;
pub use codesign::{
    inspect_community_ad_hoc_application, inspect_developer_id_application,
    CodeSignatureRequirements, CodesignCodeSignatureVerifier, CodesignRunningIdentityInspector,
    CommunityAdHocApplicationIdentity, DeveloperIdApplicationIdentity, MacOsCodeIdentity,
    MacOsCodeSignatureVerifier, RADISHLEX_COMMUNITY_DISTRIBUTION_IDENTITY,
    RADISHLEX_DEVELOPER_TEAM_ID,
};
use copy::{sync_bundle_tree, BundleCopier, DittoBundleCopier};
use manifest::{inspect_bundle_tree, VerifiedInstallPayload};

const MANAGER_PARENT: &str = "Applications";
const INPUT_METHOD_PARENT: &str = "Library/Input Methods";
const DATA_ROOT: &str = "Library/Application Support/RadishLex";
const MAX_INFO_PLIST_BYTES: usize = 64 * 1024;
#[cfg(feature = "qualification-harness")]
const QUALIFICATION_MARKER_FILE: &str = "radishlex-upgrade-qualification.marker";
#[cfg(feature = "qualification-harness")]
const QUALIFICATION_MARKER_BYTES: &[u8] = b"radishlex-upgrade-qualification-v1\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacOsInstallAdapterErrorCode {
    UnsafeUserHome,
    UnsafeTarget,
    UnsafePayload,
    InvalidPayloadManifest,
    InvalidProductManifest,
    ProductChanged,
    SignatureRejected,
    SignatureIdentityChanged,
    StagingConflict,
    CopyFailed,
    StagedQuarantineRejected,
    InvalidOperation,
    CoreRejected,
    #[cfg(feature = "qualification-harness")]
    UnsafeQualification,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOsInstallAdapterError {
    code: MacOsInstallAdapterErrorCode,
}

impl MacOsInstallAdapterError {
    pub(crate) const fn new(code: MacOsInstallAdapterErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> MacOsInstallAdapterErrorCode {
        self.code
    }
}

impl fmt::Display for MacOsInstallAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            MacOsInstallAdapterErrorCode::UnsafeUserHome => {
                "authoritative current-user home is unsafe"
            }
            MacOsInstallAdapterErrorCode::UnsafeTarget => {
                "macOS product installation target is unsafe"
            }
            MacOsInstallAdapterErrorCode::UnsafePayload => "macOS install payload root is unsafe",
            MacOsInstallAdapterErrorCode::InvalidPayloadManifest => {
                "macOS install payload manifest is invalid"
            }
            MacOsInstallAdapterErrorCode::InvalidProductManifest => {
                "macOS product manifest is invalid"
            }
            MacOsInstallAdapterErrorCode::ProductChanged => "macOS product bundle content changed",
            MacOsInstallAdapterErrorCode::SignatureRejected => {
                "macOS product code signature was rejected"
            }
            MacOsInstallAdapterErrorCode::SignatureIdentityChanged => {
                "macOS product code identity changed"
            }
            MacOsInstallAdapterErrorCode::StagingConflict => {
                "macOS program staging contains an unexpected object"
            }
            MacOsInstallAdapterErrorCode::CopyFailed => {
                "macOS metadata-preserving bundle copy failed"
            }
            MacOsInstallAdapterErrorCode::StagedQuarantineRejected => {
                "macOS staged bundle retained quarantine metadata"
            }
            MacOsInstallAdapterErrorCode::InvalidOperation => {
                "macOS install adapter operation binding is invalid"
            }
            MacOsInstallAdapterErrorCode::CoreRejected => {
                "product installation core rejected the adapter result"
            }
            #[cfg(feature = "qualification-harness")]
            MacOsInstallAdapterErrorCode::UnsafeQualification => {
                "macOS install qualification root is unsafe"
            }
            MacOsInstallAdapterErrorCode::Io => "macOS install adapter I/O failed",
        })
    }
}

impl std::error::Error for MacOsInstallAdapterError {}

pub fn inspect_running_program_identity(
    bundle_path: &Path,
    component: ProgramComponent,
) -> Result<RunningProgramIdentity, MacOsInstallAdapterError> {
    let tree = inspect_bundle_tree(bundle_path)?;
    let code_identity = CodesignRunningIdentityInspector.inspect(bundle_path, component)?;
    let confirmed_tree = inspect_bundle_tree(bundle_path)?;
    if confirmed_tree.sha256() != tree.sha256() {
        return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
    }
    let info = inspect_info_plist(bundle_path)?;
    if info.bundle_id != code_identity.bundle_id() {
        return Err(error(
            MacOsInstallAdapterErrorCode::SignatureIdentityChanged,
        ));
    }
    let release = ProductRelease::new(
        info.product_version,
        info.build_number
            .parse::<u64>()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?,
    )
    .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
    let bundle = ProgramBundleIdentity::new(
        component,
        info.bundle_id,
        tree.sha256(),
        code_identity.evidence_sha256(),
    )
    .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
    RunningProgramIdentity::new(release, bundle)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))
}

#[derive(Deserialize)]
struct RunningInfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    bundle_id: String,
    #[serde(rename = "CFBundleShortVersionString")]
    product_version: String,
    #[serde(rename = "CFBundleVersion")]
    build_number: String,
}

fn inspect_info_plist(bundle_path: &Path) -> Result<RunningInfoPlist, MacOsInstallAdapterError> {
    let info_path = bundle_path.join("Contents/Info.plist");
    let output = Command::new("/usr/bin/plutil")
        .env_clear()
        .args(["-convert", "json", "-o", "-"])
        .arg(&info_path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))?;
    if !output.status.success()
        || !output.stderr.is_empty()
        || output.stdout.is_empty()
        || output.stdout.len() > MAX_INFO_PLIST_BYTES
    {
        return Err(error(MacOsInstallAdapterErrorCode::InvalidProductManifest));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::InvalidProductManifest))
}

pub struct MacOsProductInstallAdapter {
    payload: VerifiedInstallPayload,
    layout: VerifiedUserLayout,
    signature_verifier: Box<dyn MacOsCodeSignatureVerifier>,
    copier: Box<dyn BundleCopier>,
    target_product: ProductArtifactIdentity,
}

impl fmt::Debug for MacOsProductInstallAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MacOsProductInstallAdapter")
            .field("target_product", &self.target_product)
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

impl MacOsProductInstallAdapter {
    pub fn first_install_targets_are_absent(
        authoritative_user_home: &Path,
        expected_owner_id: u32,
    ) -> Result<bool, MacOsInstallAdapterError> {
        verify_directory(
            authoritative_user_home,
            expected_owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeUserHome,
        )?;
        let library = authoritative_user_home.join("Library");
        verify_directory(
            &library,
            expected_owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )?;
        verify_directory(
            &library.join("Application Support"),
            expected_owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )?;
        verify_optional_directory(
            &authoritative_user_home.join(MANAGER_PARENT),
            expected_owner_id,
        )?;
        verify_optional_directory(
            &authoritative_user_home.join(INPUT_METHOD_PARENT),
            expected_owner_id,
        )?;
        let manager = authoritative_user_home
            .join(MANAGER_PARENT)
            .join(radishlex_ime_product_install::MANAGER_BUNDLE_NAME);
        let input_method = authoritative_user_home
            .join(INPUT_METHOD_PARENT)
            .join(radishlex_ime_product_install::INPUT_METHOD_BUNDLE_NAME);
        Ok(path_is_absent(&manager)? && path_is_absent(&input_method)?)
    }

    pub fn inspect_payload_product(
        payload_root: &Path,
        requirements: CodeSignatureRequirements,
    ) -> Result<ProductArtifactIdentity, MacOsInstallAdapterError> {
        VerifiedInstallPayload::load(
            payload_root,
            &CodesignCodeSignatureVerifier::new(requirements),
        )
        .map(|payload| payload.target_product().clone())
    }

    pub fn load(
        payload_root: &Path,
        authoritative_user_home: &Path,
        expected_owner_id: u32,
        requirements: CodeSignatureRequirements,
    ) -> Result<Self, MacOsInstallAdapterError> {
        Self::load_with_services(
            payload_root,
            authoritative_user_home,
            expected_owner_id,
            Box::new(CodesignCodeSignatureVerifier::new(requirements)),
            Box::new(DittoBundleCopier),
        )
    }

    pub fn prepare_first_install(
        payload_root: &Path,
        authoritative_user_home: &Path,
        expected_owner_id: u32,
        requirements: CodeSignatureRequirements,
    ) -> Result<Self, MacOsInstallAdapterError> {
        prepare_user_layout(authoritative_user_home, expected_owner_id)?;
        Self::load(
            payload_root,
            authoritative_user_home,
            expected_owner_id,
            requirements,
        )
    }

    #[cfg(feature = "qualification-harness")]
    pub fn load_for_qualification(
        payload_root: &Path,
        synthetic_user_home: &Path,
        expected_owner_id: u32,
        qualification_root: &Path,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let qualification_root_input = qualification_root.to_path_buf();
        let qualification_root = verify_qualification_root(qualification_root)?;
        let payload_root = verify_qualification_path(
            &qualification_root_input,
            &qualification_root,
            payload_root,
            0o700,
        )?;
        let synthetic_user_home = verify_qualification_path(
            &qualification_root_input,
            &qualification_root,
            synthetic_user_home,
            0o700,
        )?;
        Self::load_with_services(
            &payload_root,
            &synthetic_user_home,
            expected_owner_id,
            Box::new(CodesignQualificationCodeSignatureVerifier),
            Box::new(DittoBundleCopier),
        )
    }

    pub fn target_product(&self) -> &ProductArtifactIdentity {
        &self.target_product
    }

    pub fn upgrade_source_product_root(&self, release: &ProductRelease) -> Option<&Path> {
        self.payload.upgrade_source_product_root(release)
    }

    pub fn open_install_store(&self) -> Result<InstallReceiptStore, MacOsInstallAdapterError> {
        self.layout.revalidate()?;
        let root =
            VerifiedInstallRoot::verify(self.layout.home.join(DATA_ROOT), self.layout.owner_id)
                .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeTarget))?;
        InstallReceiptStore::open(root)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))
    }

    pub fn verify_installed_product(
        &self,
        expected: Option<&ProductArtifactIdentity>,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.layout.revalidate()?;
        let manager = self.layout.target_path(ProgramComponent::Manager);
        let input_method = self.layout.target_path(ProgramComponent::InputMethod);
        let Some(expected) = expected else {
            if path_is_absent(&manager)? && path_is_absent(&input_method)? {
                return Ok(());
            }
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        };
        self.verify_program(&manager, expected.program(ProgramComponent::Manager))?;
        self.verify_program(
            &input_method,
            expected.program(ProgramComponent::InputMethod),
        )
    }

    pub fn revalidate_target_product(
        &self,
    ) -> Result<ProductArtifactIdentity, MacOsInstallAdapterError> {
        self.layout.revalidate()?;
        let actual = self.payload.revalidate(self.signature_verifier.as_ref())?;
        if actual != self.target_product {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        Ok(actual)
    }

    pub fn open_program_store(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        component: ProgramComponent,
        receipt: &InstallReceipt,
    ) -> Result<ProgramSwitchStore, MacOsInstallAdapterError> {
        self.layout.revalidate()?;
        receipt_store
            .verify_guard(guard)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))?;
        let root = receipt_store.root_identity();
        if root != receipt.root_identity() || !self.layout.matches_data_root(root) {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        ProgramSwitchStore::open(
            self.layout.program_target(component)?,
            receipt.operation_id(),
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))
    }

    pub fn verify_and_record_source(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_common_binding(receipt_store, guard, program_store, receipt)?;
        if receipt.state() != InstallState::Quiesced
            || receipt.operation_kind() == InstallOperationKind::FirstInstall
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        let expected = receipt
            .source_product()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?
            .program(program_store.component());
        self.verify_program(program_store.target_path(), expected)?;
        record_core_program_source(receipt_store, guard, program_store, receipt)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))
    }

    pub fn prepare_and_record_staged(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_common_binding(receipt_store, guard, program_store, receipt)?;
        if receipt.state() != InstallState::Quiesced
            || receipt.operation_kind() == InstallOperationKind::RemovePrograms
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        let target = self.revalidate_target_product()?;
        if receipt.target_product() != Some(&target) {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        let expected = target.program(program_store.component());
        let staged_path = program_store.staged_bundle_path();
        match fs::symlink_metadata(staged_path) {
            Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
                self.copier.copy_bundle(
                    self.payload.bundle_path(program_store.component()),
                    staged_path,
                )?;
            }
            Ok(_) => {}
            Err(_) => return Err(error(MacOsInstallAdapterErrorCode::Io)),
        }
        self.verify_program(staged_path, expected)?;
        self.copier.verify_staged_bundle_metadata(staged_path)?;
        self.verify_program(staged_path, expected)?;
        sync_bundle_tree(staged_path)?;
        sync_parent(staged_path)?;
        self.verify_program(staged_path, expected)?;
        record_core_staged_program(receipt_store, guard, program_store, receipt)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))
    }

    pub fn verify_installed_target(
        &self,
        program_store: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_store_binding(program_store, receipt)?;
        let expected = receipt
            .target_product()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?
            .program(program_store.component());
        self.verify_program(program_store.target_path(), expected)
    }

    pub fn verify_restored_source(
        &self,
        program_store: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_store_binding(program_store, receipt)?;
        let expected = receipt
            .source_product()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?
            .program(program_store.component());
        self.verify_program(program_store.target_path(), expected)
    }

    fn load_with_services(
        payload_root: &Path,
        authoritative_user_home: &Path,
        expected_owner_id: u32,
        signature_verifier: Box<dyn MacOsCodeSignatureVerifier>,
        copier: Box<dyn BundleCopier>,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let layout = VerifiedUserLayout::load(authoritative_user_home, expected_owner_id)?;
        let payload = VerifiedInstallPayload::load(payload_root, signature_verifier.as_ref())?;
        let target_product = payload.target_product().clone();
        Ok(Self {
            payload,
            layout,
            signature_verifier,
            copier,
            target_product,
        })
    }

    fn verify_common_binding(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        receipt_store
            .verify_guard(guard)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))?;
        self.verify_store_binding(program_store, receipt)
    }

    fn verify_store_binding(
        &self,
        program_store: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.layout.revalidate()?;
        if !self.layout.matches_data_root(receipt.root_identity())
            || receipt.operation_id() != program_store.operation_id()
            || program_store.target_path()
                != self.layout.target_path(program_store.component()).as_path()
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        Ok(())
    }

    fn verify_program(
        &self,
        bundle_path: &Path,
        expected: &ProgramBundleIdentity,
    ) -> Result<(), MacOsInstallAdapterError> {
        let tree = inspect_bundle_tree(bundle_path)?;
        if tree.sha256() != expected.bundle_tree_sha256() {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        let code_identity = self.signature_verifier.verify(
            bundle_path,
            expected.component(),
            expected.bundle_id(),
        )?;
        let confirmed_tree = inspect_bundle_tree(bundle_path)?;
        if confirmed_tree.sha256() != tree.sha256() {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        if code_identity.bundle_id() != expected.bundle_id()
            || code_identity.evidence_sha256() != expected.code_identity_sha256()
        {
            return Err(error(
                MacOsInstallAdapterErrorCode::SignatureIdentityChanged,
            ));
        }
        Ok(())
    }
}

fn prepare_user_layout(home: &Path, owner_id: u32) -> Result<(), MacOsInstallAdapterError> {
    verify_directory(
        home,
        owner_id,
        None,
        MacOsInstallAdapterErrorCode::UnsafeUserHome,
    )?;
    let library = home.join("Library");
    verify_directory(
        &library,
        owner_id,
        None,
        MacOsInstallAdapterErrorCode::UnsafeTarget,
    )?;
    let application_support = library.join("Application Support");
    verify_directory(
        &application_support,
        owner_id,
        None,
        MacOsInstallAdapterErrorCode::UnsafeTarget,
    )?;
    ensure_private_directory(&home.join(MANAGER_PARENT), home, owner_id, None)?;
    ensure_private_directory(&home.join(INPUT_METHOD_PARENT), &library, owner_id, None)?;
    ensure_private_directory(
        &home.join(DATA_ROOT),
        &application_support,
        owner_id,
        Some(0o700),
    )?;
    Ok(())
}

fn ensure_private_directory(
    path: &Path,
    parent: &Path,
    owner_id: u32,
    exact_mode: Option<u32>,
) -> Result<(), MacOsInstallAdapterError> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => sync_directory(parent)?,
        Err(io_error) if io_error.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return Err(error(MacOsInstallAdapterErrorCode::Io)),
    }
    verify_directory(
        path,
        owner_id,
        exact_mode,
        MacOsInstallAdapterErrorCode::UnsafeTarget,
    )
    .map(|_| ())
}

fn path_is_absent(path: &Path) -> Result<bool, MacOsInstallAdapterError> {
    match fs::symlink_metadata(path) {
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(error(MacOsInstallAdapterErrorCode::Io)),
    }
}

fn verify_optional_directory(path: &Path, owner_id: u32) -> Result<(), MacOsInstallAdapterError> {
    match fs::symlink_metadata(path) {
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(error(MacOsInstallAdapterErrorCode::Io)),
        Ok(_) => verify_directory(
            path,
            owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )
        .map(|_| ()),
    }
}

fn sync_directory(path: &Path) -> Result<(), MacOsInstallAdapterError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))
}

impl InstallProgramValidationPort for MacOsProductInstallAdapter {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.verify_installed_target(manager, receipt).is_ok()
            && self.verify_installed_target(input_method, receipt).is_ok()
    }

    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.verify_restored_source(manager, receipt).is_ok()
            && self.verify_restored_source(input_method, receipt).is_ok()
    }
}

impl InstallFinalizationPort for MacOsProductInstallAdapter {
    fn validate_final_state(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        _stage: InstallFinalizationValidationStage,
    ) -> bool {
        if self.verify_store_binding(manager, receipt).is_err()
            || self.verify_store_binding(input_method, receipt).is_err()
        {
            return false;
        }
        if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
            return [manager, input_method].iter().all(|store| {
                fs::symlink_metadata(store.target_path())
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            });
        }
        self.validate_installed_targets(manager, input_method, receipt)
    }
}

#[derive(Debug)]
struct VerifiedUserLayout {
    home: PathBuf,
    owner_id: u32,
    home_identity: DirectoryIdentity,
    manager_parent_identity: DirectoryIdentity,
    input_method_parent_identity: DirectoryIdentity,
    data_root_identity: DirectoryIdentity,
}

impl VerifiedUserLayout {
    fn load(home: &Path, owner_id: u32) -> Result<Self, MacOsInstallAdapterError> {
        let home_identity = verify_directory(
            home,
            owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeUserHome,
        )?;
        let manager_parent = home.join(MANAGER_PARENT);
        let input_method_parent = home.join(INPUT_METHOD_PARENT);
        let data_root = home.join(DATA_ROOT);
        let manager_parent_identity = verify_directory(
            &manager_parent,
            owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )?;
        let input_method_parent_identity = verify_directory(
            &input_method_parent,
            owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )?;
        let data_root_identity = verify_directory(
            &data_root,
            owner_id,
            Some(0o700),
            MacOsInstallAdapterErrorCode::UnsafeTarget,
        )?;
        Ok(Self {
            home: home.to_path_buf(),
            owner_id,
            home_identity,
            manager_parent_identity,
            input_method_parent_identity,
            data_root_identity,
        })
    }

    fn revalidate(&self) -> Result<(), MacOsInstallAdapterError> {
        if verify_directory(
            &self.home,
            self.owner_id,
            None,
            MacOsInstallAdapterErrorCode::UnsafeUserHome,
        )? != self.home_identity
            || verify_directory(
                &self.home.join(MANAGER_PARENT),
                self.owner_id,
                None,
                MacOsInstallAdapterErrorCode::UnsafeTarget,
            )? != self.manager_parent_identity
            || verify_directory(
                &self.home.join(INPUT_METHOD_PARENT),
                self.owner_id,
                None,
                MacOsInstallAdapterErrorCode::UnsafeTarget,
            )? != self.input_method_parent_identity
            || verify_directory(
                &self.home.join(DATA_ROOT),
                self.owner_id,
                Some(0o700),
                MacOsInstallAdapterErrorCode::UnsafeTarget,
            )? != self.data_root_identity
        {
            return Err(error(MacOsInstallAdapterErrorCode::UnsafeTarget));
        }
        Ok(())
    }

    fn program_target(
        &self,
        component: ProgramComponent,
    ) -> Result<VerifiedProgramTarget, MacOsInstallAdapterError> {
        let parent = match component {
            ProgramComponent::Manager => self.home.join(MANAGER_PARENT),
            ProgramComponent::InputMethod => self.home.join(INPUT_METHOD_PARENT),
        };
        VerifiedProgramTarget::verify(parent, component, self.owner_id)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeTarget))
    }

    fn target_path(&self, component: ProgramComponent) -> PathBuf {
        let bundle = match component {
            ProgramComponent::Manager => radishlex_ime_product_install::MANAGER_BUNDLE_NAME,
            ProgramComponent::InputMethod => {
                radishlex_ime_product_install::INPUT_METHOD_BUNDLE_NAME
            }
        };
        match component {
            ProgramComponent::Manager => self.home.join(MANAGER_PARENT).join(bundle),
            ProgramComponent::InputMethod => self.home.join(INPUT_METHOD_PARENT).join(bundle),
        }
    }

    fn matches_data_root(&self, identity: &InstallRootIdentity) -> bool {
        identity.device_id() == self.data_root_identity.device_id
            && identity.inode() == self.data_root_identity.inode
            && identity.owner_id() == self.owner_id
            && identity.mode() == 0o700
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    device_id: u64,
    inode: u64,
}

fn verify_directory(
    path: &Path,
    owner_id: u32,
    exact_mode: Option<u32>,
    code: MacOsInstallAdapterErrorCode,
) -> Result<DirectoryIdentity, MacOsInstallAdapterError> {
    if !path.is_absolute() {
        return Err(error(code));
    }
    let canonical = fs::canonicalize(path).map_err(|_| error(code))?;
    if canonical != path {
        return Err(error(code));
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| error(code))?;
    let mode = metadata.permissions().mode() & 0o7777;
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.ino() == 0
        || exact_mode.map_or(mode & 0o022 != 0, |expected| mode != expected)
    {
        return Err(error(code));
    }
    Ok(DirectoryIdentity {
        device_id: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn sync_parent(path: &Path) -> Result<(), MacOsInstallAdapterError> {
    let parent = path
        .parent()
        .ok_or_else(|| error(MacOsInstallAdapterErrorCode::Io))?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error(MacOsInstallAdapterErrorCode::Io))
}

#[cfg(feature = "qualification-harness")]
fn verify_qualification_root(root: &Path) -> Result<PathBuf, MacOsInstallAdapterError> {
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    let temp_root = fs::canonicalize(std::env::temp_dir())
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    let root = fs::canonicalize(root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if root == temp_root || !root.starts_with(&temp_root) {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    let owner_id = verify_private_directory(&root)?;
    let marker = root.join(QUALIFICATION_MARKER_FILE);
    let metadata = fs::symlink_metadata(&marker)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != owner_id
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
        || fs::read(&marker)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?
            != QUALIFICATION_MARKER_BYTES
    {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    Ok(root)
}

#[cfg(feature = "qualification-harness")]
fn verify_qualification_path(
    qualification_root_input: &Path,
    qualification_root: &Path,
    path: &Path,
    expected_mode: u32,
) -> Result<PathBuf, MacOsInstallAdapterError> {
    let relative = path
        .strip_prefix(qualification_root_input)
        .or_else(|_| path.strip_prefix(qualification_root))
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if relative.as_os_str().is_empty() {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    let mut current = if path.starts_with(qualification_root_input) {
        qualification_root_input.to_path_buf()
    } else {
        qualification_root.to_path_buf()
    };
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
        if metadata.file_type().is_symlink() {
            return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
        }
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if canonical == qualification_root || !canonical.starts_with(qualification_root) {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    let owner_id = fs::symlink_metadata(qualification_root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?
        .uid();
    let mut current = qualification_root.to_path_buf();
    let relative = canonical
        .strip_prefix(qualification_root)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        if !matches!(component, Component::Normal(_)) {
            return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != owner_id
            || (index + 1 < components.len() && !metadata.is_dir())
        {
            return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
        }
    }
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if !metadata.is_dir() || metadata.mode() & 0o7777 != expected_mode {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    Ok(canonical)
}

#[cfg(feature = "qualification-harness")]
fn verify_private_directory(path: &Path) -> Result<u32, MacOsInstallAdapterError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::UnsafeQualification))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.mode() & 0o7777 != 0o700
    {
        return Err(error(MacOsInstallAdapterErrorCode::UnsafeQualification));
    }
    Ok(metadata.uid())
}

const fn error(code: MacOsInstallAdapterErrorCode) -> MacOsInstallAdapterError {
    MacOsInstallAdapterError::new(code)
}

#[cfg(test)]
mod tests;
