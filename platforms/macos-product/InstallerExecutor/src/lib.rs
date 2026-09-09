//! Restartable execution of authorized RadishLex macOS Installer intents.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;

use radishlex_ime_product_install::{
    commit_program_removal, commit_program_target, finish_source_preservation,
    finish_target_staging, preserve_program_source, resume_install_finalization,
    InstallFilesystemErrorCode, InstallFinalizationError, InstallFinalizationPort,
    InstallOperationKind, InstallProcessGuard, InstallProgramValidationPort, InstallReceipt,
    InstallReceiptStore, InstallState, ProductArtifactIdentity, ProgramComponent,
    ProgramSwitchErrorCode, ProgramSwitchStore,
};
pub use radishlex_ime_product_upgrade::UpgradeCoordinatorPort;
use radishlex_ime_product_upgrade::{UpgradeReceipt, UpgradeReceiptStore};
use radishlex_macos_installer_driver::{AuthorizedInstallerIntent, InstallerAction};
use radishlex_macos_product_install::{MacOsInstallAdapterErrorCode, MacOsProductInstallAdapter};
use radishlex_macos_product_install_coordinator::{
    resume_install_data_coordination, resume_upgrade_install_finalization,
    InstallDataCoordinationDisposition, InstallDataCoordinationError,
    InstallProductFinalizationError,
};
use radishlex_macos_upgrade_coordinator::{MacOsProductPreflightAdapter, MacOsUpgradeAdapterError};

mod bootstrap;
mod recovery;
pub use bootstrap::{
    bootstrap_upgrade_receipt, BootstrappedUpgradeReceipt, UpgradeBootstrapError,
    UpgradeBootstrapErrorCode,
};
pub use recovery::inspect_installer_view_with_recovery;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallerPreflightEvidence {
    available_bytes: u64,
}

impl InstallerPreflightEvidence {
    pub fn new(available_bytes: u64) -> Result<Self, InstallerExecutionError> {
        if available_bytes == 0 {
            return Err(InstallerExecutionError::PreflightNotProven);
        }
        Ok(Self { available_bytes })
    }

    pub const fn available_bytes(self) -> u64 {
        self.available_bytes
    }
}

pub trait InstallerPreflightPort {
    fn inspect_preflight(
        &mut self,
        target_product: &ProductArtifactIdentity,
    ) -> Result<InstallerPreflightEvidence, InstallerExecutionError>;
}

impl InstallerPreflightPort for MacOsProductPreflightAdapter {
    fn inspect_preflight(
        &mut self,
        target_product: &ProductArtifactIdentity,
    ) -> Result<InstallerPreflightEvidence, InstallerExecutionError> {
        if !self.matches_release(
            target_product.release().product_version(),
            target_product.release().build_number(),
        ) {
            return Err(InstallerExecutionError::Preflight(
                MacOsUpgradeAdapterError::ProductChanged,
            ));
        }
        let summary = self
            .inspect_preflight()
            .map_err(InstallerExecutionError::Preflight)?;
        InstallerPreflightEvidence::new(summary.available_bytes())
    }
}

pub trait InstallerOperationIdSource {
    fn next_operation_id(&mut self) -> Result<String, InstallerExecutionError>;
}

#[derive(Debug, Default)]
pub struct SystemInstallerOperationIdSource;

impl InstallerOperationIdSource for SystemInstallerOperationIdSource {
    fn next_operation_id(&mut self) -> Result<String, InstallerExecutionError> {
        let mut bytes = [0_u8; 16];
        getrandom::getrandom(&mut bytes)
            .map_err(|_| InstallerExecutionError::OperationIdUnavailable)?;
        let mut output = String::with_capacity(32);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}")
                .map_err(|_| InstallerExecutionError::OperationIdUnavailable)?;
        }
        Ok(output)
    }
}

pub trait InstallerProgramPort: InstallFinalizationPort + InstallProgramValidationPort {
    fn target_product(&self) -> &ProductArtifactIdentity;

    fn verify_recovery_material(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        Err(InstallerExecutionError::InvalidState)
    }

    fn open_program_store(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        component: ProgramComponent,
        receipt: &InstallReceipt,
    ) -> Result<ProgramSwitchStore, InstallerExecutionError>;

