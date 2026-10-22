//! Strict product binding after handoff; no bootstrap or execution permission.
use radishlex_ime_product_install::{
    InstallOperationKind, InstallProcessGuard, InstallReceipt, InstallReceiptStore, InstallState,
};
use radishlex_ime_product_upgrade::{
    PreparationJournalStore, PreparationReceipt, UpgradeProcessGuard, UpgradeReceipt,
    UpgradeReceiptStore, UpgradeState, VerifiedDataRoot,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;

use crate::preparation_execution::outer;
use crate::PreparationExecutionError as Error;

/// Retain the caller's outer guard and return the acquired inner guard with the
/// strictly rebound existing receipt. This only verifies metadata and files;
/// product/quiescence checks and fresh task authorization remain mandatory for
/// execution. Ordinary bootstrap and its empty-predecessor rule are unchanged.
pub fn rebind_prepared_upgrade(
    install: &InstallReceiptStore,
    outer_guard: &InstallProcessGuard,
    expected: &PreparationReceipt,
) -> Result<(UpgradeReceiptStore, UpgradeProcessGuard, UpgradeReceipt), Error> {
    let active_identity = outer::active_identity(install)?;
    let current = install
        .load_guarded(outer_guard)?
        .ok_or(Error::InvalidBinding)?;
    if current.state().is_terminal()
        || (current.manual_recovery_required()
            && !matches!(
                current.state(),
                InstallState::RollbackRequired | InstallState::ProgramsRestored
            ))
    {
        return Err(Error::InvalidPhase);
    }
    verify_outer(install, expected, &current)?;
    let journal = PreparationJournalStore::open_existing(VerifiedDataRoot::verify(
        install.data_root_path(),
        install.root_identity().owner_id(),
    )?)?;
    let inner_guard = journal.acquire_guard()?;
    let (store, receipt) =
        journal.rebind_handed_off_upgrade(&inner_guard, expected, &MacOsPreparationHasher)?;
    if receipt.target_schema_version() != UserDb::supported_schema_version() {
        return Err(Error::InvalidBinding);
    }
    verify_state_pair(&current, &receipt)?;
    verify_outer(install, expected, &current)?;
    if install.load_guarded(outer_guard)?.as_ref() != Some(&current)
        || outer::active_identity(install)? != active_identity
    {
        return Err(Error::InvalidBinding);
    }
    store.verify_current(&inner_guard, &receipt)?;
    Ok((store, inner_guard, receipt))
}

pub(super) fn verify_state_pair(
    outer: &InstallReceipt,
    inner: &UpgradeReceipt,
) -> Result<(), Error> {
    let valid = match outer.state() {
        InstallState::Prepared
        | InstallState::Quiesced
        | InstallState::TargetStaged
        | InstallState::SourcePreserved
        | InstallState::ManagerCommitted
        | InstallState::ProgramsCommitted
        | InstallState::AbortedPreserved => inner.state() == UpgradeState::Preflighted,
        InstallState::DataCoordinating => true,
        InstallState::DataSettled | InstallState::FinalVerified | InstallState::Completed => {
            inner.state() == UpgradeState::Completed
        }
        // Program rollback can also start before the data coordinator ran.
        InstallState::RollbackRequired
        | InstallState::ProgramsRestored
        | InstallState::RolledBack => {
            matches!(
                inner.state(),
                UpgradeState::Preflighted
                    | UpgradeState::AbortedPreserved
                    | UpgradeState::RolledBack
            )
        }
    };
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidPhase)
    }
}

pub(super) fn verify_outer(
    install: &InstallReceiptStore,
    preparation: &PreparationReceipt,
    current: &InstallReceipt,
) -> Result<(), Error> {
    let binding = preparation.binding();
    let previous = outer::verify_preparation(install, preparation)?;
    let target = current.target_product().ok_or(Error::InvalidBinding)?;
    if current.operation_kind() != InstallOperationKind::Upgrade
        || current.operation_id() != binding.operation_id
        || current.previous_operation_id() != Some(previous.operation_id())
        || current.root_identity() != install.root_identity()
        || current.source_product() != previous.installed_product()
        || target.release().product_version() != binding.target_release.product_version()
        || target.release().build_number() != binding.target_release.build_number()
        || target.product_manifest_sha256() != binding.target_product_sha256
    {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}
