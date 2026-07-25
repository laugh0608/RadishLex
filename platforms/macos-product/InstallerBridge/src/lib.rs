//! Versioned native bridge between the AppKit Installer and the Rust driver/executor.

#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use radishlex_ime_product_install::{
    inspect_install_status, InstallOperationKind, InstallReceipt, InstallReceiptStore,
    InstallState, InstallStatusDecision, ProductArtifactIdentity, ProductRelease,
};
use radishlex_ime_product_upgrade::{
    ProductRelease as UpgradeProductRelease, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradePostSwitchValidationReport,
    UpgradeRollbackValidationEvidence,
};
use radishlex_macos_installer_driver::{
    authorize_installer_action, inspect_installer_view, InstallerAction,
    InstallerAuthorizationError, InstallerManualPrompt, InstallerProductSituation,
    InstallerStableError, InstallerUserAuthorization, InstallerViewPhase, InstallerViewSnapshot,
    INSTALLER_VIEW_CONTRACT_VERSION,
};
use radishlex_macos_installer_executor::{
    execute_authorized_intent_with_upgrade_bootstrap, InstallerExecutionError,
    InstallerExecutionSummary, InstallerOperationIdSource, InstallerPreflightPort,
    InstallerProgramPort, SystemInstallerOperationIdSource, UpgradeCoordinatorPort,
};
use radishlex_macos_product_install::{CodeSignatureRequirements, MacOsProductInstallAdapter};
use radishlex_macos_upgrade_coordinator::{
    MacOsProductPreflightAdapter, MacOsUpgradeCoordinatorAdapter,
};

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
    authorization_flags: u32,
) -> RadishLexInstallerBridgeSnapshotV1 {
    let Some(action) = decode_action(action) else {
        return unavailable_snapshot(InstallerStableError::UnknownDriverResult);
    };
    let Some(authorization) = decode_authorization(authorization_flags) else {
        return unavailable_snapshot(InstallerStableError::UnknownDriverResult);
    };
    production_perform(action, authorization)
}

fn production_snapshot() -> RadishLexInstallerBridgeSnapshotV1 {
    match ProductionInstallerEnvironment::discover() {
        Ok(environment) => encode_installer_snapshot(environment.snapshot),
        Err(error) => unavailable_snapshot(error),
    }
}

fn production_perform(
    action: InstallerAction,
    authorization: InstallerUserAuthorization,
) -> RadishLexInstallerBridgeSnapshotV1 {
    let environment = match ProductionInstallerEnvironment::discover() {
        Ok(environment) => environment,
        Err(error) => return unavailable_snapshot(error),
    };
    let intent = match authorize_installer_action(environment.snapshot, action, authorization) {
        Ok(intent) => intent,
        Err(_) => return unavailable_snapshot(InstallerStableError::UnknownDriverResult),
    };
    if action == InstallerAction::Refresh {
        return encode_installer_snapshot(environment.snapshot);
    }
    let mut programs = match environment.load_program_adapter(action) {
        Ok(programs) => programs,
        Err(_) => {
            return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable);
        }
    };
    let store = match programs.open_install_store() {
        Ok(store) => store,
        Err(_) => return unavailable_snapshot(InstallerStableError::UnknownDriverResult),
    };
    let mut preflight =
        match MacOsProductPreflightAdapter::load(&environment.context.product_root()) {
            Ok(preflight) => preflight,
            Err(_) => {
                return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable)
            }
        };
    let mut operation_ids = SystemInstallerOperationIdSource;
    let mut upgrade = if intent.operation_kind() == Some(InstallOperationKind::Upgrade) {
        let current = match store.load() {
            Ok(Some(receipt)) => receipt,
            _ => return unavailable_snapshot(InstallerStableError::UnknownDriverResult),
        };
        let source_root = match select_upgrade_source_root(&current, |release| {
            programs.upgrade_source_product_root(release)
        }) {
            Ok(source_root) => source_root,
            Err(error) => return unavailable_snapshot(error),
        };
        let adapter = match MacOsUpgradeCoordinatorAdapter::load(
            source_root,
            &environment.context.product_root(),
        ) {
            Ok(adapter) => adapter,
            Err(_) => {
                return unavailable_snapshot(InstallerStableError::ProductIdentityUnavailable)
            }
        };
        ProductionUpgradePort::Available(Box::new(adapter))
    } else {
        ProductionUpgradePort::Unavailable
    };
    match dispatch_installer_action(
        environment.context.data_root(),
        environment.context.owner_id(),
        environment.product_situation,
        action,
        authorization,
        &store,
        &mut programs,
        &mut preflight,
        &mut operation_ids,
        &mut upgrade,
    ) {
        Ok(_) => production_snapshot(),
        Err(_) => {
            let refreshed = production_snapshot();
            if refreshed.phase == phase_value(InstallerViewPhase::Blocked)
                || refreshed.receipt_state != 0
            {
                refreshed
            } else {
                unavailable_snapshot(InstallerStableError::UnknownDriverResult)
            }
        }
    }
}

