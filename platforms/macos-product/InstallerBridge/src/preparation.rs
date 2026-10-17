//! Dedicated production dispatch for source preparation and cancellation preservation.
use super::*;
use radishlex_macos_installer_executor::CancellationTaskAuthorizationError;
use radishlex_macos_installer_executor::{
    authorize_cancellation_preservation, authorize_preparation_resume,
    authorize_source_preparation, inspect_source_preparation, prepare_upgrade_source,
    preserve_preparation_cancellation, SourcePreparationObservation,
};
use std::time::Duration;

pub(super) fn is_action(action: InstallerAction) -> bool {
    matches!(
        action,
        InstallerAction::BeginSourcePreparation
            | InstallerAction::ResumeSourcePreparation
            | InstallerAction::PreservePreparationCancellation
    )
}

fn verify_products(
    programs: &MacOsProductInstallAdapter,
    store: &InstallReceiptStore,
    observation: &SourcePreparationObservation,
) -> Result<(), InstallerStableError> {
    let rejected = InstallerStableError::ProductIdentityUnavailable;
    programs
        .verify_data_root(store.root_identity())
        .map_err(|_| rejected)?;
    if programs.revalidate_target_product().as_ref() != Ok(observation.target()) {
        return Err(rejected);
    }
    programs
        .verify_installed_product(Some(observation.source()))
        .map_err(|_| rejected)?;
    programs
        .verify_data_root(store.root_identity())
        .map_err(|_| rejected)?;
    Ok(())
}

fn verify_preparation_hosts(
    programs: &MacOsProductInstallAdapter,
    target_root: &Path,
    observation: &SourcePreparationObservation,
) -> Result<(), InstallerStableError> {
    let rejected = InstallerStableError::ProductIdentityUnavailable;
    // Require the historical product hosts that the later continuation needs,
    // without running them, opening SQLite, or inferring runtime qualification.
    let source_root = programs
        .upgrade_source_product_root(observation.source().release())
        .ok_or(rejected)?;
    MacOsUpgradeCoordinatorAdapter::load(source_root, target_root).map_err(|_| rejected)?;
    Ok(())
}

pub(super) fn project(
    store: &InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    target_root: &Path,
    view: InstallerViewSnapshot,
) -> Result<InstallerViewSnapshot, InstallerStableError> {
    let observation = inspect_source_preparation(store, programs.target_product())
        .map_err(|_| InstallerStableError::InvalidReceipt)?
        .ok_or(InstallerStableError::InvalidReceipt)?;
    verify_products(programs, store, &observation)?;
    if inspect_source_preparation(store, programs.target_product())
        .map_err(|_| InstallerStableError::InvalidReceipt)?
        .as_ref()
        != Some(&observation)
    {
        return Err(InstallerStableError::RootIdentityChanged);
    }
    let hosts = verify_preparation_hosts(programs, target_root, &observation);
    if inspect_source_preparation(store, programs.target_product())
        .map_err(|_| InstallerStableError::InvalidReceipt)?
        .as_ref()
        != Some(&observation)
    {
        return Err(InstallerStableError::RootIdentityChanged);
    }
    let projected = observation.project(view);
    if hosts.is_err() {
        return if observation.offers(InstallerAction::PreservePreparationCancellation) {
            Ok(projected.with_cancellation_preservation())
        } else {
            Err(InstallerStableError::ProductIdentityUnavailable)
        };
    }
    Ok(projected)
}

pub(super) fn dispatch(
    store: &InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
    target_root: &Path,
    action: InstallerAction,
    confirmation: InstallerUserAuthorization,
) -> Result<(), InstallerBridgeError> {
    let rejected =
        || InstallerBridgeError::Authorization(InstallerAuthorizationError::ActionNotOffered);
    let observation = inspect_source_preparation(store, programs.target_product())
        .map_err(InstallerBridgeError::Prepared)?
        .ok_or_else(rejected)?;
    if !observation.offers(action) {
        return Err(rejected());
    }
    verify_products(programs, store, &observation).map_err(|_| rejected())?;
    // Reobserve after the native product verifier; authorization never uses a
    // stale UI snapshot or a product identity/path supplied across the ABI.
    if inspect_source_preparation(store, programs.target_product())
        .map_err(InstallerBridgeError::Prepared)?
        .as_ref()
        != Some(&observation)
    {
        return Err(rejected());
    }
    if action != InstallerAction::PreservePreparationCancellation {
        verify_preparation_hosts(programs, target_root, &observation).map_err(|_| rejected())?;
    }
    if inspect_source_preparation(store, programs.target_product())
        .map_err(InstallerBridgeError::Prepared)?
        .as_ref()
        != Some(&observation)
    {
        return Err(rejected());
    }
    let task = authorize_task(&observation, action, confirmation)?;
    match task {
        PreparationTask::Prepare(auth, _control) => {
            prepare_upgrade_source(*auth, store, programs, preflight)
                .map_err(InstallerBridgeError::Prepared)?;
        }
        PreparationTask::Cancel(auth, _control) => {
            preserve_preparation_cancellation(*auth, store, programs, preflight)
                .map_err(InstallerBridgeError::Cancellation)?;
        }
    }
    Ok(())
}

/// This exact selector is used by production dispatch after fresh observations.
/// Controls live inside the selected task until its synchronous call returns.
pub(super) enum PreparationTask {
    Prepare(
        Box<radishlex_macos_installer_executor::PreparationTaskAuthorization>,
        radishlex_macos_installer_executor::PreparationTaskControl,
    ),
    Cancel(
        Box<radishlex_macos_installer_executor::CancellationPreservationAuthorization>,
        radishlex_macos_installer_executor::CancellationTaskControl,
    ),
}
pub(super) fn authorize_task(
    observation: &SourcePreparationObservation,
    action: InstallerAction,
    confirmation: InstallerUserAuthorization,
) -> Result<PreparationTask, InstallerBridgeError> {
    let rejected =
        || InstallerBridgeError::Authorization(InstallerAuthorizationError::ActionNotOffered);
    let map_grant = |error| match error {
        CancellationTaskAuthorizationError::UserConfirmation(error) => {
            InstallerBridgeError::Authorization(error)
        }
        _ => rejected(),
    };
    if !observation.offers(action) {
        return Err(rejected());
    }
    match action {
        InstallerAction::BeginSourcePreparation => {
            let (auth, control) = authorize_source_preparation(
                observation.previous().ok_or_else(rejected)?,
                observation.target(),
                confirmation,
                Duration::from_secs(300),
            )
            .map_err(map_grant)?;
            Ok(PreparationTask::Prepare(Box::new(auth), control))
        }
        InstallerAction::ResumeSourcePreparation => {
            let (auth, control) = authorize_preparation_resume(
                observation
                    .maintenance()
                    .ok_or_else(rejected)?
                    .preparation(),
                observation.source(),
                observation.target(),
                confirmation,
                Duration::from_secs(300),
            )
            .map_err(map_grant)?;
            Ok(PreparationTask::Prepare(Box::new(auth), control))
        }
        InstallerAction::PreservePreparationCancellation => {
            let (auth, control) = authorize_cancellation_preservation(
                observation.maintenance().ok_or_else(rejected)?,
                observation.source(),
                observation.target(),
                confirmation,
                Duration::from_secs(300),
            )
            .map_err(map_grant)?;
            Ok(PreparationTask::Cancel(Box::new(auth), control))
        }
        _ => Err(rejected()),
    }
}
