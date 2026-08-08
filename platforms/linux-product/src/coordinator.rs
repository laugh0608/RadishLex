use std::fmt;
use std::path::Path;

use crate::model::{
    ArtifactSlot, DpkgPackageState, LinuxArtifactIdentity, LinuxFailureCode, LinuxInstallReceipt,
    LinuxInstallReceiptError, LinuxInstallState, LinuxOperationKind, LinuxOperationRequest,
    PackageSnapshot,
};
use crate::store::{
    LinuxInstallGuard, LinuxInstallStore, LinuxInstallStoreError, StagedArtifactPaths,
};

#[derive(Debug, Clone, Copy)]
pub enum DpkgOperationContext<'a> {
    Preparing {
        request: &'a LinuxOperationRequest,
        previous: Option<&'a LinuxInstallReceipt>,
    },
    Resuming {
        receipt: &'a LinuxInstallReceipt,
    },
}

impl<'a> DpkgOperationContext<'a> {
    pub const fn operation_kind(self) -> LinuxOperationKind {
        match self {
            Self::Preparing { request, .. } => request.operation_kind(),
            Self::Resuming { receipt } => receipt.operation_kind(),
        }
    }

    pub fn operation_id(self) -> &'a str {
        match self {
            Self::Preparing { request, .. } => request.operation_id(),
            Self::Resuming { receipt } => receipt.operation_id(),
        }
    }

    pub fn source_artifact(self) -> Option<&'a LinuxArtifactIdentity> {
        match self {
            Self::Preparing { request, .. } => request.source_artifact(),
            Self::Resuming { receipt } => receipt.source_artifact(),
        }
    }

    pub fn target_artifact(self) -> Option<&'a LinuxArtifactIdentity> {
        match self {
            Self::Preparing { request, .. } => request.target_artifact(),
            Self::Resuming { receipt } => receipt.target_artifact(),
        }
    }

    pub const fn version_relation(self) -> crate::model::ArtifactVersionRelation {
        match self {
            Self::Preparing { request, .. } => request.version_relation(),
            Self::Resuming { receipt } => receipt.version_relation(),
        }
    }

    pub const fn previous_receipt(self) -> Option<&'a LinuxInstallReceipt> {
        match self {
            Self::Preparing { previous, .. } => previous,
            Self::Resuming { .. } => None,
        }
    }

    pub const fn receipt(self) -> Option<&'a LinuxInstallReceipt> {
        match self {
            Self::Preparing { .. } => None,
            Self::Resuming { receipt } => Some(receipt),
        }
    }

    pub fn knows_artifact(self, artifact: &LinuxArtifactIdentity) -> bool {
        self.source_artifact() == Some(artifact) || self.target_artifact() == Some(artifact)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgQuiescencePhase {
    TargetMutation,
    SourceRestore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgProductValidationPhase {
    Target,
    Source,
}

#[derive(Debug, Clone, Copy)]
pub enum DpkgExpectedProductState<'a> {
    Absent,
    Installed(&'a LinuxArtifactIdentity),
}

impl DpkgExpectedProductState<'_> {
    pub fn matches_snapshot(self, snapshot: &PackageSnapshot) -> bool {
        match self {
            Self::Absent => snapshot.matches_absent(),
            Self::Installed(artifact) => snapshot.matches_installed(artifact),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DpkgStagedPackage<'a> {
    artifact: &'a LinuxArtifactIdentity,
    package_path: &'a Path,
    evidence_path: &'a Path,
}

impl<'a> DpkgStagedPackage<'a> {
    fn new(artifact: &'a LinuxArtifactIdentity, paths: &'a StagedArtifactPaths) -> Self {
        Self {
            artifact,
            package_path: paths.package_path(),
            evidence_path: paths.evidence_path(),
        }
    }

    pub(crate) const fn from_paths(
        artifact: &'a LinuxArtifactIdentity,
        package_path: &'a Path,
        evidence_path: &'a Path,
    ) -> Self {
        Self {
            artifact,
            package_path,
            evidence_path,
        }
    }

    pub const fn artifact(self) -> &'a LinuxArtifactIdentity {
        self.artifact
    }

    pub const fn package_path(self) -> &'a Path {
        self.package_path
    }

    pub const fn evidence_path(self) -> &'a Path {
        self.evidence_path
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DpkgStagedOperation<'a> {
    source: Option<DpkgStagedPackage<'a>>,
    target: Option<DpkgStagedPackage<'a>>,
}

impl<'a> DpkgStagedOperation<'a> {
    pub(crate) const fn from_packages(
        source: Option<DpkgStagedPackage<'a>>,
        target: Option<DpkgStagedPackage<'a>>,
    ) -> Self {
        Self { source, target }
    }

    pub const fn source(self) -> Option<DpkgStagedPackage<'a>> {
        self.source
    }

    pub const fn target(self) -> Option<DpkgStagedPackage<'a>> {
        self.target
    }

    pub const fn package(self, slot: ArtifactSlot) -> Option<DpkgStagedPackage<'a>> {
        match slot {
            ArtifactSlot::Source => self.source,
            ArtifactSlot::Target => self.target,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DpkgRestoreRequest<'a> {
    desired_source: Option<DpkgStagedPackage<'a>>,
    recovery_target: Option<DpkgStagedPackage<'a>>,
}