struct ProductionInstallerEnvironment {
    context: InstallerBootstrapContext,
    requirements: CodeSignatureRequirements,
    target_product: ProductArtifactIdentity,
    product_situation: InstallerProductSituation,
    snapshot: InstallerViewSnapshot,
}

impl ProductionInstallerEnvironment {
    fn discover() -> Result<Self, InstallerStableError> {
        let context = InstallerBootstrapContext::discover()
            .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
        let requirements = context
            .release_requirements()
            .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
        let target_product = MacOsProductInstallAdapter::inspect_payload_product(
            &context.payload_root(),
            requirements.clone(),
        )
        .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
        let status = inspect_install_status(context.data_root(), context.owner_id());
        let blocked = inspect_installer_view(
            context.data_root(),
            context.owner_id(),
            InstallerProductSituation::IdentityUnavailable,
        );
        if status.decision() == InstallStatusDecision::FailedClosed
            || blocked.stable_error() == InstallerStableError::OperationActive
        {
            return Ok(Self {
                context,
                requirements,
                target_product,
                product_situation: InstallerProductSituation::IdentityUnavailable,
                snapshot: blocked,
            });
        }
        let product_situation = inspect_product_situation(
            &context,
            requirements.clone(),
            &target_product,
            status.decision(),
        )?;
        let snapshot =
            inspect_installer_view(context.data_root(), context.owner_id(), product_situation);
        Ok(Self {
            context,
            requirements,
            target_product,
            product_situation,
            snapshot,
        })
    }

    fn load_program_adapter(
        &self,
        action: InstallerAction,
    ) -> Result<MacOsProductInstallAdapter, ()> {
        let result = if action == InstallerAction::BeginFirstInstall
            && std::fs::symlink_metadata(self.context.data_root())
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            MacOsProductInstallAdapter::prepare_first_install(
                &self.context.payload_root(),
                self.context.user_home(),
                self.context.owner_id(),
                self.requirements.clone(),
            )
        } else {
            MacOsProductInstallAdapter::load(
                &self.context.payload_root(),
                self.context.user_home(),
                self.context.owner_id(),
                self.requirements.clone(),
            )
        };
        let adapter = result.map_err(|_| ())?;
        if adapter.target_product() != &self.target_product {
            return Err(());
        }
        Ok(adapter)
    }
}

fn inspect_product_situation(
    context: &InstallerBootstrapContext,
    requirements: CodeSignatureRequirements,
    target_product: &ProductArtifactIdentity,
    status: InstallStatusDecision,
) -> Result<InstallerProductSituation, InstallerStableError> {
    if matches!(
        status,
        InstallStatusDecision::ReadyFirstLaunch | InstallStatusDecision::ReadyNoInstallState
    ) {
        let absent = MacOsProductInstallAdapter::first_install_targets_are_absent(
            context.user_home(),
            context.owner_id(),
        )
        .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
        if status == InstallStatusDecision::ReadyNoInstallState {
            MacOsProductInstallAdapter::load(
                &context.payload_root(),
                context.user_home(),
                context.owner_id(),
                requirements,
            )
            .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
        }
        return Ok(if absent {
            InstallerProductSituation::NotInstalled
        } else {
            InstallerProductSituation::IdentityUnavailable
        });
    }
    if status == InstallStatusDecision::OperationInProgress {
        return Ok(InstallerProductSituation::IdentityUnavailable);
    }
    let adapter = MacOsProductInstallAdapter::load(
        &context.payload_root(),
        context.user_home(),
        context.owner_id(),
        requirements,
    )
    .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
    let store = adapter
        .open_install_store()
        .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
    let receipt = store
        .load()
        .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
    let expected = receipt
        .as_ref()
        .and_then(|receipt| receipt.installed_product());
    adapter
        .verify_installed_product(expected)
        .map_err(|_| InstallerStableError::ProductIdentityUnavailable)?;
    let Some(installed) = expected else {
        return Ok(InstallerProductSituation::NotInstalled);
    };
    let installed_release = installed.release();
    let target_release = target_product.release();
    if installed_release == target_release {
        Ok(InstallerProductSituation::MatchingReleaseInstalled)
    } else if installed_release.build_number() < target_release.build_number() {
        Ok(InstallerProductSituation::OlderReleaseInstalled)
    } else if installed_release.build_number() > target_release.build_number() {
        Ok(InstallerProductSituation::NewerReleaseInstalled)
    } else {
        Err(InstallerStableError::ProductIdentityUnavailable)
    }
}

