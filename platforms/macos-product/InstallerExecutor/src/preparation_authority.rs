use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecutionPoint {
    Admission,
    HistoryRootCreated,
    HistoryRootSynced,
    HistoryRootParentSynced,
    HistoryDirectoryCreated,
    HistoryDirectorySynced,
    HistoryDirectoryParentSynced,
    OriginalCreated,
    OriginalWritten,
    OriginalSynced,
    OriginalRenamed,
    OriginalDirectorySynced,
    BeforeInventory,
    BeforeReservation,
    ReservationPersisted,
    OuterPersisted,
    Capacity,
    Source(SourcePreparationCheckpoint),
}

/// Private injection seam. All production callers use the native observer.
pub(super) trait PreparationObserver {
    fn inspect(
        &mut self,
        root: &PreparationDirectoryIdentity,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        point: ExecutionPoint,
    ) -> Result<u64>;
}
pub(super) struct NativeObserver<'a> {
    pub(super) programs: &'a MacOsProductInstallAdapter,
    pub(super) preflight: &'a mut MacOsProductPreflightAdapter,
}
impl PreparationObserver for NativeObserver<'_> {
    fn inspect(
        &mut self,
        root: &PreparationDirectoryIdentity,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        _: ExecutionPoint,
    ) -> Result<u64> {
        let verify = || {
            crate::cancellation_observation::verify_cancellation_programs(
                self.programs,
                root,
                source,
                target,
            )
        };
        if !verify() {
            return Err(Error::ProductObservation);
        }
        // This host is root/manifest-bound and observes both fixed bundle IDs.
        // Its read-only contract is also valid before a preparation reservation.
        let observation = self
            .preflight
            .inspect_cancellation_quiescence(root, target.product_manifest_sha256())
            .map_err(|_| Error::NativeObservation)?;
        if !verify() {
            return Err(Error::ProductObservation);
        }
        Ok(observation.available_bytes())
    }
}

pub(super) fn observe(
    auth: &PreparationTaskAuthorization,
    observer: &mut impl PreparationObserver,
    root: &PreparationDirectoryIdentity,
    point: ExecutionPoint,
) -> Result<u64> {
    auth.require_active()?;
    let bytes = observer.inspect(root, &auth.source, &auth.target, point)?;
    auth.require_active()?;
    if bytes == 0 {
        return Err(Error::NativeObservation);
    }
    Ok(bytes)
}

pub(super) struct PreparationAuthority<'a, O> {
    pub(super) authorization: &'a PreparationTaskAuthorization,
    pub(super) install: &'a InstallReceiptStore,
    pub(super) outer_guard: &'a InstallProcessGuard,
    pub(super) expected: &'a InstallReceipt,
    pub(super) previous: &'a InstallReceipt,
    pub(super) initial: &'a PreparationReceipt,
    pub(super) observer: &'a mut O,
    pub(super) failure: Option<PreparationExecutionError>,
}
impl<O: PreparationObserver> PreparationAuthority<'_, O> {
    fn verify(&self, prep: &PreparationReceipt) -> Result<()> {
        self.authorization.require_active()?;
        let initial = self.initial;
        if prep.binding() != initial.binding()
            || prep.initial_source() != initial.initial_source()
            || prep.previous_install_evidence() != initial.previous_install_evidence()
            || prep.previous_inventory_identity() != initial.previous_inventory_identity()
            || prep.previous_release_index_identity() != initial.previous_release_index_identity()
            || prep.phase() < initial.phase()
            || !matches_products(prep, &self.authorization.source, &self.authorization.target)
        {
            return Err(Error::InvalidBinding);
        }
        if outer::verify_preparation(self.install, prep)? != *self.previous {
            return Err(Error::InvalidBinding);
        }
        check_current(self.install, self.outer_guard, self.expected)
    }
    pub(super) fn before_outer(&mut self) -> Result<()> {
        self.authorization.require_active()?;
        let current = self
            .install
            .load_guarded(self.outer_guard)?
            .ok_or(Error::InvalidBinding)?;
        if current != *self.expected
            && !(self.initial.phase() == PreparationPhase::Reserved && current == *self.previous)
        {
            return Err(Error::InvalidBinding);
        }
        if outer::verify_preparation(self.install, self.initial)? != *self.previous {
            return Err(Error::InvalidBinding);
        }
        let active = outer::active_identity(self.install)?;
        observe(
            self.authorization,
            self.observer,
            &self.initial.binding().data_root,
            ExecutionPoint::ReservationPersisted,
        )?;
        check_current(self.install, self.outer_guard, &current)?;
        if outer::active_identity(self.install)? != active {
            return Err(Error::InvalidBinding);
        }
        if outer::verify_preparation(self.install, self.initial)? != *self.previous {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    }
    pub(super) fn admit(
        &mut self,
        prep: &PreparationReceipt,
        point: ExecutionPoint,
    ) -> Result<u64> {
        self.verify(prep)?;
        let active = outer::active_identity(self.install)?;
        let result = observe(
            self.authorization,
            self.observer,
            &prep.binding().data_root,
            point,
        )?;
        self.verify(prep)?;
        if outer::active_identity(self.install)? != active {
            return Err(Error::InvalidBinding);
        }
        Ok(result)
    }
    pub(super) fn resolve<T>(
        &self,
        result: std::result::Result<T, SourcePreparationError>,
    ) -> Result<T> {
        result.map_err(|error| {
            if matches!(
                error,
                SourcePreparationError::AuthorityNotProven
                    | SourcePreparationError::InsufficientSpace
            ) {
                if let Some(failure) = self.failure {
                    return failure;
                }
            }
            Error::Preparation(error)
        })
    }
}
impl<O: PreparationObserver> SourcePreparationPort for PreparationAuthority<'_, O> {
    fn confirm_authority_and_quiescence(
        &mut self,
        prep: &PreparationReceipt,
        point: SourcePreparationCheckpoint,
    ) -> bool {
        self.failure = self.admit(prep, ExecutionPoint::Source(point)).err();
        self.failure.is_none()
    }
    fn available_bytes(&mut self) -> Option<u64> {
        // The initial record supplies immutable task binding only. Each core
        // checkpoint separately checks its current mutable preparation phase.
        let result = self.admit(self.initial, ExecutionPoint::Capacity);
        self.failure = result.as_ref().err().copied();
        result.ok()
    }
}