impl<'a> DpkgRestoreRequest<'a> {
    pub(crate) const fn from_packages(
        desired_source: Option<DpkgStagedPackage<'a>>,
        recovery_target: Option<DpkgStagedPackage<'a>>,
    ) -> Self {
        Self {
            desired_source,
            recovery_target,
        }
    }

    pub const fn desired_source(self) -> Option<DpkgStagedPackage<'a>> {
        self.desired_source
    }

    pub const fn recovery_target(self) -> Option<DpkgStagedPackage<'a>> {
        self.recovery_target
    }
}

pub trait DpkgTransactionPort {
    type QuiescencePermit;

    fn validate_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<(), DpkgPortError>;

    fn inspect_package(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<PackageSnapshot, DpkgPortError>;

    fn validate_staged_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
        staged: DpkgStagedOperation<'_>,
    ) -> Result<(), DpkgPortError>;

    fn prove_quiescent(
        &mut self,
        context: DpkgOperationContext<'_>,
        phase: DpkgQuiescencePhase,
    ) -> Result<Self::QuiescencePermit, DpkgPortError>;

    fn apply_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        package: DpkgStagedPackage<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError>;

    fn remove_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError>;

    fn validate_product(
        &mut self,
        context: DpkgOperationContext<'_>,
        phase: DpkgProductValidationPhase,
        expected: DpkgExpectedProductState<'_>,
        snapshot: &PackageSnapshot,
    ) -> Result<(), DpkgPortError>;

