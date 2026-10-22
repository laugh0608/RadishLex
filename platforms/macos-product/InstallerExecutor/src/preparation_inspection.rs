//! Fresh read-only selection for the dedicated preparation product actions.
//! This observation grants no task authority and never creates a guard or pager.
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallOperationKind, InstallReceipt,
    InstallReceiptStore, ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    inspect_maintenance, LifecycleReleaseReceipt, MaintenanceInspection, MaintenancePhase,
    PreparationJournalStore, UpgradeReceipt, UpgradeState, VerifiedDataRoot,
};
use radishlex_macos_installer_driver::{InstallerAction, InstallerViewSnapshot};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;

use crate::preparation_execution::outer;
use crate::PreparationExecutionError as Error;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreparationObservation {
    start: PreparationStart,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparationStart {
    Begin(Box<InstallReceipt>),
    Existing(Box<MaintenanceInspection>),
}
impl SourcePreparationObservation {
    pub fn source(&self) -> &ProductArtifactIdentity {
        &self.source
    }
    pub fn target(&self) -> &ProductArtifactIdentity {
        &self.target
    }
    pub fn previous(&self) -> Option<&InstallReceipt> {
        match &self.start {
            PreparationStart::Begin(value) => Some(value),
            _ => None,
        }
    }
    pub fn maintenance(&self) -> Option<&MaintenanceInspection> {
        match &self.start {
            PreparationStart::Existing(value) => Some(value),
            _ => None,
        }
    }
    pub fn project(&self, view: InstallerViewSnapshot) -> InstallerViewSnapshot {
        match &self.start {
            PreparationStart::Begin(_) => view.with_source_preparation(false, false),
            PreparationStart::Existing(maintenance) => match maintenance.phase() {
                MaintenancePhase::Preparing => {
                    view.with_source_preparation(true, !maintenance.preparation().handoff_started())
                }
                MaintenancePhase::CancellationRequested
                | MaintenancePhase::CancellationFinishingSource
                | MaintenancePhase::CancellationPreserving => view.with_cancellation_preservation(),
                _ => view,
            },
        }
    }
    pub fn offers(&self, action: InstallerAction) -> bool {
        match &self.start {
            PreparationStart::Begin(_) => action == InstallerAction::BeginSourcePreparation,
            PreparationStart::Existing(maintenance) => match maintenance.phase() {
                MaintenancePhase::Preparing => {
                    action == InstallerAction::ResumeSourcePreparation
                        || (action == InstallerAction::PreservePreparationCancellation
                            && !maintenance.preparation().handoff_started())
                }
                MaintenancePhase::CancellationRequested
                | MaintenancePhase::CancellationFinishingSource
                | MaintenancePhase::CancellationPreserving => {
                    action == InstallerAction::PreservePreparationCancellation
                }
                _ => false,
            },
        }
    }
}

/// Target is the freshly verified sealed payload, never an AppKit argument.
/// Both the outer and maintenance domains are read twice, including inode proof.
pub fn inspect_source_preparation(
    install: &InstallReceiptStore,
    target: &ProductArtifactIdentity,
) -> Result<Option<SourcePreparationObservation>> {
    let read = || {
        inspect_maintenance(
            install.data_root_path(),
            install.root_identity().owner_id(),
            &MacOsPreparationHasher,
        )
    };
    let maintenance = read()?;
    // Checks the outer guard independently of whether core has moved its receipt.
    let current_outer = install.load_for_recovery_inspection()?;
    let active = if maintenance
        .as_ref()
        .and_then(MaintenanceInspection::cancellation_outer)
        .is_some()
    {
        None
    } else {
        Some(outer::active_identity(install)?)
    };
    let observation = if let Some(current) = maintenance.as_ref() {
        if !matches!(
            current.phase(),
            MaintenancePhase::Preparing
                | MaintenancePhase::CancellationRequested
                | MaintenancePhase::CancellationFinishingSource
                | MaintenancePhase::CancellationPreserving
        ) {
            return Ok(None);
        }
        if current.preparation().binding().target_schema_version
            != radishlex_ime_userdb::UserDb::supported_schema_version()
        {
            return Err(Error::InvalidBinding);
        }
        let previous = if let Some(binding) = current.cancellation_outer() {
            crate::maintenance_inspection::verify_cancellation_outer(install, current)
                .map_err(|_| Error::InvalidBinding)?;
            let previous = InstallReceipt::decode(binding.previous_bytes())
                .map_err(|_| Error::InvalidBinding)?;
            let source = previous.installed_product().ok_or(Error::InvalidBinding)?;
            let prep = current.preparation().binding();
            validate_preparation_cancellation_outer(
                binding.previous_bytes(),
                binding.new_bytes(),
                &prep.operation_id,
                prep.previous_install_operation_id
                    .as_deref()
                    .ok_or(Error::InvalidBinding)?,
                install.root_identity(),
                source,
                target,
            )
            .map_err(|_| Error::InvalidBinding)?;
            previous
        } else {
            let current_outer = install
                .load_for_recovery_inspection()?
                .ok_or(Error::InvalidBinding)?;
            crate::maintenance_inspection::verify_outer(current, &current_outer)
                .map_err(|_| Error::InvalidBinding)?;
            outer::verify_preparation(install, current.preparation())?
        };
        let source = previous
            .installed_product()
            .ok_or(Error::InvalidBinding)?
            .clone();
        if !crate::preparation_execution::matches_products(current.preparation(), &source, target) {
            return Err(Error::InvalidBinding);
        }
        Some(SourcePreparationObservation {
            start: PreparationStart::Existing(Box::new(current.clone())),
            source,
            target: target.clone(),
        })
    } else {
        let previous = install
            .load_for_recovery_inspection()?
            .ok_or(Error::InvalidBinding)?;
        let source = previous.installed_product().ok_or(Error::InvalidBinding)?;
        let probe = if previous.operation_id() == "11111111111111111111111111111111" {
            "22222222222222222222222222222222"
        } else {
            "11111111111111111111111111111111"
        };
        let expected = InstallReceipt::new(
            probe,
            Some(previous.operation_id().into()),
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .map_err(|_| Error::InvalidBinding)?;
        if !previous.state().is_terminal()
            || previous.manual_recovery_required()
            || !expected.can_replace(&previous)
        {
            return Err(Error::InvalidBinding);
        }
        let state = install.data_root_path().join(".radishlex-upgrade-v1");
        if outer::present(&state)? {
            let journal = PreparationJournalStore::open_existing(VerifiedDataRoot::verify(
                install.data_root_path(),
                install.root_identity().owner_id(),
            )?)?;
            let (data, lifecycle) =
                journal.inspect_preparation_predecessor(&MacOsPreparationHasher)?;
            if data
                .as_ref()
                .is_some_and(|r| !predecessor_matches(r, source))
            {
                return Err(Error::InvalidBinding);
            }
            if let Some(LifecycleReleaseReceipt::Cancellation(proof)) = lifecycle {
                if proof.legacy_outer_operation_id() != previous.operation_id() {
                    return Err(Error::InvalidBinding);
                }
                outer::verify(
                    install,
                    previous.operation_id(),
                    &proof.archive().outer().previous_install_evidence(),
                    &previous.encode().map_err(|_| Error::InvalidBinding)?,
                )?;
            } else {
                outer::require_new_history_slot(install, previous.operation_id())?;
            }
        } else {
            if outer::present(
                &install
                    .data_root_path()
                    .join(".radishlex-upgrade-history-v1"),
            )? {
                return Err(Error::InvalidBinding);
            }
            outer::require_new_history_slot(install, previous.operation_id())?;
        }
        Some(SourcePreparationObservation {
            source: source.clone(),
            start: PreparationStart::Begin(Box::new(previous)),
            target: target.clone(),
        })
    };
    if install.load_for_recovery_inspection()? != current_outer
        || read()? != maintenance
        || active.is_some_and(|identity| outer::active_identity(install).ok() != Some(identity))
    {
        return Err(Error::InvalidBinding);
    }
    Ok(observation)
}
fn predecessor_matches(receipt: &UpgradeReceipt, source: &ProductArtifactIdentity) -> bool {
    let release = if receipt.state() == UpgradeState::Completed {
        receipt.target_release()
    } else {
        receipt.source_release()
    };
    release.product_version() == source.release().product_version()
        && release.build_number() == source.release().build_number()
}
