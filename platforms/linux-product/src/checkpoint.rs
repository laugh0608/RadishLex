use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxL6Checkpoint {
    Prepared,
    ArtifactsStaged,
    Quiesced,
    PackageMutatingBeforeDpkg,
    TargetAppliedBeforeProof,
    RollbackRequired,
    SourceRestoringBeforeDpkg,
    SourceAppliedBeforeProof,
}

impl LinuxL6Checkpoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::ArtifactsStaged => "artifacts_staged",
            Self::Quiesced => "quiesced",
            Self::PackageMutatingBeforeDpkg => "package_mutating_before_dpkg",
            Self::TargetAppliedBeforeProof => "target_applied_before_proof",
            Self::RollbackRequired => "rollback_required",
            Self::SourceRestoringBeforeDpkg => "source_restoring_before_dpkg",
            Self::SourceAppliedBeforeProof => "source_applied_before_proof",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxL6CheckpointError {
    checkpoint: LinuxL6Checkpoint,
}

impl LinuxL6CheckpointError {
    pub const fn unavailable(checkpoint: LinuxL6Checkpoint) -> Self {
        Self { checkpoint }
    }

    pub const fn checkpoint(self) -> LinuxL6Checkpoint {
        self.checkpoint
    }
}

impl fmt::Display for LinuxL6CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "L6 checkpoint control is unavailable at {}",
            self.checkpoint.as_str()
        )
    }
}

impl std::error::Error for LinuxL6CheckpointError {}

pub trait LinuxL6CheckpointSink {
    fn reached(&mut self, checkpoint: LinuxL6Checkpoint) -> Result<(), LinuxL6CheckpointError>;
}

#[derive(Debug, Default)]
pub(crate) struct DisabledLinuxL6Checkpoints;

impl LinuxL6CheckpointSink for DisabledLinuxL6Checkpoints {
    fn reached(&mut self, _checkpoint: LinuxL6Checkpoint) -> Result<(), LinuxL6CheckpointError> {
        Ok(())
    }
}

#[cfg(feature = "l6-acceptance-checkpoints")]
pub(crate) struct RejectingTargetValidationPort<P> {
    inner: P,
    reject_target_validation: bool,
}

#[cfg(feature = "l6-acceptance-checkpoints")]
impl<P> RejectingTargetValidationPort<P> {
    pub(crate) const fn new(inner: P, reject_target_validation: bool) -> Self {
        Self {
            inner,
            reject_target_validation,
        }
    }

    #[cfg(test)]
    pub(crate) const fn inner(&self) -> &P {
        &self.inner
    }
}

#[cfg(feature = "l6-acceptance-checkpoints")]
impl<P: crate::coordinator::DpkgTransactionPort> crate::coordinator::DpkgTransactionPort
    for RejectingTargetValidationPort<P>
{
    type QuiescencePermit = P::QuiescencePermit;

    fn validate_operation(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        self.inner.validate_operation(context)
    }

    fn inspect_package(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
    ) -> Result<crate::model::PackageSnapshot, crate::coordinator::DpkgPortError> {
        self.inner.inspect_package(context)
    }

    fn validate_staged_operation(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        staged: crate::coordinator::DpkgStagedOperation<'_>,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        self.inner.validate_staged_operation(context, staged)
    }

    fn prove_quiescent(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        phase: crate::coordinator::DpkgQuiescencePhase,
    ) -> Result<Self::QuiescencePermit, crate::coordinator::DpkgPortError> {
        self.inner.prove_quiescent(context, phase)
    }

    fn apply_package(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        package: crate::coordinator::DpkgStagedPackage<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        self.inner.apply_package(context, package, permit)
    }

    fn remove_package(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        self.inner.remove_package(context, permit)
    }

    fn validate_product(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        phase: crate::coordinator::DpkgProductValidationPhase,
        expected: crate::coordinator::DpkgExpectedProductState<'_>,
        snapshot: &crate::model::PackageSnapshot,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        if self.reject_target_validation
            && phase == crate::coordinator::DpkgProductValidationPhase::Target
        {
            return Err(crate::coordinator::DpkgPortError::new(
                crate::coordinator::DpkgPortErrorCode::ProductValidationFailed,
                crate::coordinator::DpkgPortPhase::TargetValidation,
                "L6 acceptance target validation rejection",
            ));
        }
        self.inner
            .validate_product(context, phase, expected, snapshot)
    }

    fn restore_source(
        &mut self,
        context: crate::coordinator::DpkgOperationContext<'_>,
        request: crate::coordinator::DpkgRestoreRequest<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), crate::coordinator::DpkgPortError> {
        self.inner.restore_source(context, request, permit)
    }
}
