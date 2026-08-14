use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use super::helper::{artifact, request, stage_case, FakeDpkg, OperationCase, TestEnvironment};
use crate::coordinator::{DpkgPortErrorCode, DpkgPortPhase, DpkgQuiescencePhase};
use crate::{
    prepare_operation, resume_operation, ArtifactSlot, ArtifactVersionRelation, DpkgPackageState,
    DpkgPortError, LinuxFailureCode, LinuxInstallState, LinuxInstallStoreErrorCode,
    LinuxOperationKind, PackageSnapshot, TransactionOutcome,
};

#[test]
fn five_operations_complete_without_touching_user_data() {
    for (index, kind) in [
        LinuxOperationKind::Install,
        LinuxOperationKind::Upgrade,
        LinuxOperationKind::Repair,
        LinuxOperationKind::Remove,
        LinuxOperationKind::Rollback,
    ]
    .into_iter()
    .enumerate()
    {
        let environment = TestEnvironment::new(&format!("five-{index}"));
        let old = artifact(&environment, "0.1.0-1", "old");
        let new = artifact(&environment, "0.2.0-1", "new");
        let case = match kind {
            LinuxOperationKind::Install => OperationCase {
                kind,
                relation: ArtifactVersionRelation::NotApplicable,
                source: None,
                target: Some(&new),
            },
            LinuxOperationKind::Upgrade => OperationCase {
                kind,
                relation: ArtifactVersionRelation::TargetNewer,
                source: Some(&old),
                target: Some(&new),
            },
            LinuxOperationKind::Repair => OperationCase {
                kind,
                relation: ArtifactVersionRelation::SameRelease,
                source: Some(&new),
                target: Some(&new),
            },
            LinuxOperationKind::Remove => OperationCase {
                kind,
                relation: ArtifactVersionRelation::NotApplicable,
                source: Some(&new),
                target: None,
            },
            LinuxOperationKind::Rollback => OperationCase {
                kind,
                relation: ArtifactVersionRelation::TargetOlder,
                source: Some(&new),
                target: Some(&old),
            },
        };
        let initial = case.source.map_or_else(PackageSnapshot::absent, |source| {
            PackageSnapshot::exact_installed(source.identity.clone())
        });
        let mut port = FakeDpkg::new(initial);
        if kind == LinuxOperationKind::Repair {
            port.target_valid = false;
        }
        let guard = environment.store.acquire_guard().expect("acquire guard");
        prepare_operation(
            &environment.store,
            &guard,
            request(&case, &format!("{index:032x}")),
            &mut port,
        )
        .expect("prepare operation");
        assert!(environment.state_root.join("receipt.json").is_file());
        stage_case(&environment, &guard, &case);
        assert_eq!(
            resume_operation(&environment.store, &guard, &mut port).expect("complete transaction"),
            TransactionOutcome::Completed
        );
        let receipt = environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists");
        assert_eq!(receipt.state(), LinuxInstallState::Completed);
        assert_eq!(port.user_data_touches, 0);
        assert_eq!(
            port.remove_calls,
            usize::from(kind == LinuxOperationKind::Remove)
        );
        assert_eq!(
            port.apply_calls,
            usize::from(kind != LinuxOperationKind::Remove)
        );
        assert!(port.issued_permits.is_empty());
        assert_eq!(port.consumed_permits.len(), 1);
        assert_eq!(
            port.quiescence_phases,
            [DpkgQuiescencePhase::TargetMutation]
        );
    }
}

