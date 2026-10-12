//! Dedicated prepared dispatch: reobserve exact records, then hold both guards
//! through execution. No UI path/identity or ordinary bootstrap fallback.
use super::*;
use radishlex_macos_installer_executor::{
    admit_prepared_continuation, authorize_prepared_continuation,
    authorize_prepared_upgrade_resume, continue_prepared_upgrade,
    inspect_prepared_upgrade_continuation, resume_prepared_upgrade, PreparedUpgradeData,
};
use std::time::Duration;

pub(super) fn dispatch(
    store: &InstallReceiptStore,
    programs: &mut MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
    target_root: &Path,
    confirmation: InstallerUserAuthorization,
) -> Result<(), InstallerBridgeError> {
    let failure = || {
        InstallerBridgeError::Prepared(
            radishlex_macos_installer_executor::PreparationExecutionError::InvalidBinding,
        )
    };
    let observation = inspect_prepared_upgrade_continuation(store)
        .map_err(InstallerBridgeError::Prepared)?
        .ok_or_else(failure)?;
    programs
        .verify_prepared_upgrade_material_readonly(observation.install_receipt())
        .map_err(|_| failure())?;
    let source = observation
        .install_receipt()
        .source_product()
        .ok_or_else(failure)?;
    let source_root = programs
        .upgrade_source_product_root(source.release())
        .ok_or_else(failure)?;
    let mut upgrade =
        MacOsUpgradeCoordinatorAdapter::load(source_root, target_root).map_err(|_| failure())?;
    let grant_error =
        |_| InstallerBridgeError::Authorization(InstallerAuthorizationError::ActionNotOffered);
    if observation.install_receipt().state() == InstallState::Prepared {
        if !matches!(observation.data(),PreparedUpgradeData::Active(r) if r.state()==radishlex_ime_product_upgrade::UpgradeState::Preflighted)
        {
            return Err(failure());
        }
        let (auth, _control) = authorize_prepared_continuation(
            observation.install_receipt(),
            observation.preparation(),
            confirmation,
            Duration::from_secs(300),
        )
        .map_err(grant_error)?;
        let task = admit_prepared_continuation(auth, store, programs, preflight)
            .map_err(InstallerBridgeError::Prepared)?;
        continue_prepared_upgrade(task, programs, preflight, &mut upgrade)
            .map_err(InstallerBridgeError::Prepared)?;
    } else {
        let (auth, _control) = authorize_prepared_upgrade_resume(
            observation.install_receipt(),
            observation.preparation(),
            observation.data().clone(),
            confirmation,
            Duration::from_secs(300),
        )
        .map_err(grant_error)?;
        resume_prepared_upgrade(auth, store, programs, preflight, &mut upgrade)
            .map_err(InstallerBridgeError::Prepared)?;
    }
    Ok(())
}
