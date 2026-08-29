use super::helper::{artifact, request, stage_case, FakeDpkg, OperationCase, TestEnvironment};
use crate::checkpoint::RejectingTargetValidationPort;
use crate::coordinator::resume_operation_with_checkpoints;
use crate::system::finish_staging_at_prepared_checkpoint;
use crate::{
    prepare_operation, resume_operation, ArtifactSlot, ArtifactVersionRelation, DpkgPackageState,
    DpkgQuiescencePhase, LinuxInstallState, LinuxL6Checkpoint, LinuxL6CheckpointError,
    LinuxL6CheckpointSink, LinuxOperationKind, PackageSnapshot, TransactionError,
    TransactionOutcome,
};

const OPERATION_ID: &str = "10101010101010101010101010101010";
const SOURCE_OPERATION_ID: &str = "01010101010101010101010101010101";

#[derive(Debug)]
struct InterruptAt {
    target: LinuxL6Checkpoint,
    reached: Vec<LinuxL6Checkpoint>,
}

impl InterruptAt {
    fn new(target: LinuxL6Checkpoint) -> Self {
        Self {
            target,
            reached: Vec::new(),
        }
    }
}

impl LinuxL6CheckpointSink for InterruptAt {
    fn reached(&mut self, checkpoint: LinuxL6Checkpoint) -> Result<(), LinuxL6CheckpointError> {
        self.reached.push(checkpoint);
        if checkpoint == self.target {
            Err(LinuxL6CheckpointError::unavailable(checkpoint))
        } else {
            Ok(())
        }
    }
}

#[test]
fn prepared_checkpoint_requires_complete_private_staging_and_resumes() {
    let environment = TestEnvironment::new("l6-prepared");
    let target = artifact(&environment, "0.2.0-1", "l6-prepared-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, OPERATION_ID),
        &mut port,
    )
    .expect("prepare install");
    environment
        .store
        .stage_artifact(
            &guard,
            ArtifactSlot::Target,
            &target.package_path,
            &target.evidence_path,
        )
        .expect("stage complete target pair");

    let mut checkpoints = InterruptAt::new(LinuxL6Checkpoint::Prepared);
    let error = finish_staging_at_prepared_checkpoint(&environment.store, &guard, &mut checkpoints)
        .expect_err("prepared checkpoint simulates process termination");
    assert_eq!(
        error.code(),
        crate::LinuxMaintenanceHostErrorCode::Transaction
    );
    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(receipt.state(), LinuxInstallState::Prepared);
    assert!(receipt.staged_artifact(ArtifactSlot::Target).is_some());
    drop(guard);

    let guard = environment.store.acquire_guard().expect("reacquire guard");
    environment
        .store
        .finish_staging(&guard)
        .expect("prepared resume closes staging deterministically");
    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("resume prepared checkpoint"),
        TransactionOutcome::Completed
    );
    assert_eq!(fresh_port.apply_calls, 1);
}

#[test]
fn prepared_checkpoint_is_not_emitted_for_incomplete_staging() {
    let environment = TestEnvironment::new("l6-prepared-incomplete");
    let target = artifact(&environment, "0.2.0-1", "l6-prepared-incomplete-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, OPERATION_ID),
        &mut port,
    )
    .expect("prepare install");
    let mut checkpoints = InterruptAt::new(LinuxL6Checkpoint::Prepared);
    let error = finish_staging_at_prepared_checkpoint(&environment.store, &guard, &mut checkpoints)
        .expect_err("incomplete staging must fail before checkpoint notification");
    assert_eq!(error.code(), crate::LinuxMaintenanceHostErrorCode::Store);
    assert!(checkpoints.reached.is_empty());
    assert_eq!(
        environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists")
            .state(),
        LinuxInstallState::Prepared
    );
}

