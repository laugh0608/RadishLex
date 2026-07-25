//! Versioned native bridge between the AppKit Installer and the Rust driver/executor.

#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use radishlex_ime_product_install::{InstallOperationKind, InstallReceiptStore, InstallState};
use radishlex_macos_installer_driver::{
    authorize_installer_action, inspect_installer_view, InstallerAction,
    InstallerAuthorizationError, InstallerManualPrompt, InstallerProductSituation,
    InstallerStableError, InstallerUserAuthorization, InstallerViewPhase, InstallerViewSnapshot,
    INSTALLER_VIEW_CONTRACT_VERSION,
};
use radishlex_macos_installer_executor::{
    execute_authorized_intent_with_upgrade_bootstrap, InstallerExecutionError,
    InstallerExecutionSummary, InstallerOperationIdSource, InstallerPreflightPort,
    InstallerProgramPort, UpgradeCoordinatorPort,
};
use radishlex_macos_product_install::MacOsProductInstallAdapter;

mod bootstrap;
use bootstrap::InstallerBootstrapContext;
#[cfg(test)]
use bootstrap::InstallerBootstrapError;

pub const INSTALLER_BRIDGE_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerBridgeDispatch {
    Refreshed(InstallerViewSnapshot),
    Executed(InstallerExecutionSummary),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerBridgeError {
    Authorization(InstallerAuthorizationError),
    Execution(InstallerExecutionError),
}

#[allow(clippy::too_many_arguments)]
pub fn dispatch_installer_action<P, F, I, U>(
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
    product_situation: InstallerProductSituation,
    action: InstallerAction,
    authorization: InstallerUserAuthorization,
    install_store: &InstallReceiptStore,
    programs: &mut P,
    preflight: &mut F,
    operation_ids: &mut I,
    upgrade: &mut U,
) -> Result<InstallerBridgeDispatch, InstallerBridgeError>
where
    P: InstallerProgramPort,
    F: InstallerPreflightPort,
    I: InstallerOperationIdSource,
    U: UpgradeCoordinatorPort,
{
    let data_root = data_root.as_ref();
    let snapshot = inspect_installer_view(data_root, expected_owner_id, product_situation);
    let intent = authorize_installer_action(snapshot, action, authorization)
        .map_err(InstallerBridgeError::Authorization)?;
    if action == InstallerAction::Refresh {
        return Ok(InstallerBridgeDispatch::Refreshed(snapshot));
    }
    execute_authorized_intent_with_upgrade_bootstrap(
        intent,
        install_store,
        data_root,
        expected_owner_id,
        programs,
        preflight,
        operation_ids,
        upgrade,
    )
    .map(InstallerBridgeDispatch::Executed)
    .map_err(InstallerBridgeError::Execution)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexInstallerBridgeSnapshotV1 {
    pub contract_version: u32,
    pub phase: u32,
    pub primary_action: u32,
    pub secondary_action: u32,
    pub stable_error: u32,
    pub operation_kind: u32,
    pub receipt_state: u32,
    pub progress_step: u32,
    pub manual_prompt: u32,
}

pub const fn encode_installer_snapshot(
    snapshot: InstallerViewSnapshot,
) -> RadishLexInstallerBridgeSnapshotV1 {
    RadishLexInstallerBridgeSnapshotV1 {
        contract_version: snapshot.version(),
        phase: phase_value(snapshot.phase()),
        primary_action: action_value(snapshot.primary_action()),
        secondary_action: action_value(snapshot.secondary_action()),
        stable_error: error_value(snapshot.stable_error()),
        operation_kind: operation_value(snapshot.operation_kind()),
        receipt_state: state_value(snapshot.receipt_state()),
        progress_step: snapshot.progress_step() as u32,
        manual_prompt: prompt_value(snapshot.manual_prompt()),
    }
}

#[no_mangle]
pub extern "C" fn radishlex_installer_bridge_contract_version() -> u32 {
    INSTALLER_BRIDGE_CONTRACT_VERSION
}

#[no_mangle]
pub extern "C" fn radishlex_installer_bridge_snapshot_v1() -> RadishLexInstallerBridgeSnapshotV1 {
    production_snapshot()
}

#[no_mangle]
pub extern "C" fn radishlex_installer_bridge_perform_v1(
    action: u32,
    _authorization_flags: u32,
) -> RadishLexInstallerBridgeSnapshotV1 {
    if action > 8 {
        return unavailable_snapshot(InstallerStableError::UnknownDriverResult);
    }
    production_snapshot()
}

fn production_snapshot() -> RadishLexInstallerBridgeSnapshotV1 {
    let context = match InstallerBootstrapContext::discover() {
        Ok(context) => context,
        Err(_) => {
            return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable);
        }
    };
    let requirements = match context.release_requirements() {
        Ok(requirements) => requirements,
        Err(_) => {
            return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable);
        }
    };
    if MacOsProductInstallAdapter::inspect_payload_product(&context.payload_root(), requirements)
        .is_err()
    {
        return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable);
    }
    let state_snapshot = inspect_installer_view(
        context.data_root(),
        context.owner_id(),
        InstallerProductSituation::IdentityUnavailable,
    );
    if matches!(
        state_snapshot.stable_error(),
        InstallerStableError::OperationActive
            | InstallerStableError::UnsafeDataRoot
            | InstallerStableError::UnsafeStateDirectory
            | InstallerStableError::InterruptedReceipt
            | InstallerStableError::InvalidReceipt
            | InstallerStableError::UnexpectedStateObject
            | InstallerStableError::RootIdentityChanged
            | InstallerStableError::Io
            | InstallerStableError::ManualRecoveryRequired
    ) {
        return encode_installer_snapshot(state_snapshot);
    }
    unavailable_snapshot(InstallerStableError::DriverUnavailable)
}