    fn verify_and_record_source(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError>;

    fn prepare_and_record_staged(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError>;
}

impl InstallerProgramPort for MacOsProductInstallAdapter {
    fn target_product(&self) -> &ProductArtifactIdentity {
        self.target_product()
    }

    fn verify_recovery_material(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        for program in [manager, input_method] {
            self.verify_program_recovery_material(program, receipt)
                .map_err(|error| InstallerExecutionError::InstallAdapter(error.code()))?;
        }
        Ok(())
    }

    fn open_program_store(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        component: ProgramComponent,
        receipt: &InstallReceipt,
    ) -> Result<ProgramSwitchStore, InstallerExecutionError> {
        self.open_program_store(receipt_store, guard, component, receipt)
            .map_err(|error| InstallerExecutionError::InstallAdapter(error.code()))
    }

    fn verify_and_record_source(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        self.verify_and_record_source(receipt_store, guard, program_store, receipt)
            .map_err(|error| InstallerExecutionError::InstallAdapter(error.code()))
    }

    fn prepare_and_record_staged(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        self.prepare_and_record_staged(receipt_store, guard, program_store, receipt)
            .map_err(|error| InstallerExecutionError::InstallAdapter(error.code()))
    }
}

pub struct InstallerUpgradeContext<'a, P> {
    install_store: &'a InstallReceiptStore,
    install_guard: &'a InstallProcessGuard,
    install_receipt: &'a mut InstallReceipt,
    manager: &'a ProgramSwitchStore,
    input_method: &'a ProgramSwitchStore,
    available_bytes: u64,
    programs: &'a mut P,
}

pub trait InstallerUpgradeExecution<P: InstallerProgramPort> {
    fn resume_upgrade(
        &mut self,
        context: InstallerUpgradeContext<'_, P>,
    ) -> Result<(), InstallerExecutionError>;
}

pub struct BoundUpgradeExecution<'a, U> {
    upgrade_store: &'a UpgradeReceiptStore,
    upgrade_receipt: &'a mut UpgradeReceipt,
    upgrade_port: &'a mut U,
}

impl<'a, U> BoundUpgradeExecution<'a, U> {
    pub fn new(
        upgrade_store: &'a UpgradeReceiptStore,
        upgrade_receipt: &'a mut UpgradeReceipt,
        upgrade_port: &'a mut U,
    ) -> Self {
        Self {
            upgrade_store,
            upgrade_receipt,
            upgrade_port,
        }
    }
}

