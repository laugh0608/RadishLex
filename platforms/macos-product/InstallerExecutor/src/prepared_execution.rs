//! Prepared upgrades keep their original two guards through program, data and
//! terminal release. Production entry points never accept synthetic observers.
use crate::cancellation_authorization::CancellationTaskGrant;
use crate::preparation_execution::outer;
use crate::{
    InstallerExecutionError, InstallerProgramPort, PreparationExecutionError as Error,
    PreparationTaskAuthorizationError as AuthError, PreparationTaskControl,
    PreparedContinuationTask,
};
use radishlex_ime_product_install::{
    commit_program_target_with_faults, finish_program_restore, finish_source_preservation,
    finish_target_staging, preserve_program_source_with_faults, restore_program_source_with_faults,
    InstallFailureCode, InstallProcessGuard, InstallReceipt, InstallReceiptStore, InstallState,
    ProgramComponent, ProgramSwitchStore,
};
use radishlex_ime_product_upgrade::{
    PreparationFileIdentity, PreparationJournalStore, PreparationPhase, PreparationReceipt,
    TerminalReleaseBinding, TerminalReleasePhase, TerminalReleaseReceipt, TerminalReleaseStore,
    UpgradeCoordinatorPort, UpgradeProcessGuard, UpgradeReceipt, UpgradeReceiptStore, UpgradeState,
    VerifiedDataRoot,
};
use radishlex_macos_installer_driver::InstallerUserAuthorization;
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_upgrade_coordinator::{
    MacOsPreparationHasher, MacOsProductPreflightAdapter, MacOsUpgradeCoordinatorAdapter,
};
use std::time::Duration;

#[path = "prepared_authority.rs"]
mod authority;
use authority::*;
type Result<T> = std::result::Result<T, Error>;