fn unavailable_snapshot(error: InstallerStableError) -> RadishLexInstallerBridgeSnapshotV1 {
    RadishLexInstallerBridgeSnapshotV1 {
        contract_version: INSTALLER_VIEW_CONTRACT_VERSION,
        phase: phase_value(InstallerViewPhase::Blocked),
        primary_action: action_value(InstallerAction::Refresh),
        secondary_action: action_value(InstallerAction::None),
        stable_error: error_value(error),
        operation_kind: 0,
        receipt_state: 0,
        progress_step: 0,
        manual_prompt: prompt_value(InstallerManualPrompt::None),
    }
}

const fn phase_value(phase: InstallerViewPhase) -> u32 {
    match phase {
        InstallerViewPhase::Ready => 1,
        InstallerViewPhase::AwaitingUserAction => 2,
        InstallerViewPhase::InProgress => 3,
        InstallerViewPhase::Completed => 4,
        InstallerViewPhase::RecoveryAvailable => 5,
        InstallerViewPhase::Blocked => 6,
    }
}

const fn action_value(action: InstallerAction) -> u32 {
    match action {
        InstallerAction::None => 0,
        InstallerAction::Refresh => 1,
        InstallerAction::BeginFirstInstall => 2,
        InstallerAction::BeginUpgrade => 3,
        InstallerAction::BeginRepair => 4,
        InstallerAction::ConfirmQuiescence => 5,
        InstallerAction::ResumeOperation => 6,
        InstallerAction::RetryOperation => 7,
        InstallerAction::RemovePrograms => 8,
    }
}

const fn error_value(error: InstallerStableError) -> u32 {
    match error {
        InstallerStableError::None => 0,
        InstallerStableError::OperationActive => 1,
        InstallerStableError::UnsafeDataRoot => 2,
        InstallerStableError::UnsafeStateDirectory => 3,
        InstallerStableError::InterruptedReceipt => 4,
        InstallerStableError::InvalidReceipt => 5,
        InstallerStableError::UnexpectedStateObject => 6,
        InstallerStableError::RootIdentityChanged => 7,
        InstallerStableError::Io => 8,
        InstallerStableError::InstalledReleaseIsNewer => 9,
        InstallerStableError::ProductIdentityUnavailable => 10,
        InstallerStableError::ManualRecoveryRequired => 11,
        InstallerStableError::DriverUnavailable => 12,
        InstallerStableError::UnknownDriverResult => 13,
    }
}

const fn prompt_value(prompt: InstallerManualPrompt) -> u32 {
    match prompt {
        InstallerManualPrompt::None => 0,
        InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager => 1,
    }
}

const fn operation_value(operation: Option<InstallOperationKind>) -> u32 {
    match operation {
        None => 0,
        Some(InstallOperationKind::FirstInstall) => 1,
        Some(InstallOperationKind::Upgrade) => 2,
        Some(InstallOperationKind::Repair) => 3,
        Some(InstallOperationKind::RemovePrograms) => 4,
    }
}

const fn state_value(state: Option<InstallState>) -> u32 {
    match state {
        None => 0,
        Some(InstallState::Prepared) => 1,
        Some(InstallState::Quiesced) => 2,
        Some(InstallState::TargetStaged) => 3,
        Some(InstallState::SourcePreserved) => 4,
        Some(InstallState::ManagerCommitted) => 5,
        Some(InstallState::ProgramsCommitted) => 6,
        Some(InstallState::DataCoordinating) => 7,
        Some(InstallState::DataSettled) => 8,
        Some(InstallState::FinalVerified) => 9,
        Some(InstallState::Completed) => 10,
        Some(InstallState::AbortedPreserved) => 11,
        Some(InstallState::RollbackRequired) => 12,
        Some(InstallState::ProgramsRestored) => 13,
        Some(InstallState::RolledBack) => 14,
    }
}

#[cfg(test)]
mod tests;