impl<P, U> InstallerUpgradeExecution<P> for BoundUpgradeExecution<'_, U>
where
    P: InstallerProgramPort,
    U: UpgradeCoordinatorPort,
{
    fn resume_upgrade(
        &mut self,
        context: InstallerUpgradeContext<'_, P>,
    ) -> Result<(), InstallerExecutionError> {
        let InstallerUpgradeContext {
            install_store,
            install_guard,
            install_receipt,
            manager,
            input_method,
            available_bytes,
            programs,
        } = context;
        let upgrade_guard = self
            .upgrade_store
            .acquire_guard()
            .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?;
        self.upgrade_store
            .verify_current(&upgrade_guard, self.upgrade_receipt)
            .map_err(|error| InstallerExecutionError::UpgradeFilesystem(error.code()))?;
        if recovery::is_pre_switch_abort(self.upgrade_receipt) {
            return Err(InstallerExecutionError::InvalidIntent);
        }
        if matches!(
            install_receipt.state(),
            InstallState::DataSettled | InstallState::FinalVerified | InstallState::Completed
        ) {
            resume_upgrade_install_finalization(
                install_store,
                install_guard,
                install_receipt,
                manager,
                input_method,
                self.upgrade_store,
                &upgrade_guard,
                self.upgrade_receipt,
                programs,
            )
            .map_err(InstallerExecutionError::UpgradeFinalization)?;
            return Ok(());
        }
        let summary = resume_install_data_coordination(
            install_store,
            install_guard,
            install_receipt,
            manager,
            input_method,
            self.upgrade_store,
            &upgrade_guard,
            self.upgrade_receipt,
            available_bytes,
            self.upgrade_port,
            programs,
        )
        .map_err(InstallerExecutionError::DataCoordination)?;
        if summary.disposition() == InstallDataCoordinationDisposition::DataSettled {
            resume_upgrade_install_finalization(
                install_store,
                install_guard,
                install_receipt,
                manager,
                input_method,
                self.upgrade_store,
                &upgrade_guard,
                self.upgrade_receipt,
                programs,
            )
            .map_err(InstallerExecutionError::UpgradeFinalization)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NoUpgradeExecution;

impl<P: InstallerProgramPort> InstallerUpgradeExecution<P> for NoUpgradeExecution {
    fn resume_upgrade(
        &mut self,
        _context: InstallerUpgradeContext<'_, P>,
    ) -> Result<(), InstallerExecutionError> {
        Err(InstallerExecutionError::UpgradeContextRequired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerExecutionDisposition {
    AwaitingUserAction,
    Completed,
    AbortedPreserved,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallerExecutionSummary {
    disposition: InstallerExecutionDisposition,
    operation_kind: InstallOperationKind,
    state: InstallState,
}

impl InstallerExecutionSummary {
    pub const fn disposition(self) -> InstallerExecutionDisposition {
        self.disposition
    }

    pub const fn operation_kind(self) -> InstallOperationKind {
        self.operation_kind
    }

    pub const fn state(self) -> InstallState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerExecutionError {
    InvalidIntent,
    InvalidState,
    PreflightNotProven,
    OperationIdUnavailable,
    UpgradeContextRequired,
    InstallFilesystem(InstallFilesystemErrorCode),
    ProgramSwitch(ProgramSwitchErrorCode),
    InstallAdapter(MacOsInstallAdapterErrorCode),
    Preflight(MacOsUpgradeAdapterError),
    DataCoordination(InstallDataCoordinationError),
    InstallFinalization(InstallFinalizationError),
    UpgradeFinalization(InstallProductFinalizationError),
    UpgradeFilesystem(radishlex_ime_product_upgrade::UpgradeFilesystemErrorCode),
    UpgradeBootstrap(UpgradeBootstrapErrorCode),
    RecoveryEvidence(radishlex_macos_product_install::RecoveryEvidenceError),
}

impl fmt::Display for InstallerExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidIntent => "Installer intent is stale or invalid",
            Self::InvalidState => "Installer transaction state is inconsistent",
            Self::PreflightNotProven => "Installer platform preflight was not proven",
            Self::OperationIdUnavailable => "Installer operation identity is unavailable",
            Self::UpgradeContextRequired => "Installer upgrade context is unavailable",
            Self::InstallFilesystem(_) => "Installer receipt filesystem failure",
            Self::ProgramSwitch(_) => "Installer program switch failure",
            Self::InstallAdapter(_) => "Installer product adapter failure",
            Self::Preflight(_) => "Installer product preflight failure",
            Self::DataCoordination(_) => "Installer data coordination failure",
            Self::InstallFinalization(_) => "Installer finalization failure",
            Self::UpgradeFinalization(_) => "Installer upgrade finalization failure",
            Self::UpgradeFilesystem(_) => "Installer upgrade receipt filesystem failure",
            Self::UpgradeBootstrap(_) => "Installer upgrade receipt bootstrap failure",
            Self::RecoveryEvidence(_) => "Installer recovery preservation evidence failure",
        })
    }
}

impl std::error::Error for InstallerExecutionError {}

impl InstallerExecutionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidIntent => "invalid_intent",
            Self::InvalidState => "invalid_state",
            Self::PreflightNotProven => "preflight_not_proven",
            Self::OperationIdUnavailable => "operation_id_unavailable",
            Self::UpgradeContextRequired => "upgrade_context_required",
            Self::InstallFilesystem(_) => "install_filesystem_failure",
            Self::ProgramSwitch(_) => "program_switch_failure",
            Self::InstallAdapter(_) => "install_adapter_failure",
            Self::Preflight(_) => "preflight_failure",
            Self::DataCoordination(_) => "data_coordination_failure",
            Self::InstallFinalization(_) => "install_finalization_failure",
            Self::UpgradeFinalization(_) => "upgrade_finalization_failure",
            Self::UpgradeFilesystem(_) => "upgrade_filesystem_failure",
            Self::UpgradeBootstrap(_) => "upgrade_bootstrap_failure",
            Self::RecoveryEvidence(_) => "recovery_evidence_failure",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_authorized_intent_with_upgrade_bootstrap<P, F, I, U>(
    intent: AuthorizedInstallerIntent,
    install_store: &InstallReceiptStore,
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
    programs: &mut P,
    preflight: &mut F,
    operation_ids: &mut I,
    upgrade_port: &mut U,
) -> Result<InstallerExecutionSummary, InstallerExecutionError>
where
    P: InstallerProgramPort,
    F: InstallerPreflightPort,
    I: InstallerOperationIdSource,
    U: UpgradeCoordinatorPort,
{
    let data_root = data_root.as_ref();
    if intent.action() == InstallerAction::AbortPreSwitchUpgrade {
        return recovery::execute_pre_switch_recovery(
            intent,
            install_store,
            data_root,
            expected_owner_id,
            programs,
            preflight,
            upgrade_port,
        );
    }
    // An existing preservation baseline belongs to the explicit recovery action.
    // A stale ordinary Resume intent must not bypass its checks.
    if intent.operation_kind() == Some(InstallOperationKind::Upgrade) && intent.resume_existing() {
        let outer = install_store
            .load_for_recovery_inspection()
            .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?
            .ok_or(InstallerExecutionError::InvalidState)?;
        if radishlex_macos_product_install::PreSwitchRecoveryEvidence::load(data_root, &outer)
            .map_err(InstallerExecutionError::RecoveryEvidence)?
            .is_some()
        {
            return Err(InstallerExecutionError::InvalidIntent);
        }
    }
    if intent.operation_kind() != Some(InstallOperationKind::Upgrade) {
        return execute_authorized_intent(
            intent,
            install_store,
            programs,
            preflight,
            operation_ids,
            &mut NoUpgradeExecution,
        );
    }
    if intent.action() == InstallerAction::BeginUpgrade {
        let summary = execute_authorized_intent(
            intent,
            install_store,
            programs,
            preflight,
            operation_ids,
            &mut NoUpgradeExecution,
        )?;
        let outer = install_store
            .load()
            .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?
            .ok_or(InstallerExecutionError::InvalidState)?;
        bootstrap_upgrade_receipt(&outer, data_root, expected_owner_id)
            .map_err(|error| InstallerExecutionError::UpgradeBootstrap(error.code()))?;
        return Ok(summary);
    }
    let outer = install_store
        .load()
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?
        .ok_or(InstallerExecutionError::InvalidState)?;
    let bootstrapped = bootstrap_upgrade_receipt(&outer, data_root, expected_owner_id)
        .map_err(|error| InstallerExecutionError::UpgradeBootstrap(error.code()))?;
    let (upgrade_store, mut upgrade_receipt) = bootstrapped.into_parts();
    let mut bound = BoundUpgradeExecution::new(&upgrade_store, &mut upgrade_receipt, upgrade_port);
    execute_authorized_intent(
        intent,
        install_store,
        programs,
        preflight,
        operation_ids,
        &mut bound,
    )
}

pub fn execute_authorized_intent<P, F, I, U>(
    intent: AuthorizedInstallerIntent,
    install_store: &InstallReceiptStore,
    programs: &mut P,
    preflight: &mut F,
    operation_ids: &mut I,
    upgrade: &mut U,
) -> Result<InstallerExecutionSummary, InstallerExecutionError>
where
    P: InstallerProgramPort,
    F: InstallerPreflightPort,
    I: InstallerOperationIdSource,
    U: InstallerUpgradeExecution<P>,
{
    if intent.action() == InstallerAction::Refresh || !intent.requires_platform_preflight() {
        return Err(InstallerExecutionError::InvalidIntent);
    }
    let guard = install_store
        .acquire_guard()
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;
    let current = install_store
        .load_guarded(&guard)
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;

    if begins_new_operation(intent.action()) {
        let receipt = begin_operation(
            intent,
            install_store,
            &guard,
            current.as_ref(),
            programs.target_product(),
            preflight,
            operation_ids,
        )?;
        return summary(&receipt);
    }

    let mut receipt = resume_existing(intent, current)?;
    if receipt.state() == InstallState::Prepared {
        let evidence = preflight.inspect_preflight(programs.target_product())?;
        if evidence.available_bytes() == 0 {
            return Err(InstallerExecutionError::PreflightNotProven);
        }
        receipt
            .advance(InstallState::Quiesced)
            .map_err(|_| InstallerExecutionError::InvalidState)?;
        install_store
            .persist(&guard, &receipt)
            .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;
    }

    let evidence = preflight.inspect_preflight(programs.target_product())?;
    drive_operation(
        install_store,
        &guard,
        &mut receipt,
        evidence.available_bytes(),
        programs,
        upgrade,
    )?;
    summary(&receipt)
}

fn begin_operation<F, I>(
    intent: AuthorizedInstallerIntent,
    install_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    current: Option<&InstallReceipt>,
    target_product: &ProductArtifactIdentity,
    preflight: &mut F,
    operation_ids: &mut I,
) -> Result<InstallReceipt, InstallerExecutionError>
where
    F: InstallerPreflightPort,
    I: InstallerOperationIdSource,
{
    let operation_kind = intent
        .operation_kind()
        .ok_or(InstallerExecutionError::InvalidIntent)?;
    let previous = match current {
        Some(receipt) if receipt.state().is_terminal() => Some(receipt),
        Some(_) => return Err(InstallerExecutionError::InvalidIntent),
        None => None,
    };
    let source = previous
        .and_then(InstallReceipt::installed_product)
        .cloned();
    let target = match operation_kind {
        InstallOperationKind::RemovePrograms => None,
        _ => Some(target_product.clone()),
    };
    let expected_action =
        action_for_new_operation(operation_kind, intent.action(), source.is_none());
    if !expected_action {
        return Err(InstallerExecutionError::InvalidIntent);
    }
    preflight.inspect_preflight(target_product)?;
    let operation_id = operation_ids.next_operation_id()?;
    let receipt = InstallReceipt::new(
        operation_id,
        previous.map(|receipt| receipt.operation_id().to_owned()),
        operation_kind,
        install_store.root_identity().clone(),
        source,
        target,
    )
    .map_err(|_| InstallerExecutionError::InvalidState)?;
    install_store
        .persist(guard, &receipt)
        .map_err(|error| InstallerExecutionError::InstallFilesystem(error.code()))?;
    Ok(receipt)
}

fn resume_existing(
    intent: AuthorizedInstallerIntent,
    current: Option<InstallReceipt>,
) -> Result<InstallReceipt, InstallerExecutionError> {
    let receipt = current.ok_or(InstallerExecutionError::InvalidIntent)?;
    if receipt.state().is_terminal()
        || intent.operation_kind() != Some(receipt.operation_kind())
        || !matches!(
            intent.action(),
            InstallerAction::ConfirmQuiescence | InstallerAction::ResumeOperation
        )
        || (intent.action() == InstallerAction::ConfirmQuiescence
            && receipt.state() != InstallState::Prepared)
        || (intent.action() == InstallerAction::ResumeOperation
            && receipt.state() == InstallState::Prepared)
    {
        return Err(InstallerExecutionError::InvalidIntent);
    }
    Ok(receipt)
}

fn drive_operation<P, U>(
    install_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &mut InstallReceipt,
    available_bytes: u64,
    programs: &mut P,
    upgrade: &mut U,
) -> Result<(), InstallerExecutionError>
where
    P: InstallerProgramPort,
    U: InstallerUpgradeExecution<P>,
{
    let manager =
        programs.open_program_store(install_store, guard, ProgramComponent::Manager, receipt)?;
    let input_method = programs.open_program_store(
        install_store,
        guard,
        ProgramComponent::InputMethod,
        receipt,
    )?;
    loop {
        match receipt.state() {
            InstallState::Quiesced => {
                for store in [&manager, &input_method] {
                    if receipt.operation_kind() != InstallOperationKind::FirstInstall {
                        programs.verify_and_record_source(install_store, guard, store, receipt)?;
                    }
                    if receipt.operation_kind() != InstallOperationKind::RemovePrograms {
                        programs.prepare_and_record_staged(install_store, guard, store, receipt)?;
                    }
                }
                if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
                    preserve_sources(install_store, guard, &manager, &input_method, receipt)?;
                } else {
                    finish_target_staging(install_store, guard, &manager, &input_method, receipt)
                        .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))?;
                }
            }
            InstallState::TargetStaged => {
                if receipt.operation_kind() == InstallOperationKind::FirstInstall {
                    commit_program_target(install_store, guard, &manager, receipt)
                        .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))?;
                } else {
                    preserve_sources(install_store, guard, &manager, &input_method, receipt)?;
                }
            }
            InstallState::SourcePreserved => {
                commit_component(install_store, guard, &manager, receipt)?;
            }
            InstallState::ManagerCommitted => {
                commit_component(install_store, guard, &input_method, receipt)?;
            }
            InstallState::ProgramsCommitted
            | InstallState::DataCoordinating
            | InstallState::DataSettled
            | InstallState::RollbackRequired
            | InstallState::ProgramsRestored
            | InstallState::FinalVerified
                if receipt.operation_kind() == InstallOperationKind::Upgrade =>
            {
                upgrade.resume_upgrade(InstallerUpgradeContext {
                    install_store,
                    install_guard: guard,
                    install_receipt: receipt,
                    manager: &manager,
                    input_method: &input_method,
                    available_bytes,
                    programs,
                })?;
            }
            InstallState::ProgramsCommitted | InstallState::FinalVerified => {
                resume_install_finalization(
                    install_store,
                    guard,
                    receipt,
                    &manager,
                    &input_method,
                    programs,
                )
                .map_err(InstallerExecutionError::InstallFinalization)?;
            }
            InstallState::Completed | InstallState::AbortedPreserved | InstallState::RolledBack => {
                return Ok(())
            }
            InstallState::Prepared => return Err(InstallerExecutionError::InvalidState),
            InstallState::DataCoordinating
            | InstallState::DataSettled
            | InstallState::RollbackRequired
            | InstallState::ProgramsRestored => {
                return Err(InstallerExecutionError::InvalidState);
            }
        }
    }
}