#[test]
fn terminal_receipt_chain_appends_across_install_and_upgrade() {
    let environment = TestEnvironment::new("receipt-chain");
    let old = artifact(&environment, "0.1.0-1", "old");
    let new = artifact(&environment, "0.2.0-1", "new");
    let install = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&old),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&install, "11111111111111111111111111111111"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &install);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("complete install"),
        TransactionOutcome::Completed
    );

    let upgrade = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    prepare_operation(
        &environment.store,
        &guard,
        request(&upgrade, "22222222222222222222222222222222"),
        &mut port,
    )
    .expect("append upgrade");
    stage_case(&environment, &guard, &upgrade);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("complete upgrade"),
        TransactionOutcome::Completed
    );

    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(
        receipt.operation_chain(),
        &[
            "11111111111111111111111111111111".to_owned(),
            "22222222222222222222222222222222".to_owned(),
        ]
    );
    assert!(environment
        .state_root
        .join("operations/11111111111111111111111111111111")
        .is_dir());
    assert!(environment
        .state_root
        .join("operations/22222222222222222222222222222222")
        .is_dir());
    assert_eq!(port.user_data_touches, 0);
}

#[test]
fn resume_with_fresh_port_uses_only_receipt_operation_context() {
    let environment = TestEnvironment::new("fresh-port");
    let target = artifact(&environment, "0.2.0-1", "fresh-target");
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
        request(&case, "01010101010101010101010101010101"),
        &mut preparing_port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);

    let mut fresh_port = FakeDpkg::new(PackageSnapshot::absent());
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut fresh_port)
            .expect("fresh port resumes from receipt context"),
        TransactionOutcome::Completed
    );
    assert_eq!(fresh_port.apply_calls, 1);
    assert_eq!(fresh_port.consumed_permits.len(), 1);
    assert!(fresh_port.issued_permits.is_empty());
}

#[test]
fn initial_staged_relationship_failures_abort_before_quiescence_or_mutation() {
    for (index, code, expected_failure) in [
        (
            6,
            DpkgPortErrorCode::DependencyUnavailable,
            LinuxFailureCode::DependencyUnavailable,
        ),
        (
            7,
            DpkgPortErrorCode::VersionRelationInvalid,
            LinuxFailureCode::VersionRelationInvalid,
        ),
    ] {
        let environment = TestEnvironment::new(&format!("staged-preflight-{index}"));
        let target = artifact(
            &environment,
            "0.2.0-1",
            &format!("preflight-target-{index}"),
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
            request(&case, &format!("{index:032x}")),
            &mut port,
        )
        .expect("prepare install");
        stage_case(&environment, &guard, &case);
        port.staged_validation_error_on_call = Some((1, code));

        assert_eq!(
            resume_operation(&environment.store, &guard, &mut port)
                .expect("staged relationship failure aborts safely"),
            TransactionOutcome::AbortedPreserved
        );
        let receipt = environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists");
        assert_eq!(receipt.failure_code(), Some(expected_failure));
        assert_eq!(
            port.staged_validation_states,
            [LinuxInstallState::ArtifactsStaged]
        );
        assert!(port.quiescence_phases.is_empty());
        assert!(port.issued_permits.is_empty());
        assert_eq!(port.apply_calls, 0);
        assert_eq!(port.remove_calls, 0);
        assert_eq!(port.restore_calls, 0);
    }
}

#[test]
fn running_programs_abort_before_package_mutation() {
    let environment = TestEnvironment::new("quiescence");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    port.quiescence_error = Some(DpkgPortErrorCode::ProgramsRunning);
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("abort safely"),
        TransactionOutcome::AbortedPreserved
    );
    assert_eq!(port.apply_calls, 0);
    assert_eq!(port.remove_calls, 0);
}

#[test]
fn quiescence_inspection_failure_is_not_misreported_as_programs_running() {
    let environment = TestEnvironment::new("quiescence-inspection");
    let target = artifact(&environment, "0.2.0-1", "inspection-target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    port.quiescence_error = Some(DpkgPortErrorCode::ProcessInspectionUnavailable);
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "05050505050505050505050505050505"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("abort before mutation"),
        TransactionOutcome::AbortedPreserved
    );
    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(
        receipt.failure_code(),
        Some(LinuxFailureCode::ProcessInspectionUnavailable)
    );
    assert_ne!(
        receipt.failure_code(),
        Some(LinuxFailureCode::ProgramsRunning)
    );
}

