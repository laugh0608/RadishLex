use std::path::Path;

use radishlex_ime_product_install::{InstallProgramValidationPort, VerifiedInstallRoot};
use radishlex_ime_product_upgrade::{UpgradeFailureCode, UpgradeState, VerifiedDataRoot};
use radishlex_macos_installer_driver::{
    inspect_installer_view, InstallerProductSituation, InstallerStableError, InstallerViewSnapshot,
};
use radishlex_macos_product_install::{PreSwitchRecoveryEvidence, RecoveryEvidenceError};
use radishlex_macos_product_install_coordinator::{
    abort_pre_switch_install_upgrade, PreSwitchRecoveryCheckpoint, PreSwitchRecoveryValidationPort,
};

use super::*;

/// Inspect both existing receipts without bootstrap, SQLite or filesystem mutation.
pub fn inspect_installer_view_with_recovery(
    root: impl AsRef<Path>,
    owner: u32,
    situation: InstallerProductSituation,
) -> InstallerViewSnapshot {
    let root = root.as_ref();
    let snapshot = inspect_installer_view(root, owner, situation);
    if snapshot.operation_kind() != Some(InstallOperationKind::Upgrade)
        || !matches!(
            snapshot.receipt_state(),
            Some(
                InstallState::DataCoordinating
                    | InstallState::RollbackRequired
                    | InstallState::ProgramsRestored
            )
        )
        || snapshot.stable_error() == InstallerStableError::OperationActive
    {
        return snapshot;
    }
    match inspect_eligibility(root, owner) {
        Ok(Some((receipt, started))) => snapshot.with_pre_switch_recovery(&receipt, started),
        Ok(None) => snapshot,
        Err(InstallerExecutionError::InstallFilesystem(
            InstallFilesystemErrorCode::OperationAlreadyActive,
        ))
        | Err(InstallerExecutionError::UpgradeFilesystem(
            radishlex_ime_product_upgrade::UpgradeFilesystemErrorCode::OperationAlreadyActive,
        )) => snapshot.with_recovery_inspection_error(InstallerStableError::OperationActive),
        Err(_) => snapshot.with_recovery_inspection_error(InstallerStableError::InvalidReceipt),
    }
}

fn inspect_eligibility(
    root: &Path,
    owner: u32,
) -> Result<Option<(InstallReceipt, bool)>, InstallerExecutionError> {
    let outer_store = InstallReceiptStore::open_existing(
        VerifiedInstallRoot::verify(root, owner)
            .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?,
    )
    .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;
    let outer = outer_store
        .load_for_recovery_inspection()
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?
        .ok_or(InstallerExecutionError::InvalidState)?;
    let inner_store = open_existing_upgrade(root, owner)?;
    let inner = inner_store
        .load_for_recovery_inspection()
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?
        .ok_or(InstallerExecutionError::InvalidState)?;
    bootstrap::verify_existing_upgrade_binding(&outer, &inner, &inner_store)
        .map_err(|error| InstallerExecutionError::UpgradeBootstrap(error.code()))?;
    let evidence = PreSwitchRecoveryEvidence::load(root, &outer)
        .map_err(InstallerExecutionError::RecoveryEvidence)?;
    if let Some(evidence) = &evidence {
        verify_data_evidence(evidence, &inner)?;
        return Ok(Some((outer, true)));
    }
    if is_pre_switch_abort(&inner) {
        return Err(InstallerExecutionError::RecoveryEvidence(
            RecoveryEvidenceError::InvalidEvidence,
        ));
    }
    if outer.state() == InstallState::DataCoordinating
        && inner.state() == UpgradeState::CandidateVerified
        && inner.failure_code().is_none()
        && outer.failure_code().is_none()
        && !outer.manual_recovery_required()
    {
        return Ok(Some((outer, false)));
    }
    Ok(None)
}