fn preserve_sources(
    install_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), InstallerExecutionError> {
    for store in [manager, input_method] {
        preserve_program_source(install_store, guard, store, receipt)
            .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))?;
    }
    finish_source_preservation(install_store, guard, manager, input_method, receipt)
        .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))
}

fn commit_component(
    install_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), InstallerExecutionError> {
    let result = if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
        commit_program_removal(install_store, guard, store, receipt)
    } else {
        commit_program_target(install_store, guard, store, receipt)
    };
    result.map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))
}

fn begins_new_operation(action: InstallerAction) -> bool {
    matches!(
        action,
        InstallerAction::BeginFirstInstall
            | InstallerAction::BeginUpgrade
            | InstallerAction::BeginRepair
            | InstallerAction::RetryOperation
            | InstallerAction::RemovePrograms
    )
}

fn action_for_new_operation(
    kind: InstallOperationKind,
    action: InstallerAction,
    source_absent: bool,
) -> bool {
    matches!(
        (kind, action, source_absent),
        (
            InstallOperationKind::FirstInstall,
            InstallerAction::BeginFirstInstall | InstallerAction::RetryOperation,
            true
        ) | (
            InstallOperationKind::Upgrade,
            InstallerAction::BeginUpgrade | InstallerAction::RetryOperation,
            false
        ) | (
            InstallOperationKind::Repair,
            InstallerAction::BeginRepair | InstallerAction::RetryOperation,
            false
        ) | (
            InstallOperationKind::RemovePrograms,
            InstallerAction::RemovePrograms | InstallerAction::RetryOperation,
            false
        )
    )
}

fn summary(receipt: &InstallReceipt) -> Result<InstallerExecutionSummary, InstallerExecutionError> {
    let disposition = match receipt.state() {
        InstallState::Prepared => InstallerExecutionDisposition::AwaitingUserAction,
        InstallState::Completed => InstallerExecutionDisposition::Completed,
        InstallState::AbortedPreserved => InstallerExecutionDisposition::AbortedPreserved,
        InstallState::RolledBack => InstallerExecutionDisposition::RolledBack,
        _ => return Err(InstallerExecutionError::InvalidState),
    };
    Ok(InstallerExecutionSummary {
        disposition,
        operation_kind: receipt.operation_kind(),
        state: receipt.state(),
    })
}

#[cfg(test)]
mod tests;
