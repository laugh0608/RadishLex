//! Stable presentation and authorization contract for the RadishLex macOS Installer.

#![forbid(unsafe_code)]

use std::path::Path;

use radishlex_ime_product_install::{
    inspect_install_status, InstallFailureCode, InstallOperationKind, InstallStartupGateErrorCode,
    InstallState, InstallStatusDecision,
};

pub const INSTALLER_VIEW_CONTRACT_VERSION: u32 = 1;
pub const MANAGER_TARGET: &str = "Applications/RadishLex Manager.app";
pub const INPUT_METHOD_TARGET: &str = "Library/Input Methods/RadishLexInputMethod.app";
pub const DATA_ROOT: &str = "Library/Application Support/RadishLex";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerProductSituation {
    NotInstalled,
    OlderReleaseInstalled,
    MatchingReleaseInstalled,
    NewerReleaseInstalled,
    IdentityUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerViewPhase {
    Ready,
    AwaitingUserAction,
    InProgress,
    Completed,
    RecoveryAvailable,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerAction {
    None,
    Refresh,
    BeginFirstInstall,
    BeginUpgrade,
    BeginRepair,
    ConfirmQuiescence,
    ResumeOperation,
    RetryOperation,
    RemovePrograms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerStableError {
    None,
    OperationActive,
    UnsafeDataRoot,
    UnsafeStateDirectory,
    InterruptedReceipt,
    InvalidReceipt,
    UnexpectedStateObject,
    RootIdentityChanged,
    Io,
    InstalledReleaseIsNewer,
    ProductIdentityUnavailable,
    ManualRecoveryRequired,
    DriverUnavailable,
    UnknownDriverResult,
}

impl InstallerStableError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OperationActive => "operation_active",
            Self::UnsafeDataRoot => "unsafe_data_root",
            Self::UnsafeStateDirectory => "unsafe_state_directory",
            Self::InterruptedReceipt => "interrupted_receipt",
            Self::InvalidReceipt => "invalid_receipt",
            Self::UnexpectedStateObject => "unexpected_state_object",
            Self::RootIdentityChanged => "root_identity_changed",
            Self::Io => "io",
            Self::InstalledReleaseIsNewer => "installed_release_is_newer",
            Self::ProductIdentityUnavailable => "product_identity_unavailable",
            Self::ManualRecoveryRequired => "manual_recovery_required",
            Self::DriverUnavailable => "driver_unavailable",
            Self::UnknownDriverResult => "unknown_driver_result",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerManualPrompt {
    None,
    SelectNeutralInputSourceAndCloseManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerDataPolicy {
    RetainApplicationSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallerViewSnapshot {
    version: u32,
    phase: InstallerViewPhase,
    primary_action: InstallerAction,
    secondary_action: InstallerAction,
    stable_error: InstallerStableError,
    manual_prompt: InstallerManualPrompt,
    data_policy: InstallerDataPolicy,
    operation_kind: Option<InstallOperationKind>,
    receipt_state: Option<InstallState>,
    failure_code: Option<InstallFailureCode>,
    progress_step: u8,
}

impl InstallerViewSnapshot {
    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn phase(self) -> InstallerViewPhase {
        self.phase
    }

    pub const fn primary_action(self) -> InstallerAction {
        self.primary_action
    }

    pub const fn secondary_action(self) -> InstallerAction {
        self.secondary_action
    }

    pub const fn stable_error(self) -> InstallerStableError {
        self.stable_error
    }

    pub const fn manual_prompt(self) -> InstallerManualPrompt {
        self.manual_prompt
    }

    pub const fn data_policy(self) -> InstallerDataPolicy {
        self.data_policy
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

    pub const fn progress_step(self) -> u8 {
        self.progress_step
    }

    pub fn offers(self, action: InstallerAction) -> bool {
        action != InstallerAction::None
            && (self.primary_action == action || self.secondary_action == action)
    }

    pub fn stable_summary(self) -> String {
        format!(
            "phase={} action={} error={} state={}",
            phase_code(self.phase),
            action_code(self.primary_action),
            self.stable_error.code(),
            self.receipt_state.map_or("none", state_code),
        )
    }

    pub const fn phase_code(self) -> &'static str {
        phase_code(self.phase)
    }

    pub const fn primary_action_code(self) -> &'static str {
        action_code(self.primary_action)
    }

    pub const fn secondary_action_code(self) -> &'static str {
        action_code(self.secondary_action)
    }

    pub const fn receipt_state_code(self) -> &'static str {
        match self.receipt_state {
            Some(state) => state_code(state),
            None => "none",
        }
    }

    pub const fn operation_code(self) -> &'static str {
        match self.operation_kind {
            Some(kind) => operation_code(kind),
            None => "none",
        }
    }

    pub const fn manual_prompt_code(self) -> &'static str {
        match self.manual_prompt {
            InstallerManualPrompt::None => "none",
            InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager => {
                "select_neutral_input_source_and_close_manager"
            }
        }
    }

    pub const fn data_policy_code(self) -> &'static str {
        match self.data_policy {
            InstallerDataPolicy::RetainApplicationSupport => "retain_application_support",
        }
    }
}

