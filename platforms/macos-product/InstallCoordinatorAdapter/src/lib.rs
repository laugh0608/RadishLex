//! Guard-bound composition of RadishLex program installation and data upgrade transactions.

#![forbid(unsafe_code)]

use std::fmt;

use radishlex_ime_product_install::{
    finish_program_restore, restore_program_source, resume_install_finalization,
    InstallFailureCode, InstallFilesystemError, InstallFilesystemErrorCode,
    InstallFinalizationError, InstallFinalizationPort, InstallFinalizationValidationStage,
    InstallOperationKind, InstallProcessGuard, InstallProgramValidationPort, InstallReceipt,
    InstallReceiptStore, InstallState, ProductRelease, ProgramComponent, ProgramSwitchError,
    ProgramSwitchErrorCode, ProgramSwitchStore,
};
use radishlex_ime_product_upgrade::{
    ProductRelease as UpgradeProductRelease, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradeCoordinatorDisposition, UpgradeCoordinatorError,
    UpgradeCoordinatorPort, UpgradePostSwitchValidationReport, UpgradeProcessGuard, UpgradeReceipt,
    UpgradeReceiptStore, UpgradeRollbackValidationEvidence, UpgradeState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallProgramValidationStage {
    InstalledTarget,
    RestoredSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDataCoordinationDisposition {
    DataSettled,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallDataCoordinationSummary {
    disposition: InstallDataCoordinationDisposition,
}

impl InstallDataCoordinationSummary {
    pub const fn disposition(self) -> InstallDataCoordinationDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDataCoordinationError {
    InstallFilesystem(InstallFilesystemErrorCode),
    ProgramSwitch(ProgramSwitchErrorCode),
    Upgrade(UpgradeCoordinatorError),
    InvalidBinding,
    InvalidState,
    ProgramValidationNotProven(InstallProgramValidationStage),
}

impl fmt::Display for InstallDataCoordinationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallFilesystem(_) => {
                formatter.write_str("install data coordination filesystem failure")
            }
            Self::ProgramSwitch(_) => {
                formatter.write_str("install data coordination program switch failure")
            }
            Self::Upgrade(_) => formatter.write_str("product data coordination failure"),
            Self::InvalidBinding => {
                formatter.write_str("install and data operations are not bound")
            }
            Self::InvalidState => {
                formatter.write_str("install and data operation states are inconsistent")
            }
            Self::ProgramValidationNotProven(_) => {
                formatter.write_str("install program validation was not proven")
            }
        }
    }
}

impl std::error::Error for InstallDataCoordinationError {}

impl From<InstallFilesystemError> for InstallDataCoordinationError {
    fn from(error: InstallFilesystemError) -> Self {
        Self::InstallFilesystem(error.code())
    }
}

impl From<ProgramSwitchError> for InstallDataCoordinationError {
    fn from(error: ProgramSwitchError) -> Self {
        Self::ProgramSwitch(error.code())
    }
}

impl From<UpgradeCoordinatorError> for InstallDataCoordinationError {
    fn from(error: UpgradeCoordinatorError) -> Self {
        Self::Upgrade(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallProductFinalizationError {
    InstallFinalization(InstallFinalizationError),
    Upgrade(UpgradeCoordinatorError),
    InvalidBinding,
    InvalidState,
}

impl fmt::Display for InstallProductFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InstallFinalization(_) => "product install finalization failed",
            Self::Upgrade(_) => "product data finalization evidence is unavailable",
            Self::InvalidBinding => "product finalization receipts are not bound",
            Self::InvalidState => "product finalization states are inconsistent",
        })
    }
}

impl std::error::Error for InstallProductFinalizationError {}

