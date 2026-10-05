use super::*;
use radishlex_ime_product_install::{validate_preparation_cancellation_outer, InstallProcessGuard};
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationOuterBinding, CancellationReleaseCheckpoint,
    CancellationSourceCheckpoint, CancellationSourcePort, CancellationSourceReceipt,
    PreparationCancellationCheckpoint, PreparationCancellationPort, PreparationCancellationRequest,
    UpgradeProcessGuard,
};
use radishlex_macos_product_install_coordinator::{
    CancellationProductAuthority, CancellationReleaseProductAuthority,
    CancellationSourceQualification,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecutionCheckpoint {
    Request(PreparationCancellationCheckpoint),
    Source(CancellationSourceCheckpoint),
    Capacity,
    Archive(CancellationArchiveCheckpoint),
    Release(CancellationReleaseCheckpoint),
}

/// Kept private: production always uses the manifest-bound native observer.
/// Synthetic observers exist only in this module's isolated unit tests.
pub(super) trait ExecutionObserver {
    fn inspect(
        &mut self,
        preparation: &PreparationReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: ExecutionCheckpoint,
    ) -> Result<u64>;
}
pub(super) struct NativeObserver<'a> {
    pub(super) programs: &'a MacOsProductInstallAdapter,
    pub(super) preflight: &'a mut MacOsProductPreflightAdapter,
}
impl ExecutionObserver for NativeObserver<'_> {
    fn inspect(
        &mut self,
        preparation: &PreparationReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        _: ExecutionCheckpoint,
    ) -> Result<u64> {
        let binding = preparation.binding();
        let verify = || {
            super::super::cancellation_observation::verify_cancellation_programs(
                self.programs,
                &binding.data_root,
                source,
                target,
            )
        };
        if !verify() {
            return Err(CancellationExecutionError::ProductObservation);
        }
        let observation = self
            .preflight
            .inspect_cancellation_quiescence(&binding.data_root, &binding.target_product_sha256)
            .map_err(|_| CancellationExecutionError::NativeObservation)?;
        if !verify() {
            return Err(CancellationExecutionError::ProductObservation);
        }
        Ok(observation.available_bytes())
    }
}

