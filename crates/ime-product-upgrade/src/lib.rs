//! Product data upgrade state and receipt contracts for RadishLex.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(unix)]
mod filesystem;
#[cfg(unix)]
pub use filesystem::{
    inspect_startup_gate, StartupGateDecision, StartupGateErrorCode, StartupGateResult,
    UpgradeCandidateSummary, UpgradeCandidateValidationDisposition,
    UpgradeCandidateValidationReport, UpgradeCandidateValidationSummary,
    UpgradeCompletionDisposition, UpgradeFilesystemError, UpgradeFilesystemErrorCode,
    UpgradeInputMethodValidationEvidence, UpgradeManagerValidationEvidence,
    UpgradePostSwitchValidationDisposition, UpgradePostSwitchValidationReport,
    UpgradePostSwitchValidationSummary, UpgradeProcessGuard, UpgradeReceiptStore,
    UpgradeRollbackRestoreDisposition, UpgradeRollbackRestoreSummary,
    UpgradeRollbackValidationDisposition, UpgradeRollbackValidationEvidence,
    UpgradeRollbackValidationSummary, UpgradeSettingsBackupSummary, UpgradeSnapshotSpaceBudget,
    UpgradeSnapshotSummary, UpgradeSwitchDisposition, UpgradeSwitchSummary, VerifiedDataRoot,
    UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION, UPGRADE_VALIDATION_EVIDENCE_VERSION,
};

