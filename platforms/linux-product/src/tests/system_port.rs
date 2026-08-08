use std::collections::VecDeque;

use crate::coordinator::{
    DpkgExpectedProductState, DpkgOperationContext, DpkgPortErrorCode, DpkgPortPhase,
    DpkgProductValidationPhase, DpkgQuiescencePhase, DpkgRestoreRequest, DpkgStagedOperation,
    DpkgStagedPackage, DpkgTransactionPort,
};
use crate::debian::{
    status_snapshot, ArtifactFixture, DebianCommandInvocation, DebianInvocationPhase,
    DpkgStatusSnapshot, PrivateStagedDeb, StagedDebSlot, VerifiedArtifactRelationship,
};
use crate::model::{
    ArtifactFileIdentity, ArtifactSlot, ArtifactVersionRelation, DpkgPackageState,
    LinuxFailureCode, LinuxInstallReceipt, LinuxInstallRootIdentity, LinuxInstallState,
    LinuxOperationKind, LinuxOperationRequest, PackageSnapshot, StagedArtifactEvidence,
};
use crate::system::{
    DebianCommandExecutor, DebianCommandOutput, DebianCommandTermination, DebianExecutionError,
    DebianExecutionErrorCode, DpkgSystemObserver, LinuxDpkgTransactionPort,
    LinuxSystemObservationError, LinuxSystemObservationErrorCode,
};

const OPERATION_ID: &str = "0123456789abcdef0123456789abcdef";

#[derive(Debug)]
struct FakePermit;

struct FakeObserver {
    statuses: VecDeque<DpkgStatusSnapshot>,
    relationships: Vec<VerifiedArtifactRelationship>,
    environment_error: Option<LinuxSystemObservationErrorCode>,
    quiescence_error: Option<LinuxSystemObservationErrorCode>,
    staged_calls: usize,
    installed_validations: usize,
    absent_validations: usize,
}

impl FakeObserver {
    fn new(
        statuses: impl IntoIterator<Item = DpkgStatusSnapshot>,
        relationships: Vec<VerifiedArtifactRelationship>,
    ) -> Self {
        Self {
            statuses: statuses.into_iter().collect(),
            relationships,
            environment_error: None,
            quiescence_error: None,
            staged_calls: 0,
            installed_validations: 0,
            absent_validations: 0,
        }
    }

    fn current_status(&mut self) -> DpkgStatusSnapshot {
        if self.statuses.len() > 1 {
            self.statuses.pop_front().expect("queued status")
        } else {
            self.statuses.front().expect("current status").clone()
        }
    }
}

impl DpkgSystemObserver for FakeObserver {
    type QuiescencePermit = FakePermit;

    fn validate_environment(&mut self) -> Result<(), LinuxSystemObservationError> {
        match self.environment_error {
            Some(code) => Err(LinuxSystemObservationError::new(code, "fake environment")),
            None => Ok(()),
        }
    }

    fn inspect_status(&mut self) -> Result<DpkgStatusSnapshot, LinuxSystemObservationError> {
        Ok(self.current_status())
    }

    fn verify_staged_package(
        &mut self,
        package: DpkgStagedPackage<'_>,
    ) -> Result<VerifiedArtifactRelationship, LinuxSystemObservationError> {
        self.staged_calls += 1;
        self.relationships
            .iter()
            .find(|relationship| {
                relationship
                    .matches_linux_artifact_identity(package.artifact())
                    .unwrap_or(false)
            })
            .cloned()
            .ok_or_else(|| {
                LinuxSystemObservationError::new(
                    LinuxSystemObservationErrorCode::ArtifactInvalid,
                    "fake staged artifact mismatch",
                )
            })
    }

    fn prove_quiescent(&mut self) -> Result<Self::QuiescencePermit, LinuxSystemObservationError> {
        match self.quiescence_error {
            Some(code) => Err(LinuxSystemObservationError::new(code, "fake quiescence")),
            None => Ok(FakePermit),
        }
    }