pub(super) struct PreservationAuthority<'a, O> {
    pub(super) authorization: &'a CancellationPreservationAuthorization,
    pub(super) install: &'a InstallReceiptStore,
    pub(super) outer_guard: &'a InstallProcessGuard,
    pub(super) inner_guard: &'a UpgradeProcessGuard,
    pub(super) archive: &'a CancellationArchiveStore,
    pub(super) observer: &'a mut O,
    pub(super) original_outer: Option<CancellationOuterBinding>,
    pub(super) failure: Option<CancellationExecutionError>,
}
impl<O: ExecutionObserver> PreservationAuthority<'_, O> {
    pub(super) fn verify_admission_binding(&self) -> Result<()> {
        self.authorization.require_active()?;
        self.install
            .verify_guard(self.outer_guard)
            .map_err(|e| CancellationExecutionError::InstallFilesystem(e.code()))?;
        let prep = &self.authorization.preparation;
        let original = self
            .original_outer
            .as_ref()
            .ok_or(CancellationExecutionError::InvalidBinding)?;
        if self.archive.observe_preparation_outer(
            self.inner_guard,
            prep,
            &MacOsPreparationHasher,
        )? != *original
        {
            return Err(CancellationExecutionError::InvalidBinding);
        }
        validate_preparation_cancellation_outer(
            original.previous_bytes(),
            original.new_bytes(),
            &prep.binding().operation_id,
            prep.binding()
                .previous_install_operation_id
                .as_deref()
                .ok_or(CancellationExecutionError::InvalidBinding)?,
            self.install.root_identity(),
            &self.authorization.source,
            &self.authorization.target,
        )
        .map_err(|_| CancellationExecutionError::InvalidBinding)?;
        Ok(())
    }
    fn observe(&mut self, point: ExecutionCheckpoint) -> Result<u64> {
        self.authorization.require_active()?;
        let available = self.observer.inspect(
            &self.authorization.preparation,
            &self.authorization.source,
            &self.authorization.target,
            point,
        )?;
        self.authorization.require_active()?;
        if available == 0 {
            return Err(CancellationExecutionError::NativeObservation);
        }
        Ok(available)
    }
    fn admission(&mut self, prep: &PreparationReceipt, point: ExecutionCheckpoint) -> Result<u64> {
        if prep != &self.authorization.preparation {
            return Err(CancellationExecutionError::InvalidBinding);
        }
        self.verify_admission_binding()?;
        let available = self.observe(point)?;
        self.verify_admission_binding()?;
        Ok(available)
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
                if let Some(error) = self.failure {
                    return error;
                }
            }
            CancellationExecutionError::Preparation(error)
        })
    }
}
impl<O: ExecutionObserver> PreparationCancellationPort for PreservationAuthority<'_, O> {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        checkpoint: PreparationCancellationCheckpoint,
    ) -> bool {
        self.failure = self
            .admission(
                request.preparation(),
                ExecutionCheckpoint::Request(checkpoint),
            )
            .err();
        self.failure.is_none()
    }
}
impl<O: ExecutionObserver> CancellationSourcePort for PreservationAuthority<'_, O> {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        _: &CancellationSourceReceipt,
        checkpoint: CancellationSourceCheckpoint,
    ) -> bool {
        self.failure = self
            .admission(
                request.preparation(),
                ExecutionCheckpoint::Source(checkpoint),
            )
            .err();
        self.failure.is_none()
    }
    fn available_bytes(&mut self) -> Option<u64> {
        let result = self.admission(
            &self.authorization.preparation.clone(),
            ExecutionCheckpoint::Capacity,
        );
        self.failure = result.as_ref().err().copied();
        result.ok()
    }
}
impl<O: ExecutionObserver> CancellationProductAuthority for PreservationAuthority<'_, O> {
    fn confirm_source_and_quiescence(
        &mut self,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationArchiveCheckpoint,
    ) -> bool {
        self.failure =
            if source != &self.authorization.source || target != &self.authorization.target {
                Some(CancellationExecutionError::InvalidBinding)
            } else {
                self.observe(ExecutionCheckpoint::Archive(checkpoint)).err()
            };
        self.failure.is_none()
    }
}
impl<O: ExecutionObserver> CancellationReleaseProductAuthority for PreservationAuthority<'_, O> {
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> bool {
        self.failure = if proof.preparation() != &self.authorization.preparation
            || source != &self.authorization.source
            || target != &self.authorization.target
        {
            Some(CancellationExecutionError::InvalidBinding)
        } else {
            self.observe(ExecutionCheckpoint::Release(checkpoint)).err()
        };
        self.failure.is_none()
    }
}

/// Used only by the preservation entry. It must never be consulted there and
/// cannot authorize final release if that invariant is accidentally violated.
pub(super) struct PreservationOnly;
impl CancellationSourceQualificationAuthority for PreservationOnly {
    fn verify_source_qualification(
        &mut self,
        _: &CancellationReleaseReceipt,
        _: &ProductArtifactIdentity,
        _: CancellationReleaseCheckpoint,
    ) -> std::result::Result<CancellationSourceQualification, CancellationQualificationError> {
        Err(CancellationQualificationError::Unavailable)
    }
}

pub(super) struct FinalReleaseObserver<'a, O> {
    pub(super) authorization: &'a CancellationTaskAuthorization,
    pub(super) observer: &'a mut O,
    pub(super) failure: Option<CancellationExecutionError>,
}
impl<O: ExecutionObserver> CancellationReleaseProductAuthority for FinalReleaseObserver<'_, O> {
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> bool {
        let result = (|| {
            if !self.authorization.verify(proof, source) {
                return Err(CancellationExecutionError::TaskAuthorization);
            }
            let available = self.observer.inspect(
                proof.preparation(),
                source,
                target,
                ExecutionCheckpoint::Release(checkpoint),
            )?;
            if !self.authorization.verify(proof, source) {
                return Err(CancellationExecutionError::TaskAuthorization);
            }
            if available == 0 {
                return Err(CancellationExecutionError::NativeObservation);
            }
            Ok(())
        })();
        self.failure = result.err();
        self.failure.is_none()
    }
}