pub const UPGRADE_RECEIPT_FORMAT: &str = "radishlex-product-upgrade-receipt-v1";
pub const MAX_UPGRADE_RECEIPT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataLayout {
    #[serde(rename = "application-support-v1")]
    ApplicationSupportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeArtifactSlot {
    DataRoot,
    SourceDatabase,
    SnapshotDatabase,
    CandidateDatabase,
    BackupDatabase,
    SourceSettings,
    BackupSettings,
    RimeRoot,
}

impl UpgradeArtifactSlot {
    const fn expects_directory(self) -> bool {
        matches!(self, Self::DataRoot | Self::RimeRoot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeState {
    Preflighted,
    Quiesced,
    SnapshotReady,
    CandidateMigrated,
    CandidateVerified,
    SwitchPrepared,
    Switched,
    PostSwitchVerified,
    Completed,
    AbortedPreserved,
    RollbackRequired,
    RolledBack,
}

impl UpgradeState {
    const fn is_pre_switch(self) -> bool {
        matches!(
            self,
            Self::Preflighted
                | Self::Quiesced
                | Self::SnapshotReady
                | Self::CandidateMigrated
                | Self::CandidateVerified
                | Self::SwitchPrepared
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::AbortedPreserved | Self::RolledBack
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeFailureCode {
    UnsupportedReceipt,
    ReceiptInconsistent,
    OperationAlreadyActive,
    UnsafeDataRoot,
    UnexpectedDataObject,
    IdentityChanged,
    ProcessNotQuiescent,
    DatabaseBusy,
    DatabaseCorrupt,
    FutureSchema,
    MigrationFailed,
    InsufficientSpace,
    PermissionDenied,
    SnapshotFailed,
    ManagerValidationFailed,
    InputMethodValidationFailed,
    SwitchFailed,
    PostSwitchValidationFailed,
    RollbackFailed,
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
    ) -> Result<Self, UpgradeReceiptError> {
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

    fn validate(&self, field: &'static str) -> Result<(), UpgradeReceiptError> {
        if !valid_product_version(&self.product_version) {
            return Err(UpgradeReceiptError::invalid(
                field,
                "product version must use numeric major.minor.patch",
            ));
        }
        if self.build_number == 0 {
            return Err(UpgradeReceiptError::invalid(
                field,
                "build number must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeArtifactIdentity {
    slot: UpgradeArtifactSlot,
    device_id: u64,
    inode: u64,
    owner_id: u32,
    mode: u32,
    link_count: u64,
    byte_len: u64,
}

impl UpgradeArtifactIdentity {
    pub fn private_file(
        slot: UpgradeArtifactSlot,
        device_id: u64,
        inode: u64,
        owner_id: u32,
        byte_len: u64,
    ) -> Result<Self, UpgradeReceiptError> {
        Self::new(slot, device_id, inode, owner_id, 0o600, 1, byte_len)
    }

    pub fn private_directory(
        slot: UpgradeArtifactSlot,
        device_id: u64,
        inode: u64,
        owner_id: u32,
        link_count: u64,
    ) -> Result<Self, UpgradeReceiptError> {
        Self::new(slot, device_id, inode, owner_id, 0o700, link_count, 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slot: UpgradeArtifactSlot,
        device_id: u64,
        inode: u64,
        owner_id: u32,
        mode: u32,
        link_count: u64,
        byte_len: u64,
    ) -> Result<Self, UpgradeReceiptError> {
        let identity = Self {
            slot,
            device_id,
            inode,
            owner_id,
            mode,
            link_count,
            byte_len,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn slot(&self) -> UpgradeArtifactSlot {
        self.slot
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

    pub const fn link_count(&self) -> u64 {
        self.link_count
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    fn validate(&self) -> Result<(), UpgradeReceiptError> {
        if self.inode == 0 {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "inode must be positive",
            ));
        }
        if self.link_count == 0 || (!self.slot.expects_directory() && self.link_count != 1) {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "file artifacts require one link and directory link count must be positive",
            ));
        }
        let expected_mode = if self.slot.expects_directory() {
            0o700
        } else {
            0o600
        };
        if self.mode != expected_mode {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                format!("artifact mode must be {expected_mode:04o}"),
            ));
        }
        if self.slot.expects_directory() && self.byte_len != 0 {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "directory identity must not record content length",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeReceipt {
    receipt_format: String,
    operation_id: String,
    previous_operation_id: Option<String>,
    data_layout: DataLayout,
    source_release: ProductRelease,
    target_release: ProductRelease,
    source_schema_version: Option<i64>,
    target_schema_version: i64,
    state: UpgradeState,
    failure_code: Option<UpgradeFailureCode>,
    failure_after_state: Option<UpgradeState>,
    manual_recovery_required: bool,
    artifacts: Vec<UpgradeArtifactIdentity>,
}

impl UpgradeReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: impl Into<String>,
        previous_operation_id: Option<String>,
        source_release: ProductRelease,
        target_release: ProductRelease,
        source_schema_version: Option<i64>,
        target_schema_version: i64,
        artifacts: Vec<UpgradeArtifactIdentity>,
    ) -> Result<Self, UpgradeReceiptError> {
        let mut artifacts = artifacts;
        artifacts.sort_by_key(|item| item.slot);
        let receipt = Self {
            receipt_format: UPGRADE_RECEIPT_FORMAT.to_owned(),
            operation_id: operation_id.into(),
            previous_operation_id,
            data_layout: DataLayout::ApplicationSupportV1,
            source_release,
            target_release,
            source_schema_version,
            target_schema_version,
            state: UpgradeState::Preflighted,
            failure_code: None,
            failure_after_state: None,
            manual_recovery_required: false,
            artifacts,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, UpgradeReceiptError> {
        if bytes.is_empty() || bytes.len() > MAX_UPGRADE_RECEIPT_BYTES {
            return Err(UpgradeReceiptError::invalid(
                "receipt",
                "receipt size is outside the accepted range",
            ));
        }
        if !bytes.ends_with(b"\n") {
            return Err(UpgradeReceiptError::invalid(
                "receipt",
                "receipt must end with one newline",
            ));
        }
        let receipt: Self =
            serde_json::from_slice(bytes).map_err(|_| UpgradeReceiptError::MalformedJson)?;
        receipt.validate()?;
        if receipt.encode()?.as_slice() != bytes {
            return Err(UpgradeReceiptError::invalid(
                "receipt",
                "receipt is not in canonical encoding",
            ));
        }
        Ok(receipt)
    }

    pub fn encode(&self) -> Result<Vec<u8>, UpgradeReceiptError> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self).map_err(|_| UpgradeReceiptError::MalformedJson)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub const fn state(&self) -> UpgradeState {
        self.state
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn previous_operation_id(&self) -> Option<&str> {
        self.previous_operation_id.as_deref()
    }

    pub const fn data_layout(&self) -> DataLayout {
        self.data_layout
    }

    pub fn source_release(&self) -> &ProductRelease {
        &self.source_release
    }

    pub fn target_release(&self) -> &ProductRelease {
        &self.target_release
    }

    pub const fn source_schema_version(&self) -> Option<i64> {
        self.source_schema_version
    }

    pub const fn target_schema_version(&self) -> i64 {
        self.target_schema_version
    }

    pub const fn failure_code(&self) -> Option<UpgradeFailureCode> {
        self.failure_code
    }

    pub const fn failure_after_state(&self) -> Option<UpgradeState> {
        self.failure_after_state
    }

    pub const fn manual_recovery_required(&self) -> bool {
        self.manual_recovery_required
    }

    pub fn artifacts(&self) -> &[UpgradeArtifactIdentity] {
        &self.artifacts
    }

    pub fn record_artifact(
        &mut self,
        artifact: UpgradeArtifactIdentity,
    ) -> Result<(), UpgradeReceiptError> {
        if self.state.is_terminal() {
            return Err(UpgradeReceiptError::invalid(
                "state",
                "terminal receipt cannot record another artifact",
            ));
        }
        if self
            .artifacts
            .iter()
            .any(|existing| existing.slot == artifact.slot)
        {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "artifact slot is already recorded",
            ));
        }
        artifact.validate()?;
        let previous_artifacts = self.artifacts.clone();
        self.artifacts.push(artifact);
        self.artifacts.sort_by_key(|item| item.slot);
        if let Err(error) = self.validate() {
            self.artifacts = previous_artifacts;
            return Err(error);
        }
        Ok(())
    }

    pub fn advance(&mut self, next: UpgradeState) -> Result<(), UpgradeReceiptError> {
        if !is_normal_successor(self.state, next) {
            return Err(UpgradeReceiptError::invalid(
                "state",
                format!(
                    "transition from {:?} to {next:?} is not allowed",
                    self.state
                ),
            ));
        }
        let previous_state = self.state;
        self.state = next;
        if let Err(error) = self.validate() {
            self.state = previous_state;
            return Err(error);
        }
        Ok(())
    }

    pub fn abort_preserved(
        &mut self,
        failure_code: UpgradeFailureCode,
        manual_recovery_required: bool,
    ) -> Result<(), UpgradeReceiptError> {
        if !self.state.is_pre_switch() {
            return Err(UpgradeReceiptError::invalid(
                "state",
                "only a pre-switch operation can abort with original data preserved",
            ));
        }
        let previous_state = self.state;
        let previous_failure_code = self.failure_code;
        let previous_failure_after_state = self.failure_after_state;
        let previous_manual_recovery_required = self.manual_recovery_required;
        let failure_after_state = previous_state;
        self.state = UpgradeState::AbortedPreserved;
        self.failure_code = Some(failure_code);
        self.failure_after_state = Some(failure_after_state);
        self.manual_recovery_required = manual_recovery_required;
        if let Err(error) = self.validate() {
            self.state = previous_state;
            self.failure_code = previous_failure_code;
            self.failure_after_state = previous_failure_after_state;
            self.manual_recovery_required = previous_manual_recovery_required;
            return Err(error);
        }
        Ok(())
    }

    pub fn require_rollback(
        &mut self,
        failure_code: UpgradeFailureCode,
    ) -> Result<(), UpgradeReceiptError> {
        if !matches!(
            self.state,
            UpgradeState::Switched | UpgradeState::PostSwitchVerified
        ) {
            return Err(UpgradeReceiptError::invalid(
                "state",
                "rollback can only be required after the candidate was switched and before completion",
            ));
        }
        let previous_state = self.state;
        let previous_failure_code = self.failure_code;
        let previous_failure_after_state = self.failure_after_state;
        let previous_manual_recovery_required = self.manual_recovery_required;
        self.state = UpgradeState::RollbackRequired;
        self.failure_code = Some(failure_code);
        self.failure_after_state = Some(previous_state);
        self.manual_recovery_required = true;
        if let Err(error) = self.validate() {
            self.state = previous_state;
            self.failure_code = previous_failure_code;
            self.failure_after_state = previous_failure_after_state;
            self.manual_recovery_required = previous_manual_recovery_required;
            return Err(error);
        }
        Ok(())
    }

    pub fn mark_rolled_back(&mut self) -> Result<(), UpgradeReceiptError> {
        if self.state != UpgradeState::RollbackRequired {
            return Err(UpgradeReceiptError::invalid(
                "state",
                "only a rollback-required operation can finish rollback",
            ));
        }
        let previous_state = self.state;
        let previous_manual_recovery_required = self.manual_recovery_required;
        self.state = UpgradeState::RolledBack;
        self.manual_recovery_required = false;
        if let Err(error) = self.validate() {
            self.state = previous_state;
            self.manual_recovery_required = previous_manual_recovery_required;
            return Err(error);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), UpgradeReceiptError> {
        if self.receipt_format != UPGRADE_RECEIPT_FORMAT {
            return Err(UpgradeReceiptError::invalid(
                "receipt_format",
                "unsupported receipt format",
            ));
        }
        validate_operation_id("operation_id", &self.operation_id)?;
        if let Some(previous) = &self.previous_operation_id {
            validate_operation_id("previous_operation_id", previous)?;
            if previous == &self.operation_id {
                return Err(UpgradeReceiptError::invalid(
                    "previous_operation_id",
                    "previous operation must differ from current operation",
                ));
            }
        }
        self.source_release.validate("source_release")?;
        self.target_release.validate("target_release")?;
        if self.source_release == self.target_release {
            return Err(UpgradeReceiptError::invalid(
                "target_release",
                "target release must differ from source release",
            ));
        }
        if self
            .source_schema_version
            .is_some_and(|version| version < 0)
        {
            return Err(UpgradeReceiptError::invalid(
                "source_schema_version",
                "source schema version must be non-negative",
            ));
        }
        if self.target_schema_version <= 0 {
            return Err(UpgradeReceiptError::invalid(
                "target_schema_version",
                "target schema version must be positive",
            ));
        }
        if self.artifacts.is_empty() {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "at least the fixed data root identity is required",
            ));
        }
        let mut slots = BTreeSet::new();
        let mut previous_slot = None;
        for artifact in &self.artifacts {
            artifact.validate()?;
            if previous_slot.is_some_and(|previous| previous >= artifact.slot) {
                return Err(UpgradeReceiptError::invalid(
                    "artifacts",
                    "artifact slots must use canonical order",
                ));
            }
            if !slots.insert(artifact.slot) {
                return Err(UpgradeReceiptError::invalid(
                    "artifacts",
                    "artifact slots must be unique",
                ));
            }
            previous_slot = Some(artifact.slot);
        }
        if !slots.contains(&UpgradeArtifactSlot::DataRoot) {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "fixed data root identity is required",
            ));
        }
        if self.source_schema_version.is_some()
            != slots.contains(&UpgradeArtifactSlot::SourceDatabase)
        {
            return Err(UpgradeReceiptError::invalid(
                "source_schema_version",
                "source database identity and schema presence must agree",
            ));
        }
        if slots.contains(&UpgradeArtifactSlot::SourceSettings)
            != slots.contains(&UpgradeArtifactSlot::BackupSettings)
            && !matches!(
                self.state,
                UpgradeState::Preflighted | UpgradeState::Quiesced
            )
        {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "settings source and backup identities must agree after snapshot",
            ));
        }
        let evidence_state = self.failure_after_state.unwrap_or(self.state);
        if state_at_least(evidence_state, UpgradeState::SnapshotReady)
            && !slots.contains(&UpgradeArtifactSlot::SnapshotDatabase)
        {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "snapshot identity is required for this state",
            ));
        }
        if state_at_least(evidence_state, UpgradeState::CandidateMigrated)
            && !slots.contains(&UpgradeArtifactSlot::CandidateDatabase)
        {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "candidate identity is required for this state",
            ));
        }
        if state_at_least(evidence_state, UpgradeState::SwitchPrepared)
            && !slots.contains(&UpgradeArtifactSlot::BackupDatabase)
        {
            return Err(UpgradeReceiptError::invalid(
                "artifacts",
                "backup identity is required for this state",
            ));
        }

        match self.state {
            UpgradeState::AbortedPreserved => {
                if self.failure_code.is_none()
                    || !self
                        .failure_after_state
                        .is_some_and(UpgradeState::is_pre_switch)
                {
                    return Err(UpgradeReceiptError::invalid(
                        "failure_code",
                        "aborted receipt requires a failure and pre-switch stage",
                    ));
                }
            }
            UpgradeState::RollbackRequired => {
                if self.failure_code.is_none()
                    || !matches!(
                        self.failure_after_state,
                        Some(UpgradeState::Switched | UpgradeState::PostSwitchVerified)
                    )
                    || !self.manual_recovery_required
                {
                    return Err(UpgradeReceiptError::invalid(
                        "rollback_required",
                        "rollback-required receipt needs a failure and recovery gate",
                    ));
                }
            }
            UpgradeState::RolledBack => {
                if self.failure_code.is_none()
                    || !matches!(
                        self.failure_after_state,
                        Some(UpgradeState::Switched | UpgradeState::PostSwitchVerified)
                    )
                    || self.manual_recovery_required
                {
                    return Err(UpgradeReceiptError::invalid(
                        "rolled_back",
                        "rolled-back receipt must retain failure and clear recovery gate",
                    ));
                }
            }
            _ => {
                if self.failure_code.is_some()
                    || self.failure_after_state.is_some()
                    || self.manual_recovery_required
                {
                    return Err(UpgradeReceiptError::invalid(
                        "state",
                        "normal state cannot carry failure or manual recovery fields",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn can_replace(&self, previous: &Self) -> bool {
        let immutable_fields_match = self.receipt_format == previous.receipt_format
            && self.operation_id == previous.operation_id
            && self.previous_operation_id == previous.previous_operation_id
            && self.data_layout == previous.data_layout
            && self.source_release == previous.source_release
            && self.target_release == previous.target_release
            && self.source_schema_version == previous.source_schema_version
            && self.target_schema_version == previous.target_schema_version;
        if !immutable_fields_match || self.validate().is_err() || previous.validate().is_err() {
            return false;
        }

        let artifacts_are_append_only = previous.artifacts.iter().all(|previous_artifact| {
            self.artifacts
                .iter()
                .any(|artifact| artifact == previous_artifact)
        });
        if !artifacts_are_append_only {
            return false;
        }

        if self.state == previous.state {
            return !self.state.is_terminal()
                && self.failure_code == previous.failure_code
                && self.failure_after_state == previous.failure_after_state
                && self.manual_recovery_required == previous.manual_recovery_required;
        }

        is_normal_successor(previous.state, self.state)
            || (previous.state.is_pre_switch()
                && self.state == UpgradeState::AbortedPreserved
                && self.failure_after_state == Some(previous.state))
            || (matches!(
                previous.state,
                UpgradeState::Switched | UpgradeState::PostSwitchVerified
            ) && self.state == UpgradeState::RollbackRequired)
            || (previous.state == UpgradeState::RollbackRequired
                && self.state == UpgradeState::RolledBack)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeReceiptError {
    Invalid {
        field: &'static str,
        message: String,
    },
    MalformedJson,
}

impl UpgradeReceiptError {
    fn invalid(field: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for UpgradeReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { field, message } => write!(formatter, "invalid {field}: {message}"),
            Self::MalformedJson => formatter.write_str("invalid receipt JSON"),
        }
    }
}

impl std::error::Error for UpgradeReceiptError {}

fn validate_operation_id(
    field: &'static str,
    operation_id: &str,
) -> Result<(), UpgradeReceiptError> {
    if operation_id.len() != 32
        || !operation_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(UpgradeReceiptError::invalid(
            field,
            "operation ID must be 32 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn valid_product_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let valid_part = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if valid_part(major) && valid_part(minor) && valid_part(patch)
    )
}

fn state_at_least(actual: UpgradeState, required: UpgradeState) -> bool {
    fn normal_rank(state: UpgradeState) -> Option<u8> {
        match state {
            UpgradeState::Preflighted => Some(0),
            UpgradeState::Quiesced => Some(1),
            UpgradeState::SnapshotReady => Some(2),
            UpgradeState::CandidateMigrated => Some(3),
            UpgradeState::CandidateVerified => Some(4),
            UpgradeState::SwitchPrepared => Some(5),
            UpgradeState::Switched => Some(6),
            UpgradeState::PostSwitchVerified => Some(7),
            UpgradeState::Completed => Some(8),
            UpgradeState::AbortedPreserved
            | UpgradeState::RollbackRequired
            | UpgradeState::RolledBack => None,
        }
    }

    matches!(
        (normal_rank(actual), normal_rank(required)),
        (Some(actual), Some(required)) if actual >= required
    )
}

fn is_normal_successor(current: UpgradeState, next: UpgradeState) -> bool {
    matches!(
        (current, next),
        (UpgradeState::Preflighted, UpgradeState::Quiesced)
            | (UpgradeState::Quiesced, UpgradeState::SnapshotReady)
            | (UpgradeState::SnapshotReady, UpgradeState::CandidateMigrated)
            | (
                UpgradeState::CandidateMigrated,
                UpgradeState::CandidateVerified
            )
            | (
                UpgradeState::CandidateVerified,
                UpgradeState::SwitchPrepared
            )
            | (UpgradeState::SwitchPrepared, UpgradeState::Switched)
            | (UpgradeState::Switched, UpgradeState::PostSwitchVerified)
            | (UpgradeState::PostSwitchVerified, UpgradeState::Completed)
    )
}

#[cfg(test)]
mod tests;