    fn validate_installed_product(
        &mut self,
        artifact: &crate::LinuxArtifactIdentity,
        relationship: &VerifiedArtifactRelationship,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError> {
        self.installed_validations += 1;
        if relationship
            .matches_linux_artifact_identity(artifact)
            .unwrap_or(false)
            && status.validate_installed_relationship(relationship).is_ok()
        {
            Ok(())
        } else {
            Err(LinuxSystemObservationError::new(
                LinuxSystemObservationErrorCode::ProductIdentityChanged,
                "fake installed product mismatch",
            ))
        }
    }

    fn validate_absent_product(
        &mut self,
        _relationship: Option<&VerifiedArtifactRelationship>,
        status: &DpkgStatusSnapshot,
    ) -> Result<(), LinuxSystemObservationError> {
        self.absent_validations += 1;
        if status
            .records()
            .iter()
            .all(|record| record.package() != "radishlex")
        {
            Ok(())
        } else {
            Err(LinuxSystemObservationError::new(
                LinuxSystemObservationErrorCode::ProductIdentityChanged,
                "fake removed product remains",
            ))
        }
    }
}

struct FakeExecutor {
    outputs: VecDeque<DebianCommandOutput>,
    invocations: Vec<DebianCommandInvocation>,
    error: Option<DebianExecutionErrorCode>,
}

impl FakeExecutor {
    fn new(outputs: impl IntoIterator<Item = DebianCommandOutput>) -> Self {
        Self {
            outputs: outputs.into_iter().collect(),
            invocations: Vec::new(),
            error: None,
        }
    }

    fn with_error(mut self, error: DebianExecutionErrorCode) -> Self {
        self.error = Some(error);
        self
    }
}

impl DebianCommandExecutor for FakeExecutor {
    fn execute(
        &mut self,
        invocation: &DebianCommandInvocation,
    ) -> Result<DebianCommandOutput, DebianExecutionError> {
        self.invocations.push(invocation.clone());
        if let Some(code) = self.error.take() {
            return Err(DebianExecutionError::new(code, "fake command failure"));
        }
        Ok(self.outputs.pop_front().unwrap_or_else(success))
    }
}

fn success() -> DebianCommandOutput {
    DebianCommandOutput::new(DebianCommandTermination::Exited(0), Vec::new(), Vec::new())
}

fn architecture_success() -> DebianCommandOutput {
    DebianCommandOutput::new(
        DebianCommandTermination::Exited(0),
        b"arm64\n".to_vec(),
        Vec::new(),
    )
}

fn dependency_only_status(relationship: &VerifiedArtifactRelationship) -> DpkgStatusSnapshot {
    let value = status_snapshot(relationship, None, false);
    let (_, dependencies) = value
        .split_once("\n\n")
        .expect("product and dependency paragraphs");
    DpkgStatusSnapshot::parse(dependencies.as_bytes()).expect("dependency-only status")
}

fn installed_status(relationship: &VerifiedArtifactRelationship) -> DpkgStatusSnapshot {
    DpkgStatusSnapshot::parse(status_snapshot(relationship, None, false).as_bytes())
        .expect("installed status")
}

fn partial_status(relationship: &VerifiedArtifactRelationship) -> DpkgStatusSnapshot {
    let value = status_snapshot(relationship, None, false).replacen(
        "Status: install ok installed",
        "Status: install reinstreq half-configured",
        1,
    );
    DpkgStatusSnapshot::parse(value.as_bytes()).expect("partial status")
}

fn request(
    kind: LinuxOperationKind,
    source: Option<&VerifiedArtifactRelationship>,
    target: Option<&VerifiedArtifactRelationship>,
) -> LinuxOperationRequest {
    let relation = match kind {
        LinuxOperationKind::Install | LinuxOperationKind::Remove => {
            ArtifactVersionRelation::NotApplicable
        }
        LinuxOperationKind::Upgrade => ArtifactVersionRelation::TargetNewer,
        LinuxOperationKind::Repair => ArtifactVersionRelation::SameRelease,
        LinuxOperationKind::Rollback => ArtifactVersionRelation::TargetOlder,
    };
    LinuxOperationRequest::new(
        OPERATION_ID,
        kind,
        relation,
        source.map(verified_identity),
        target.map(verified_identity),
    )
    .expect("operation request")
}

fn verified_identity(relationship: &VerifiedArtifactRelationship) -> crate::LinuxArtifactIdentity {
    relationship
        .to_linux_artifact_identity()
        .expect("Linux artifact identity")
}

fn staged_package<'a>(
    relationship: &'a VerifiedArtifactRelationship,
    slot: StagedDebSlot,
) -> DpkgStagedPackage<'a> {
    let artifact = relationship
        .to_linux_artifact_identity()
        .expect("temporary identity");
    let artifact = Box::leak(Box::new(artifact));
    let package = PrivateStagedDeb::new(OPERATION_ID, slot).expect("fixed staged path");
    let package_path = Box::leak(Box::new(package.path().to_owned()));
    let evidence_path = Box::leak(Box::new(package.path().with_file_name(match slot {
        StagedDebSlot::Source => "source.evidence.json",
        StagedDebSlot::Target => "target.evidence.json",
    })));
    DpkgStagedPackage::from_paths(artifact, package_path, evidence_path)
}