impl From<InstallFinalizationError> for InstallProductFinalizationError {
    fn from(error: InstallFinalizationError) -> Self {
        Self::InstallFinalization(error)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resume_upgrade_install_finalization<V>(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_store: &UpgradeReceiptStore,
    upgrade_guard: &UpgradeProcessGuard,
    upgrade_receipt: &UpgradeReceipt,
    program_validation: &mut V,
) -> Result<(), InstallProductFinalizationError>
where
    V: InstallProgramValidationPort,
{
    validate_finalization_binding(
        install_store,
        install_guard,
        install_receipt,
        manager,
        input_method,
        upgrade_store,
        upgrade_guard,
        upgrade_receipt,
    )?;
    let mut port = UpgradeInstallFinalizationPort {
        upgrade_store,
        upgrade_guard,
        upgrade_receipt,
        program_validation,
    };
    resume_install_finalization(
        install_store,
        install_guard,
        install_receipt,
        manager,
        input_method,
        &mut port,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn resume_install_data_coordination<U, V>(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_store: &UpgradeReceiptStore,
    upgrade_guard: &UpgradeProcessGuard,
    upgrade_receipt: &mut UpgradeReceipt,
    reported_available_bytes: u64,
    upgrade_port: &mut U,
    program_validation: &mut V,
) -> Result<InstallDataCoordinationSummary, InstallDataCoordinationError>
where
    U: UpgradeCoordinatorPort,
    V: InstallProgramValidationPort,
{
    validate_binding(
        install_store,
        install_guard,
        install_receipt,
        manager,
        input_method,
        upgrade_store,
        upgrade_guard,
        upgrade_receipt,
    )?;

    if install_receipt.state() == InstallState::ProgramsCommitted {
        if upgrade_receipt.state() != UpgradeState::Preflighted {
            return Err(InstallDataCoordinationError::InvalidState);
        }
        prove_installed_target(
            upgrade_port,
            program_validation,
            manager,
            input_method,
            install_receipt,
            UpgradeCoordinatorCheckpoint::BeforeQuiesced,
        )?;
        advance_and_persist(
            install_store,
            install_guard,
            install_receipt,
            InstallState::DataCoordinating,
        )?;
    }

    validate_state_pair(install_receipt.state(), upgrade_receipt.state())?;
    let upgrade_summary = {
        let mut bound_port = ProgramBoundUpgradePort {
            upgrade_port,
            program_validation,
            manager,
            input_method,
            install_receipt,
            installed_validation_failed: false,
        };
        let result = upgrade_store.resume_userdb_upgrade(
            upgrade_guard,
            upgrade_receipt,
            reported_available_bytes,
            &mut bound_port,
        );
        if bound_port.installed_validation_failed {
            return Err(InstallDataCoordinationError::ProgramValidationNotProven(
                InstallProgramValidationStage::InstalledTarget,
            ));
        }
        result?
    };

    match upgrade_summary.disposition() {
        UpgradeCoordinatorDisposition::Completed => settle_completed_data(
            install_store,
            install_guard,
            install_receipt,
            manager,
            input_method,
            upgrade_port,
            program_validation,
        ),
        UpgradeCoordinatorDisposition::AbortedPreserved
        | UpgradeCoordinatorDisposition::RolledBack => settle_failed_data(
            install_store,
            install_guard,
            install_receipt,
            manager,
            input_method,
            upgrade_port,
            program_validation,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_binding(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_store: &UpgradeReceiptStore,
    upgrade_guard: &UpgradeProcessGuard,
    upgrade_receipt: &UpgradeReceipt,
) -> Result<(), InstallDataCoordinationError> {
    install_store.verify_current(install_guard, install_receipt)?;
    manager.verify_binding(install_store, install_guard, install_receipt)?;
    input_method.verify_binding(install_store, install_guard, install_receipt)?;
    if install_receipt.operation_kind() != InstallOperationKind::Upgrade
        || install_receipt.operation_id() != upgrade_receipt.operation_id()
        || manager.operation_id() != install_receipt.operation_id()
        || input_method.operation_id() != install_receipt.operation_id()
        || manager.component() != ProgramComponent::Manager
        || input_method.component() != ProgramComponent::InputMethod
        || !data_root_matches(install_receipt, upgrade_store.data_root_identity())
        || !data_root_matches(install_receipt, upgrade_receipt_data_root(upgrade_receipt)?)
        || !release_pair_matches(install_receipt, upgrade_receipt)
    {
        return Err(InstallDataCoordinationError::InvalidBinding);
    }
    upgrade_store
        .verify_current(upgrade_guard, upgrade_receipt)
        .map_err(|error| {
            InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::Filesystem(error.code()))
        })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_finalization_binding(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_store: &UpgradeReceiptStore,
    upgrade_guard: &UpgradeProcessGuard,
    upgrade_receipt: &UpgradeReceipt,
) -> Result<(), InstallProductFinalizationError> {
    install_store
        .verify_current(install_guard, install_receipt)
        .map_err(|error| {
            InstallProductFinalizationError::InstallFinalization(
                InstallFinalizationError::InstallFilesystem(error.code()),
            )
        })?;
    manager
        .verify_binding(install_store, install_guard, install_receipt)
        .map_err(|error| {
            InstallProductFinalizationError::InstallFinalization(
                InstallFinalizationError::ProgramSwitch(error.code()),
            )
        })?;
    input_method
        .verify_binding(install_store, install_guard, install_receipt)
        .map_err(|error| {
            InstallProductFinalizationError::InstallFinalization(
                InstallFinalizationError::ProgramSwitch(error.code()),
            )
        })?;
    if install_receipt.operation_kind() != InstallOperationKind::Upgrade
        || !matches!(
            install_receipt.state(),
            InstallState::DataSettled | InstallState::FinalVerified | InstallState::Completed
        )
        || upgrade_receipt.state() != UpgradeState::Completed
        || install_receipt.operation_id() != upgrade_receipt.operation_id()
        || manager.operation_id() != install_receipt.operation_id()
        || input_method.operation_id() != install_receipt.operation_id()
        || manager.component() != ProgramComponent::Manager
        || input_method.component() != ProgramComponent::InputMethod
        || !data_root_matches(install_receipt, upgrade_store.data_root_identity())
        || !data_root_matches(
            install_receipt,
            upgrade_receipt_data_root(upgrade_receipt)
                .map_err(|_| InstallProductFinalizationError::InvalidBinding)?,
        )
        || !release_pair_matches(install_receipt, upgrade_receipt)
    {
        return Err(InstallProductFinalizationError::InvalidBinding);
    }
    upgrade_store
        .verify_current(upgrade_guard, upgrade_receipt)
        .map_err(|error| {
            InstallProductFinalizationError::Upgrade(UpgradeCoordinatorError::Filesystem(
                error.code(),
            ))
        })?;
    Ok(())
}

struct UpgradeInstallFinalizationPort<'a, V> {
    upgrade_store: &'a UpgradeReceiptStore,
    upgrade_guard: &'a UpgradeProcessGuard,
    upgrade_receipt: &'a UpgradeReceipt,
    program_validation: &'a mut V,
}

impl<V> InstallFinalizationPort for UpgradeInstallFinalizationPort<'_, V>
where
    V: InstallProgramValidationPort,
{
    fn validate_final_state(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        _stage: InstallFinalizationValidationStage,
    ) -> bool {
        self.upgrade_receipt.state() == UpgradeState::Completed
            && receipt.operation_id() == self.upgrade_receipt.operation_id()
            && data_root_matches(receipt, self.upgrade_store.data_root_identity())
            && upgrade_receipt_data_root(self.upgrade_receipt)
                .is_ok_and(|root| data_root_matches(receipt, root))
            && release_pair_matches(receipt, self.upgrade_receipt)
            && self
                .upgrade_store
                .verify_current(self.upgrade_guard, self.upgrade_receipt)
                .is_ok()
            && self
                .program_validation
                .validate_installed_targets(manager, input_method, receipt)
    }
}

fn validate_state_pair(
    install_state: InstallState,
    upgrade_state: UpgradeState,
) -> Result<(), InstallDataCoordinationError> {
    let valid = match install_state {
        InstallState::DataCoordinating => true,
        InstallState::DataSettled => upgrade_state == UpgradeState::Completed,
        InstallState::RollbackRequired
        | InstallState::ProgramsRestored
        | InstallState::RolledBack => matches!(
            upgrade_state,
            UpgradeState::AbortedPreserved | UpgradeState::RolledBack
        ),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(InstallDataCoordinationError::InvalidState)
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_completed_data<U, V>(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_port: &mut U,
    program_validation: &mut V,
) -> Result<InstallDataCoordinationSummary, InstallDataCoordinationError>
where
    U: UpgradeCoordinatorPort,
    V: InstallProgramValidationPort,
{
    match install_receipt.state() {
        InstallState::DataCoordinating => {
            prove_installed_target(
                upgrade_port,
                program_validation,
                manager,
                input_method,
                install_receipt,
                UpgradeCoordinatorCheckpoint::BeforeCompletion,
            )?;
            advance_and_persist(
                install_store,
                install_guard,
                install_receipt,
                InstallState::DataSettled,
            )?;
        }
        InstallState::DataSettled => {}
        _ => return Err(InstallDataCoordinationError::InvalidState),
    }
    Ok(InstallDataCoordinationSummary {
        disposition: InstallDataCoordinationDisposition::DataSettled,
    })
}

#[allow(clippy::too_many_arguments)]
fn settle_failed_data<U, V>(
    install_store: &InstallReceiptStore,
    install_guard: &InstallProcessGuard,
    install_receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    upgrade_port: &mut U,
    program_validation: &mut V,
) -> Result<InstallDataCoordinationSummary, InstallDataCoordinationError>
where
    U: UpgradeCoordinatorPort,
    V: InstallProgramValidationPort,
{
    if install_receipt.state() == InstallState::DataCoordinating {
        prove_installed_target(
            upgrade_port,
            program_validation,
            manager,
            input_method,
            install_receipt,
            UpgradeCoordinatorCheckpoint::BeforeRollbackRestore,
        )?;
        let mut next = install_receipt.clone();
        next.require_rollback(InstallFailureCode::DataCoordinationFailed)
            .map_err(|_| InstallDataCoordinationError::InvalidState)?;
        install_store.persist(install_guard, &next)?;
        *install_receipt = next;
    }

    if install_receipt.state() == InstallState::RollbackRequired {
        restore_program_source(install_store, install_guard, manager, install_receipt)?;
        restore_program_source(install_store, install_guard, input_method, install_receipt)?;
        if !program_validation.validate_restored_sources(manager, input_method, install_receipt) {
            return Err(InstallDataCoordinationError::ProgramValidationNotProven(
                InstallProgramValidationStage::RestoredSource,
            ));
        }
        finish_program_restore(
            install_store,
            install_guard,
            manager,
            input_method,
            install_receipt,
        )?;
    }

    if install_receipt.state() == InstallState::ProgramsRestored {
        if !program_validation.validate_restored_sources(manager, input_method, install_receipt) {
            return Err(InstallDataCoordinationError::ProgramValidationNotProven(
                InstallProgramValidationStage::RestoredSource,
            ));
        }
        let mut next = install_receipt.clone();
        next.mark_rolled_back()
            .map_err(|_| InstallDataCoordinationError::InvalidState)?;
        install_store.persist(install_guard, &next)?;
        *install_receipt = next;
    }

    if install_receipt.state() != InstallState::RolledBack {
        return Err(InstallDataCoordinationError::InvalidState);
    }
    Ok(InstallDataCoordinationSummary {
        disposition: InstallDataCoordinationDisposition::RolledBack,
    })
}

fn prove_installed_target<U, V>(
    upgrade_port: &mut U,
    program_validation: &mut V,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    install_receipt: &InstallReceipt,
    checkpoint: UpgradeCoordinatorCheckpoint,
) -> Result<(), InstallDataCoordinationError>
where
    U: UpgradeCoordinatorPort,
    V: InstallProgramValidationPort,
{
    if !upgrade_port.confirm_quiescence(checkpoint) {
        return Err(InstallDataCoordinationError::Upgrade(
            UpgradeCoordinatorError::QuiescenceNotProven(checkpoint),
        ));
    }
    if !program_validation.validate_installed_targets(manager, input_method, install_receipt) {
        return Err(InstallDataCoordinationError::ProgramValidationNotProven(
            InstallProgramValidationStage::InstalledTarget,
        ));
    }
    Ok(())
}

fn advance_and_persist(
    store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &mut InstallReceipt,
    next_state: InstallState,
) -> Result<(), InstallDataCoordinationError> {
    let mut next = receipt.clone();
    next.advance(next_state)
        .map_err(|_| InstallDataCoordinationError::InvalidState)?;
    store.persist(guard, &next)?;
    *receipt = next;
    Ok(())
}

fn data_root_matches(
    install_receipt: &InstallReceipt,
    data_root: &radishlex_ime_product_upgrade::UpgradeArtifactIdentity,
) -> bool {
    let install_root = install_receipt.root_identity();
    data_root.device_id() == install_root.device_id()
        && data_root.inode() == install_root.inode()
        && data_root.owner_id() == install_root.owner_id()
        && data_root.mode() == install_root.mode()
}

fn upgrade_receipt_data_root(
    receipt: &UpgradeReceipt,
) -> Result<&radishlex_ime_product_upgrade::UpgradeArtifactIdentity, InstallDataCoordinationError> {
    receipt
        .artifacts()
        .iter()
        .find(|artifact| {
            artifact.slot() == radishlex_ime_product_upgrade::UpgradeArtifactSlot::DataRoot
        })
        .ok_or(InstallDataCoordinationError::InvalidBinding)
}

fn release_pair_matches(
    install_receipt: &InstallReceipt,
    upgrade_receipt: &UpgradeReceipt,
) -> bool {
    let Some(source) = install_receipt.source_product() else {
        return false;
    };
    let Some(target) = install_receipt.target_product() else {
        return false;
    };
    releases_match(source.release(), upgrade_receipt.source_release())
        && releases_match(target.release(), upgrade_receipt.target_release())
}

fn releases_match(left: &ProductRelease, right: &UpgradeProductRelease) -> bool {
    left.product_version() == right.product_version() && left.build_number() == right.build_number()
}

struct ProgramBoundUpgradePort<'a, U, V> {
    upgrade_port: &'a mut U,
    program_validation: &'a mut V,
    manager: &'a ProgramSwitchStore,
    input_method: &'a ProgramSwitchStore,
    install_receipt: &'a InstallReceipt,
    installed_validation_failed: bool,
}

impl<U, V> UpgradeCoordinatorPort for ProgramBoundUpgradePort<'_, U, V>
where
    U: UpgradeCoordinatorPort,
    V: InstallProgramValidationPort,
{
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        if !self.upgrade_port.confirm_quiescence(checkpoint) {
            return false;
        }
        let valid = self.program_validation.validate_installed_targets(
            self.manager,
            self.input_method,
            self.install_receipt,
        );
        self.installed_validation_failed |= !valid;
        valid
    }

    fn validate_candidate(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        self.upgrade_port
            .validate_candidate(target_release, target_schema_version)
    }

    fn validate_post_switch(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        self.upgrade_port
            .validate_post_switch(target_release, target_schema_version)
    }

    fn validate_restored_source(
        &mut self,
        source_release: &UpgradeProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        self.upgrade_port
            .validate_restored_source(source_release, source_schema_version)
    }
}

#[cfg(test)]
mod tests;