#[test]
fn install_artifacts_staged_checkpoint_preserves_absent_package_and_resumes_once() {
    let environment = TestEnvironment::new("l6-install-artifacts-staged");
    let target = artifact(
        &environment,
        "0.1.0-1",
        "l6-install-artifacts-staged-target",
    );
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, OPERATION_ID),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);

    let mut checkpoints = InterruptAt::new(LinuxL6Checkpoint::ArtifactsStaged);
    let error =
        resume_operation_with_checkpoints(&environment.store, &guard, &mut port, &mut checkpoints)
            .expect_err("artifacts-staged checkpoint simulates process termination");
    assert!(matches!(error, TransactionError::Checkpoint(_)));
    assert_eq!(
        checkpoints.reached,
        vec![LinuxL6Checkpoint::ArtifactsStaged]
    );
    assert_eq!(port.staged_validation_calls, 0);
    assert!(port.quiescence_phases.is_empty());
    assert_eq!(port.apply_calls, 0);
    assert_eq!(port.snapshot.state(), DpkgPackageState::NotInstalled);

    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(receipt.operation_kind(), LinuxOperationKind::Install);
    assert_eq!(
        receipt.version_relation(),
        ArtifactVersionRelation::NotApplicable
    );
    assert_eq!(receipt.state(), LinuxInstallState::ArtifactsStaged);
    assert_eq!(receipt.operation_chain(), [OPERATION_ID]);
    assert!(receipt.source_artifact().is_none());
    assert_eq!(receipt.target_artifact(), Some(&target.identity));
    assert!(receipt.staged_artifact(ArtifactSlot::Source).is_none());
    assert!(receipt.staged_artifact(ArtifactSlot::Target).is_some());
    assert!(receipt.target_proof().is_none());
    assert!(receipt.source_proof().is_none());
    assert!(receipt.failure_code().is_none());
    assert!(!receipt.manual_recovery_required());
    drop(guard);

    let guard = environment.store.acquire_guard().expect("reacquire guard");
    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("resume artifacts-staged install"),
        TransactionOutcome::Completed
    );
    assert_eq!(fresh_port.staged_validation_calls, 2);
    assert_eq!(
        fresh_port.staged_validation_states,
        [
            LinuxInstallState::ArtifactsStaged,
            LinuxInstallState::PackageMutating,
        ]
    );
    assert_eq!(
        fresh_port.quiescence_phases,
        [DpkgQuiescencePhase::TargetMutation]
    );
    assert_eq!(fresh_port.consumed_permits.len(), 1);
    assert!(fresh_port.issued_permits.is_empty());
    assert_eq!(fresh_port.apply_calls, 1);
    assert_eq!(fresh_port.user_data_touches, 0);
    assert_eq!(fresh_port.snapshot.state(), DpkgPackageState::Installed);
    assert_eq!(fresh_port.snapshot.artifact(), Some(&target.identity));

    let completed = environment
        .store
        .load_receipt()
        .expect("load completed receipt")
        .expect("completed receipt exists");
    assert_eq!(completed.state(), LinuxInstallState::Completed);
    assert_eq!(
        completed.target_proof().and_then(PackageSnapshot::artifact),
        Some(&target.identity)
    );
}