fn receipt(
    kind: LinuxOperationKind,
    source: Option<&VerifiedArtifactRelationship>,
    target: Option<&VerifiedArtifactRelationship>,
    state: LinuxInstallState,
) -> LinuxInstallReceipt {
    let request = request(kind, source, target);
    let initial = source.map_or_else(PackageSnapshot::absent, |relationship| {
        PackageSnapshot::exact_installed(verified_identity(relationship))
    });
    let root = LinuxInstallRootIdentity::new(1, 1, 0, 0, 0o755).expect("root identity");
    let mut receipt =
        LinuxInstallReceipt::from_request(request, vec![OPERATION_ID.to_owned()], root, initial)
            .expect("prepared receipt");
    let required = match kind {
        LinuxOperationKind::Install | LinuxOperationKind::Repair => {
            vec![(ArtifactSlot::Target, target)]
        }
        LinuxOperationKind::Upgrade | LinuxOperationKind::Rollback => vec![
            (ArtifactSlot::Source, source),
            (ArtifactSlot::Target, target),
        ],
        LinuxOperationKind::Remove => vec![(ArtifactSlot::Source, source)],
    };
    for (index, (slot, relationship)) in required.into_iter().enumerate() {
        let identity = verified_identity(relationship.expect("required relationship"));
        let package = ArtifactFileIdentity::new(
            1,
            10 + index as u64 * 2,
            0,
            0,
            0o600,
            1,
            identity.package_size(),
            identity.package_sha256(),
        )
        .expect("package proof");
        let evidence = ArtifactFileIdentity::new(
            1,
            11 + index as u64 * 2,
            0,
            0,
            0o600,
            1,
            identity.evidence_size(),
            identity.evidence_sha256(),
        )
        .expect("evidence proof");
        receipt
            .record_staged_artifact(
                StagedArtifactEvidence::new(slot, identity, package, evidence)
                    .expect("staged proof"),
            )
            .expect("record staged proof");
    }
    receipt
        .advance(LinuxInstallState::ArtifactsStaged)
        .expect("artifacts staged");
    if state == LinuxInstallState::ArtifactsStaged {
        return receipt;
    }
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("quiesced");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("package mutating");
    if state == LinuxInstallState::PackageMutating {
        return receipt;
    }
    receipt
        .require_rollback(LinuxFailureCode::PackageMutationFailed)
        .expect("rollback required");
    receipt
        .advance(LinuxInstallState::SourceRestoring)
        .expect("source restoring");
    assert_eq!(state, LinuxInstallState::SourceRestoring);
    receipt
}

#[test]
fn operation_preflight_binds_verified_relationship_architecture_and_dependencies() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let request = request(LinuxOperationKind::Install, None, Some(&target));
    let status = dependency_only_status(&target);
    let observer = FakeObserver::new([status], vec![target.clone()]);
    let executor = FakeExecutor::new([architecture_success()]);
    let mut port = LinuxDpkgTransactionPort::with_parts(observer, executor, vec![target.clone()]);
    port.validate_operation(DpkgOperationContext::Preparing {
        request: &request,
        previous: None,
    })
    .expect("verified operation preflight");
    assert_eq!(port.executor().invocations.len(), 1);
    assert_eq!(
        port.executor().invocations[0].phase(),
        DebianInvocationPhase::ArchitecturePreflight
    );

    let executor = FakeExecutor::new([DebianCommandOutput::new(
        DebianCommandTermination::Exited(0),
        b"amd64\n".to_vec(),
        Vec::new(),
    )]);
    let observer = FakeObserver::new([dependency_only_status(&target)], vec![target.clone()]);
    let mut wrong_arch = LinuxDpkgTransactionPort::with_parts(observer, executor, vec![target]);
    let error = wrong_arch
        .validate_operation(DpkgOperationContext::Preparing {
            request: &request,
            previous: None,
        })
        .expect_err("wrong architecture must fail");
    assert_eq!(error.code(), DpkgPortErrorCode::EnvironmentUnsupported);
}

