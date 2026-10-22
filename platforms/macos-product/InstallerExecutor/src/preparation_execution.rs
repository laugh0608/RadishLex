//! Product task entry for WAL source preparation. Both guards span reservation,
//! SQLite maintenance, old-slot preservation and the new v1 handoff.
use radishlex_ime_product_install::{
    InstallFilesystemErrorCode, InstallOperationKind, InstallProcessGuard, InstallReceipt,
    InstallReceiptStore, ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    LifecycleReleaseReceipt, PreparationBinding, PreparationDirectoryIdentity,
    PreparationInstallEvidence, PreparationJournalStore, PreparationPhase, PreparationReceipt,
    ProductRelease, SourcePreparationCheckpoint, SourcePreparationError, SourcePreparationPort,
    UpgradeReceipt, UpgradeReceiptStore, VerifiedDataRoot,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_installer_driver::InstallerUserAuthorization;
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_upgrade_coordinator::{MacOsPreparationHasher, MacOsProductPreflightAdapter};
use std::time::Duration;

use crate::cancellation_authorization::CancellationTaskGrant;
pub use crate::CancellationTaskAuthorizationError as PreparationTaskAuthorizationError;
pub use crate::CancellationTaskControl as PreparationTaskControl;
use crate::{InstallerOperationIdSource, SystemInstallerOperationIdSource};

#[path = "preparation_authority.rs"]
mod authority;
#[path = "preparation_outer.rs"]
pub(super) mod outer;
use authority::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationExecutionError {
    TaskAuthorization,
    InvalidBinding,
    InvalidPhase,
    OperationId,
    InstallFilesystem(InstallFilesystemErrorCode),
    Preparation(SourcePreparationError),
    ProductObservation,
    NativeObservation,
    Execution(crate::InstallerExecutionError),
    Coordination(radishlex_ime_product_upgrade::UpgradeCoordinatorError),
    Io,
}
impl PreparationExecutionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TaskAuthorization => "preparation_task_authorization",
            Self::InvalidBinding => "preparation_binding_changed",
            Self::InvalidPhase => "preparation_phase_rejected",
            Self::OperationId => "preparation_operation_id_unavailable",
            Self::InstallFilesystem(_) => "preparation_install_filesystem",
            Self::Preparation(_) => "preparation_source_rejected",
            Self::ProductObservation => "preparation_product_observation",
            Self::NativeObservation => "preparation_native_observation",
            Self::Execution(_) => "prepared_upgrade_execution",
            Self::Coordination(_) => "prepared_upgrade_coordination",
            Self::Io => "preparation_persistence_failed",
        }
    }
}
impl std::fmt::Display for PreparationExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for PreparationExecutionError {}
impl From<SourcePreparationError> for PreparationExecutionError {
    fn from(value: SourcePreparationError) -> Self {
        Self::Preparation(value)
    }
}
impl From<radishlex_ime_product_upgrade::PreparationJournalError> for PreparationExecutionError {
    fn from(value: radishlex_ime_product_upgrade::PreparationJournalError) -> Self {
        Self::Preparation(value.into())
    }
}
impl From<radishlex_ime_product_upgrade::UpgradeFilesystemError> for PreparationExecutionError {
    fn from(value: radishlex_ime_product_upgrade::UpgradeFilesystemError) -> Self {
        Self::Preparation(value.into())
    }
}
impl From<radishlex_ime_product_install::InstallFilesystemError> for PreparationExecutionError {
    fn from(value: radishlex_ime_product_install::InstallFilesystemError) -> Self {
        Self::InstallFilesystem(value.code())
    }
}
impl From<crate::InstallerExecutionError> for PreparationExecutionError {
    fn from(value: crate::InstallerExecutionError) -> Self {
        Self::Execution(value)
    }
}
impl From<radishlex_ime_product_upgrade::UpgradeCoordinatorError> for PreparationExecutionError {
    fn from(value: radishlex_ime_product_upgrade::UpgradeCoordinatorError) -> Self {
        Self::Coordination(value)
    }
}
type Result<T> = std::result::Result<T, PreparationExecutionError>;
use PreparationExecutionError as Error;

enum TaskStart {
    New(Box<InstallReceipt>),
    Resume(Box<PreparationReceipt>),
}

/// Ephemeral, exact-root/product task. Starting and resuming both require fresh
/// explicit confirmation; a persisted preparation never grants permission.
pub struct PreparationTaskAuthorization {
    start: TaskStart,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    grant: CancellationTaskGrant,
}