#[test]
fn upgrade_quiesced_checkpoint_reproves_relationship_and_quiescence() {
    let environment = TestEnvironment::new("l6-upgrade-quiesced");
    let source = artifact(&environment, "0.1.0-1", "l6-quiesced-source");
    let target = artifact(&environment, "0.2.0-1", "l6-quiesced-target");
    let install_case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&source),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut install_port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&install_case, SOURCE_OPERATION_ID),
        &mut install_port,
    )
    .expect("prepare source install");
    stage_case(&environment, &guard, &install_case);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut install_port)
            .expect("complete source install"),
        TransactionOutcome::Completed
    );

    let upgrade_case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&source),
        target: Some(&target),
    };
    let mut checkpoint_port =
        FakeDpkg::new(PackageSnapshot::exact_installed(source.identity.clone()));
    prepare_operation(
        &environment.store,
        &guard,
        request(&upgrade_case, OPERATION_ID),
        &mut checkpoint_port,
    )
    .expect("prepare source-to-target upgrade");
    stage_case(&environment, &guard, &upgrade_case);

    let mut checkpoints = InterruptAt::new(LinuxL6Checkpoint::Quiesced);
    let error = resume_operation_with_checkpoints(
        &environment.store,
        &guard,
        &mut checkpoint_port,
        &mut checkpoints,
    )
    .expect_err("quiesced checkpoint simulates process termination");
    assert!(matches!(error, TransactionError::Checkpoint(_)));
    assert_eq!(
        checkpoints.reached,
        vec![
            LinuxL6Checkpoint::ArtifactsStaged,
            LinuxL6Checkpoint::Quiesced,
        ]
    );
    assert_eq!(
        checkpoint_port.staged_validation_states,
        [LinuxInstallState::ArtifactsStaged]
    );
    assert_eq!(
        checkpoint_port.quiescence_phases,
        [DpkgQuiescencePhase::TargetMutation]
    );
    assert_eq!(checkpoint_port.apply_calls, 0);
    assert!(checkpoint_port.consumed_permits.is_empty());
    assert_eq!(checkpoint_port.issued_permits.len(), 1);
    assert_eq!(checkpoint_port.snapshot.artifact(), Some(&source.identity));

    let receipt = environment
        .store
        .load_receipt()
        .expect("load quiesced receipt")
        .expect("quiesced receipt exists");
    assert_eq!(receipt.operation_kind(), LinuxOperationKind::Upgrade);
    assert_eq!(
        receipt.version_relation(),
        ArtifactVersionRelation::TargetNewer
    );
    assert_eq!(receipt.state(), LinuxInstallState::Quiesced);
    assert_eq!(
        receipt.operation_chain(),
        [SOURCE_OPERATION_ID, OPERATION_ID]
    );
    assert_eq!(receipt.source_artifact(), Some(&source.identity));
    assert_eq!(receipt.target_artifact(), Some(&target.identity));
    assert!(receipt.staged_artifact(ArtifactSlot::Source).is_some());
    assert!(receipt.staged_artifact(ArtifactSlot::Target).is_some());
    assert!(receipt.target_proof().is_none());
    assert!(receipt.source_proof().is_none());
    assert!(receipt.failure_code().is_none());
    assert!(!receipt.manual_recovery_required());
    drop(guard);

    let guard = environment.store.acquire_guard().expect("reacquire guard");
    let mut fresh_port = FakeDpkg::new(PackageSnapshot::exact_installed(source.identity.clone()));
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("resume quiesced upgrade"),
        TransactionOutcome::Completed
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [
            LinuxInstallState::Quiesced,
            LinuxInstallState::PackageMutating,
        ]
    );
    assert_eq!(
        fresh_port.quiescence_phases,
        [DpkgQuiescencePhase::TargetMutation]
    );
    assert_eq!(fresh_port.apply_calls, 1);
    assert_eq!(fresh_port.consumed_permits.len(), 1);
    assert!(fresh_port.issued_permits.is_empty());
    assert_eq!(fresh_port.user_data_touches, 0);
    assert_eq!(fresh_port.snapshot.artifact(), Some(&target.identity));
}

#[test]
fn forward_checkpoints_cover_staged_quiesced_before_and_after_dpkg() {
    for (checkpoint, expected_state, first_apply_calls) in [
        (
            LinuxL6Checkpoint::ArtifactsStaged,
            LinuxInstallState::ArtifactsStaged,
            0,
        ),
        (LinuxL6Checkpoint::Quiesced, LinuxInstallState::Quiesced, 0),
        (
            LinuxL6Checkpoint::PackageMutatingBeforeDpkg,
            LinuxInstallState::PackageMutating,
            0,
        ),
        (
            LinuxL6Checkpoint::TargetAppliedBeforeProof,
            LinuxInstallState::PackageMutating,
            1,
        ),
    ] {
        let environment = TestEnvironment::new(checkpoint.as_str());
        let old = artifact(&environment, "0.1.0-1", "l6-forward-old");
        let new = artifact(&environment, "0.2.0-1", "l6-forward-new");
        let case = OperationCase {
            kind: LinuxOperationKind::Upgrade,
            relation: ArtifactVersionRelation::TargetNewer,
            source: Some(&old),
            target: Some(&new),
        };
        let guard = environment.store.acquire_guard().expect("acquire guard");
        let mut port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
        prepare_operation(
            &environment.store,
            &guard,
            request(&case, OPERATION_ID),
            &mut port,
        )
        .expect("prepare upgrade");
        stage_case(&environment, &guard, &case);
        let mut checkpoints = InterruptAt::new(checkpoint);
        let error = resume_operation_with_checkpoints(
            &environment.store,
            &guard,
            &mut port,
            &mut checkpoints,
        )
        .expect_err("checkpoint interrupts the synthetic worker");
        assert!(matches!(error, TransactionError::Checkpoint(_)));
        assert_eq!(port.apply_calls, first_apply_calls);
        assert_eq!(
            environment
                .store
                .load_receipt()
                .expect("load receipt")
                .expect("receipt exists")
                .state(),
            expected_state
        );
        let snapshot = port.snapshot.clone();
        drop(guard);

        let guard = environment.store.acquire_guard().expect("reacquire guard");
        let mut fresh_port = FakeDpkg::new(snapshot);
        assert_eq!(
            resume_operation(&environment.store, &guard, &mut fresh_port)
                .expect("resume forward checkpoint"),
            TransactionOutcome::Completed
        );
        assert_eq!(first_apply_calls + fresh_port.apply_calls, 1);
    }
}