    fn restore_source(
        &mut self,
        context: DpkgOperationContext<'_>,
        request: DpkgRestoreRequest<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgPortErrorCode {
    ArtifactInvalid,
    EnvironmentUnsupported,
    DependencyUnavailable,
    VersionRelationInvalid,
    ProgramsRunning,
    ProcessInspectionUnavailable,
    PackageStateUnavailable,
    PackageStateUnknown,
    PackageStateUnexpected,
    CommandInvalid,
    MutationFailed,
    ProductValidationFailed,
    PermissionDenied,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgPortPhase {
    OperationValidation,
    PackageInspection,
    StagedOperationValidation,
    Quiescence,
    TargetMutation,
    TargetValidation,
    SourceRestore,
    SourceValidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgPortError {
    code: DpkgPortErrorCode,
    phase: DpkgPortPhase,
    message: String,
}

impl DpkgPortError {
    pub fn new(code: DpkgPortErrorCode, phase: DpkgPortPhase, message: impl Into<String>) -> Self {
        Self {
            code,
            phase,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> DpkgPortErrorCode {
        self.code
    }

    pub const fn phase(&self) -> DpkgPortPhase {
        self.phase
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn failure_code(&self) -> LinuxFailureCode {
        match self.code {
            DpkgPortErrorCode::ArtifactInvalid => LinuxFailureCode::ArtifactInvalid,
            DpkgPortErrorCode::EnvironmentUnsupported => LinuxFailureCode::EnvironmentUnsupported,
            DpkgPortErrorCode::DependencyUnavailable => LinuxFailureCode::DependencyUnavailable,
            DpkgPortErrorCode::VersionRelationInvalid => LinuxFailureCode::VersionRelationInvalid,
            DpkgPortErrorCode::ProgramsRunning => LinuxFailureCode::ProgramsRunning,
            DpkgPortErrorCode::ProcessInspectionUnavailable => {
                LinuxFailureCode::ProcessInspectionUnavailable
            }
            DpkgPortErrorCode::PackageStateUnavailable => LinuxFailureCode::PackageStateUnavailable,
            DpkgPortErrorCode::PackageStateUnknown => LinuxFailureCode::PackageStateUnknown,
            DpkgPortErrorCode::PackageStateUnexpected => LinuxFailureCode::PackageStateUnexpected,
            DpkgPortErrorCode::CommandInvalid => LinuxFailureCode::CommandInvalid,
            DpkgPortErrorCode::MutationFailed => match self.phase {
                DpkgPortPhase::SourceRestore => LinuxFailureCode::SourceRestoreFailed,
                _ => LinuxFailureCode::PackageMutationFailed,
            },
            DpkgPortErrorCode::ProductValidationFailed => match self.phase {
                DpkgPortPhase::SourceRestore | DpkgPortPhase::SourceValidation => {
                    LinuxFailureCode::SourceValidationFailed
                }
                _ => LinuxFailureCode::TargetValidationFailed,
            },
            DpkgPortErrorCode::PermissionDenied => LinuxFailureCode::PermissionDenied,
            DpkgPortErrorCode::Io => LinuxFailureCode::Io,
        }
    }
}

impl fmt::Display for DpkgPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?}/{:?}: {}",
            self.phase, self.code, self.message
        )
    }
}

impl std::error::Error for DpkgPortError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionOutcome {
    Completed,
    AbortedPreserved,
    RolledBack,
}

#[derive(Debug)]
pub enum TransactionError {
    Store(LinuxInstallStoreError),
    Receipt(LinuxInstallReceiptError),
    Port {
        failure: LinuxFailureCode,
        source: DpkgPortError,
    },
    RecoveryBlocked {
        failure: LinuxFailureCode,
        source: DpkgPortError,
    },
    OperationNotStaged,
}

impl TransactionError {
    pub const fn failure_code(&self) -> Option<LinuxFailureCode> {
        match self {
            Self::Port { failure, .. } | Self::RecoveryBlocked { failure, .. } => Some(*failure),
            Self::Store(_) | Self::Receipt(_) | Self::OperationNotStaged => None,
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "Linux install store: {error}"),
            Self::Receipt(error) => write!(formatter, "Linux install receipt: {error}"),
            Self::Port { failure, source } => {
                write!(formatter, "dpkg port failed ({failure:?}): {source}")
            }
            Self::RecoveryBlocked { failure, source } => write!(
                formatter,
                "source recovery remains blocked ({failure:?}): {source}"
            ),
            Self::OperationNotStaged => {
                formatter.write_str("Linux install operation is still awaiting artifact staging")
            }
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<LinuxInstallStoreError> for TransactionError {
    fn from(value: LinuxInstallStoreError) -> Self {
        Self::Store(value)
    }
}

impl From<LinuxInstallReceiptError> for TransactionError {
    fn from(value: LinuxInstallReceiptError) -> Self {
        Self::Receipt(value)
    }
}

pub fn prepare_operation(
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    request: LinuxOperationRequest,
    port: &mut impl DpkgTransactionPort,
) -> Result<LinuxInstallReceipt, TransactionError> {
    store.verify_guard(guard)?;
    let previous = store.load_receipt()?;
    let context = DpkgOperationContext::Preparing {
        request: &request,
        previous: previous.as_ref(),
    };
    port.validate_operation(context)
        .map_err(transaction_port_error)?;

    let mut operation_chain = previous
        .as_ref()
        .map_or_else(Vec::new, |receipt| receipt.operation_chain().to_vec());
    operation_chain.push(request.operation_id().to_owned());

    let initial_package = port
        .inspect_package(context)
        .map_err(transaction_port_error)?;
    if initial_package.state() == DpkgPackageState::Unknown {
        return Err(transaction_port_error(DpkgPortError::new(
            DpkgPortErrorCode::PackageStateUnknown,
            DpkgPortPhase::PackageInspection,
            "dpkg returned an unknown package state",
        )));
    }

    let receipt = LinuxInstallReceipt::from_request(
        request,
        operation_chain,
        store.root_identity().clone(),
        initial_package,
    )?;
    store.persist_receipt(guard, &receipt)?;
    Ok(receipt)
}

pub fn resume_operation<P: DpkgTransactionPort>(
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    port: &mut P,
) -> Result<TransactionOutcome, TransactionError> {
    store.verify_guard(guard)?;
    let mut receipt = store
        .load_receipt()?
        .ok_or(TransactionError::OperationNotStaged)?;
    let mut target_permit = None;

    loop {
        match receipt.state() {
            LinuxInstallState::Prepared => return Err(TransactionError::OperationNotStaged),
            LinuxInstallState::ArtifactsStaged => {
                if let Err(error) = validate_staged_preflight(store, port, &receipt) {
                    receipt.abort_preserved(error.failure_code())?;
                    store.persist_receipt(guard, &receipt)?;
                    return Ok(TransactionOutcome::AbortedPreserved);
                }
                let context = DpkgOperationContext::Resuming { receipt: &receipt };
                match port.prove_quiescent(context, DpkgQuiescencePhase::TargetMutation) {
                    Ok(permit) => target_permit = Some(permit),
                    Err(error) => {
                        receipt.abort_preserved(error.failure_code())?;
                        store.persist_receipt(guard, &receipt)?;
                        return Ok(TransactionOutcome::AbortedPreserved);
                    }
                }
                receipt.advance(LinuxInstallState::Quiesced)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::Quiesced => {
                if target_permit.is_none() {
                    if let Err(error) = validate_staged_preflight(store, port, &receipt) {
                        receipt.abort_preserved(error.failure_code())?;
                        store.persist_receipt(guard, &receipt)?;
                        return Ok(TransactionOutcome::AbortedPreserved);
                    }
                    let context = DpkgOperationContext::Resuming { receipt: &receipt };
                    match port.prove_quiescent(context, DpkgQuiescencePhase::TargetMutation) {
                        Ok(permit) => target_permit = Some(permit),
                        Err(error) => {
                            receipt.abort_preserved(error.failure_code())?;
                            store.persist_receipt(guard, &receipt)?;
                            return Ok(TransactionOutcome::AbortedPreserved);
                        }
                    }
                }
                receipt.advance(LinuxInstallState::PackageMutating)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::PackageMutating => {
                if let Err(failure) = drive_target(store, port, &mut receipt, target_permit.take())
                {
                    receipt.require_rollback(failure)?;
                    store.persist_receipt(guard, &receipt)?;
                    continue;
                }
                store.persist_receipt(guard, &receipt)?;
                receipt.advance(LinuxInstallState::PackageVerified)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::PackageVerified => {
                let failure = validate_target_state(port, &receipt).err();
                if let Some(failure) = failure {
                    receipt.require_rollback(failure)?;
                    store.persist_receipt(guard, &receipt)?;
                    continue;
                }
                receipt.advance(LinuxInstallState::Completed)?;
                store.persist_receipt(guard, &receipt)?;
                return Ok(TransactionOutcome::Completed);
            }
            LinuxInstallState::RollbackRequired => {
                receipt.advance(LinuxInstallState::SourceRestoring)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::SourceRestoring => {
                if let Err((failure, source)) = drive_source(store, port, &mut receipt) {
                    return Err(TransactionError::RecoveryBlocked { failure, source });
                }
                store.persist_receipt(guard, &receipt)?;
                receipt.advance(LinuxInstallState::SourceVerified)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::SourceVerified => {
                validate_source_state(port, &receipt).map_err(|source| {
                    TransactionError::RecoveryBlocked {
                        failure: source.failure_code(),
                        source,
                    }
                })?;
                receipt.finish_rollback()?;
                store.persist_receipt(guard, &receipt)?;
                return Ok(TransactionOutcome::RolledBack);
            }
            LinuxInstallState::Completed => return Ok(TransactionOutcome::Completed),
            LinuxInstallState::AbortedPreserved => return Ok(TransactionOutcome::AbortedPreserved),
            LinuxInstallState::RolledBack => return Ok(TransactionOutcome::RolledBack),
        }
    }
}

fn drive_target<P: DpkgTransactionPort>(
    store: &LinuxInstallStore,
    port: &mut P,
    receipt: &mut LinuxInstallReceipt,
    permit: Option<P::QuiescencePermit>,
) -> Result<(), LinuxFailureCode> {
    let context = DpkgOperationContext::Resuming { receipt };
    let resolved_staged =
        ResolvedStagedOperation::load(store, receipt).map_err(|error| error.failure_code())?;
    let staged = resolved_staged
        .as_port_operation(receipt)
        .map_err(|error| error.failure_code())?;
    port.validate_staged_operation(context, staged)
        .map_err(|error| error.failure_code())?;
    let snapshot = port
        .inspect_package(context)
        .map_err(|error| error.failure_code())?;
    if snapshot.state() == DpkgPackageState::Unknown {
        return Err(LinuxFailureCode::PackageStateUnknown);
    }
    if receipt.matches_target(&snapshot) {
        let expected = target_expectation(receipt)?;
        match port.validate_product(
            context,
            DpkgProductValidationPhase::Target,
            expected,
            &snapshot,
        ) {
            Ok(()) => {
                if receipt.target_proof().is_none() {
                    receipt
                        .record_target_proof(snapshot)
                        .map_err(|_| LinuxFailureCode::ReceiptInconsistent)?;
                }
                return Ok(());
            }
            Err(error)
                if receipt.operation_kind() == LinuxOperationKind::Repair
                    && error.code() == DpkgPortErrorCode::ProductValidationFailed => {}
            Err(error) => return Err(error.failure_code()),
        }
    }

    if !receipt.source_snapshot_is_mutation_precondition(&snapshot)
        && !target_is_recoverable(receipt, &snapshot)
    {
        return Err(LinuxFailureCode::PackageStateUnexpected);
    }
    let permit = permit
        .map_or_else(
            || port.prove_quiescent(context, DpkgQuiescencePhase::TargetMutation),
            Ok,
        )
        .map_err(|error| error.failure_code())?;

    if receipt.operation_kind() == LinuxOperationKind::Remove {
        port.remove_package(context, permit)
            .map_err(|error| error.failure_code())?;
    } else {
        let target = staged
            .target()
            .ok_or(LinuxFailureCode::ReceiptInconsistent)?;
        port.apply_package(context, target, permit)
            .map_err(|error| error.failure_code())?;
    }

    let context = DpkgOperationContext::Resuming { receipt };
    let target_snapshot = port
        .inspect_package(context)
        .map_err(|error| error.failure_code())?;
    let expected = target_expectation(receipt)?;
    if !receipt.matches_target(&target_snapshot) {
        return Err(LinuxFailureCode::TargetValidationFailed);
    }
    port.validate_product(
        context,
        DpkgProductValidationPhase::Target,
        expected,
        &target_snapshot,
    )
    .map_err(|error| error.failure_code())?;
    receipt
        .record_target_proof(target_snapshot)
        .map_err(|_| LinuxFailureCode::ReceiptInconsistent)
}

fn drive_source<P: DpkgTransactionPort>(
    store: &LinuxInstallStore,
    port: &mut P,
    receipt: &mut LinuxInstallReceipt,
) -> Result<(), (LinuxFailureCode, DpkgPortError)> {
    let context = DpkgOperationContext::Resuming { receipt };
    let resolved_staged = ResolvedStagedOperation::load(store, receipt)
        .map_err(|error| (error.failure_code(), error))?;
    let staged = resolved_staged
        .as_port_operation(receipt)
        .map_err(|error| (error.failure_code(), error))?;
    port.validate_staged_operation(context, staged)
        .map_err(|error| (error.failure_code(), error))?;
    if receipt.source_proof().is_some() {
        return Ok(());
    }
    let mut snapshot = port
        .inspect_package(context)
        .map_err(|error| (error.failure_code(), error))?;
    let expected = source_expectation(receipt);
    let source_valid = if receipt.matches_source(&snapshot) {
        match port.validate_product(
            context,
            DpkgProductValidationPhase::Source,
            expected,
            &snapshot,
        ) {
            Ok(()) => true,
            Err(error) if error.code() == DpkgPortErrorCode::ProductValidationFailed => false,
            Err(error) => return Err((error.failure_code(), error)),
        }
    } else {
        false
    };
    if !source_valid {
        let desired_source = match receipt.restore_staged_artifact() {
            Some(evidence) => Some(
                staged
                    .package(evidence.slot())
                    .ok_or_else(|| receipt_port_error(LinuxFailureCode::ReceiptInconsistent))?,
            ),
            None => None,
        };
        let permit = port
            .prove_quiescent(context, DpkgQuiescencePhase::SourceRestore)
            .map_err(|error| (error.failure_code(), error))?;
        port.restore_source(
            context,
            DpkgRestoreRequest {
                desired_source,
                recovery_target: staged.target(),
            },
            permit,
        )
        .map_err(|error| (error.failure_code(), error))?;
        snapshot = port
            .inspect_package(context)
            .map_err(|error| (error.failure_code(), error))?;
    }
    if !receipt.matches_source(&snapshot) {
        return Err(source_validation_error(
            "source package state does not match the receipt",
        ));
    }
    port.validate_product(
        context,
        DpkgProductValidationPhase::Source,
        expected,
        &snapshot,
    )
    .map_err(|error| (error.failure_code(), error))?;
    receipt
        .record_source_proof(snapshot)
        .map_err(|_| receipt_port_error(LinuxFailureCode::ReceiptInconsistent))
}

struct ResolvedStagedOperation {
    source: Option<StagedArtifactPaths>,
    target: Option<StagedArtifactPaths>,
}

impl ResolvedStagedOperation {
    fn load(
        store: &LinuxInstallStore,
        receipt: &LinuxInstallReceipt,
    ) -> Result<Self, DpkgPortError> {
        Ok(Self {
            source: load_staged_slot(store, receipt, ArtifactSlot::Source)?,
            target: load_staged_slot(store, receipt, ArtifactSlot::Target)?,
        })
    }

    fn as_port_operation<'a>(
        &'a self,
        receipt: &'a LinuxInstallReceipt,
    ) -> Result<DpkgStagedOperation<'a>, DpkgPortError> {
        Ok(DpkgStagedOperation {
            source: staged_package_for_slot(self.source.as_ref(), receipt, ArtifactSlot::Source)?,
            target: staged_package_for_slot(self.target.as_ref(), receipt, ArtifactSlot::Target)?,
        })
    }
}

fn validate_staged_preflight(
    store: &LinuxInstallStore,
    port: &mut impl DpkgTransactionPort,
    receipt: &LinuxInstallReceipt,
) -> Result<(), DpkgPortError> {
    let resolved = ResolvedStagedOperation::load(store, receipt)?;
    let staged = resolved.as_port_operation(receipt)?;
    port.validate_staged_operation(DpkgOperationContext::Resuming { receipt }, staged)
}

fn load_staged_slot(
    store: &LinuxInstallStore,
    receipt: &LinuxInstallReceipt,
    slot: ArtifactSlot,
) -> Result<Option<StagedArtifactPaths>, DpkgPortError> {
    receipt
        .staged_artifact(slot)
        .map(|_| store.staged_artifact_paths(receipt.operation_id(), slot))
        .transpose()
        .map_err(|error| staged_operation_error(error.to_string()))
}

fn staged_package_for_slot<'a>(
    paths: Option<&'a StagedArtifactPaths>,
    receipt: &'a LinuxInstallReceipt,
    slot: ArtifactSlot,
) -> Result<Option<DpkgStagedPackage<'a>>, DpkgPortError> {
    match (receipt.staged_artifact(slot), paths) {
        (None, None) => Ok(None),
        (Some(_), Some(paths)) => receipt
            .artifact_for_slot(slot)
            .map(|artifact| Some(DpkgStagedPackage::new(artifact, paths)))
            .ok_or_else(|| staged_operation_error("staged slot has no artifact identity")),
        _ => Err(staged_operation_error(
            "staged slot paths do not match the receipt inventory",
        )),
    }
}

fn validate_target_state(
    port: &mut impl DpkgTransactionPort,
    receipt: &LinuxInstallReceipt,
) -> Result<(), LinuxFailureCode> {
    let context = DpkgOperationContext::Resuming { receipt };
    let snapshot = port
        .inspect_package(context)
        .map_err(|error| error.failure_code())?;
    if !receipt.matches_target(&snapshot) {
        return Err(LinuxFailureCode::TargetValidationFailed);
    }
    port.validate_product(
        context,
        DpkgProductValidationPhase::Target,
        target_expectation(receipt)?,
        &snapshot,
    )
    .map_err(|error| error.failure_code())
}

fn validate_source_state(
    port: &mut impl DpkgTransactionPort,
    receipt: &LinuxInstallReceipt,
) -> Result<(), DpkgPortError> {
    let context = DpkgOperationContext::Resuming { receipt };
    let snapshot = port.inspect_package(context)?;
    if !receipt.matches_source(&snapshot) {
        return Err(source_validation_error(
            "restored source package state no longer matches the receipt",
        )
        .1);
    }
    port.validate_product(
        context,
        DpkgProductValidationPhase::Source,
        source_expectation(receipt),
        &snapshot,
    )
}

fn target_expectation(
    receipt: &LinuxInstallReceipt,
) -> Result<DpkgExpectedProductState<'_>, LinuxFailureCode> {
    if receipt.operation_kind() == LinuxOperationKind::Remove {
        Ok(DpkgExpectedProductState::Absent)
    } else {
        receipt
            .target_artifact()
            .map(DpkgExpectedProductState::Installed)
            .ok_or(LinuxFailureCode::ReceiptInconsistent)
    }
}

fn source_expectation(receipt: &LinuxInstallReceipt) -> DpkgExpectedProductState<'_> {
    receipt.source_artifact().map_or(
        DpkgExpectedProductState::Absent,
        DpkgExpectedProductState::Installed,
    )
}

fn target_is_recoverable(receipt: &LinuxInstallReceipt, snapshot: &PackageSnapshot) -> bool {
    receipt.target_artifact().is_some_and(|target| {
        snapshot.state().is_recoverable() && snapshot.artifact() == Some(target)
    })
}

fn transaction_port_error(source: DpkgPortError) -> TransactionError {
    TransactionError::Port {
        failure: source.failure_code(),
        source,
    }
}

fn staged_operation_error(message: impl Into<String>) -> DpkgPortError {
    DpkgPortError::new(
        DpkgPortErrorCode::ArtifactInvalid,
        DpkgPortPhase::StagedOperationValidation,
        message,
    )
}

fn receipt_port_error(failure: LinuxFailureCode) -> (LinuxFailureCode, DpkgPortError) {
    (
        failure,
        DpkgPortError::new(
            DpkgPortErrorCode::ProductValidationFailed,
            DpkgPortPhase::SourceValidation,
            "Linux install receipt is inconsistent",
        ),
    )
}

fn source_validation_error(message: &'static str) -> (LinuxFailureCode, DpkgPortError) {
    let error = DpkgPortError::new(
        DpkgPortErrorCode::ProductValidationFailed,
        DpkgPortPhase::SourceValidation,
        message,
    );
    (error.failure_code(), error)
}