pub fn authorize_source_preparation(
    previous: &InstallReceipt,
    target: &ProductArtifactIdentity,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> std::result::Result<
    (PreparationTaskAuthorization, PreparationTaskControl),
    PreparationTaskAuthorizationError,
> {
    let source = previous
        .installed_product()
        .ok_or(PreparationTaskAuthorizationError::InvalidBinding)?;
    if !previous.state().is_terminal()
        || previous.manual_recovery_required()
        || previous.encode().is_err()
    {
        return Err(PreparationTaskAuthorizationError::InvalidBinding);
    }
    // Validate the normal upgrade/version contract before granting any task.
    let probe = if previous.operation_id() == "11111111111111111111111111111111" {
        "22222222222222222222222222222222"
    } else {
        "11111111111111111111111111111111"
    };
    let next = expected_outer(previous, target, probe)
        .map_err(|_| PreparationTaskAuthorizationError::InvalidBinding)?;
    if !next.can_replace(previous) {
        return Err(PreparationTaskAuthorizationError::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        PreparationTaskAuthorization {
            start: TaskStart::New(Box::new(previous.clone())),
            source: source.clone(),
            target: target.clone(),
            grant,
        },
        control,
    ))
}

pub fn authorize_preparation_resume(
    preparation: &PreparationReceipt,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> std::result::Result<
    (PreparationTaskAuthorization, PreparationTaskControl),
    PreparationTaskAuthorizationError,
> {
    if preparation.encode().is_err()
        || preparation.previous_install_evidence().is_none()
        || !matches_products(preparation, source, target)
    {
        return Err(PreparationTaskAuthorizationError::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        PreparationTaskAuthorization {
            start: TaskStart::Resume(Box::new(preparation.clone())),
            source: source.clone(),
            target: target.clone(),
            grant,
        },
        control,
    ))
}

/// Success returns a nonterminal Preflighted data receipt and a Prepared outer.
/// No program staging, switching, final cancellation release or UI action runs.
pub fn prepare_upgrade_source(
    authorization: PreparationTaskAuthorization,
    install: &InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
) -> Result<UpgradeReceipt> {
    execute(
        authorization,
        install,
        &mut NativeObserver {
            programs,
            preflight,
        },
        &mut SystemInstallerOperationIdSource,
    )
}

fn execute(
    authorization: PreparationTaskAuthorization,
    install: &InstallReceiptStore,
    observer: &mut impl PreparationObserver,
    operation_ids: &mut impl InstallerOperationIdSource,
) -> Result<UpgradeReceipt> {
    authorization.require_active()?;
    let root = directory_identity(install);
    if authorization.root() != root {
        return Err(Error::InvalidBinding);
    }
    let outer_guard = install.acquire_guard()?;
    let verified = || VerifiedDataRoot::verify(install.data_root_path(), root.owner_id);
    // Existing unknown/cancellation slots always fail; only a genuinely absent
    // data-state directory may be created by a new task.
    let (journal, inner_guard) =
        if outer::present(&install.data_root_path().join(".radishlex-upgrade-v1"))? {
            let journal = PreparationJournalStore::open_existing(verified()?)?;
            let guard = journal.acquire_guard()?;
            (journal, guard)
        } else {
            if !matches!(authorization.start, TaskStart::New(_))
                || outer::present(
                    &install
                        .data_root_path()
                        .join(".radishlex-upgrade-history-v1"),
                )?
            {
                return Err(Error::InvalidBinding);
            }
            let previous = match &authorization.start {
                TaskStart::New(value) => value,
                _ => unreachable!(),
            };
            check_current(install, &outer_guard, previous)?;
            observe(&authorization, observer, &root, ExecutionPoint::Admission)?;
            check_current(install, &outer_guard, previous)?;
            let store = UpgradeReceiptStore::open(verified()?)?;
            let guard = store.acquire_guard()?;
            (PreparationJournalStore::attach(store, &guard)?, guard)
        };
    let (preparation, previous) = match &authorization.start {
        TaskStart::New(previous) => {
            check_current(install, &outer_guard, previous)?;
            if journal.load_guarded(&inner_guard)?.is_some() {
                return Err(Error::InvalidPhase);
            }
            observe(&authorization, observer, &root, ExecutionPoint::Admission)?;
            check_current(install, &outer_guard, previous)?;
            let operation = operation_ids
                .next_operation_id()
                .map_err(|_| Error::OperationId)?;
            let new_outer = expected_outer(previous, &authorization.target, &operation)?;
            if !new_outer.can_replace(previous) {
                return Err(Error::InvalidBinding);
            }
            let (data, lifecycle) = journal.observe_preparation_predecessor(
                &inner_guard,
                &operation,
                &MacOsPreparationHasher,
            )?;
            if data.as_ref().is_some_and(|old| {
                let release =
                    if old.state() == radishlex_ime_product_upgrade::UpgradeState::Completed {
                        old.target_release()
                    } else {
                        old.source_release()
                    };
                !same_release(release, &authorization.source)
            }) {
                return Err(Error::InvalidBinding);
            }
            let reused = match &lifecycle {
                Some(LifecycleReleaseReceipt::Cancellation(proof)) => {
                    if proof.legacy_outer_operation_id() != previous.operation_id() {
                        return Err(Error::InvalidBinding);
                    }
                    Some(proof.archive().outer().previous_install_evidence())
                }
                _ => None,
            };
            let evidence =
                outer::seal(install, &outer_guard, previous, reused.as_ref(), |point| {
                    // Outer metadata creation also runs under a freshly checked
                    // inner guard; a native callback cannot swap its directory
                    // or introduce a preparation and let sealing continue.
                    if journal.load_guarded(&inner_guard)?.is_some() {
                        return Err(Error::InvalidBinding);
                    }
                    observe(&authorization, observer, &root, point)?;
                    if journal.load_guarded(&inner_guard)?.is_some() {
                        return Err(Error::InvalidBinding);
                    }
                    Ok(())
                })?;
            observe(
                &authorization,
                observer,
                &root,
                ExecutionPoint::BeforeInventory,
            )?;
            outer::verify(
                install,
                previous.operation_id(),
                &evidence,
                &previous.encode().map_err(|_| Error::InvalidBinding)?,
            )?;
            check_current(install, &outer_guard, previous)?;
            let inventory = journal.capture_previous_inventory(
                &inner_guard,
                &operation,
                Some(previous.operation_id()),
                data.as_ref().map(UpgradeReceipt::operation_id),
                &MacOsPreparationHasher,
            )?;
            let identity = journal.previous_inventory_identity(
                &inner_guard,
                &inventory,
                &MacOsPreparationHasher,
            )?;
            let family = journal.observe_source_family(&inner_guard, &MacOsPreparationHasher)?;
            let mut prep = PreparationReceipt::new(
                PreparationBinding {
                    operation_id: operation,
                    previous_install_operation_id: Some(previous.operation_id().to_owned()),
                    previous_data_operation_id: data
                        .as_ref()
                        .map(|value| value.operation_id().to_owned()),
                    source_release: data_release(&authorization.source)?,
                    target_release: data_release(&authorization.target)?,
                    source_product_sha256: authorization.source.product_manifest_sha256().into(),
                    target_product_sha256: authorization.target.product_manifest_sha256().into(),
                    previous_install_receipt_sha256: Some(evidence.previous_receipt.sha256.clone()),
                    previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
                    previous_inventory_sha256: identity.as_ref().map(|value| value.sha256.clone()),
                    target_schema_version: UserDb::supported_schema_version(),
                    data_root: root.clone(),
                    state_directory: journal.state_directory_identity(),
                },
                family,
            )
            .map_err(|_| Error::InvalidBinding)?;
            if let Some(identity) = identity {
                if let Some(index) = inventory.release_index_identity() {
                    prep.bind_released_predecessor(identity, index.clone())
                        .map_err(|_| Error::InvalidBinding)?;
                } else {
                    prep.bind_previous_inventory(identity)
                        .map_err(|_| Error::InvalidBinding)?;
                }
            }
            prep.bind_previous_install(evidence)
                .map_err(|_| Error::InvalidBinding)?;
            observe(
                &authorization,
                observer,
                &root,
                ExecutionPoint::BeforeReservation,
            )?;
            check_current(install, &outer_guard, previous)?;
            outer::verify_preparation(install, &prep)?;
            if journal.observe_source_family(&inner_guard, &MacOsPreparationHasher)?
                != *prep.initial_source()
                || journal
                    .previous_inventory_identity(&inner_guard, &inventory, &MacOsPreparationHasher)?
                    .as_ref()
                    != prep.previous_inventory_identity()
            {
                return Err(Error::InvalidBinding);
            }
            journal.persist(&inner_guard, None, &prep)?;
            (prep, previous.as_ref().clone())
        }
        TaskStart::Resume(expected) => {
            let current = journal.load_preparation_for_operation(
                &inner_guard,
                &expected.binding().operation_id,
                &MacOsPreparationHasher,
            )?;
            if &current != expected.as_ref() {
                return Err(Error::InvalidBinding);
            }
            let previous = outer::verify_preparation(install, &current)?;
            (current, previous)
        }
    };
    let expected = expected_outer(
        &previous,
        &authorization.target,
        &preparation.binding().operation_id,
    )?;
    let mut port = PreparationAuthority {
        authorization: &authorization,
        install,
        outer_guard: &outer_guard,
        expected: &expected,
        previous: &previous,
        initial: &preparation,
        observer,
        failure: None,
    };
    port.before_outer()?;
    if journal.load_preparation_for_operation(
        &inner_guard,
        &preparation.binding().operation_id,
        &MacOsPreparationHasher,
    )? != preparation
    {
        return Err(Error::InvalidBinding);
    }
    if let Some(active) = journal.load_guarded(&inner_guard)? {
        // A reservation recovered after rename may still need directory fsync.
        // Make it durable before publishing the new outer, without creating a
        // marker again once handoff has already moved it into history.
        if active != preparation {
            return Err(Error::InvalidBinding);
        }
        journal.persist(&inner_guard, Some(&preparation), &preparation)?;
    }
    authorization.require_active()?;
    outer::verify_preparation(install, &preparation)?;
    install.persist(&outer_guard, &expected)?;
    outer::sync_active(install, &expected)?;
    port.admit(&preparation, ExecutionPoint::OuterPersisted)?;
    let mut current = preparation.clone();
    if current.phase() <= PreparationPhase::SourcePrepared {
        let result =
            journal.prepare_userdb_source(&inner_guard, &mut port, &MacOsPreparationHasher);
        current = port.resolve(result)?;
    }
    if current.phase() == PreparationPhase::SourcePrepared {
        let result =
            journal.archive_previous_upgrade(&inner_guard, &mut port, &MacOsPreparationHasher);
        current = port.resolve(result)?;
    }
    let result = journal.handoff_userdb_source(
        &inner_guard,
        &current.binding().operation_id,
        &mut port,
        &MacOsPreparationHasher,
    );
    let (_, receipt) = port.resolve(result)?;
    Ok(receipt)
}

fn expected_outer(
    previous: &InstallReceipt,
    target: &ProductArtifactIdentity,
    operation: &str,
) -> Result<InstallReceipt> {
    InstallReceipt::new(
        operation,
        Some(previous.operation_id().into()),
        InstallOperationKind::Upgrade,
        previous.root_identity().clone(),
        previous.installed_product().cloned(),
        Some(target.clone()),
    )
    .map_err(|_| Error::InvalidBinding)
}
fn data_release(product: &ProductArtifactIdentity) -> Result<ProductRelease> {
    ProductRelease::new(
        product.release().product_version(),
        product.release().build_number(),
    )
    .map_err(|_| Error::InvalidBinding)
}
fn same_release(release: &ProductRelease, product: &ProductArtifactIdentity) -> bool {
    release.product_version() == product.release().product_version()
        && release.build_number() == product.release().build_number()
}
pub(crate) fn matches_products(
    prep: &PreparationReceipt,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
) -> bool {
    let b = prep.binding();
    same_release(&b.source_release, source)
        && same_release(&b.target_release, target)
        && b.source_product_sha256 == source.product_manifest_sha256()
        && b.target_product_sha256 == target.product_manifest_sha256()
}
fn directory_identity(install: &InstallReceiptStore) -> PreparationDirectoryIdentity {
    let r = install.root_identity();
    PreparationDirectoryIdentity {
        device_id: r.device_id(),
        inode: r.inode(),
        owner_id: r.owner_id(),
        mode: r.mode(),
    }
}
fn check_current(
    install: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    expected: &InstallReceipt,
) -> Result<()> {
    if install.load_guarded(guard)?.as_ref() != Some(expected) {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}
impl PreparationTaskAuthorization {
    fn root(&self) -> PreparationDirectoryIdentity {
        match &self.start {
            TaskStart::New(value) => {
                let r = value.root_identity();
                PreparationDirectoryIdentity {
                    device_id: r.device_id(),
                    inode: r.inode(),
                    owner_id: r.owner_id(),
                    mode: r.mode(),
                }
            }
            TaskStart::Resume(value) => value.binding().data_root.clone(),
        }
    }
    fn require_active(&self) -> Result<()> {
        if self.grant.active() {
            Ok(())
        } else {
            Err(Error::TaskAuthorization)
        }
    }
}

#[cfg(test)]
#[path = "preparation_execution_tests.rs"]
pub(crate) mod tests;
