use std::fmt;
use std::path::PathBuf;

use crate::model::{DpkgPackageState, LinuxArtifactIdentity, LinuxInstallState};

const SYSTEM_DPKG_STATUS_PATH: &str = "/var/lib/dpkg/status";
const SYSTEM_PRODUCT_MANIFEST_PATH: &str = "/usr/share/radishlex/product-manifest.json";
const SYSTEM_MANAGER_PATH: &str = "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager";
const SYSTEM_MANAGER_FFI_PATH: &str =
    "/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/libradishlex_ime_ffi.so";
const SYSTEM_FCITX_ADDON_PATH: &str = "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so";
const SYSTEM_FCITX_FFI_PATH: &str = "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so";
const SYSTEM_RIME_DATA_ROOT: &str = "/usr/share/radishlex/rime";
const SYSTEM_FCITX_ADDON_METADATA_PATH: &str = "/usr/share/fcitx5/addon/radishlex.conf";
const SYSTEM_FCITX_INPUT_METHOD_METADATA_PATH: &str =
    "/usr/share/fcitx5/inputmethod/radishlex.conf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LinuxStartupBuildIdentity {
    DevelopmentStaged = 1,
    DebianSystemProduct = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LinuxStartupComponent {
    Manager = 1,
    FcitxAddon = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LinuxStartupDecision {
    AllowedDevelopment = 1,
    AllowedProduct = 2,
    MaintenanceRequired = 3,
    FailedClosed = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LinuxStartupReason {
    DevelopmentStateAbsent = 1,
    InstalledReceiptVerified = 2,
    ActiveGuard = 10,
    GuardInvalid = 11,
    InterruptedReceipt = 12,
    InterruptedReceiptInvalid = 13,
    OperationInProgress = 14,
    ReceiptMissing = 15,
    ReceiptInvalid = 16,
    UnexpectedStateObject = 17,
    RootIdentityChanged = 18,
    PackageStateUnavailable = 19,
    PackageStateUnknown = 20,
    PackageStateIncomplete = 21,
    UnmanagedPackageState = 22,
    PackageIdentityChanged = 23,
    RemovedProgram = 24,
    ProductNotInstalled = 25,
    ComponentIdentityChanged = 26,
    DependencyUnavailable = 27,
    DevelopmentIsolationViolation = 28,
    PermissionDenied = 29,
    Io = 30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxStartupOutcome {
    decision: LinuxStartupDecision,
    reason: LinuxStartupReason,
    receipt_state: Option<LinuxInstallState>,
}

impl LinuxStartupOutcome {
    pub const fn decision(self) -> LinuxStartupDecision {
        self.decision
    }

    pub const fn reason(self) -> LinuxStartupReason {
        self.reason
    }

    pub const fn receipt_state(self) -> Option<LinuxInstallState> {
        self.receipt_state
    }

    pub const fn receipt_state_code(self) -> u32 {
        match self.receipt_state {
            None => 0,
            Some(LinuxInstallState::Prepared) => 1,
            Some(LinuxInstallState::ArtifactsStaged) => 2,
            Some(LinuxInstallState::Quiesced) => 3,
            Some(LinuxInstallState::PackageMutating) => 4,
            Some(LinuxInstallState::PackageVerified) => 5,
            Some(LinuxInstallState::Completed) => 6,
            Some(LinuxInstallState::AbortedPreserved) => 7,
            Some(LinuxInstallState::RollbackRequired) => 8,
            Some(LinuxInstallState::SourceRestoring) => 9,
            Some(LinuxInstallState::SourceVerified) => 10,
            Some(LinuxInstallState::RolledBack) => 11,
        }
    }

    pub(super) const fn new(
        decision: LinuxStartupDecision,
        reason: LinuxStartupReason,
        receipt_state: Option<LinuxInstallState>,
    ) -> Self {
        Self {
            decision,
            reason,
            receipt_state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxPackageObservation {
    state: DpkgPackageState,
    package_version: Option<String>,
    architecture: Option<String>,
}

impl LinuxPackageObservation {
    pub fn new(
        state: DpkgPackageState,
        package_version: Option<String>,
        architecture: Option<String>,
    ) -> Result<Self, LinuxStartupPortError> {
        let observation = Self {
            state,
            package_version,
            architecture,
        };
        observation.validate()?;
        Ok(observation)
    }

    pub fn not_installed() -> Self {
        Self {
            state: DpkgPackageState::NotInstalled,
            package_version: None,
            architecture: None,
        }
    }

    pub fn installed(artifact: &LinuxArtifactIdentity) -> Self {
        Self {
            state: DpkgPackageState::Installed,
            package_version: Some(artifact.package_version().to_owned()),
            architecture: Some(artifact.architecture().to_owned()),
        }
    }

    pub const fn state(&self) -> DpkgPackageState {
        self.state
    }

    fn validate(&self) -> Result<(), LinuxStartupPortError> {
        let carries_identity = self.package_version.is_some() || self.architecture.is_some();
        if self.package_version.is_some() != self.architecture.is_some()
            || (self.state.is_recoverable() && !carries_identity)
            || ((self.state.is_absent() || self.state == DpkgPackageState::Unknown)
                && carries_identity)
        {
            return Err(LinuxStartupPortError::new(
                LinuxStartupPortErrorCode::PackageStateUnknown,
                "package observation fields are inconsistent",
            ));
        }
        Ok(())
    }

    pub(super) fn matches(&self, artifact: &LinuxArtifactIdentity) -> bool {
        self.state == DpkgPackageState::Installed
            && self.package_version.as_deref() == Some(artifact.package_version())
            && self.architecture.as_deref() == Some(artifact.architecture())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxStartupPortErrorCode {
    PackageStateUnavailable,
    PackageStateUnknown,
    PackageIdentityChanged,
    ComponentIdentityChanged,
    DependencyUnavailable,
    PermissionDenied,
    Io,
}

#[derive(Debug)]
pub struct LinuxStartupPortError {
    code: LinuxStartupPortErrorCode,
    message: String,
}

impl LinuxStartupPortError {
    pub fn new(code: LinuxStartupPortErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> LinuxStartupPortErrorCode {
        self.code
    }
}

impl fmt::Display for LinuxStartupPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LinuxStartupPortError {}

pub trait LinuxStartupPort {
    fn inspect_package(&self) -> Result<LinuxPackageObservation, LinuxStartupPortError>;

    fn validate_component(
        &self,
        component: LinuxStartupComponent,
        artifact: &LinuxArtifactIdentity,
    ) -> Result<(), LinuxStartupPortError>;
}

#[derive(Debug, Clone)]
pub struct LinuxStartupPaths {
    pub(super) state_root: PathBuf,
    pub(super) guard_path: PathBuf,
    pub(super) dpkg_status_path: PathBuf,
    pub(super) product_manifest_path: PathBuf,
    pub(super) manager_component_path: PathBuf,
    pub(super) manager_ffi_path: PathBuf,
    pub(super) fcitx_component_path: PathBuf,
    pub(super) fcitx_ffi_path: PathBuf,
    pub(super) rime_data_root: PathBuf,
    pub(super) fcitx_addon_metadata_path: PathBuf,
    pub(super) fcitx_input_method_metadata_path: PathBuf,
    pub(super) expected_owner_id: u32,
    pub(super) expected_group_id: u32,
}

impl LinuxStartupPaths {
    pub fn system() -> Self {
        Self {
            state_root: PathBuf::from(crate::store::SYSTEM_STATE_ROOT),
            guard_path: PathBuf::from(crate::store::SYSTEM_GUARD_PATH),
            dpkg_status_path: PathBuf::from(SYSTEM_DPKG_STATUS_PATH),
            product_manifest_path: PathBuf::from(SYSTEM_PRODUCT_MANIFEST_PATH),
            manager_component_path: PathBuf::from(SYSTEM_MANAGER_PATH),
            manager_ffi_path: PathBuf::from(SYSTEM_MANAGER_FFI_PATH),
            fcitx_component_path: PathBuf::from(SYSTEM_FCITX_ADDON_PATH),
            fcitx_ffi_path: PathBuf::from(SYSTEM_FCITX_FFI_PATH),
            rime_data_root: PathBuf::from(SYSTEM_RIME_DATA_ROOT),
            fcitx_addon_metadata_path: PathBuf::from(SYSTEM_FCITX_ADDON_METADATA_PATH),
            fcitx_input_method_metadata_path: PathBuf::from(
                SYSTEM_FCITX_INPUT_METHOD_METADATA_PATH,
            ),
            expected_owner_id: 0,
            expected_group_id: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_testing(
        root: PathBuf,
        expected_owner_id: u32,
        expected_group_id: u32,
    ) -> Self {
        Self {
            state_root: root.join("install-v1"),
            guard_path: root.join("install-v1.lock"),
            dpkg_status_path: root.join("dpkg-status"),
            product_manifest_path: root.join("product-manifest.json"),
            manager_component_path: root.join("components/manager/radishlex_manager"),
            manager_ffi_path: root.join("components/manager/lib/libradishlex_ime_ffi.so"),
            fcitx_component_path: root.join("components/fcitx/radishlex.so"),
            fcitx_ffi_path: root.join("components/fcitx/libradishlex_ime_ffi.so"),
            rime_data_root: root.join("components/rime"),
            fcitx_addon_metadata_path: root.join("metadata/addon/radishlex.conf"),
            fcitx_input_method_metadata_path: root.join("metadata/inputmethod/radishlex.conf"),
            expected_owner_id,
            expected_group_id,
        }
    }
}
