use std::fmt;

use crate::{
    InstallFilesystemError, InstallFilesystemErrorCode, InstallOperationKind, InstallProcessGuard,
    InstallReceipt, InstallReceiptStore, InstallState, ProgramComponent, ProgramSwitchError,
    ProgramSwitchErrorCode, ProgramSwitchStore,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFinalizationValidationStage {
    BeforeFinalVerified,
    BeforeCompleted,
}

pub trait InstallFinalizationPort {
    fn validate_final_state(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        stage: InstallFinalizationValidationStage,
    ) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFinalizationError {
    InstallFilesystem(InstallFilesystemErrorCode),
    ProgramSwitch(ProgramSwitchErrorCode),
    InvalidState,
    FinalStateNotProven(InstallFinalizationValidationStage),
}

impl fmt::Display for InstallFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InstallFilesystem(_) => "install finalization filesystem failure",
            Self::ProgramSwitch(_) => "install finalization program binding failure",
            Self::InvalidState => "install finalization state is inconsistent",
            Self::FinalStateNotProven(_) => "install final state was not proven",
        })
    }
}

impl std::error::Error for InstallFinalizationError {}

impl From<InstallFilesystemError> for InstallFinalizationError {
    fn from(error: InstallFilesystemError) -> Self {
        Self::InstallFilesystem(error.code())
    }
}

impl From<ProgramSwitchError> for InstallFinalizationError {
    fn from(error: ProgramSwitchError) -> Self {
        Self::ProgramSwitch(error.code())
    }
}

pub fn resume_install_finalization<P>(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    port: &mut P,
) -> Result<(), InstallFinalizationError>
where
    P: InstallFinalizationPort,
{
    receipt_store.verify_current(guard, receipt)?;
    manager.verify_binding(receipt_store, guard, receipt)?;
    input_method.verify_binding(receipt_store, guard, receipt)?;
    if manager.component() != ProgramComponent::Manager
        || input_method.component() != ProgramComponent::InputMethod
        || !valid_finalization_state(receipt.operation_kind(), receipt.state())
    {
        return Err(InstallFinalizationError::InvalidState);
    }

    if receipt.state() == InstallState::Completed {
        if !port.validate_final_state(
            manager,
            input_method,
            receipt,
            InstallFinalizationValidationStage::BeforeCompleted,
        ) {
            return Err(InstallFinalizationError::FinalStateNotProven(
                InstallFinalizationValidationStage::BeforeCompleted,
            ));
        }
        return Ok(());
    }

    let stage = match receipt.state() {
        InstallState::ProgramsCommitted | InstallState::DataSettled => {
            InstallFinalizationValidationStage::BeforeFinalVerified
        }
        InstallState::FinalVerified => InstallFinalizationValidationStage::BeforeCompleted,
        _ => return Err(InstallFinalizationError::InvalidState),
    };
    persist_validated_transition(
        receipt_store,
        guard,
        receipt,
        manager,
        input_method,
        port,
        stage,
    )?;

    if receipt.state() == InstallState::FinalVerified {
        persist_validated_transition(
            receipt_store,
            guard,
            receipt,
            manager,
            input_method,
            port,
            InstallFinalizationValidationStage::BeforeCompleted,
        )?;
    }

    if receipt.state() != InstallState::Completed {
        return Err(InstallFinalizationError::InvalidState);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_validated_transition<P>(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &mut InstallReceipt,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    port: &mut P,
    stage: InstallFinalizationValidationStage,
) -> Result<(), InstallFinalizationError>
where
    P: InstallFinalizationPort,
{
    if !port.validate_final_state(manager, input_method, receipt, stage) {
        return Err(InstallFinalizationError::FinalStateNotProven(stage));
    }
    let mut next = receipt.clone();
    let next_state = match stage {
        InstallFinalizationValidationStage::BeforeFinalVerified => InstallState::FinalVerified,
        InstallFinalizationValidationStage::BeforeCompleted => InstallState::Completed,
    };
    next.advance(next_state)
        .map_err(|_| InstallFinalizationError::InvalidState)?;
    receipt_store.persist(guard, &next)?;
    *receipt = next;
    Ok(())
}

fn valid_finalization_state(kind: InstallOperationKind, state: InstallState) -> bool {
    match kind {
        InstallOperationKind::Upgrade => matches!(
            state,
            InstallState::DataSettled | InstallState::FinalVerified | InstallState::Completed
        ),
        InstallOperationKind::FirstInstall
        | InstallOperationKind::Repair
        | InstallOperationKind::RemovePrograms => matches!(
            state,
            InstallState::ProgramsCommitted | InstallState::FinalVerified | InstallState::Completed
        ),
    }
}