#[test]
fn staged_upgrade_recomputes_relation_and_checks_both_dependency_profiles() {
    let source = ArtifactFixture::new("26.7.1+37-1").verify();
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Upgrade,
        Some(&source),
        Some(&target),
        LinuxInstallState::ArtifactsStaged,
    );
    let context = DpkgOperationContext::Resuming { receipt: &receipt };
    let staged = DpkgStagedOperation::from_packages(
        Some(staged_package(&source, StagedDebSlot::Source)),
        Some(staged_package(&target, StagedDebSlot::Target)),
    );
    let observer = FakeObserver::new(
        [installed_status(&source)],
        vec![source.clone(), target.clone()],
    );
    let mut port = LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([]), vec![]);
    port.validate_staged_operation(context, staged)
        .expect("staged upgrade relationship");
    assert_eq!(port.observer().staged_calls, 2);
}

#[test]
fn quiescence_failures_never_reach_the_command_executor() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::ArtifactsStaged,
    );
    let mut observer = FakeObserver::new([dependency_only_status(&target)], vec![target]);
    observer.quiescence_error = Some(LinuxSystemObservationErrorCode::ProgramsRunning);
    let mut port = LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([]), vec![]);
    let error = port
        .prove_quiescent(
            DpkgOperationContext::Resuming { receipt: &receipt },
            DpkgQuiescencePhase::TargetMutation,
        )
        .expect_err("running programs must block");
    assert_eq!(error.code(), DpkgPortErrorCode::ProgramsRunning);
    assert!(port.executor().invocations.is_empty());
}

#[test]
fn target_apply_remove_and_source_restore_use_only_typed_commands() {
    let source = ArtifactFixture::new("26.7.1+37-1").verify();
    let target = ArtifactFixture::new("26.7.1+38-1").verify();

    let apply_receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::PackageMutating,
    );
    let observer = FakeObserver::new([dependency_only_status(&target)], vec![target.clone()]);
    let mut apply =
        LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([success()]), vec![]);
    apply
        .apply_package(
            DpkgOperationContext::Resuming {
                receipt: &apply_receipt,
            },
            staged_package(&target, StagedDebSlot::Target),
            FakePermit,
        )
        .expect("target apply");
    assert_eq!(
        apply.executor().invocations[0].phase(),
        DebianInvocationPhase::TargetApply
    );

    let remove_receipt = receipt(
        LinuxOperationKind::Remove,
        Some(&source),
        None,
        LinuxInstallState::PackageMutating,
    );
    let observer = FakeObserver::new([installed_status(&source)], vec![source.clone()]);
    let mut remove =
        LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([success()]), vec![]);
    remove
        .remove_package(
            DpkgOperationContext::Resuming {
                receipt: &remove_receipt,
            },
            FakePermit,
        )
        .expect("target remove");
    assert_eq!(
        remove.executor().invocations[0].phase(),
        DebianInvocationPhase::TargetRemove
    );

    let restore_receipt = receipt(
        LinuxOperationKind::Upgrade,
        Some(&source),
        Some(&target),
        LinuxInstallState::SourceRestoring,
    );
    let observer = FakeObserver::new([installed_status(&target)], vec![source.clone(), target]);
    let mut restore =
        LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([success()]), vec![]);
    restore
        .restore_source(
            DpkgOperationContext::Resuming {
                receipt: &restore_receipt,
            },
            DpkgRestoreRequest::from_packages(
                Some(staged_package(&source, StagedDebSlot::Source)),
                None,
            ),
            FakePermit,
        )
        .expect("source restore");
    assert_eq!(
        restore.executor().invocations[0].phase(),
        DebianInvocationPhase::SourceRestore
    );
}