#[test]
fn typed_port_errors_map_to_exact_stable_failure_codes() {
    let direct_cases = [
        (
            DpkgPortErrorCode::ArtifactInvalid,
            LinuxFailureCode::ArtifactInvalid,
        ),
        (
            DpkgPortErrorCode::EnvironmentUnsupported,
            LinuxFailureCode::EnvironmentUnsupported,
        ),
        (
            DpkgPortErrorCode::DependencyUnavailable,
            LinuxFailureCode::DependencyUnavailable,
        ),
        (
            DpkgPortErrorCode::VersionRelationInvalid,
            LinuxFailureCode::VersionRelationInvalid,
        ),
        (
            DpkgPortErrorCode::ProgramsRunning,
            LinuxFailureCode::ProgramsRunning,
        ),
        (
            DpkgPortErrorCode::ProcessInspectionUnavailable,
            LinuxFailureCode::ProcessInspectionUnavailable,
        ),
        (
            DpkgPortErrorCode::PackageStateUnavailable,
            LinuxFailureCode::PackageStateUnavailable,
        ),
        (
            DpkgPortErrorCode::PackageStateUnknown,
            LinuxFailureCode::PackageStateUnknown,
        ),
        (
            DpkgPortErrorCode::PackageStateUnexpected,
            LinuxFailureCode::PackageStateUnexpected,
        ),
        (
            DpkgPortErrorCode::CommandInvalid,
            LinuxFailureCode::CommandInvalid,
        ),
        (
            DpkgPortErrorCode::PermissionDenied,
            LinuxFailureCode::PermissionDenied,
        ),
        (DpkgPortErrorCode::Io, LinuxFailureCode::Io),
    ];
    for (code, expected) in direct_cases {
        let error = DpkgPortError::new(code, DpkgPortPhase::Quiescence, "synthetic failure");
        assert_eq!(error.failure_code(), expected);
    }

    let phased_cases = [
        (
            DpkgPortErrorCode::MutationFailed,
            DpkgPortPhase::TargetMutation,
            LinuxFailureCode::PackageMutationFailed,
        ),
        (
            DpkgPortErrorCode::MutationFailed,
            DpkgPortPhase::SourceRestore,
            LinuxFailureCode::SourceRestoreFailed,
        ),
        (
            DpkgPortErrorCode::ProductValidationFailed,
            DpkgPortPhase::TargetValidation,
            LinuxFailureCode::TargetValidationFailed,
        ),
        (
            DpkgPortErrorCode::ProductValidationFailed,
            DpkgPortPhase::SourceValidation,
            LinuxFailureCode::SourceValidationFailed,
        ),
    ];
    for (code, phase, expected) in phased_cases {
        let error = DpkgPortError::new(code, phase, "synthetic phased failure");
        assert_eq!(error.failure_code(), expected);
    }
}

#[test]
fn unknown_package_state_refuses_to_create_receipt() {
    let environment = TestEnvironment::new("unknown-state");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(
        PackageSnapshot::new(DpkgPackageState::Unknown, None).expect("valid unknown snapshot"),
    );
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let error = prepare_operation(
        &environment.store,
        &guard,
        request(&case, "cccccccccccccccccccccccccccccccc"),
        &mut port,
    )
    .expect_err("unknown dpkg state must fail closed");
    assert_eq!(
        error.failure_code(),
        Some(crate::LinuxFailureCode::PackageStateUnknown)
    );
    assert!(environment
        .store
        .load_receipt()
        .expect("load receipt")
        .is_none());
}