pub fn inspect_installer_view(
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
    product_situation: InstallerProductSituation,
) -> InstallerViewSnapshot {
    let status = inspect_install_status(data_root, expected_owner_id);
    match status.decision() {
        InstallStatusDecision::ReadyFirstLaunch | InstallStatusDecision::ReadyNoInstallState => {
            ready_snapshot(product_situation)
        }
        InstallStatusDecision::OperationInProgress => {
            if status.error_code() == InstallStartupGateErrorCode::ActiveGuard {
                return snapshot(
                    InstallerViewPhase::InProgress,
                    InstallerAction::Refresh,
                    InstallerAction::None,
                    InstallerStableError::OperationActive,
                    InstallerManualPrompt::None,
                    None,
                    None,
                    None,
                );
            }
            let Some(kind) = status.operation_kind() else {
                return blocked_snapshot(InstallerStableError::InvalidReceipt);
            };
            let Some(state) = status.receipt_state() else {
                return blocked_snapshot(InstallerStableError::InvalidReceipt);
            };
            if state == InstallState::Prepared {
                snapshot(
                    InstallerViewPhase::AwaitingUserAction,
                    InstallerAction::ConfirmQuiescence,
                    InstallerAction::None,
                    InstallerStableError::None,
                    InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager,
                    Some(kind),
                    Some(state),
                    status.failure_code(),
                )
            } else if status.manual_recovery_required() {
                snapshot(
                    InstallerViewPhase::RecoveryAvailable,
                    InstallerAction::ResumeOperation,
                    InstallerAction::None,
                    InstallerStableError::ManualRecoveryRequired,
                    InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager,
                    Some(kind),
                    Some(state),
                    status.failure_code(),
                )
            } else {
                snapshot(
                    InstallerViewPhase::InProgress,
                    InstallerAction::ResumeOperation,
                    InstallerAction::None,
                    InstallerStableError::None,
                    InstallerManualPrompt::None,
                    Some(kind),
                    Some(state),
                    status.failure_code(),
                )
            }
        }
        InstallStatusDecision::TerminalReceipt => {
            let Some(kind) = status.operation_kind() else {
                return blocked_snapshot(InstallerStableError::InvalidReceipt);
            };
            let Some(state) = status.receipt_state() else {
                return blocked_snapshot(InstallerStableError::InvalidReceipt);
            };
            match state {
                InstallState::Completed => {
                    completed_snapshot(kind, product_situation, status.failure_code())
                }
                InstallState::AbortedPreserved | InstallState::RolledBack => snapshot(
                    InstallerViewPhase::RecoveryAvailable,
                    InstallerAction::RetryOperation,
                    InstallerAction::RemovePrograms,
                    InstallerStableError::None,
                    InstallerManualPrompt::None,
                    Some(kind),
                    Some(state),
                    status.failure_code(),
                ),
                _ => blocked_snapshot(InstallerStableError::InvalidReceipt),
            }
        }
        InstallStatusDecision::FailedClosed => {
            blocked_snapshot(map_status_error(status.error_code()))
        }
    }
}

fn completed_snapshot(
    completed_kind: InstallOperationKind,
    product_situation: InstallerProductSituation,
    failure_code: Option<InstallFailureCode>,
) -> InstallerViewSnapshot {
    match (completed_kind, product_situation) {
        (InstallOperationKind::RemovePrograms, InstallerProductSituation::NotInstalled) => {
            snapshot(
                InstallerViewPhase::Completed,
                InstallerAction::BeginFirstInstall,
                InstallerAction::None,
                InstallerStableError::None,
                InstallerManualPrompt::None,
                Some(InstallOperationKind::FirstInstall),
                Some(InstallState::Completed),
                failure_code,
            )
        }
        (
            InstallOperationKind::FirstInstall
            | InstallOperationKind::Upgrade
            | InstallOperationKind::Repair,
            InstallerProductSituation::OlderReleaseInstalled
            | InstallerProductSituation::MatchingReleaseInstalled
            | InstallerProductSituation::NewerReleaseInstalled,
        ) => ready_snapshot(product_situation),
        _ => blocked_snapshot(InstallerStableError::ProductIdentityUnavailable),
    }
}

