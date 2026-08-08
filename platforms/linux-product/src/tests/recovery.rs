use super::helper::{artifact, request, stage_case, FakeDpkg, OperationCase, TestEnvironment};
use crate::coordinator::{DpkgPortErrorCode, DpkgQuiescencePhase};
use crate::{
    prepare_operation, resume_operation, ArtifactVersionRelation, DpkgPackageState,
    LinuxFailureCode, LinuxInstallState, LinuxOperationKind, PackageSnapshot, TransactionError,
    TransactionOutcome,
};

#[test]
fn interrupted_mutation_retries_source_restore_and_rolls_back() {
    let environment = TestEnvironment::new("retry-rollback");
    let old = artifact(&environment, "0.1.0-1", "old");
    let new = artifact(&environment, "0.2.0-1", "new");
    let case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
    port.fail_apply_once = true;
    port.fail_restore_once = true;
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        &mut port,
    )
    .expect("prepare upgrade");
    stage_case(&environment, &guard, &case);

    let error = resume_operation(&environment.store, &guard, &mut port)
        .expect_err("first restore attempt must remain blocked");
    assert!(matches!(error, TransactionError::RecoveryBlocked { .. }));
    assert_eq!(
        environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists")
            .state(),
        LinuxInstallState::SourceRestoring
    );
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("retry source restore"),
        TransactionOutcome::RolledBack
    );
    assert_eq!(port.apply_calls, 1);
    assert_eq!(port.restore_calls, 2);
    assert!(port.issued_permits.is_empty());
    assert_eq!(port.consumed_permits.len(), 3);
    assert_eq!(
        port.quiescence_phases,
        [
            DpkgQuiescencePhase::TargetMutation,
            DpkgQuiescencePhase::SourceRestore,
            DpkgQuiescencePhase::SourceRestore,
        ]
    );
    assert_eq!(port.user_data_touches, 0);
}

#[test]
fn quiesced_crash_retry_reproves_and_consumes_a_new_permit() {
    let environment = TestEnvironment::new("quiesced-reproof");
    let target = artifact(&environment, "0.2.0-1", "quiesced-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "02020202020202020202020202020202"),
        &mut preparing_port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("simulate persisted quiescence before crash");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiesced receipt");

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("resume quiesced operation"),
        TransactionOutcome::Completed
    );
    assert_eq!(
        fresh_port.quiescence_phases,
        [DpkgQuiescencePhase::TargetMutation]
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [
            LinuxInstallState::Quiesced,
            LinuxInstallState::PackageMutating,
        ]
    );
    assert_eq!(fresh_port.consumed_permits.len(), 1);
    assert!(fresh_port.issued_permits.is_empty());
}

#[test]
fn quiesced_crash_staged_failure_aborts_before_reproving_quiescence() {
    let environment = TestEnvironment::new("quiesced-staged-failure");
    let target = artifact(&environment, "0.2.0-1", "quiesced-failure-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "08080808080808080808080808080808"),
        &mut preparing_port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("simulate persisted quiescence before crash");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiesced receipt");

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    fresh_port.staged_validation_error_on_call =
        Some((1, DpkgPortErrorCode::DependencyUnavailable));
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("fresh staged failure aborts preserved"),
        TransactionOutcome::AbortedPreserved
    );
    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(
        receipt.failure_code(),
        Some(LinuxFailureCode::DependencyUnavailable)
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [LinuxInstallState::Quiesced]
    );
    assert!(fresh_port.quiescence_phases.is_empty());
    assert!(fresh_port.issued_permits.is_empty());
    assert_eq!(fresh_port.apply_calls, 0);
    assert_eq!(fresh_port.remove_calls, 0);
    assert_eq!(fresh_port.restore_calls, 0);
}

#[test]
fn source_restoring_crash_retry_reproves_with_a_fresh_port() {
    let environment = TestEnvironment::new("source-reproof");
    let old = artifact(&environment, "0.1.0-1", "source-old");
    let new = artifact(&environment, "0.2.0-1", "source-new");
    let case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut first_port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
    first_port.fail_apply_once = true;
    first_port.fail_restore_once = true;
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "03030303030303030303030303030303"),
        &mut first_port,
    )
    .expect("prepare upgrade");
    stage_case(&environment, &guard, &case);
    resume_operation(&environment.store, &guard, &mut first_port)
        .expect_err("first source restore is interrupted");
    assert_eq!(
        environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists")
            .state(),
        LinuxInstallState::SourceRestoring
    );

    let mut fresh_port = FakeDpkg::new(first_port.snapshot.clone());
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("fresh port restores source"),
        TransactionOutcome::RolledBack
    );
    assert_eq!(
        fresh_port.quiescence_phases,
        [DpkgQuiescencePhase::SourceRestore]
    );
    assert_eq!(fresh_port.restore_calls, 1);
    assert_eq!(fresh_port.consumed_permits.len(), 1);
    assert!(fresh_port.issued_permits.is_empty());
}

#[test]
fn package_mutating_retry_revalidates_staged_relationship_before_new_mutation() {
    let environment = TestEnvironment::new("package-mutating-staged-reproof");
    let target = artifact(&environment, "0.2.0-1", "package-mutating-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "09090909090909090909090909090909"),
        &mut preparing_port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record pre-crash quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist pre-crash quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("record pre-crash mutation state");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation state");

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    fresh_port.staged_validation_error_on_call =
        Some((1, DpkgPortErrorCode::DependencyUnavailable));
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("failed target preflight restores the unchanged source"),
        TransactionOutcome::RolledBack
    );
    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(receipt.state(), LinuxInstallState::RolledBack);
    assert_eq!(
        receipt.failure_code(),
        Some(LinuxFailureCode::DependencyUnavailable)
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [
            LinuxInstallState::PackageMutating,
            LinuxInstallState::SourceRestoring,
        ]
    );
    assert!(fresh_port.quiescence_phases.is_empty());
    assert_eq!(fresh_port.apply_calls, 0);
    assert_eq!(fresh_port.remove_calls, 0);
    assert_eq!(fresh_port.restore_calls, 0);
}