#[test]
fn first_install_partial_recovery_reinstalls_the_verified_target_before_remove() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::SourceRestoring,
    );
    let observer = FakeObserver::new(
        [partial_status(&target), installed_status(&target)],
        vec![target.clone()],
    );
    let mut port = LinuxDpkgTransactionPort::with_parts(
        observer,
        FakeExecutor::new([success(), success()]),
        vec![],
    );
    port.restore_source(
        DpkgOperationContext::Resuming { receipt: &receipt },
        DpkgRestoreRequest::from_packages(
            None,
            Some(staged_package(&target, StagedDebSlot::Target)),
        ),
        FakePermit,
    )
    .expect("first-install source absence recovery");
    assert_eq!(
        port.executor()
            .invocations
            .iter()
            .map(DebianCommandInvocation::phase)
            .collect::<Vec<_>>(),
        [
            DebianInvocationPhase::SourceRestore,
            DebianInvocationPhase::TargetRemove,
        ]
    );
}

#[test]
fn signaled_and_nonzero_mutations_remain_typed_failures() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::PackageMutating,
    );
    for termination in [
        DebianCommandTermination::Exited(1),
        DebianCommandTermination::Signaled,
    ] {
        let observer = FakeObserver::new([dependency_only_status(&target)], vec![target.clone()]);
        let output = DebianCommandOutput::new(termination, Vec::new(), b"redacted".to_vec());
        let mut port =
            LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([output]), vec![]);
        let error = port
            .apply_package(
                DpkgOperationContext::Resuming { receipt: &receipt },
                staged_package(&target, StagedDebSlot::Target),
                FakePermit,
            )
            .expect_err("failed mutation must remain typed");
        assert_eq!(error.code(), DpkgPortErrorCode::MutationFailed);
        assert_eq!(error.phase(), DpkgPortPhase::TargetMutation);
    }
}

#[test]
fn executor_failures_map_to_stable_transaction_codes() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::PackageMutating,
    );
    for (execution, expected) in [
        (
            DebianExecutionErrorCode::ProgramIdentity,
            DpkgPortErrorCode::CommandInvalid,
        ),
        (
            DebianExecutionErrorCode::DiagnosticsOverflow,
            DpkgPortErrorCode::CommandInvalid,
        ),
        (
            DebianExecutionErrorCode::TimedOut,
            DpkgPortErrorCode::MutationFailed,
        ),
        (
            DebianExecutionErrorCode::PermissionDenied,
            DpkgPortErrorCode::PermissionDenied,
        ),
        (DebianExecutionErrorCode::Io, DpkgPortErrorCode::Io),
    ] {
        let observer = FakeObserver::new([dependency_only_status(&target)], vec![target.clone()]);
        let executor = FakeExecutor::new([]).with_error(execution);
        let mut port = LinuxDpkgTransactionPort::with_parts(observer, executor, vec![]);
        let error = port
            .apply_package(
                DpkgOperationContext::Resuming { receipt: &receipt },
                staged_package(&target, StagedDebSlot::Target),
                FakePermit,
            )
            .expect_err("executor failure remains typed");
        assert_eq!(error.code(), expected);
        assert_eq!(error.phase(), DpkgPortPhase::TargetMutation);
    }
}

#[test]
fn fresh_product_validation_rehydrates_staged_relationship_and_checks_status() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let receipt = receipt(
        LinuxOperationKind::Install,
        None,
        Some(&target),
        LinuxInstallState::PackageMutating,
    );
    let identity = verified_identity(&target);
    let snapshot = PackageSnapshot::exact_installed(identity.clone());
    let observer = FakeObserver::new([installed_status(&target)], vec![target]);
    let mut port = LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([]), vec![]);
    port.validate_product(
        DpkgOperationContext::Resuming { receipt: &receipt },
        DpkgProductValidationPhase::Target,
        DpkgExpectedProductState::Installed(&identity),
        &snapshot,
    )
    .expect("fresh product validation");
    assert_eq!(port.observer().staged_calls, 1);
    assert_eq!(port.observer().installed_validations, 1);
}

#[test]
fn absent_snapshot_projection_is_exact_for_first_install() {
    let target = ArtifactFixture::new("26.7.1+38-1").verify();
    let request = request(LinuxOperationKind::Install, None, Some(&target));
    let observer = FakeObserver::new([dependency_only_status(&target)], vec![target]);
    let mut port = LinuxDpkgTransactionPort::with_parts(observer, FakeExecutor::new([]), vec![]);
    let snapshot = port
        .inspect_package(DpkgOperationContext::Preparing {
            request: &request,
            previous: None,
        })
        .expect("absent package snapshot");
    assert_eq!(snapshot.state(), DpkgPackageState::NotInstalled);
    assert!(snapshot.artifact().is_none());
}