fn upgrade_source_release(receipt: &InstallReceipt) -> Option<&ProductRelease> {
    if !receipt.state().is_terminal() && receipt.operation_kind() == InstallOperationKind::Upgrade {
        receipt
            .source_product()
            .map(ProductArtifactIdentity::release)
    } else {
        receipt
            .installed_product()
            .map(ProductArtifactIdentity::release)
    }
}

fn select_upgrade_source_root<'a>(
    receipt: &InstallReceipt,
    lookup: impl FnOnce(&ProductRelease) -> Option<&'a Path>,
) -> Result<&'a Path, InstallerStableError> {
    let release =
        upgrade_source_release(receipt).ok_or(InstallerStableError::ProductIdentityUnavailable)?;
    lookup(release).ok_or(InstallerStableError::DriverUnavailable)
}

#[derive(Debug)]
enum ProductionUpgradePort {
    Available(Box<MacOsUpgradeCoordinatorAdapter>),
    Unavailable,
}

impl UpgradeCoordinatorPort for ProductionUpgradePort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        match self {
            Self::Available(adapter) => adapter.confirm_quiescence(checkpoint),
            Self::Unavailable => false,
        }
    }

    fn validate_candidate(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        match self {
            Self::Available(adapter) => {
                adapter.validate_candidate(target_release, target_schema_version)
            }
            Self::Unavailable => UpgradeCandidateValidationReport::manager_failed(),
        }
    }

    fn validate_post_switch(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        match self {
            Self::Available(adapter) => {
                adapter.validate_post_switch(target_release, target_schema_version)
            }
            Self::Unavailable => UpgradePostSwitchValidationReport::manager_failed(),
        }
    }

    fn validate_restored_source(
        &mut self,
        source_release: &UpgradeProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        match self {
            Self::Available(adapter) => {
                adapter.validate_restored_source(source_release, source_schema_version)
            }
            Self::Unavailable => None,
        }
    }
}

fn decode_action(value: u32) -> Option<InstallerAction> {
    match value {
        1 => Some(InstallerAction::Refresh),
        2 => Some(InstallerAction::BeginFirstInstall),
        3 => Some(InstallerAction::BeginUpgrade),
        4 => Some(InstallerAction::BeginRepair),
        5 => Some(InstallerAction::ConfirmQuiescence),
        6 => Some(InstallerAction::ResumeOperation),
        7 => Some(InstallerAction::RetryOperation),
        8 => Some(InstallerAction::RemovePrograms),
        _ => None,
    }
}

fn decode_authorization(flags: u32) -> Option<InstallerUserAuthorization> {
    const EXPLICIT: u32 = 1 << 0;
    const DATA_RETENTION: u32 = 1 << 1;
    const NEUTRAL_INPUT_SOURCE: u32 = 1 << 2;
    const MANAGER_CLOSED: u32 = 1 << 3;
    const KNOWN: u32 = EXPLICIT | DATA_RETENTION | NEUTRAL_INPUT_SOURCE | MANAGER_CLOSED;
    if flags & !KNOWN != 0 {
        return None;
    }
    Some(InstallerUserAuthorization {
        explicit_action_confirmed: flags & EXPLICIT != 0,
        data_retention_acknowledged: flags & DATA_RETENTION != 0,
        neutral_input_source_selected: flags & NEUTRAL_INPUT_SOURCE != 0,
        manager_closed: flags & MANAGER_CLOSED != 0,
    })
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
