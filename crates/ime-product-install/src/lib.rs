//! Product program installation transaction contracts for RadishLex.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

mod artifact;
pub use artifact::{InstallArtifactEvidence, InstallArtifactSlot, ProgramFilesystemIdentity};
#[cfg(unix)]
mod filesystem;
#[cfg(unix)]
mod finalization;
#[cfg(unix)]
mod program_switch;
#[cfg(unix)]
pub use filesystem::{
    inspect_install_startup_gate, inspect_install_startup_gate_with, inspect_install_status,
    InstallFilesystemError, InstallFilesystemErrorCode, InstallProcessGuard, InstallReceiptStore,
    VerifiedInstallRoot,
};
#[cfg(unix)]
pub use finalization::{
    resume_install_finalization, InstallFinalizationError, InstallFinalizationPort,
    InstallFinalizationValidationStage,
};
#[cfg(unix)]
pub use program_switch::{
    commit_program_removal, commit_program_target, commit_program_target_with_faults,
    finish_program_restore, finish_source_preservation, finish_target_staging,
    preserve_program_source, preserve_program_source_with_faults, record_program_source,
    record_staged_program, restore_program_source, restore_program_source_with_faults,
    NoProgramSwitchFaults, ProgramSwitchAction, ProgramSwitchBoundary, ProgramSwitchError,
    ProgramSwitchErrorCode, ProgramSwitchFaultInjector, ProgramSwitchFaultPoint,
    ProgramSwitchStore, VerifiedProgramTarget, INPUT_METHOD_BUNDLE_NAME, MANAGER_BUNDLE_NAME,
};

#[cfg(unix)]
pub trait InstallProgramValidationPort {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool;

    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool;
}

pub const INSTALL_RECEIPT_FORMAT: &str = "radishlex-product-install-receipt-v1";
pub const INSTALL_PRODUCT_ID: &str = "radishlex-macos";
pub const MAX_INSTALL_RECEIPT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallOperationKind {
    FirstInstall,
    Upgrade,
    Repair,
    RemovePrograms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    Prepared,
    Quiesced,
    TargetStaged,
    SourcePreserved,
    ManagerCommitted,
    ProgramsCommitted,
    DataCoordinating,
    DataSettled,
    FinalVerified,
    Completed,
    AbortedPreserved,
    RollbackRequired,
    ProgramsRestored,
    RolledBack,
}

