//! Join persisted maintenance observations to the strict current outer receipt.
use std::io::Cursor;
use std::path::Path;

use radishlex_ime_product_install::{
    InstallOperationKind, InstallReceipt, InstallReceiptStore, InstallState,
    ProductArtifactIdentity, VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::{
    inspect_maintenance, MaintenanceInspection, MaintenancePhase, PreparationBinding,
    PreparationHasher, PreparationJournalError, SourcePreparationError, UpgradeFilesystemErrorCode,
};
use radishlex_macos_installer_driver::{
    InstallerProductSituation, InstallerStableError, InstallerViewPhase, InstallerViewSnapshot,
};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;

pub(super) fn inspect(
    root: &Path,
    owner: u32,
    situation: InstallerProductSituation,
    snapshot: InstallerViewSnapshot,
) -> Option<InstallerViewSnapshot> {
    let result = inspect_maintenance(root, owner, &MacOsPreparationHasher);
    let observation = match result {
        Ok(None) => return None,
        Ok(Some(value)) => value,
        Err(error) => return Some(snapshot.with_recovery_inspection_error(map_error(error))),
    };
    if !matches!(
        snapshot.stable_error(),
        InstallerStableError::None
            | InstallerStableError::ProductIdentityUnavailable
            | InstallerStableError::ManualRecoveryRequired
    ) {
        return Some(snapshot.with_recovery_inspection_error(snapshot.stable_error()));
    }
    let checked = || -> Result<(), InstallerStableError> {
        let store = InstallReceiptStore::open_existing(
            VerifiedInstallRoot::verify(root, owner)
                .map_err(|_| InstallerStableError::UnsafeDataRoot)?,
        )
        .map_err(|_| InstallerStableError::InvalidReceipt)?;
        let identity = super::preparation_execution::outer::active_identity(&store)
            .map_err(|_| InstallerStableError::InvalidReceipt)?;
        let outer = store
            .load_for_recovery_inspection()
            .map_err(|_| InstallerStableError::InvalidReceipt)?
            .ok_or(InstallerStableError::InvalidReceipt)?;
        if matches!(
            observation.phase(),
            MaintenancePhase::PreparationHandedOff | MaintenancePhase::PreparationTerminalRelease
        ) {
            super::preparation_rebinding::verify_outer(&store, observation.preparation(), &outer)
                .map_err(|_| InstallerStableError::InvalidReceipt)?;
            super::preparation_rebinding::verify_state_pair(
                &outer,
                observation
                    .handed_off_receipt()
                    .ok_or(InstallerStableError::InvalidReceipt)?,
            )
            .map_err(|_| InstallerStableError::InvalidReceipt)?;
            if observation
                .terminal_release()
                .is_some_and(|r| !super::prepared_execution::release_pair(&outer, r))
            {
                return Err(InstallerStableError::InvalidReceipt);
            }
        } else {
            verify_outer(&observation, &outer)?;
        }
        // Reobserve both domains: a view is not a lock and grants no intent.
        if inspect_maintenance(root, owner, &MacOsPreparationHasher).map_err(map_error)?
            != Some(observation.clone())
            || store
                .load_for_recovery_inspection()
                .map_err(|_| InstallerStableError::InvalidReceipt)?
                .as_ref()
                != Some(&outer)
            || super::preparation_execution::outer::active_identity(&store)
                .map_err(|_| InstallerStableError::InvalidReceipt)?
                != identity
        {
            return Err(InstallerStableError::RootIdentityChanged);
        }
        if matches!(
            observation.phase(),
            MaintenancePhase::PreparationHandedOff | MaintenancePhase::PreparationTerminalRelease
        ) {
            super::preparation_rebinding::verify_outer(&store, observation.preparation(), &outer)
                .map_err(|_| InstallerStableError::InvalidReceipt)?;
        }
        Ok(())
    };
    if let Err(error) = checked() {
        return Some(snapshot.with_recovery_inspection_error(error));
    }
    if matches!(
        observation.phase(),
        MaintenancePhase::PreparationHandedOff | MaintenancePhase::PreparationTerminalRelease
    ) {
        let store = match VerifiedInstallRoot::verify(root, owner)
            .and_then(InstallReceiptStore::open_existing)
        {
            Ok(store) => store,
            Err(_) => {
                return Some(
                    snapshot.with_recovery_inspection_error(InstallerStableError::InvalidReceipt),
                )
            }
        };
        let continuation = super::inspect_prepared_upgrade_continuation(&store);
        return Some(match continuation {
            Ok(Some(current)) => snapshot.with_prepared_continuation(current.install_receipt()),
            _ => snapshot.with_recovery_inspection_error(InstallerStableError::InvalidReceipt),
        });
    }
    if observation.phase() == MaintenancePhase::CancellationReleased
        && situation == InstallerProductSituation::IdentityUnavailable
    {
        return Some(
            snapshot
                .with_recovery_inspection_error(InstallerStableError::ProductIdentityUnavailable),
        );
    }
    let phase = match observation.phase() {
        MaintenancePhase::Preparing
        | MaintenancePhase::PreparationHandedOff
        | MaintenancePhase::PreparationTerminalRelease => InstallerViewPhase::PreparingSource,
        MaintenancePhase::CancellationRequested => InstallerViewPhase::CancellationRequested,
        MaintenancePhase::CancellationFinishingSource => {
            InstallerViewPhase::CancellationFinishingSource
        }
        MaintenancePhase::CancellationPreserving => InstallerViewPhase::CancellationPreserving,
        MaintenancePhase::CancellationAwaitingQualification => {
            InstallerViewPhase::CancellationAwaitingQualification
        }
        MaintenancePhase::CancellationReleased => InstallerViewPhase::CancellationCompleted,
    };
    Some(snapshot.with_maintenance(phase))
}

fn verify_outer(
    observation: &MaintenanceInspection,
    outer: &InstallReceipt,
) -> Result<(), InstallerStableError> {
    let binding = observation.preparation().binding();
    let root = outer.root_identity();
    let rejected = InstallerStableError::InvalidReceipt;
    if root.device_id() != binding.data_root.device_id
        || root.inode() != binding.data_root.inode
        || root.owner_id() != binding.data_root.owner_id
        || root.mode() != binding.data_root.mode
    {
        return Err(rejected);
    }
    if outer.operation_id() == binding.operation_id {
        if matches!(
            observation.phase(),
            MaintenancePhase::CancellationAwaitingQualification
                | MaintenancePhase::CancellationReleased
        ) || outer.operation_kind() != InstallOperationKind::Upgrade
            || outer.state() != InstallState::Prepared
            || !outer.artifacts().is_empty()
            || !matches_product(outer.source_product(), binding, true)
            || !matches_product(outer.target_product(), binding, false)
        {
            return Err(rejected);
        }
    } else {
        if binding.previous_install_operation_id.as_deref() != Some(outer.operation_id())
            || !outer.state().is_terminal()
            || !matches_product(outer.installed_product(), binding, true)
        {
            return Err(rejected);
        }
        let bytes = outer.encode().map_err(|_| rejected)?;
        let digest = MacOsPreparationHasher
            .sha256(&mut Cursor::new(bytes))
            .map_err(|_| rejected)?;
        let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        if binding.previous_install_receipt_sha256.as_ref() != Some(&hash) {
            return Err(rejected);
        }
    }
    Ok(())
}

fn matches_product(
    product: Option<&ProductArtifactIdentity>,
    binding: &PreparationBinding,
    source: bool,
) -> bool {
    let Some(product) = product else {
        return false;
    };
    let (release, digest) = if source {
        (&binding.source_release, &binding.source_product_sha256)
    } else {
        (&binding.target_release, &binding.target_product_sha256)
    };
    product.release().product_version() == release.product_version()
        && product.release().build_number() == release.build_number()
        && product.product_manifest_sha256() == digest
}

fn map_error(error: SourcePreparationError) -> InstallerStableError {
    match error {
        SourcePreparationError::Journal(PreparationJournalError::Filesystem(
            UpgradeFilesystemErrorCode::OperationAlreadyActive,
        )) => InstallerStableError::OperationActive,
        SourcePreparationError::Io
        | SourcePreparationError::Journal(PreparationJournalError::Io) => InstallerStableError::Io,
        _ => InstallerStableError::InvalidReceipt,
    }
}

/// Call after taking the outer guard and before ordinary v1 mutation. Old
/// authorized intents cannot bypass newly recorded maintenance or its failures.
pub(super) fn require_no_maintenance(
    store: &InstallReceiptStore,
) -> Result<(), super::InstallerExecutionError> {
    match inspect_maintenance(
        store.data_root_path(),
        store.root_identity().owner_id(),
        &MacOsPreparationHasher,
    ) {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(super::InstallerExecutionError::InvalidIntent),
        Err(_) => Err(super::InstallerExecutionError::InvalidState),
    }
}

#[cfg(test)]
#[path = "maintenance_inspection_tests.rs"]
mod tests;