#[test]
fn guard_is_exclusive_and_released_by_exact_inode() {
    let environment = TestEnvironment::new("guard");
    let first = environment
        .store
        .acquire_guard()
        .expect("acquire first guard");
    std::thread::scope(|scope| {
        let attempts: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| environment.store.acquire_guard()))
            .collect();
        for attempt in attempts {
            let error = attempt
                .join()
                .expect("guard contender must not panic")
                .expect_err("concurrent guard must be rejected");
            assert_eq!(error.code(), LinuxInstallStoreErrorCode::GuardActive);
        }
    });
    environment
        .store
        .verify_guard(&first)
        .expect("active guard remains valid after contention");
    assert!(environment.guard_path.is_file());
    drop(first);
    assert!(!environment.guard_path.exists());

    let stale = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&environment.guard_path)
        .expect("create stale guard file");
    stale.sync_all().expect("sync stale guard file");
    stale
        .set_permissions(fs::Permissions::from_mode(0o000))
        .expect("simulate crash before guard mode commit");
    drop(stale);
    let recovered = environment
        .store
        .acquire_guard()
        .expect("crash-stale guard file can be relocked");
    assert!(environment.guard_path.is_file());
    drop(recovered);
    assert!(!environment.guard_path.exists());
}

#[test]
fn terminal_current_operation_rejects_missing_or_nonrequired_slots() {
    for (label, operation_id) in [
        ("missing-required", "abababababababababababababababab"),
        ("extra-nonrequired", "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"),
    ] {
        let environment = TestEnvironment::new(label);
        let target = artifact(&environment, "0.2.0-1", label);
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
            request(&case, operation_id),
            &mut port,
        )
        .expect("prepare install");
        stage_case(&environment, &guard, &case);
        assert_eq!(
            resume_operation(&environment.store, &guard, &mut port)
                .expect("complete synthetic install"),
            TransactionOutcome::Completed
        );

        let operation_root = environment.state_root.join("operations").join(operation_id);
        if label == "missing-required" {
            fs::remove_file(operation_root.join("target.deb"))
                .expect("remove required staged package");
        } else {
            fs::copy(
                operation_root.join("target.deb"),
                operation_root.join("source.deb"),
            )
            .expect("add non-required package");
            fs::copy(
                operation_root.join("target.evidence.json"),
                operation_root.join("source.evidence.json"),
            )
            .expect("add non-required evidence");
        }
        let error = environment
            .store
            .load_receipt()
            .expect_err("terminal slot drift must fail closed");
        assert_eq!(error.code(), LinuxInstallStoreErrorCode::ArtifactInvalid);
    }
}

#[test]
fn staged_artifact_tamper_and_hardlinks_fail_closed() {
    let environment = TestEnvironment::new("artifact-fail-closed");
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
        request(&case, "dddddddddddddddddddddddddddddddd"),
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
        .expect("stage target");
    let staged = environment
        .store
        .staged_artifact_paths("dddddddddddddddddddddddddddddddd", ArtifactSlot::Target)
        .expect("resolve staged target")
        .package_path()
        .to_owned();
    OpenOptions::new()
        .append(true)
        .open(&staged)
        .and_then(|mut file| file.write_all(b"tamper"))
        .expect("tamper staged file");
    let error = environment
        .store
        .finish_staging(&guard)
        .expect_err("tampered staged artifact must fail closed");
    assert_eq!(error.code(), LinuxInstallStoreErrorCode::ArtifactInvalid);

    let second_environment = TestEnvironment::new("hardlink");
    let linked = artifact(&second_environment, "0.2.0-1", "linked");
    let alias_directory = second_environment.artifacts_root.join("alias");
    fs::create_dir(&alias_directory).expect("create alias directory");
    let alias = alias_directory.join(linked.identity.package_filename());
    fs::hard_link(&linked.package_path, &alias).expect("create hardlink");
    let second_case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&linked),
    };
    let mut second_port = FakeDpkg::new(PackageSnapshot::absent());
    let second_guard = second_environment
        .store
        .acquire_guard()
        .expect("acquire second guard");
    prepare_operation(
        &second_environment.store,
        &second_guard,
        request(&second_case, "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        &mut second_port,
    )
    .expect("prepare second install");
    let hardlink_error = second_environment
        .store
        .stage_artifact(
            &second_guard,
            ArtifactSlot::Target,
            &linked.package_path,
            &linked.evidence_path,
        )
        .expect_err("hardlinked source must fail closed");
    assert_eq!(
        hardlink_error.code(),
        LinuxInstallStoreErrorCode::ArtifactInvalid
    );
}