impl InstallState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::AbortedPreserved | Self::RolledBack
        )
    }

    const fn is_before_program_commit(self) -> bool {
        matches!(
            self,
            Self::Prepared | Self::Quiesced | Self::TargetStaged | Self::SourcePreserved
        )
    }

    const fn requires_program_rollback(self) -> bool {
        matches!(
            self,
            Self::ManagerCommitted
                | Self::ProgramsCommitted
                | Self::DataCoordinating
                | Self::DataSettled
                | Self::FinalVerified
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallFailureCode {
    UnsupportedReceipt,
    ReceiptInconsistent,
    OperationAlreadyActive,
    UnsafeDataRoot,
    UnexpectedStateObject,
    IdentityChanged,
    ProcessNotQuiescent,
    InsufficientSpace,
    SourceArtifactInvalid,
    TargetArtifactInvalid,
    StagingFailed,
    PreserveFailed,
    ManagerCommitFailed,
    InputMethodCommitFailed,
    DataCoordinationFailed,
    TargetValidationFailed,
    RollbackFailed,
    PermissionDenied,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductRelease {
    product_version: String,
    build_number: u64,
}

impl ProductRelease {
    pub fn new(
        product_version: impl Into<String>,
        build_number: u64,
    ) -> Result<Self, InstallReceiptError> {
        let release = Self {
            product_version: product_version.into(),
            build_number,
        };
        release.validate("release")?;
        Ok(release)
    }

    pub fn product_version(&self) -> &str {
        &self.product_version
    }

    pub const fn build_number(&self) -> u64 {
        self.build_number
    }

    fn validate(&self, field: &'static str) -> Result<(), InstallReceiptError> {
        parse_version(&self.product_version).ok_or_else(|| {
            InstallReceiptError::invalid(
                field,
                "product version must use numeric major.minor.patch",
            )
        })?;
        if self.build_number == 0 {
            return Err(InstallReceiptError::invalid(
                field,
                "build number must be positive",
            ));
        }
        Ok(())
    }

    fn release_cmp(&self, other: &Self) -> Ordering {
        let left = parse_version(&self.product_version).expect("validated release");
        let right = parse_version(&other.product_version).expect("validated release");
        left.cmp(&right)
            .then_with(|| self.build_number.cmp(&other.build_number))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramComponent {
    Manager,
    InputMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramBundleIdentity {
    component: ProgramComponent,
    bundle_id: String,
    bundle_tree_sha256: String,
    code_identity_sha256: String,
}

impl ProgramBundleIdentity {
    pub fn new(
        component: ProgramComponent,
        bundle_id: impl Into<String>,
        bundle_tree_sha256: impl Into<String>,
        code_identity_sha256: impl Into<String>,
    ) -> Result<Self, InstallReceiptError> {
        let identity = Self {
            component,
            bundle_id: bundle_id.into(),
            bundle_tree_sha256: bundle_tree_sha256.into(),
            code_identity_sha256: code_identity_sha256.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn component(&self) -> ProgramComponent {
        self.component
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    pub fn bundle_tree_sha256(&self) -> &str {
        &self.bundle_tree_sha256
    }

    pub fn code_identity_sha256(&self) -> &str {
        &self.code_identity_sha256
    }

    fn validate(&self) -> Result<(), InstallReceiptError> {
        if !valid_bundle_id(&self.bundle_id) {
            return Err(InstallReceiptError::invalid(
                "bundle_identity",
                "bundle ID is invalid",
            ));
        }
        validate_sha256(&self.bundle_tree_sha256, "bundle_tree_sha256")?;
        validate_sha256(&self.code_identity_sha256, "code_identity_sha256")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductArtifactIdentity {
    product_id: String,
    release: ProductRelease,
    product_manifest_sha256: String,
    manager: ProgramBundleIdentity,
    input_method: ProgramBundleIdentity,
}

impl ProductArtifactIdentity {
    pub fn new(
        product_id: impl Into<String>,
        release: ProductRelease,
        product_manifest_sha256: impl Into<String>,
        manager: ProgramBundleIdentity,
        input_method: ProgramBundleIdentity,
    ) -> Result<Self, InstallReceiptError> {
        let identity = Self {
            product_id: product_id.into(),
            release,
            product_manifest_sha256: product_manifest_sha256.into(),
            manager,
            input_method,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    pub fn release(&self) -> &ProductRelease {
        &self.release
    }

    pub fn product_manifest_sha256(&self) -> &str {
        &self.product_manifest_sha256
    }

    pub fn program(&self, component: ProgramComponent) -> &ProgramBundleIdentity {
        match component {
            ProgramComponent::Manager => &self.manager,
            ProgramComponent::InputMethod => &self.input_method,
        }
    }

    fn validate(&self) -> Result<(), InstallReceiptError> {
        if self.product_id != INSTALL_PRODUCT_ID {
            return Err(InstallReceiptError::invalid(
                "product_identity",
                "product ID differs from the accepted macOS product",
            ));
        }
        self.release.validate("product_identity")?;
        validate_sha256(&self.product_manifest_sha256, "product_manifest_sha256")?;
        self.manager.validate()?;
        self.input_method.validate()?;
        if self.manager.component != ProgramComponent::Manager
            || self.input_method.component != ProgramComponent::InputMethod
            || self.manager.bundle_id == self.input_method.bundle_id
        {
            return Err(InstallReceiptError::invalid(
                "product_identity",
                "program component identities are inconsistent",
            ));
        }
        Ok(())
    }

    fn compatible_with(&self, other: &Self) -> bool {
        self.product_id == other.product_id
            && self.manager.bundle_id == other.manager.bundle_id
            && self.input_method.bundle_id == other.input_method.bundle_id
    }

    fn matches_running(&self, running: &RunningProgramIdentity) -> bool {
        self.release == running.release && self.program(running.bundle.component) == &running.bundle
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProgramIdentity {
    release: ProductRelease,
    bundle: ProgramBundleIdentity,
}

impl RunningProgramIdentity {
    pub fn new(
        release: ProductRelease,
        bundle: ProgramBundleIdentity,
    ) -> Result<Self, InstallReceiptError> {
        let identity = Self { release, bundle };
        identity.release.validate("running_program")?;
        identity.bundle.validate()?;
        Ok(identity)
    }

    pub const fn component(&self) -> ProgramComponent {
        self.bundle.component
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRootIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    mode: u32,
}

impl InstallRootIdentity {
    pub fn new(
        device_id: u64,
        inode: u64,
        owner_id: u32,
        mode: u32,
    ) -> Result<Self, InstallReceiptError> {
        let identity = Self {
            device_id,
            inode,
            owner_id,
            mode,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn device_id(&self) -> u64 {
        self.device_id
    }

    pub const fn inode(&self) -> u64 {
        self.inode
    }

    pub const fn owner_id(&self) -> u32 {
        self.owner_id
    }

    pub const fn mode(&self) -> u32 {
        self.mode
    }

    fn validate(&self) -> Result<(), InstallReceiptError> {
        if self.inode == 0 || self.mode != 0o700 {
            return Err(InstallReceiptError::invalid(
                "root_identity",
                "install root identity requires a positive inode and mode 0700",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallReceipt {
    receipt_format: String,
    operation_id: String,
    previous_operation_id: Option<String>,
    operation_kind: InstallOperationKind,
    root_identity: InstallRootIdentity,
    source_product: Option<ProductArtifactIdentity>,
    target_product: Option<ProductArtifactIdentity>,
    state: InstallState,
    failure_code: Option<InstallFailureCode>,
    failure_after_state: Option<InstallState>,
    manual_recovery_required: bool,
    artifacts: Vec<InstallArtifactEvidence>,
}

impl InstallReceipt {
    pub fn new(
        operation_id: impl Into<String>,
        previous_operation_id: Option<String>,
        operation_kind: InstallOperationKind,
        root_identity: InstallRootIdentity,
        source_product: Option<ProductArtifactIdentity>,
        target_product: Option<ProductArtifactIdentity>,
    ) -> Result<Self, InstallReceiptError> {
        let receipt = Self {
            receipt_format: INSTALL_RECEIPT_FORMAT.to_owned(),
            operation_id: operation_id.into(),
            previous_operation_id,
            operation_kind,
            root_identity,
            source_product,
            target_product,
            state: InstallState::Prepared,
            failure_code: None,
            failure_after_state: None,
            manual_recovery_required: false,
            artifacts: Vec::new(),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub const fn operation_kind(&self) -> InstallOperationKind {
        self.operation_kind
    }

    pub fn root_identity(&self) -> &InstallRootIdentity {
        &self.root_identity
    }

    pub fn source_product(&self) -> Option<&ProductArtifactIdentity> {
        self.source_product.as_ref()
    }

    pub fn target_product(&self) -> Option<&ProductArtifactIdentity> {
        self.target_product.as_ref()
    }

    pub fn installed_product(&self) -> Option<&ProductArtifactIdentity> {
        match self.state {
            InstallState::Completed => self.target_product.as_ref(),
            InstallState::AbortedPreserved | InstallState::RolledBack => {
                self.source_product.as_ref()
            }
            _ => None,
        }
    }

    pub const fn state(&self) -> InstallState {
        self.state
    }

    pub const fn failure_code(&self) -> Option<InstallFailureCode> {
        self.failure_code
    }

    pub const fn manual_recovery_required(&self) -> bool {
        self.manual_recovery_required
    }

    pub fn artifacts(&self) -> &[InstallArtifactEvidence] {
        &self.artifacts
    }

    pub fn artifact(&self, slot: InstallArtifactSlot) -> Option<&InstallArtifactEvidence> {
        self.artifacts.iter().find(|item| item.slot == slot)
    }

    pub fn record_artifact(
        &mut self,
        evidence: InstallArtifactEvidence,
    ) -> Result<(), InstallReceiptError> {
        self.ensure_open()?;
        if self
            .artifacts
            .iter()
            .any(|existing| existing.slot == evidence.slot)
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "artifact slot is already recorded",
            ));
        }
        self.validate_artifact_for_operation(&evidence)?;
        self.validate_artifact_recording_state(evidence.slot)?;
        let mut next_artifacts = self.artifacts.clone();
        next_artifacts.push(evidence);
        next_artifacts.sort_by_key(|item| item.slot);
        let previous_artifacts = std::mem::replace(&mut self.artifacts, next_artifacts);
        if let Err(validation_error) = self.validate() {
            self.artifacts = previous_artifacts;
            return Err(validation_error);
        }
        Ok(())
    }

    pub fn advance(&mut self, next: InstallState) -> Result<(), InstallReceiptError> {
        self.ensure_open()?;
        if !normal_transition(self.operation_kind, self.state, next) {
            return Err(InstallReceiptError::invalid(
                "state",
                "install state transition is not allowed",
            ));
        }
        self.validate_evidence_for_state(next)?;
        self.state = next;
        self.validate()
    }

    pub fn abort_preserved(
        &mut self,
        failure_code: InstallFailureCode,
    ) -> Result<(), InstallReceiptError> {
        self.ensure_open()?;
        if !self.state.is_before_program_commit() {
            return Err(InstallReceiptError::invalid(
                "state",
                "program changes require rollback",
            ));
        }
        self.failure_after_state = Some(self.state);
        self.failure_code = Some(failure_code);
        self.manual_recovery_required = false;
        self.state = InstallState::AbortedPreserved;
        self.validate()
    }

    pub fn require_rollback(
        &mut self,
        failure_code: InstallFailureCode,
    ) -> Result<(), InstallReceiptError> {
        self.ensure_open()?;
        if !self.state.requires_program_rollback() {
            return Err(InstallReceiptError::invalid(
                "state",
                "rollback requires a committed program change",
            ));
        }
        self.failure_after_state = Some(self.state);
        self.failure_code = Some(failure_code);
        self.manual_recovery_required = true;
        self.state = InstallState::RollbackRequired;
        self.validate()
    }

    pub fn mark_programs_restored(&mut self) -> Result<(), InstallReceiptError> {
        if self.state != InstallState::RollbackRequired {
            return Err(InstallReceiptError::invalid(
                "state",
                "program restore requires rollback_required",
            ));
        }
        self.state = InstallState::ProgramsRestored;
        self.validate()
    }

    pub fn mark_rolled_back(&mut self) -> Result<(), InstallReceiptError> {
        if self.state != InstallState::ProgramsRestored {
            return Err(InstallReceiptError::invalid(
                "state",
                "rollback completion requires restored programs",
            ));
        }
        self.manual_recovery_required = false;
        self.state = InstallState::RolledBack;
        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, InstallReceiptError> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self)
            .map_err(|_| InstallReceiptError::invalid("receipt", "cannot encode receipt"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_INSTALL_RECEIPT_BYTES {
            return Err(InstallReceiptError::invalid(
                "receipt",
                "receipt exceeds size limit",
            ));
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, InstallReceiptError> {
        if bytes.is_empty()
            || bytes.len() > MAX_INSTALL_RECEIPT_BYTES
            || !bytes.ends_with(b"\n")
            || bytes[..bytes.len() - 1].contains(&b'\n')
        {
            return Err(InstallReceiptError::invalid(
                "receipt",
                "receipt bytes are not canonical",
            ));
        }
        let receipt: Self = serde_json::from_slice(&bytes[..bytes.len() - 1])
            .map_err(|_| InstallReceiptError::invalid("receipt", "receipt JSON is invalid"))?;
        receipt.validate()?;
        if receipt.encode()? != bytes {
            return Err(InstallReceiptError::invalid(
                "receipt",
                "receipt JSON is not canonical",
            ));
        }
        Ok(receipt)
    }

    pub fn can_replace(&self, current: &Self) -> bool {
        if self.validate().is_err() || current.validate().is_err() {
            return false;
        }
        if self.operation_id != current.operation_id {
            return self.can_start_after(current);
        }
        if self.receipt_format != current.receipt_format
            || self.previous_operation_id != current.previous_operation_id
            || self.operation_kind != current.operation_kind
            || self.root_identity != current.root_identity
            || self.source_product != current.source_product
            || self.target_product != current.target_product
            || !artifacts_are_append_only(&current.artifacts, &self.artifacts)
        {
            return false;
        }
        if self.state == current.state {
            return self.failure_code == current.failure_code
                && self.failure_after_state == current.failure_after_state
                && self.manual_recovery_required == current.manual_recovery_required
                && self.artifacts.len() > current.artifacts.len();
        }
        replacement_transition(current, self)
    }

    fn can_start_after(&self, current: &Self) -> bool {
        current.state.is_terminal()
            && self.state == InstallState::Prepared
            && self.root_identity == current.root_identity
            && self.previous_operation_id.as_deref() == Some(current.operation_id())
            && self.source_product.as_ref() == current.installed_product()
            && self.artifacts.is_empty()
            && self.failure_code.is_none()
            && self.failure_after_state.is_none()
            && !self.manual_recovery_required
    }

    fn validate(&self) -> Result<(), InstallReceiptError> {
        if self.receipt_format != INSTALL_RECEIPT_FORMAT {
            return Err(InstallReceiptError::invalid(
                "receipt_format",
                "unsupported install receipt format",
            ));
        }
        validate_operation_id(&self.operation_id, "operation_id")?;
        if let Some(previous) = &self.previous_operation_id {
            validate_operation_id(previous, "previous_operation_id")?;
            if previous == &self.operation_id {
                return Err(InstallReceiptError::invalid(
                    "previous_operation_id",
                    "previous operation must differ",
                ));
            }
        }
        self.root_identity.validate()?;
        if let Some(source) = &self.source_product {
            source.validate()?;
        }
        if let Some(target) = &self.target_product {
            target.validate()?;
        }
        self.validate_operation_products()?;
        validate_artifact_order(&self.artifacts)?;
        for evidence in &self.artifacts {
            self.validate_artifact_for_operation(evidence)?;
        }
        self.validate_failure_state()?;
        self.validate_artifact_history()?;
        self.validate_evidence_for_state(self.state)
    }

    fn validate_operation_products(&self) -> Result<(), InstallReceiptError> {
        match (
            self.operation_kind,
            self.source_product.as_ref(),
            self.target_product.as_ref(),
        ) {
            (InstallOperationKind::FirstInstall, None, Some(_))
            | (InstallOperationKind::RemovePrograms, Some(_), None) => Ok(()),
            (InstallOperationKind::Upgrade, Some(source), Some(target)) => {
                if !source.compatible_with(target)
                    || target.release.release_cmp(&source.release) != Ordering::Greater
                {
                    return Err(InstallReceiptError::invalid(
                        "operation_products",
                        "upgrade target must be a compatible higher release",
                    ));
                }
                Ok(())
            }
            (InstallOperationKind::Repair, Some(source), Some(target)) => {
                if !source.compatible_with(target) || source.release != target.release {
                    return Err(InstallReceiptError::invalid(
                        "operation_products",
                        "repair requires compatible products at the same release",
                    ));
                }
                Ok(())
            }
            _ => Err(InstallReceiptError::invalid(
                "operation_products",
                "source and target do not match operation kind",
            )),
        }
    }

    fn validate_artifact_for_operation(
        &self,
        evidence: &InstallArtifactEvidence,
    ) -> Result<(), InstallReceiptError> {
        let expected_product = if evidence.slot.is_target() {
            self.target_product.as_ref()
        } else {
            self.source_product.as_ref()
        }
        .ok_or_else(|| {
            InstallReceiptError::invalid(
                "artifact_evidence",
                "artifact role is unavailable for operation",
            )
        })?;
        if evidence.identity != *expected_product.program(evidence.slot.component()) {
            return Err(InstallReceiptError::invalid(
                "artifact_evidence",
                "artifact identity differs from operation product",
            ));
        }
        evidence.filesystem_identity.validate()?;
        Ok(())
    }

    fn validate_artifact_recording_state(
        &self,
        slot: InstallArtifactSlot,
    ) -> Result<(), InstallReceiptError> {
        let allowed = match slot {
            InstallArtifactSlot::SourceManager | InstallArtifactSlot::SourceInputMethod => {
                self.state == InstallState::Quiesced
            }
            InstallArtifactSlot::StagedManager | InstallArtifactSlot::StagedInputMethod => {
                self.state == InstallState::Quiesced
            }
            InstallArtifactSlot::BackupManager | InstallArtifactSlot::BackupInputMethod => {
                self.state == InstallState::TargetStaged
                    || (self.operation_kind == InstallOperationKind::RemovePrograms
                        && self.state == InstallState::Quiesced)
            }
            InstallArtifactSlot::InstalledManager => {
                self.state == InstallState::SourcePreserved
                    || (self.operation_kind == InstallOperationKind::FirstInstall
                        && self.state == InstallState::TargetStaged)
            }
            InstallArtifactSlot::InstalledInputMethod => {
                self.state == InstallState::ManagerCommitted
            }
        };
        if allowed {
            Ok(())
        } else {
            Err(InstallReceiptError::invalid(
                "artifact_evidence",
                "artifact cannot be recorded in current state",
            ))
        }
    }

    fn validate_evidence_for_state(&self, state: InstallState) -> Result<(), InstallReceiptError> {
        let progress_state = self.effective_progress_state(state);
        let has = |slot| self.artifacts.iter().any(|item| item.slot == slot);
        let source_recorded =
            has(InstallArtifactSlot::SourceManager) && has(InstallArtifactSlot::SourceInputMethod);
        let target_staged =
            has(InstallArtifactSlot::StagedManager) && has(InstallArtifactSlot::StagedInputMethod);
        let source_preserved =
            has(InstallArtifactSlot::BackupManager) && has(InstallArtifactSlot::BackupInputMethod);
        let manager_installed = has(InstallArtifactSlot::InstalledManager);
        let input_installed = has(InstallArtifactSlot::InstalledInputMethod);

        if matches!(
            progress_state,
            InstallState::TargetStaged
                | InstallState::SourcePreserved
                | InstallState::ManagerCommitted
                | InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && matches!(
            self.operation_kind,
            InstallOperationKind::Upgrade | InstallOperationKind::Repair
        ) && !source_recorded
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "source program evidence is incomplete",
            ));
        }
        if matches!(
            progress_state,
            InstallState::SourcePreserved
                | InstallState::ManagerCommitted
                | InstallState::ProgramsCommitted
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && self.operation_kind == InstallOperationKind::RemovePrograms
            && !source_recorded
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "source program evidence is incomplete",
            ));
        }
        if matches!(
            progress_state,
            InstallState::TargetStaged
                | InstallState::SourcePreserved
                | InstallState::ManagerCommitted
                | InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && self.operation_kind != InstallOperationKind::RemovePrograms
            && !target_staged
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "target staging evidence is incomplete",
            ));
        }
        if matches!(
            progress_state,
            InstallState::SourcePreserved
                | InstallState::ManagerCommitted
                | InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && matches!(
            self.operation_kind,
            InstallOperationKind::Upgrade
                | InstallOperationKind::Repair
                | InstallOperationKind::RemovePrograms
        ) && !source_preserved
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "source preservation evidence is incomplete",
            ));
        }
        if matches!(
            progress_state,
            InstallState::ManagerCommitted
                | InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && self.operation_kind != InstallOperationKind::RemovePrograms
            && !manager_installed
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "installed Manager evidence is missing",
            ));
        }
        if matches!(
            progress_state,
            InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) && self.operation_kind != InstallOperationKind::RemovePrograms
            && !input_installed
        {
            return Err(InstallReceiptError::invalid(
                "artifacts",
                "installed InputMethod evidence is missing",
            ));
        }
        self.validate_filesystem_identity_continuity()
    }

    fn validate_filesystem_identity_continuity(&self) -> Result<(), InstallReceiptError> {
        for (before_slot, after_slot) in [
            (
                InstallArtifactSlot::SourceManager,
                InstallArtifactSlot::BackupManager,
            ),
            (
                InstallArtifactSlot::SourceInputMethod,
                InstallArtifactSlot::BackupInputMethod,
            ),
            (
                InstallArtifactSlot::StagedManager,
                InstallArtifactSlot::InstalledManager,
            ),
            (
                InstallArtifactSlot::StagedInputMethod,
                InstallArtifactSlot::InstalledInputMethod,
            ),
        ] {
            let before = self.artifacts.iter().find(|item| item.slot == before_slot);
            let after = self.artifacts.iter().find(|item| item.slot == after_slot);
            if let (Some(before), Some(after)) = (before, after) {
                if before.filesystem_identity != after.filesystem_identity {
                    return Err(InstallReceiptError::invalid(
                        "artifacts",
                        "renamed artifact filesystem identity changed",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_artifact_history(&self) -> Result<(), InstallReceiptError> {
        let progress = progress_rank(
            self.operation_kind,
            self.effective_progress_state(self.state),
        )
        .ok_or_else(|| {
            InstallReceiptError::invalid("state", "install state has no operation progress")
        })?;
        for artifact in &self.artifacts {
            let minimum = match artifact.slot {
                InstallArtifactSlot::SourceManager
                | InstallArtifactSlot::SourceInputMethod
                | InstallArtifactSlot::StagedManager
                | InstallArtifactSlot::StagedInputMethod => 1,
                InstallArtifactSlot::BackupManager | InstallArtifactSlot::BackupInputMethod => {
                    if self.operation_kind == InstallOperationKind::RemovePrograms {
                        1
                    } else {
                        2
                    }
                }
                InstallArtifactSlot::InstalledManager => {
                    if self.operation_kind == InstallOperationKind::FirstInstall {
                        2
                    } else {
                        3
                    }
                }
                InstallArtifactSlot::InstalledInputMethod => 4,
            };
            if progress < minimum {
                return Err(InstallReceiptError::invalid(
                    "artifacts",
                    "artifact evidence appears before its operation stage",
                ));
            }
        }
        Ok(())
    }

    fn effective_progress_state(&self, state: InstallState) -> InstallState {
        if matches!(
            state,
            InstallState::AbortedPreserved
                | InstallState::RollbackRequired
                | InstallState::ProgramsRestored
                | InstallState::RolledBack
        ) {
            self.failure_after_state.unwrap_or(state)
        } else {
            state
        }
    }

    fn validate_failure_state(&self) -> Result<(), InstallReceiptError> {
        match self.state {
            InstallState::AbortedPreserved => {
                if self.failure_code.is_none()
                    || !self
                        .failure_after_state
                        .is_some_and(InstallState::is_before_program_commit)
                    || self.manual_recovery_required
                {
                    return Err(InstallReceiptError::invalid(
                        "failure",
                        "aborted receipt failure fields are inconsistent",
                    ));
                }
            }
            InstallState::RollbackRequired | InstallState::ProgramsRestored => {
                if self.failure_code.is_none()
                    || !self
                        .failure_after_state
                        .is_some_and(InstallState::requires_program_rollback)
                    || !self.manual_recovery_required
                {
                    return Err(InstallReceiptError::invalid(
                        "failure",
                        "rollback receipt failure fields are inconsistent",
                    ));
                }
            }
            InstallState::RolledBack => {
                if self.failure_code.is_none()
                    || !self
                        .failure_after_state
                        .is_some_and(InstallState::requires_program_rollback)
                    || self.manual_recovery_required
                {
                    return Err(InstallReceiptError::invalid(
                        "failure",
                        "rolled back receipt failure fields are inconsistent",
                    ));
                }
            }
            _ => {
                if self.failure_code.is_some()
                    || self.failure_after_state.is_some()
                    || self.manual_recovery_required
                {
                    return Err(InstallReceiptError::invalid(
                        "failure",
                        "normal receipt must not contain failure fields",
                    ));
                }
            }
        }
        Ok(())
    }

    fn ensure_open(&self) -> Result<(), InstallReceiptError> {
        if self.state.is_terminal()
            || matches!(
                self.state,
                InstallState::RollbackRequired | InstallState::ProgramsRestored
            )
        {
            return Err(InstallReceiptError::invalid(
                "state",
                "install receipt is not open for normal mutation",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStartupGateDecision {
    AllowedFirstLaunch,
    AllowedNoInstallState,
    AllowedTerminalReceipt,
    BlockedInstallInProgress,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStartupGateErrorCode {
    None,
    InstallInProgress,
    ActiveGuard,
    UnsafeDataRoot,
    UnsafeStateDirectory,
    InterruptedReceipt,
    InvalidReceipt,
    UnexpectedStateObject,
    RootIdentityChanged,
    ProgramIdentityChanged,
    RemovedProgram,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStatusDecision {
    ReadyFirstLaunch,
    ReadyNoInstallState,
    OperationInProgress,
    TerminalReceipt,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallStatusResult {
    decision: InstallStatusDecision,
    error_code: InstallStartupGateErrorCode,
    operation_kind: Option<InstallOperationKind>,
    receipt_state: Option<InstallState>,
    failure_code: Option<InstallFailureCode>,
    manual_recovery_required: bool,
}

impl InstallStatusResult {
    pub const fn decision(self) -> InstallStatusDecision {
        self.decision
    }

    pub const fn error_code(self) -> InstallStartupGateErrorCode {
        self.error_code
    }

    pub const fn operation_kind(self) -> Option<InstallOperationKind> {
        self.operation_kind
    }

    pub const fn receipt_state(self) -> Option<InstallState> {
        self.receipt_state
    }

    pub const fn failure_code(self) -> Option<InstallFailureCode> {
        self.failure_code
    }

    pub const fn manual_recovery_required(self) -> bool {
        self.manual_recovery_required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallStartupGateResult {
    decision: InstallStartupGateDecision,
    error_code: InstallStartupGateErrorCode,
    receipt_state: Option<InstallState>,
}

impl InstallStartupGateResult {
    pub const fn decision(self) -> InstallStartupGateDecision {
        self.decision
    }

    pub const fn error_code(self) -> InstallStartupGateErrorCode {
        self.error_code
    }

    pub const fn receipt_state(self) -> Option<InstallState> {
        self.receipt_state
    }

    pub const fn is_allowed(self) -> bool {
        matches!(
            self.decision,
            InstallStartupGateDecision::AllowedFirstLaunch
                | InstallStartupGateDecision::AllowedNoInstallState
                | InstallStartupGateDecision::AllowedTerminalReceipt
        )
    }
}

pub fn evaluate_install_startup_receipt(
    receipt: &InstallReceipt,
    running: &RunningProgramIdentity,
) -> InstallStartupGateResult {
    if !receipt.state.is_terminal() {
        return InstallStartupGateResult {
            decision: InstallStartupGateDecision::BlockedInstallInProgress,
            error_code: InstallStartupGateErrorCode::InstallInProgress,
            receipt_state: Some(receipt.state),
        };
    }
    let expected = match receipt.state {
        InstallState::Completed => receipt.target_product.as_ref(),
        InstallState::AbortedPreserved | InstallState::RolledBack => {
            receipt.source_product.as_ref()
        }
        _ => None,
    };
    match expected {
        Some(product) if product.matches_running(running) => InstallStartupGateResult {
            decision: InstallStartupGateDecision::AllowedTerminalReceipt,
            error_code: InstallStartupGateErrorCode::None,
            receipt_state: Some(receipt.state),
        },
        None if receipt.operation_kind == InstallOperationKind::RemovePrograms
            && receipt.state == InstallState::Completed =>
        {
            InstallStartupGateResult {
                decision: InstallStartupGateDecision::FailedClosed,
                error_code: InstallStartupGateErrorCode::RemovedProgram,
                receipt_state: Some(receipt.state),
            }
        }
        _ => InstallStartupGateResult {
            decision: InstallStartupGateDecision::FailedClosed,
            error_code: InstallStartupGateErrorCode::ProgramIdentityChanged,
            receipt_state: Some(receipt.state),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReceiptError {
    field: &'static str,
    message: String,
}

impl InstallReceiptError {
    fn invalid(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    pub const fn field(&self) -> &'static str {
        self.field
    }
}

impl fmt::Display for InstallReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for InstallReceiptError {}

fn normal_transition(
    kind: InstallOperationKind,
    current: InstallState,
    next: InstallState,
) -> bool {
    match (current, next) {
        (InstallState::Prepared, InstallState::Quiesced) => true,
        (InstallState::Quiesced, InstallState::TargetStaged) => {
            kind != InstallOperationKind::RemovePrograms
        }
        (InstallState::Quiesced, InstallState::SourcePreserved) => {
            kind == InstallOperationKind::RemovePrograms
        }
        (InstallState::TargetStaged, InstallState::SourcePreserved) => {
            matches!(
                kind,
                InstallOperationKind::Upgrade | InstallOperationKind::Repair
            )
        }
        (InstallState::TargetStaged, InstallState::ManagerCommitted) => {
            kind == InstallOperationKind::FirstInstall
        }
        (InstallState::SourcePreserved, InstallState::ManagerCommitted)
        | (InstallState::ManagerCommitted, InstallState::ProgramsCommitted)
        | (InstallState::DataCoordinating, InstallState::DataSettled)
        | (InstallState::DataSettled, InstallState::FinalVerified)
        | (InstallState::FinalVerified, InstallState::Completed) => true,
        (InstallState::ProgramsCommitted, InstallState::DataCoordinating) => {
            kind == InstallOperationKind::Upgrade
        }
        (InstallState::ProgramsCommitted, InstallState::FinalVerified) => {
            kind != InstallOperationKind::Upgrade
        }
        _ => false,
    }
}

fn replacement_transition(current: &InstallReceipt, next: &InstallReceipt) -> bool {
    if normal_transition(current.operation_kind, current.state, next.state) {
        return current.failure_code.is_none()
            && next.failure_code.is_none()
            && current.failure_after_state.is_none()
            && next.failure_after_state.is_none()
            && !current.manual_recovery_required
            && !next.manual_recovery_required;
    }
    match (current.state, next.state) {
        (state, InstallState::AbortedPreserved) if state.is_before_program_commit() => {
            next.failure_code.is_some()
                && next.failure_after_state == Some(state)
                && !next.manual_recovery_required
        }
        (state, InstallState::RollbackRequired) if state.requires_program_rollback() => {
            next.failure_code.is_some()
                && next.failure_after_state == Some(state)
                && next.manual_recovery_required
        }
        (InstallState::RollbackRequired, InstallState::ProgramsRestored) => {
            current.failure_code == next.failure_code
                && current.failure_after_state == next.failure_after_state
                && next.manual_recovery_required
        }
        (InstallState::ProgramsRestored, InstallState::RolledBack) => {
            current.failure_code == next.failure_code
                && current.failure_after_state == next.failure_after_state
                && !next.manual_recovery_required
        }
        _ => false,
    }
}

fn progress_rank(kind: InstallOperationKind, state: InstallState) -> Option<u8> {
    match state {
        InstallState::Prepared => Some(0),
        InstallState::Quiesced => Some(1),
        InstallState::TargetStaged => (kind != InstallOperationKind::RemovePrograms).then_some(2),
        InstallState::SourcePreserved => matches!(
            kind,
            InstallOperationKind::Upgrade
                | InstallOperationKind::Repair
                | InstallOperationKind::RemovePrograms
        )
        .then_some(3),
        InstallState::ManagerCommitted => Some(4),
        InstallState::ProgramsCommitted => Some(5),
        InstallState::DataCoordinating => (kind == InstallOperationKind::Upgrade).then_some(6),
        InstallState::DataSettled => (kind == InstallOperationKind::Upgrade).then_some(7),
        InstallState::FinalVerified => Some(8),
        InstallState::Completed => Some(9),
        InstallState::AbortedPreserved
        | InstallState::RollbackRequired
        | InstallState::ProgramsRestored
        | InstallState::RolledBack => None,
    }
}

fn artifacts_are_append_only(
    current: &[InstallArtifactEvidence],
    next: &[InstallArtifactEvidence],
) -> bool {
    current
        .iter()
        .all(|artifact| next.iter().any(|candidate| candidate == artifact))
}

fn validate_artifact_order(
    artifacts: &[InstallArtifactEvidence],
) -> Result<(), InstallReceiptError> {
    let slots: Vec<_> = artifacts.iter().map(|item| item.slot).collect();
    let unique: BTreeSet<_> = slots.iter().copied().collect();
    if unique.len() != slots.len() || !slots.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(InstallReceiptError::invalid(
            "artifacts",
            "artifact evidence must be unique and sorted",
        ));
    }
    Ok(())
}

fn validate_operation_id(value: &str, field: &'static str) -> Result<(), InstallReceiptError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(InstallReceiptError::invalid(
            field,
            "operation ID must be 32 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), InstallReceiptError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(InstallReceiptError::invalid(
            field,
            "SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parse_version_part(parts.next()?)?;
    let minor = parse_version_part(parts.next()?)?;
    let patch = parse_version_part(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn parse_version_part(value: &str) -> Option<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn valid_bundle_id(value: &str) -> bool {
    value.len() <= 255
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && part
                    .bytes()
                    .last()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests;