/// An exact observed state, not execution permission. Release states use their
/// own guarded reader because some v1 slots may already have moved to history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedUpgradeData {
    Active(Box<UpgradeReceipt>),
    Releasing(Box<TerminalReleaseReceipt>),
}
impl PreparedUpgradeData {
    fn receipt(&self) -> &UpgradeReceipt {
        match self {
            Self::Active(r) => r,
            Self::Releasing(r) => r.data_receipt(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUpgradeObservation {
    outer: InstallReceipt,
    preparation: PreparationReceipt,
    data: PreparedUpgradeData,
}
impl PreparedUpgradeObservation {
    pub fn install_receipt(&self) -> &InstallReceipt {
        &self.outer
    }
    pub fn preparation(&self) -> &PreparationReceipt {
        &self.preparation
    }
    pub fn data(&self) -> &PreparedUpgradeData {
        &self.data
    }
}

/// Fixed-root metadata/physical projection only. Never acquires a guard, opens
/// SQLite, creates a directory or reaffirms fsync. Dispatch must reobserve it.
pub fn inspect_prepared_upgrade_continuation(
    install: &InstallReceiptStore,
) -> Result<Option<PreparedUpgradeObservation>> {
    let inspect = || {
        radishlex_ime_product_upgrade::inspect_maintenance(
            install.data_root_path(),
            install.root_identity().owner_id(),
            &MacOsPreparationHasher,
        )
    };
    let Some(observation) = inspect()? else {
        return Ok(None);
    };
    use radishlex_ime_product_upgrade::MaintenancePhase;
    if !matches!(
        observation.phase(),
        MaintenancePhase::PreparationHandedOff | MaintenancePhase::PreparationTerminalRelease
    ) {
        return Ok(None);
    }
    let identity = outer::active_identity(install)?;
    let current = install
        .load_for_recovery_inspection()?
        .ok_or(Error::InvalidBinding)?;
    let preparation = observation.preparation().clone();
    let proof = outer::preparation_proof_identity(install, &preparation)?;
    crate::preparation_rebinding::verify_outer(install, &preparation, &current)?;
    let receipt = observation
        .handed_off_receipt()
        .ok_or(Error::InvalidBinding)?;
    crate::preparation_rebinding::verify_state_pair(&current, receipt)?;
    let data = match observation.terminal_release() {
        Some(release) if release_pair(&current, release) => {
            PreparedUpgradeData::Releasing(Box::new(release.clone()))
        }
        Some(_) => return Err(Error::InvalidBinding),
        None if !current.state().is_terminal() => {
            PreparedUpgradeData::Active(Box::new(receipt.clone()))
        }
        None => return Err(Error::InvalidPhase),
    };
    if inspect()?.as_ref() != Some(&observation)
        || install.load_for_recovery_inspection()?.as_ref() != Some(&current)
        || outer::active_identity(install)? != identity
        || outer::preparation_proof_identity(install, &preparation)? != proof
    {
        return Err(Error::InvalidBinding);
    }
    Ok(Some(PreparedUpgradeObservation {
        outer: current,
        preparation,
        data,
    }))
}
pub struct PreparedUpgradeResumeAuthorization {
    outer: InstallReceipt,
    preparation: PreparationReceipt,
    data: PreparedUpgradeData,
    grant: CancellationTaskGrant,
}

pub fn authorize_prepared_upgrade_resume(
    current: &InstallReceipt,
    preparation: &PreparationReceipt,
    data: PreparedUpgradeData,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> std::result::Result<(PreparedUpgradeResumeAuthorization, PreparationTaskControl), AuthError> {
    let b = preparation.binding();
    let r = data.receipt();
    let product_matches = |source: bool| {
        let (p, release, hash) = if source {
            (
                current.source_product(),
                &b.source_release,
                &b.source_product_sha256,
            )
        } else {
            (
                current.target_product(),
                &b.target_release,
                &b.target_product_sha256,
            )
        };
        p.is_some_and(|p| {
            p.release().product_version() == release.product_version()
                && p.release().build_number() == release.build_number()
                && p.product_manifest_sha256() == hash
        })
    };
    let root = current.root_identity();
    let initial = current.state() == InstallState::Prepared;
    if current.encode().is_err()
        || preparation.encode().is_err()
        || r.encode().is_err()
        || preparation.phase() != PreparationPhase::HandoffReady
        || current.operation_kind() != radishlex_ime_product_install::InstallOperationKind::Upgrade
        || (current.manual_recovery_required()
            && !matches!(
                current.state(),
                InstallState::RollbackRequired | InstallState::ProgramsRestored
            ))
        || (r.manual_recovery_required() && r.state() != UpgradeState::RollbackRequired)
        || initial
        || current.operation_id() != b.operation_id
        || current.previous_operation_id() != b.previous_install_operation_id.as_deref()
        || r.operation_id() != b.operation_id
        || r.previous_operation_id() != b.previous_data_operation_id.as_deref()
        || r.source_release() != &b.source_release
        || r.target_release() != &b.target_release
        || r.target_schema_version() != b.target_schema_version
        || (root.device_id(), root.inode(), root.owner_id(), root.mode())
            != (
                b.data_root.device_id,
                b.data_root.inode,
                b.data_root.owner_id,
                b.data_root.mode,
            )
        || !product_matches(true)
        || !product_matches(false)
        || crate::preparation_rebinding::verify_state_pair(current, r).is_err()
        || matches!(&data, PreparedUpgradeData::Active(_) if current.state().is_terminal())
        || matches!(&data, PreparedUpgradeData::Releasing(proof) if !release_pair(current, proof) || proof.encode().is_err())
    {
        return Err(AuthError::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        PreparedUpgradeResumeAuthorization {
            outer: current.clone(),
            preparation: preparation.clone(),
            data,
            grant,
        },
        control,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUpgradeSummary {
    outer: InstallReceipt,
    release: TerminalReleaseReceipt,
}
impl PreparedUpgradeSummary {
    pub fn install_receipt(&self) -> &InstallReceipt {
        &self.outer
    }
    pub fn data_receipt(&self) -> &UpgradeReceipt {
        self.release.data_receipt()
    }
    pub fn release_proof(&self) -> &TerminalReleaseReceipt {
        &self.release
    }
}

pub fn continue_prepared_upgrade(
    task: PreparedContinuationTask<'_>,
    programs: &mut MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
    upgrade: &mut MacOsUpgradeCoordinatorAdapter,
) -> Result<PreparedUpgradeSummary> {
    task.revalidate(programs, preflight)?;
    execute(
        Task::from(task),
        programs,
        &mut NativeObserver { preflight },
        upgrade,
    )
}

pub fn resume_prepared_upgrade(
    authorization: PreparedUpgradeResumeAuthorization,
    install: &InstallReceiptStore,
    programs: &mut MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
    upgrade: &mut MacOsUpgradeCoordinatorAdapter,
) -> Result<PreparedUpgradeSummary> {
    execute(
        bind_resume(authorization, install)?,
        programs,
        &mut NativeObserver { preflight },
        upgrade,
    )
}

enum DataStore {
    Active(UpgradeReceiptStore),
    Releasing(TerminalReleaseStore),
}
struct Task<'a> {
    install: &'a InstallReceiptStore,
    outer_guard: InstallProcessGuard,
    inner_guard: UpgradeProcessGuard,
    store: DataStore,
    current: PreparedUpgradeData,
    outer: InstallReceipt,
    preparation: PreparationReceipt,
    grant: CancellationTaskGrant,
    outer_identity: PreparationFileIdentity,
    proof_identity: PreparationFileIdentity,
}
impl<'a> From<PreparedContinuationTask<'a>> for Task<'a> {
    fn from(t: PreparedContinuationTask<'a>) -> Self {
        Self {
            install: t.install,
            outer_guard: t.outer_guard,
            inner_guard: t.inner_guard,
            store: DataStore::Active(t.data),
            current: PreparedUpgradeData::Active(Box::new(t.receipt)),
            outer: t.authorization.outer,
            preparation: t.authorization.preparation,
            grant: t.authorization.grant,
            outer_identity: t.outer_identity,
            proof_identity: t.proof_identity,
        }
    }
}
fn root(install: &InstallReceiptStore) -> Result<VerifiedDataRoot> {
    Ok(VerifiedDataRoot::verify(
        install.data_root_path(),
        install.root_identity().owner_id(),
    )?)
}
fn bind_resume(
    auth: PreparedUpgradeResumeAuthorization,
    install: &InstallReceiptStore,
) -> Result<Task<'_>> {
    if !auth.grant.active() {
        return Err(Error::TaskAuthorization);
    }
    if auth.outer.root_identity() != install.root_identity() {
        return Err(Error::InvalidBinding);
    }
    let outer_guard = install.acquire_guard()?;
    if install.load_guarded(&outer_guard)?.as_ref() != Some(&auth.outer) {
        return Err(Error::InvalidBinding);
    }
    crate::preparation_rebinding::verify_outer(install, &auth.preparation, &auth.outer)?;
    let outer_identity = outer::active_identity(install)?;
    let proof_identity = outer::preparation_proof_identity(install, &auth.preparation)?;
    let (store, inner_guard) = match &auth.data {
        PreparedUpgradeData::Active(expected) => {
            let (store, guard, current) =
                crate::rebind_prepared_upgrade(install, &outer_guard, &auth.preparation)?;
            if &current != expected.as_ref() {
                return Err(Error::InvalidBinding);
            }
            (DataStore::Active(store), guard)
        }
        PreparedUpgradeData::Releasing(expected) => {
            let store = TerminalReleaseStore::open_existing(root(install)?)?;
            let guard = store.acquire_guard()?;
            let proof = store.inspect_terminal_release(
                &guard,
                auth.outer.operation_id(),
                &MacOsPreparationHasher,
            )?;
            if &proof != expected.as_ref() || !release_pair(&auth.outer, &proof) {
                return Err(Error::InvalidBinding);
            }
            (DataStore::Releasing(store), guard)
        }
    };
    if install.load_guarded(&outer_guard)?.as_ref() != Some(&auth.outer)
        || outer::active_identity(install)? != outer_identity
        || outer::preparation_proof_identity(install, &auth.preparation)? != proof_identity
    {
        return Err(Error::InvalidBinding);
    }
    Ok(Task {
        install,
        outer_guard,
        inner_guard,
        store,
        current: auth.data,
        outer: auth.outer,
        preparation: auth.preparation,
        grant: auth.grant,
        outer_identity,
        proof_identity,
    })
}

pub(super) fn release_pair(outer: &InstallReceipt, proof: &TerminalReleaseReceipt) -> bool {
    let success = proof.data_receipt().state() == UpgradeState::Completed;
    let product = if success {
        outer.target_product()
    } else {
        outer.source_product()
    };
    let state = if success {
        matches!(
            outer.state(),
            InstallState::DataSettled | InstallState::FinalVerified | InstallState::Completed
        )
    } else {
        matches!(
            outer.state(),
            InstallState::ProgramsRestored | InstallState::RolledBack
        )
    };
    state
        && proof.binding().operation_id == outer.operation_id()
        && product.is_some_and(|p| {
            p.release().product_version() == proof.binding().installed_release.product_version()
                && p.release().build_number() == proof.binding().installed_release.build_number()
                && p.product_manifest_sha256() == proof.binding().installed_product_sha256
        })
}

fn switch_error(e: radishlex_ime_product_install::ProgramSwitchError) -> Error {
    InstallerExecutionError::ProgramSwitch(e.code()).into()
}

fn execute<P: ContinuationPrograms, O: ContinuationObserver, U: UpgradeCoordinatorPort>(
    mut task: Task<'_>,
    programs: &mut P,
    observer: &mut O,
    upgrade: &mut U,
) -> Result<PreparedUpgradeSummary> {
    let install = task.install;
    if !task.grant.active() {
        return Err(Error::TaskAuthorization);
    }
    // Opening stores may create only the fixed empty private transaction dirs.
    // Program contents are checked before any subsequent change.
    let manager = programs.open_program_store(
        install,
        &task.outer_guard,
        ProgramComponent::Manager,
        &task.outer,
    )?;
    let input_method = programs.open_program_store(
        install,
        &task.outer_guard,
        ProgramComponent::InputMethod,
        &task.outer,
    )?;
    let data_store = match &task.store {
        DataStore::Active(s) => GuardStore::Active(s),
        DataStore::Releasing(s) => GuardStore::Releasing(s),
    };
    let mut authority = Authority::new(
        install,
        &task.outer_guard,
        &task.inner_guard,
        &task.preparation,
        &task.grant,
        data_store,
        &manager,
        &input_method,
        &task.outer,
        &task.outer_identity,
        &task.proof_identity,
        &task.current,
    )?;
    authority.check(programs, observer, Point::Admission)?;
    outer::sync_preparation_history(install, &task.preparation, || {
        authority
            .check(programs, observer, Point::History)
            .map(|_| ())
    })?;
    outer::sync_active(install, &task.outer)?;
    if let (DataStore::Active(store), PreparedUpgradeData::Active(receipt)) =
        (&task.store, &task.current)
    {
        store.reaffirm_current(&task.inner_guard, receipt)?;
    }
    authority.check(programs, observer, Point::Durable)?;
    if let (DataStore::Active(store), PreparedUpgradeData::Active(receipt)) =
        (&task.store, &mut task.current)
    {
        drive_programs(&mut authority, programs, observer, &mut task.outer)?;
        if matches!(
            task.outer.state(),
            InstallState::ProgramsCommitted | InstallState::DataCoordinating
        ) {
            if task.outer.state() == InstallState::ProgramsCommitted {
                advance(
                    &mut authority,
                    programs,
                    observer,
                    &mut task.outer,
                    InstallState::DataCoordinating,
                )?;
            }
            let available = authority.check(programs, observer, Point::Data)?;
            let mut port = BoundDataPort {
                authority: &mut authority,
                programs,
                observer,
                upgrade,
                failure: None,
            };
            let result =
                store.resume_userdb_upgrade(&task.inner_guard, receipt, available, &mut port);
            if let Some(error) = port.failure {
                return Err(error);
            }
            result?;
            authority.check(programs, observer, Point::DataSettled)?;
        }
        settle_data(&mut authority, programs, observer, &mut task.outer, receipt)?;
    }
    authority.active_data = false;
    let terminal = TerminalReleaseStore::open_existing(root(install)?)?;
    let mut release = match &task.current {
        PreparedUpgradeData::Active(receipt) => {
            let product = if receipt.state() == UpgradeState::Completed {
                task.outer.target_product()
            } else {
                task.outer.source_product()
            }
            .ok_or(Error::InvalidBinding)?;
            let binding = TerminalReleaseBinding {
                operation_id: receipt.operation_id().into(),
                installed_release: radishlex_ime_product_upgrade::ProductRelease::new(
                    product.release().product_version(),
                    product.release().build_number(),
                )
                .map_err(|_| Error::InvalidBinding)?,
                installed_product_sha256: product.product_manifest_sha256().into(),
            };
            prepare_release(
                &terminal,
                &task.inner_guard,
                &binding,
                &mut authority,
                programs,
                observer,
            )?
        }
        PreparedUpgradeData::Releasing(proof) => {
            if task.outer.state().is_terminal() {
                proof.as_ref().clone()
            } else {
                prepare_release(
                    &terminal,
                    &task.inner_guard,
                    proof.binding(),
                    &mut authority,
                    programs,
                    observer,
                )?
            }
        }
    };
    if release.phase() != TerminalReleasePhase::ReleaseReady || !release_pair(&task.outer, &release)
    {
        return Err(Error::InvalidPhase);
    }
    authority.release = Some(release.clone());
    authority.release_identity = Some(outer::terminal_identity(
        install,
        task.outer.operation_id(),
    )?);
    authority.check(programs, observer, Point::BeforeOuterTerminal)?;
    match task.outer.state() {
        InstallState::DataSettled => {
            advance(
                &mut authority,
                programs,
                observer,
                &mut task.outer,
                InstallState::FinalVerified,
            )?;
            advance(
                &mut authority,
                programs,
                observer,
                &mut task.outer,
                InstallState::Completed,
            )?;
        }
        InstallState::FinalVerified => advance(
            &mut authority,
            programs,
            observer,
            &mut task.outer,
            InstallState::Completed,
        )?,
        InstallState::ProgramsRestored => {
            authority.check(programs, observer, Point::BeforePersist)?;
            task.outer
                .mark_rolled_back()
                .map_err(|_| Error::InvalidPhase)?;
            install.persist(&task.outer_guard, &task.outer)?;
            authority.adopt_outer(&task.outer)?;
            authority.check(programs, observer, Point::Persisted)?;
        }
        InstallState::Completed | InstallState::RolledBack => (),
        _ => return Err(Error::InvalidPhase),
    }
    if !release_pair(&task.outer, &release) {
        return Err(Error::InvalidBinding);
    }
    let mut port = ReleasePort {
        authority: &mut authority,
        programs,
        observer,
        failure: None,
    };
    let result = terminal.finish_terminal_release(
        &task.inner_guard,
        task.outer.operation_id(),
        &mut port,
        &MacOsPreparationHasher,
    );
    if let Some(error) = port.failure {
        return Err(error);
    }
    let (_, finished) = result?;
    release = finished;
    authority.check(programs, observer, Point::Finished)?;
    Ok(PreparedUpgradeSummary {
        outer: task.outer,
        release,
    })
}

fn advance<P: ContinuationPrograms, O: ContinuationObserver>(
    a: &mut Authority<'_>,
    p: &mut P,
    o: &mut O,
    r: &mut InstallReceipt,
    next: InstallState,
) -> Result<()> {
    a.check(p, o, Point::BeforePersist)?;
    r.advance(next).map_err(|_| Error::InvalidPhase)?;
    a.install.persist(a.outer_guard, r)?;
    a.adopt_outer(r)?;
    a.check(p, o, Point::Persisted)?;
    Ok(())
}
fn drive_programs<P: ContinuationPrograms, O: ContinuationObserver>(
    a: &mut Authority<'_>,
    p: &mut P,
    o: &mut O,
    r: &mut InstallReceipt,
) -> Result<()> {
    loop {
        match r.state() {
            InstallState::Prepared => advance(a, p, o, r, InstallState::Quiesced)?,
            InstallState::Quiesced => {
                for store in [a.manager, a.input_method] {
                    a.check(p, o, Point::Source(store.component()))?;
                    p.verify_and_record_source(a.install, a.outer_guard, store, r)?;
                    a.adopt_outer(r)?;
                    a.check(p, o, Point::Stage(store.component()))?;
                    p.prepare_and_record_staged(a.install, a.outer_guard, store, r)?;
                    a.adopt_outer(r)?;
                    a.check(p, o, Point::Staged(store.component()))?;
                }
                finish_target_staging(a.install, a.outer_guard, a.manager, a.input_method, r)
                    .map_err(switch_error)?;
                a.adopt_outer(r)?;
                a.check(p, o, Point::Programs(r.state()))?;
            }
            InstallState::TargetStaged => {
                for store in [a.manager, a.input_method] {
                    a.check(p, o, Point::Preserve(store.component()))?;
                    let mut port = SwitchPort {
                        authority: a,
                        programs: p,
                        observer: o,
                        component: store.component(),
                        failure: None,
                    };
                    let result = preserve_program_source_with_faults(
                        port.authority.install,
                        port.authority.outer_guard,
                        store,
                        r,
                        &mut port,
                    );
                    if let Some(e) = port.failure {
                        return Err(e);
                    }
                    result.map_err(switch_error)?;
                    a.adopt_outer(r)?;
                }
                finish_source_preservation(a.install, a.outer_guard, a.manager, a.input_method, r)
                    .map_err(switch_error)?;
                a.adopt_outer(r)?;
                a.check(p, o, Point::Programs(r.state()))?;
            }
            InstallState::SourcePreserved | InstallState::ManagerCommitted => {
                let store = if r.state() == InstallState::SourcePreserved {
                    a.manager
                } else {
                    a.input_method
                };
                a.check(p, o, Point::Commit(store.component()))?;
                let mut port = SwitchPort {
                    authority: a,
                    programs: p,
                    observer: o,
                    component: store.component(),
                    failure: None,
                };
                let result = commit_program_target_with_faults(
                    port.authority.install,
                    port.authority.outer_guard,
                    store,
                    r,
                    &mut port,
                );
                if let Some(e) = port.failure {
                    return Err(e);
                }
                result.map_err(switch_error)?;
                a.adopt_outer(r)?;
                a.check(p, o, Point::Programs(r.state()))?;
            }
            _ => return Ok(()),
        }
    }
}
fn settle_data<P: ContinuationPrograms, O: ContinuationObserver>(
    a: &mut Authority<'_>,
    p: &mut P,
    o: &mut O,
    r: &mut InstallReceipt,
    data: &UpgradeReceipt,
) -> Result<()> {
    if data.state() == UpgradeState::Completed {
        if r.state() == InstallState::DataCoordinating {
            advance(a, p, o, r, InstallState::DataSettled)?;
        }
        return if matches!(
            r.state(),
            InstallState::DataSettled | InstallState::FinalVerified
        ) {
            Ok(())
        } else {
            Err(Error::InvalidPhase)
        };
    }
    if !matches!(
        data.state(),
        UpgradeState::AbortedPreserved | UpgradeState::RolledBack
    ) {
        return Err(Error::InvalidPhase);
    }
    if r.state() == InstallState::DataCoordinating {
        a.check(p, o, Point::BeforePersist)?;
        r.require_rollback(InstallFailureCode::DataCoordinationFailed)
            .map_err(|_| Error::InvalidPhase)?;
        a.install.persist(a.outer_guard, r)?;
        a.adopt_outer(r)?;
    }
    if r.state() == InstallState::RollbackRequired {
        for store in [a.manager, a.input_method] {
            a.check(p, o, Point::Restore(store.component()))?;
            let mut port = SwitchPort {
                authority: a,
                programs: p,
                observer: o,
                component: store.component(),
                failure: None,
            };
            let result = restore_program_source_with_faults(
                port.authority.install,
                port.authority.outer_guard,
                store,
                r,
                &mut port,
            );
            if let Some(e) = port.failure {
                return Err(e);
            }
            result.map_err(switch_error)?;
            a.adopt_outer(r)?;
        }
        a.check(p, o, Point::BeforePersist)?;
        finish_program_restore(a.install, a.outer_guard, a.manager, a.input_method, r)
            .map_err(switch_error)?;
        a.adopt_outer(r)?;
        a.check(p, o, Point::Programs(r.state()))?;
    }
    if r.state() != InstallState::ProgramsRestored {
        return Err(Error::InvalidPhase);
    }
    Ok(())
}
fn prepare_release<P: ContinuationPrograms, O: ContinuationObserver>(
    s: &TerminalReleaseStore,
    g: &UpgradeProcessGuard,
    b: &TerminalReleaseBinding,
    a: &mut Authority<'_>,
    p: &mut P,
    o: &mut O,
) -> Result<TerminalReleaseReceipt> {
    let mut port = ReleasePort {
        authority: a,
        programs: p,
        observer: o,
        failure: None,
    };
    let result = s.prepare_terminal_release(g, b, &mut port, &MacOsPreparationHasher);
    if let Some(e) = port.failure {
        return Err(e);
    }
    Ok(result?)
}

#[cfg(test)]
#[path = "prepared_execution_tests.rs"]
mod tests;