#[test]
fn package_mutating_retry_revalidates_staging_even_when_target_is_already_installed() {
    let environment = TestEnvironment::new("package-mutating-installed-target-reproof");
    let target = artifact(&environment, "0.2.0-1", "installed-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::absent());
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b"),
        &mut preparing_port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("record package mutation");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation");

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::exact_installed(target.identity));
    fresh_port.staged_validation_error_on_call =
        Some((1, DpkgPortErrorCode::DependencyUnavailable));
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("invalid staging forces source recovery"),
        TransactionOutcome::RolledBack
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [
            LinuxInstallState::PackageMutating,
            LinuxInstallState::SourceRestoring,
        ]
    );
    assert_eq!(fresh_port.apply_calls, 0);
    assert_eq!(fresh_port.restore_calls, 1);
}

#[test]
fn source_restoring_retry_revalidates_staged_relationship_before_restore() {
    let environment = TestEnvironment::new("source-restoring-staged-reproof");
    let old = artifact(&environment, "0.1.0-1", "source-reproof-old");
    let new = artifact(&environment, "0.2.0-1", "source-reproof-new");
    let case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"),
        &mut preparing_port,
    )
    .expect("prepare upgrade");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("record package mutation");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation");
    receipt
        .require_rollback(LinuxFailureCode::PackageMutationFailed)
        .expect("require rollback");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist rollback requirement");
    receipt
        .advance(LinuxInstallState::SourceRestoring)
        .expect("enter source restore");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist source restore state");

    let partial_target =
        PackageSnapshot::new(DpkgPackageState::HalfConfigured, Some(new.identity.clone()))
            .expect("valid partial target");
    let mut fresh_port = FakeDpkg::new(partial_target);
    fresh_port.staged_validation_error_on_call =
        Some((1, DpkgPortErrorCode::VersionRelationInvalid));
    let error = resume_operation(&environment.store, &guard, &mut fresh_port)
        .expect_err("source relationship failure blocks recovery");
    assert_eq!(
        error.failure_code(),
        Some(LinuxFailureCode::VersionRelationInvalid)
    );
    assert!(matches!(error, TransactionError::RecoveryBlocked { .. }));
    assert_eq!(
        environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists")
            .state(),
        LinuxInstallState::SourceRestoring
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [LinuxInstallState::SourceRestoring]
    );
    assert!(fresh_port.quiescence_phases.is_empty());
    assert!(fresh_port.issued_permits.is_empty());
    assert_eq!(fresh_port.restore_calls, 0);
}

#[test]
fn source_restoring_retry_revalidates_staging_even_when_source_is_already_restored() {
    let environment = TestEnvironment::new("source-restoring-restored-source-reproof");
    let old = artifact(&environment, "0.1.0-1", "already-restored-old");
    let new = artifact(&environment, "0.2.0-1", "already-restored-new");
    let case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut preparing_port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c"),
        &mut preparing_port,
    )
    .expect("prepare upgrade");
    stage_case(&environment, &guard, &case);
    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("record package mutation");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation");
    receipt
        .require_rollback(LinuxFailureCode::PackageMutationFailed)
        .expect("require rollback");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist rollback requirement");
    receipt
        .advance(LinuxInstallState::SourceRestoring)
        .expect("enter source restore");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist source restore state");

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity));
    fresh_port.staged_validation_error_on_call =
        Some((1, DpkgPortErrorCode::VersionRelationInvalid));
    let error = resume_operation(&environment.store, &guard, &mut fresh_port)
        .expect_err("invalid staging blocks an otherwise restored source");
    assert!(matches!(error, TransactionError::RecoveryBlocked { .. }));
    assert_eq!(
        error.failure_code(),
        Some(LinuxFailureCode::VersionRelationInvalid)
    );
    assert_eq!(
        fresh_port.staged_validation_states,
        [LinuxInstallState::SourceRestoring]
    );
    assert_eq!(fresh_port.restore_calls, 0);
}

#[test]
fn first_install_failure_provides_target_as_recovery_material() {
    let environment = TestEnvironment::new("install-recovery-target");
    let target = artifact(&environment, "0.2.0-1", "install-recovery");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    port.fail_apply_once = true;
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "04040404040404040404040404040404"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);

    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port)
            .expect("failed first install restores absence"),
        TransactionOutcome::RolledBack
    );
    assert_eq!(port.first_install_recovery_target, Some(target.identity));
    assert_eq!(port.restore_calls, 1);
    assert_eq!(
        port.quiescence_phases,
        [
            DpkgQuiescencePhase::TargetMutation,
            DpkgQuiescencePhase::SourceRestore,
        ]
    );
}

#[test]
fn retry_after_target_proof_does_not_repeat_package_mutation() {
    let environment = TestEnvironment::new("target-proof-retry");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "abababababababababababababababab"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    environment
        .store
        .finish_staging(&guard)
        .expect("finishing staging is idempotent");

    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("enter package mutation");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation");

    let target_snapshot = PackageSnapshot::exact_installed(target.identity.clone());
    port.snapshot = target_snapshot.clone();
    receipt
        .record_target_proof(target_snapshot)
        .expect("record target proof before simulated crash");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist target proof before simulated crash");

    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port)
            .expect("resume from persisted target proof"),
        TransactionOutcome::Completed
    );
    assert_eq!(port.apply_calls, 0);
    assert_eq!(port.remove_calls, 0);
}