pub(super) fn is_pre_switch_abort(receipt: &UpgradeReceipt) -> bool {
    receipt.state() == UpgradeState::AbortedPreserved
        && receipt.failure_code() == Some(UpgradeFailureCode::SwitchFailed)
        && receipt.failure_after_state() == Some(UpgradeState::CandidateVerified)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_pre_switch_recovery<P, F, U>(
    intent: AuthorizedInstallerIntent,
    outer_store: &InstallReceiptStore,
    root: &Path,
    owner: u32,
    programs: &mut P,
    preflight: &mut F,
    upgrade: &mut U,
) -> Result<InstallerExecutionSummary, InstallerExecutionError>
where
    P: InstallerProgramPort,
    F: InstallerPreflightPort,
    U: UpgradeCoordinatorPort,
{
    if intent.action() != InstallerAction::AbortPreSwitchUpgrade
        || intent.operation_kind() != Some(InstallOperationKind::Upgrade)
        || !intent.resume_existing()
        || !intent.requires_platform_preflight()
    {
        return Err(InstallerExecutionError::InvalidIntent);
    }
    let outer_guard = outer_store
        .acquire_guard()
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;
    let mut outer = outer_store
        .load_guarded(&outer_guard)
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?
        .ok_or(InstallerExecutionError::InvalidState)?;
    if !intent.matches_recovery_receipt(&outer)
        || outer.target_product() != Some(programs.target_product())
        || outer.operation_kind() != InstallOperationKind::Upgrade
        || !matches!(
            outer.state(),
            InstallState::DataCoordinating
                | InstallState::RollbackRequired
                | InstallState::ProgramsRestored
        )
    {
        return Err(InstallerExecutionError::InvalidIntent);
    }
    let inner_store = open_existing_upgrade(root, owner)?;
    let inner_guard = inner_store
        .acquire_guard()
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?;
    let mut inner = inner_store
        .load_guarded(&inner_guard)
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?
        .ok_or(InstallerExecutionError::InvalidState)?;
    bootstrap::verify_existing_upgrade_binding(&outer, &inner, &inner_store)
        .map_err(|error| InstallerExecutionError::UpgradeBootstrap(error.code()))?;
    preflight.inspect_preflight(programs.target_product())?;
    let manager = programs.open_program_store(
        outer_store,
        &outer_guard,
        ProgramComponent::Manager,
        &outer,
    )?;
    let input_method = programs.open_program_store(
        outer_store,
        &outer_guard,
        ProgramComponent::InputMethod,
        &outer,
    )?;
    let evidence = PreSwitchRecoveryEvidence::load(root, &outer)
        .map_err(InstallerExecutionError::RecoveryEvidence)?;
    if let Some(evidence) = &evidence {
        verify_data_evidence(evidence, &inner)?;
    } else if inner.state() != UpgradeState::CandidateVerified {
        return Err(InstallerExecutionError::RecoveryEvidence(
            RecoveryEvidenceError::InvalidEvidence,
        ));
    }
    let original_data_receipt = inner
        .encode()
        .map_err(|_| InstallerExecutionError::InvalidState)?;
    let mut validation = RecoveryValidation {
        root,
        outer_store,
        outer_guard: &outer_guard,
        programs,
        evidence,
        original_data_receipt,
        error: None,
    };
    let result = abort_pre_switch_install_upgrade(
        outer_store,
        &outer_guard,
        &mut outer,
        &manager,
        &input_method,
        &inner_store,
        &inner_guard,
        &mut inner,
        upgrade,
        &mut validation,
    );
    if let Some(error) = validation.error {
        return Err(error);
    }
    result.map_err(InstallerExecutionError::DataCoordination)?;
    let evidence = validation
        .evidence
        .as_ref()
        .ok_or(InstallerExecutionError::InvalidState)?;
    evidence
        .verify_preserved(outer_store, &outer_guard, &outer)
        .map_err(InstallerExecutionError::RecoveryEvidence)?;
    verify_data_evidence(evidence, &inner)?;
    inner_store
        .verify_current(&inner_guard, &inner)
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?;
    summary(&outer)
}

fn open_existing_upgrade(
    root: &Path,
    owner: u32,
) -> Result<UpgradeReceiptStore, InstallerExecutionError> {
    let verified = VerifiedDataRoot::verify(root, owner)
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?;
    UpgradeReceiptStore::open_existing(verified)
        .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))
}

fn verify_data_evidence(
    evidence: &PreSwitchRecoveryEvidence,
    current: &UpgradeReceipt,
) -> Result<(), InstallerExecutionError> {
    let mut original = UpgradeReceipt::decode(evidence.initial_data_receipt()).map_err(|_| {
        InstallerExecutionError::RecoveryEvidence(RecoveryEvidenceError::InvalidEvidence)
    })?;
    if original.state() != UpgradeState::CandidateVerified || original.failure_code().is_some() {
        return Err(InstallerExecutionError::RecoveryEvidence(
            RecoveryEvidenceError::InvalidEvidence,
        ));
    }
    if current.state() == UpgradeState::AbortedPreserved {
        original
            .abort_preserved(UpgradeFailureCode::SwitchFailed, false)
            .map_err(|_| InstallerExecutionError::InvalidState)?;
    }
    if &original != current {
        return Err(InstallerExecutionError::RecoveryEvidence(
            RecoveryEvidenceError::BindingChanged,
        ));
    }
    Ok(())
}

struct RecoveryValidation<'a, P> {
    root: &'a Path,
    outer_store: &'a InstallReceiptStore,
    outer_guard: &'a InstallProcessGuard,
    programs: &'a mut P,
    evidence: Option<PreSwitchRecoveryEvidence>,
    original_data_receipt: Vec<u8>,
    error: Option<InstallerExecutionError>,
}

impl<P: InstallerProgramPort> InstallProgramValidationPort for RecoveryValidation<'_, P> {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.programs
            .validate_installed_targets(manager, input_method, receipt)
    }

    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.programs
            .validate_restored_sources(manager, input_method, receipt)
    }
}

impl<P: InstallerProgramPort> PreSwitchRecoveryValidationPort for RecoveryValidation<'_, P> {
    fn validate_recovery_checkpoint(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        checkpoint: PreSwitchRecoveryCheckpoint,
    ) -> bool {
        let result = self.verify_checkpoint(manager, input_method, receipt, checkpoint);
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }
}

impl<P: InstallerProgramPort> RecoveryValidation<'_, P> {
    fn verify_checkpoint(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        checkpoint: PreSwitchRecoveryCheckpoint,
    ) -> Result<(), InstallerExecutionError> {
        self.programs
            .verify_recovery_material(manager, input_method, receipt)?;
        if self.evidence.is_none() {
            if checkpoint != PreSwitchRecoveryCheckpoint::BeforeAbort {
                return Err(InstallerExecutionError::InvalidState);
            }
            self.evidence = Some(
                PreSwitchRecoveryEvidence::create(
                    self.root,
                    self.outer_store,
                    self.outer_guard,
                    receipt,
                    &self.original_data_receipt,
                )
                .map_err(InstallerExecutionError::RecoveryEvidence)?,
            );
        }
        self.evidence
            .as_ref()
            .ok_or(InstallerExecutionError::InvalidState)?
            .verify_preserved(self.outer_store, self.outer_guard, receipt)
            .map_err(InstallerExecutionError::RecoveryEvidence)
    }
}
