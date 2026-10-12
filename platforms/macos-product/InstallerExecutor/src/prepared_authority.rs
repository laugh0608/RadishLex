//! Authority checks shared by actual program moves, data callbacks and release.
use super::*;
use radishlex_ime_product_install::{ProgramSwitchFaultInjector, ProgramSwitchFaultPoint};
use radishlex_ime_product_upgrade::{
    ProductRelease, TerminalReleaseCheckpoint, TerminalReleaseOuterRequirement,
    TerminalReleasePort, UpgradeCandidateValidationReport, UpgradeCoordinatorCheckpoint,
    UpgradePostSwitchValidationReport, UpgradeRollbackValidationEvidence,
};
use std::fs;
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    Admission,
    History,
    Durable,
    BeforePersist,
    Persisted,
    Source(ProgramComponent),
    Stage(ProgramComponent),
    Staged(ProgramComponent),
    Preserve(ProgramComponent),
    Commit(ProgramComponent),
    Restore(ProgramComponent),
    Switch(ProgramComponent, ProgramSwitchFaultPoint),
    Programs(InstallState),
    Data,
    DataCheckpoint(UpgradeCoordinatorCheckpoint),
    Candidate,
    PostSwitch,
    SourceValidation,
    DataSettled,
    Release(TerminalReleaseCheckpoint),
    BeforeOuterTerminal,
    Finished,
}
pub(super) trait ContinuationPrograms: InstallerProgramPort {
    fn verify_material(
        &mut self,
        install: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        stores: [&ProgramSwitchStore; 2],
        receipt: &InstallReceipt,
    ) -> Result<()>;
}
impl ContinuationPrograms for MacOsProductInstallAdapter {
    fn verify_material(
        &mut self,
        install: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        stores: [&ProgramSwitchStore; 2],
        receipt: &InstallReceipt,
    ) -> Result<()> {
        self.verify_data_root(install.root_identity())
            .map_err(|_| Error::ProductObservation)?;
        for store in stores {
            store
                .verify_binding(install, guard, receipt)
                .map_err(switch_error)?;
            self.verify_program_continuation_material(store, receipt)
                .map_err(|_| Error::ProductObservation)?;
        }
        Ok(())
    }
}
pub(super) trait ContinuationObserver {
    fn observe(
        &mut self,
        preparation: &PreparationReceipt,
        outer: &InstallReceipt,
        point: Point,
    ) -> Result<u64>;
}
pub(super) struct NativeObserver<'a> {
    pub(super) preflight: &'a mut MacOsProductPreflightAdapter,
}
impl ContinuationObserver for NativeObserver<'_> {
    fn observe(&mut self, p: &PreparationReceipt, _: &InstallReceipt, _: Point) -> Result<u64> {
        self.preflight
            .inspect_cancellation_quiescence(
                &p.binding().data_root,
                &p.binding().target_product_sha256,
            )
            .map(|r| r.available_bytes())
            .map_err(|_| Error::NativeObservation)
    }
}
pub(super) enum GuardStore<'a> {
    Active(&'a UpgradeReceiptStore),
    Releasing(&'a TerminalReleaseStore),
}
pub(super) struct Authority<'a> {
    pub(super) install: &'a InstallReceiptStore,
    pub(super) outer_guard: &'a InstallProcessGuard,
    inner_guard: &'a UpgradeProcessGuard,
    preparation: &'a PreparationReceipt,
    grant: &'a CancellationTaskGrant,
    guard_store: GuardStore<'a>,
    pub(super) manager: &'a ProgramSwitchStore,
    pub(super) input_method: &'a ProgramSwitchStore,
    current: InstallReceipt,
    outer_identity: PreparationFileIdentity,
    proof_identity: PreparationFileIdentity,
    data_identity: Option<(UpgradeReceipt, PreparationFileIdentity)>,
    pub(super) active_data: bool,
    pub(super) release: Option<TerminalReleaseReceipt>,
    pub(super) release_identity: Option<PreparationFileIdentity>,
}
impl<'a> Authority<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        install: &'a InstallReceiptStore,
        outer_guard: &'a InstallProcessGuard,
        inner_guard: &'a UpgradeProcessGuard,
        preparation: &'a PreparationReceipt,
        grant: &'a CancellationTaskGrant,
        guard_store: GuardStore<'a>,
        manager: &'a ProgramSwitchStore,
        input_method: &'a ProgramSwitchStore,
        current: &InstallReceipt,
        outer_identity: &PreparationFileIdentity,
        proof_identity: &PreparationFileIdentity,
        data: &PreparedUpgradeData,
    ) -> Result<Self> {
        let data_identity = match data {
            PreparedUpgradeData::Active(r) => {
                Some((r.as_ref().clone(), outer::active_data_identity(install)?))
            }
            PreparedUpgradeData::Releasing(_) => None,
        };
        Ok(Self {
            install,
            outer_guard,
            inner_guard,
            preparation,
            grant,
            guard_store,
            manager,
            input_method,
            current: current.clone(),
            outer_identity: outer_identity.clone(),
            proof_identity: proof_identity.clone(),
            active_data: data_identity.is_some(),
            data_identity,
            release: None,
            release_identity: None,
        })
    }
    fn verify(&mut self) -> Result<()> {
        if !self.grant.active() {
            return Err(Error::TaskAuthorization);
        }
        if self.install.load_guarded(self.outer_guard)?.as_ref() != Some(&self.current)
            || outer::active_identity(self.install)? != self.outer_identity
            || outer::preparation_proof_identity(self.install, self.preparation)?
                != self.proof_identity
        {
            return Err(Error::InvalidBinding);
        }
        crate::preparation_rebinding::verify_outer(self.install, self.preparation, &self.current)?;
        match self.guard_store {
            GuardStore::Active(s) => s.verify_process_guard(self.inner_guard)?,
            GuardStore::Releasing(s) => s.verify_process_guard(self.inner_guard)?,
        }
        if self.active_data {
            let journal = PreparationJournalStore::open_existing(root(self.install)?)?;
            let (_, current) = journal.rebind_handed_off_upgrade(
                self.inner_guard,
                self.preparation,
                &MacOsPreparationHasher,
            )?;
            if current.manual_recovery_required()
                && current.state() != UpgradeState::RollbackRequired
            {
                return Err(Error::InvalidPhase);
            }
            crate::preparation_rebinding::verify_state_pair(&self.current, &current)?;
            let identity = outer::active_data_identity(self.install)?;
            if let Some((old, old_identity)) = &self.data_identity {
                // The coordinator may perform more than one legal v1 step
                // between callbacks (notably SwitchPrepared -> Switched).
                // Rebinding fixes all immutable fields; already observed
                // artifacts must also remain. Around callbacks, NO change is
                // accepted, including a valid successor or equal-byte inode.
                if (current == *old && identity != *old_identity)
                    || old
                        .artifacts()
                        .iter()
                        .any(|item| !current.artifacts().contains(item))
                {
                    return Err(Error::InvalidBinding);
                }
            }
            self.data_identity = Some((current, identity));
        }
        if let Some(expected) = &self.release {
            let s = TerminalReleaseStore::open_existing(root(self.install)?)?;
            if s.inspect_terminal_release(
                self.inner_guard,
                self.current.operation_id(),
                &MacOsPreparationHasher,
            )? != *expected
                || !release_pair(&self.current, expected)
                || self.release_identity.as_ref()
                    != Some(&outer::terminal_identity(
                        self.install,
                        self.current.operation_id(),
                    )?)
            {
                return Err(Error::InvalidBinding);
            }
        }
        if !self.grant.active() {
            return Err(Error::TaskAuthorization);
        }
        Ok(())
    }
    pub(super) fn adopt_outer(&mut self, expected: &InstallReceipt) -> Result<()> {
        if !self.grant.active() {
            return Err(Error::TaskAuthorization);
        }
        if self.install.load_guarded(self.outer_guard)?.as_ref() != Some(expected)
            || (expected != &self.current && !expected.can_replace(&self.current))
        {
            return Err(Error::InvalidBinding);
        }
        if expected == &self.current && outer::active_identity(self.install)? != self.outer_identity
        {
            return Err(Error::InvalidBinding);
        }
        self.current = expected.clone();
        self.outer_identity = outer::active_identity(self.install)?;
        Ok(())
    }
    pub(super) fn check<P: ContinuationPrograms, O: ContinuationObserver>(
        &mut self,
        p: &mut P,
        o: &mut O,
        point: Point,
    ) -> Result<u64> {
        self.verify()?;
        p.verify_material(
            self.install,
            self.outer_guard,
            [self.manager, self.input_method],
            &self.current,
        )?;
        let stamp = program_stamps([self.manager, self.input_method])?;
        let data = self.data_identity.clone();
        let outer = self.outer_identity.clone();
        let available = o.observe(self.preparation, &self.current, point)?;
        self.verify()?;
        p.verify_material(
            self.install,
            self.outer_guard,
            [self.manager, self.input_method],
            &self.current,
        )?;
        if available == 0 {
            return Err(Error::NativeObservation);
        }
        if self.outer_identity != outer
            || self.data_identity != data
            || program_stamps([self.manager, self.input_method])? != stamp
        {
            return Err(Error::InvalidBinding);
        }
        Ok(available)
    }
    pub(super) fn around<P: ContinuationPrograms, O: ContinuationObserver, T>(
        &mut self,
        p: &mut P,
        o: &mut O,
        point: Point,
        run: impl FnOnce() -> T,
    ) -> Result<T> {
        self.check(p, o, point)?;
        let data = self.data_identity.clone();
        let outer = self.outer_identity.clone();
        let stamp = program_stamps([self.manager, self.input_method])?;
        let value = run();
        self.check(p, o, point)?;
        if self.data_identity != data
            || self.outer_identity != outer
            || program_stamps([self.manager, self.input_method])? != stamp
        {
            return Err(Error::InvalidBinding);
        }
        Ok(value)
    }
}
type Stamp = Option<(u64, u64, u32, u32)>;
fn program_stamps(stores: [&ProgramSwitchStore; 2]) -> Result<Vec<Stamp>> {
    let mut stamps = Vec::new();
    for s in stores {
        for path in [
            s.target_path(),
            s.staged_bundle_path(),
            s.source_backup_bundle_path(),
        ] {
            match fs::symlink_metadata(path) {
                Ok(m) => stamps.push(Some((m.dev(), m.ino(), m.uid(), m.mode()))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => stamps.push(None),
                Err(_) => return Err(Error::Io),
            }
        }
    }
    Ok(stamps)
}

pub(super) struct SwitchPort<'a, 'b, P, O> {
    pub(super) authority: &'a mut Authority<'b>,
    pub(super) programs: &'a mut P,
    pub(super) observer: &'a mut O,
    pub(super) component: ProgramComponent,
    pub(super) failure: Option<Error>,
}
impl<P: ContinuationPrograms, O: ContinuationObserver> ProgramSwitchFaultInjector
    for SwitchPort<'_, '_, P, O>
{
    fn should_fail(&mut self, point: ProgramSwitchFaultPoint) -> bool {
        self.failure = self
            .authority
            .check(
                self.programs,
                self.observer,
                Point::Switch(self.component, point),
            )
            .err();
        self.failure.is_some()
    }
}
pub(super) struct BoundDataPort<'a, 'b, P, O, U> {
    pub(super) authority: &'a mut Authority<'b>,
    pub(super) programs: &'a mut P,
    pub(super) observer: &'a mut O,
    pub(super) upgrade: &'a mut U,
    pub(super) failure: Option<Error>,
}
impl<P: ContinuationPrograms, O: ContinuationObserver, U: UpgradeCoordinatorPort>
    BoundDataPort<'_, '_, P, O, U>
{
    fn call<T>(&mut self, point: Point, run: impl FnOnce(&mut U) -> T) -> Option<T> {
        if self.failure.is_some() {
            return None;
        }
        match self
            .authority
            .around(self.programs, self.observer, point, || run(self.upgrade))
        {
            Ok(v) => Some(v),
            Err(e) => {
                self.failure = Some(e);
                None
            }
        }
    }
}
impl<P: ContinuationPrograms, O: ContinuationObserver, U: UpgradeCoordinatorPort>
    UpgradeCoordinatorPort for BoundDataPort<'_, '_, P, O, U>
{
    fn confirm_quiescence(&mut self, point: UpgradeCoordinatorCheckpoint) -> bool {
        self.call(Point::DataCheckpoint(point), |u| {
            u.confirm_quiescence(point)
        })
        .unwrap_or(false)
    }
    fn validate_candidate(
        &mut self,
        r: &ProductRelease,
        s: i64,
    ) -> UpgradeCandidateValidationReport {
        self.call(Point::Candidate, |u| u.validate_candidate(r, s))
            .unwrap_or_else(UpgradeCandidateValidationReport::manager_failed)
    }
    fn validate_post_switch(
        &mut self,
        r: &ProductRelease,
        s: i64,
    ) -> UpgradePostSwitchValidationReport {
        self.call(Point::PostSwitch, |u| u.validate_post_switch(r, s))
            .unwrap_or_else(UpgradePostSwitchValidationReport::manager_failed)
    }
    fn validate_restored_source(
        &mut self,
        r: &ProductRelease,
        s: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        self.call(Point::SourceValidation, |u| {
            u.validate_restored_source(r, s)
        })
        .flatten()
    }
}
pub(super) struct ReleasePort<'a, 'b, P, O> {
    pub(super) authority: &'a mut Authority<'b>,
    pub(super) programs: &'a mut P,
    pub(super) observer: &'a mut O,
    pub(super) failure: Option<Error>,
}
impl<P: ContinuationPrograms, O: ContinuationObserver> TerminalReleasePort
    for ReleasePort<'_, '_, P, O>
{
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &TerminalReleaseReceipt,
        point: TerminalReleaseCheckpoint,
        requirement: TerminalReleaseOuterRequirement,
    ) -> bool {
        let matching = match requirement {
            TerminalReleaseOuterRequirement::Nonterminal => {
                !self.authority.current.state().is_terminal()
            }
            TerminalReleaseOuterRequirement::MatchingTerminal => {
                self.authority.current.state().is_terminal()
            }
        };
        self.failure = if !matching || !release_pair(&self.authority.current, proof) {
            Some(Error::InvalidBinding)
        } else {
            self.authority
                .check(self.programs, self.observer, Point::Release(point))
                .err()
        };
        self.failure.is_none()
    }
}