fn ready_snapshot(product_situation: InstallerProductSituation) -> InstallerViewSnapshot {
    match product_situation {
        InstallerProductSituation::NotInstalled => snapshot(
            InstallerViewPhase::Ready,
            InstallerAction::BeginFirstInstall,
            InstallerAction::None,
            InstallerStableError::None,
            InstallerManualPrompt::None,
            Some(InstallOperationKind::FirstInstall),
            None,
            None,
        ),
        InstallerProductSituation::OlderReleaseInstalled => snapshot(
            InstallerViewPhase::Ready,
            InstallerAction::BeginUpgrade,
            InstallerAction::RemovePrograms,
            InstallerStableError::None,
            InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager,
            Some(InstallOperationKind::Upgrade),
            None,
            None,
        ),
        InstallerProductSituation::MatchingReleaseInstalled => snapshot(
            InstallerViewPhase::Ready,
            InstallerAction::BeginRepair,
            InstallerAction::RemovePrograms,
            InstallerStableError::None,
            InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager,
            Some(InstallOperationKind::Repair),
            None,
            None,
        ),
        InstallerProductSituation::NewerReleaseInstalled => {
            blocked_snapshot(InstallerStableError::InstalledReleaseIsNewer)
        }
        InstallerProductSituation::IdentityUnavailable => {
            blocked_snapshot(InstallerStableError::ProductIdentityUnavailable)
        }
    }
}