#[test]
fn rollback_checkpoints_reject_target_only_in_acceptance_identity_and_recover_source() {
    for (checkpoint, expected_state, first_restore_calls) in [
        (
            LinuxL6Checkpoint::RollbackRequired,
            LinuxInstallState::RollbackRequired,
            0,
        ),
        (
            LinuxL6Checkpoint::SourceRestoringBeforeDpkg,
            LinuxInstallState::SourceRestoring,
            0,
        ),
        (
            LinuxL6Checkpoint::SourceAppliedBeforeProof,
            LinuxInstallState::SourceRestoring,
            1,
        ),
    ] {
        let environment = TestEnvironment::new(checkpoint.as_str());
        let old = artifact(&environment, "0.1.0-1", "l6-rollback-old");
        let new = artifact(&environment, "0.2.0-1", "l6-rollback-new");
        let case = OperationCase {
            kind: LinuxOperationKind::Upgrade,
            relation: ArtifactVersionRelation::TargetNewer,
            source: Some(&old),
            target: Some(&new),
        };
        let guard = environment.store.acquire_guard().expect("acquire guard");
        let inner = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
        let mut port = RejectingTargetValidationPort::new(inner, true);
        prepare_operation(
            &environment.store,
            &guard,
            request(&case, OPERATION_ID),
            &mut port,
        )
        .expect("prepare rollback-path upgrade");
        stage_case(&environment, &guard, &case);
        let mut checkpoints = InterruptAt::new(checkpoint);
        let error = resume_operation_with_checkpoints(
            &environment.store,
            &guard,
            &mut port,
            &mut checkpoints,
        )
        .expect_err("rollback checkpoint interrupts the synthetic worker");
        assert!(matches!(error, TransactionError::Checkpoint(_)));
        assert_eq!(port.inner().apply_calls, 1);
        assert_eq!(port.inner().restore_calls, first_restore_calls);
        assert_eq!(
            environment
                .store
                .load_receipt()
                .expect("load receipt")
                .expect("receipt exists")
                .state(),
            expected_state
        );
        let snapshot = port.inner().snapshot.clone();
        drop(guard);

        let guard = environment.store.acquire_guard().expect("reacquire guard");
        let mut fresh_port = FakeDpkg::new(snapshot);
        assert_eq!(
            resume_operation(&environment.store, &guard, &mut fresh_port)
                .expect("resume rollback checkpoint"),
            TransactionOutcome::RolledBack
        );
        assert_eq!(first_restore_calls + fresh_port.restore_calls, 1);
        assert_eq!(fresh_port.snapshot.artifact(), Some(&old.identity));
    }
}

#[test]
fn production_command_rejects_acceptance_runtime_switches() {
    let arguments = vec![
        "resume".to_owned(),
        "--operation-id".to_owned(),
        OPERATION_ID.to_owned(),
        "--authorized-system-mutation".to_owned(),
        "--preserve-user-data".to_owned(),
        "--authorized-l6-crash".to_owned(),
    ];
    assert!(crate::LinuxMaintenanceCommand::parse(&arguments).is_err());
}