fn blocked_snapshot(error: InstallerStableError) -> InstallerViewSnapshot {
    snapshot(
        InstallerViewPhase::Blocked,
        InstallerAction::Refresh,
        InstallerAction::None,
        error,
        InstallerManualPrompt::None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn snapshot(
    phase: InstallerViewPhase,
    primary_action: InstallerAction,
    secondary_action: InstallerAction,
    stable_error: InstallerStableError,
    manual_prompt: InstallerManualPrompt,
    operation_kind: Option<InstallOperationKind>,
    receipt_state: Option<InstallState>,
    failure_code: Option<InstallFailureCode>,
) -> InstallerViewSnapshot {
    InstallerViewSnapshot {
        version: INSTALLER_VIEW_CONTRACT_VERSION,
        phase,
        primary_action,
        secondary_action,
        stable_error,
        manual_prompt,
        data_policy: InstallerDataPolicy::RetainApplicationSupport,
        operation_kind,
        receipt_state,
        failure_code,
        progress_step: receipt_state.map_or(0, progress_step),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstallerUserAuthorization {
    pub explicit_action_confirmed: bool,
    pub data_retention_acknowledged: bool,
    pub neutral_input_source_selected: bool,
    pub manager_closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerAuthorizationError {
    ActionNotOffered,
    ExplicitConfirmationRequired,
    DataRetentionAcknowledgementRequired,
    ManualQuiescenceAcknowledgementRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizedInstallerIntent {
    action: InstallerAction,
    operation_kind: Option<InstallOperationKind>,
    resume_existing: bool,
    requires_platform_preflight: bool,
}

impl AuthorizedInstallerIntent {
    pub const fn action(self) -> InstallerAction {
        self.action
    }

    pub const fn operation_kind(self) -> Option<InstallOperationKind> {
        self.operation_kind
    }

    pub const fn resume_existing(self) -> bool {
        self.resume_existing
    }

    pub const fn requires_platform_preflight(self) -> bool {
        self.requires_platform_preflight
    }
}

pub fn authorize_installer_action(
    snapshot: InstallerViewSnapshot,
    action: InstallerAction,
    authorization: InstallerUserAuthorization,
) -> Result<AuthorizedInstallerIntent, InstallerAuthorizationError> {
    if !snapshot.offers(action) {
        return Err(InstallerAuthorizationError::ActionNotOffered);
    }
    if action == InstallerAction::Refresh {
        return Ok(AuthorizedInstallerIntent {
            action,
            operation_kind: snapshot.operation_kind,
            resume_existing: false,
            requires_platform_preflight: false,
        });
    }
    if !authorization.explicit_action_confirmed {
        return Err(InstallerAuthorizationError::ExplicitConfirmationRequired);
    }
    if action == InstallerAction::RemovePrograms && !authorization.data_retention_acknowledged {
        return Err(InstallerAuthorizationError::DataRetentionAcknowledgementRequired);
    }
    if matches!(
        action,
        InstallerAction::BeginUpgrade
            | InstallerAction::BeginRepair
            | InstallerAction::ConfirmQuiescence
            | InstallerAction::RetryOperation
            | InstallerAction::RemovePrograms
    ) && (!authorization.neutral_input_source_selected || !authorization.manager_closed)
    {
        return Err(InstallerAuthorizationError::ManualQuiescenceAcknowledgementRequired);
    }
    let operation_kind = match action {
        InstallerAction::BeginFirstInstall => Some(InstallOperationKind::FirstInstall),
        InstallerAction::BeginUpgrade => Some(InstallOperationKind::Upgrade),
        InstallerAction::BeginRepair => Some(InstallOperationKind::Repair),
        InstallerAction::RemovePrograms => Some(InstallOperationKind::RemovePrograms),
        InstallerAction::ConfirmQuiescence
        | InstallerAction::ResumeOperation
        | InstallerAction::RetryOperation => snapshot.operation_kind,
        InstallerAction::None | InstallerAction::Refresh => None,
    };
    Ok(AuthorizedInstallerIntent {
        action,
        operation_kind,
        resume_existing: matches!(
            action,
            InstallerAction::ConfirmQuiescence | InstallerAction::ResumeOperation
        ),
        requires_platform_preflight: true,
    })
}

const fn map_status_error(error: InstallStartupGateErrorCode) -> InstallerStableError {
    match error {
        InstallStartupGateErrorCode::None | InstallStartupGateErrorCode::InstallInProgress => {
            InstallerStableError::None
        }
        InstallStartupGateErrorCode::ActiveGuard => InstallerStableError::OperationActive,
        InstallStartupGateErrorCode::UnsafeDataRoot => InstallerStableError::UnsafeDataRoot,
        InstallStartupGateErrorCode::UnsafeStateDirectory => {
            InstallerStableError::UnsafeStateDirectory
        }
        InstallStartupGateErrorCode::InterruptedReceipt => InstallerStableError::InterruptedReceipt,
        InstallStartupGateErrorCode::InvalidReceipt => InstallerStableError::InvalidReceipt,
        InstallStartupGateErrorCode::UnexpectedStateObject => {
            InstallerStableError::UnexpectedStateObject
        }
        InstallStartupGateErrorCode::RootIdentityChanged
        | InstallStartupGateErrorCode::ProgramIdentityChanged
        | InstallStartupGateErrorCode::RemovedProgram => InstallerStableError::RootIdentityChanged,
        InstallStartupGateErrorCode::Io => InstallerStableError::Io,
    }
}

const fn progress_step(state: InstallState) -> u8 {
    match state {
        InstallState::Prepared => 1,
        InstallState::Quiesced => 2,
        InstallState::TargetStaged => 3,
        InstallState::SourcePreserved => 4,
        InstallState::ManagerCommitted => 5,
        InstallState::ProgramsCommitted => 6,
        InstallState::DataCoordinating => 7,
        InstallState::DataSettled => 8,
        InstallState::FinalVerified => 9,
        InstallState::Completed => 10,
        InstallState::AbortedPreserved => 10,
        InstallState::RollbackRequired => 7,
        InstallState::ProgramsRestored => 8,
        InstallState::RolledBack => 10,
    }
}

const fn phase_code(phase: InstallerViewPhase) -> &'static str {
    match phase {
        InstallerViewPhase::Ready => "ready",
        InstallerViewPhase::AwaitingUserAction => "awaiting_user_action",
        InstallerViewPhase::InProgress => "in_progress",
        InstallerViewPhase::Completed => "completed",
        InstallerViewPhase::RecoveryAvailable => "recovery_available",
        InstallerViewPhase::Blocked => "blocked",
    }
}

const fn action_code(action: InstallerAction) -> &'static str {
    match action {
        InstallerAction::None => "none",
        InstallerAction::Refresh => "refresh",
        InstallerAction::BeginFirstInstall => "begin_first_install",
        InstallerAction::BeginUpgrade => "begin_upgrade",
        InstallerAction::BeginRepair => "begin_repair",
        InstallerAction::ConfirmQuiescence => "confirm_quiescence",
        InstallerAction::ResumeOperation => "resume_operation",
        InstallerAction::RetryOperation => "retry_operation",
        InstallerAction::RemovePrograms => "remove_programs",
    }
}

const fn state_code(state: InstallState) -> &'static str {
    match state {
        InstallState::Prepared => "prepared",
        InstallState::Quiesced => "quiesced",
        InstallState::TargetStaged => "target_staged",
        InstallState::SourcePreserved => "source_preserved",
        InstallState::ManagerCommitted => "manager_committed",
        InstallState::ProgramsCommitted => "programs_committed",
        InstallState::DataCoordinating => "data_coordinating",
        InstallState::DataSettled => "data_settled",
        InstallState::FinalVerified => "final_verified",
        InstallState::Completed => "completed",
        InstallState::AbortedPreserved => "aborted_preserved",
        InstallState::RollbackRequired => "rollback_required",
        InstallState::ProgramsRestored => "programs_restored",
        InstallState::RolledBack => "rolled_back",
    }
}

const fn operation_code(kind: InstallOperationKind) -> &'static str {
    match kind {
        InstallOperationKind::FirstInstall => "first_install",
        InstallOperationKind::Upgrade => "upgrade",
        InstallOperationKind::Repair => "repair",
        InstallOperationKind::RemovePrograms => "remove_programs",
    }
}

#[cfg(test)]
mod tests;
