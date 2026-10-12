//! Revocation, recovery and retained unknown-material matrices.
use super::*;

#[test]
fn failed_data_restarts_before_each_program_restore_and_terminal_release() {
    let points = [
        Point::Restore(ProgramComponent::Manager),
        Point::Restore(ProgramComponent::InputMethod),
        Point::Switch(
            ProgramComponent::Manager,
            ProgramSwitchFaultPoint::new(
                ProgramSwitchAction::ReturnTargetToStage,
                ProgramSwitchBoundary::AfterRename,
            ),
        ),
        Point::Switch(
            ProgramComponent::InputMethod,
            ProgramSwitchFaultPoint::new(
                ProgramSwitchAction::RestoreSource,
                ProgramSwitchBoundary::AfterRename,
            ),
        ),
        Point::Programs(InstallState::ProgramsRestored),
        Point::BeforeOuterTerminal,
        Point::Release(TerminalReleaseCheckpoint::BeforeMarkerMove),
    ];
    for outcome in [Outcome::Aborted, Outcome::RolledBack] {
        for stop in points {
            let (f, mut p, prep) = seed();
            let install = f.install();
            assert!(
                run_again(
                    &f,
                    &prep,
                    &install,
                    &mut p,
                    &mut Native {
                        stop: Some(stop),
                        ..Native::default()
                    },
                    outcome
                )
                .is_err(),
                "{outcome:?}: {stop:?}"
            );
            let s = run_again(&f, &prep, &install, &mut p, &mut Native::default(), outcome)
                .unwrap_or_else(|e| panic!("{outcome:?}: {stop:?}: {e}"));
            assert_released(&f, &s, outcome);
        }
    }
}

#[test]
fn every_observed_mutation_boundary_checks_revocation_and_retains_the_original_proof() {
    for outcome in [Outcome::Completed, Outcome::RolledBack] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let mut native = Native::default();
        run_again(&f, &prep, &install, &mut p, &mut native, outcome).unwrap();
        let mut points = Vec::new();
        for point in native.seen {
            if !points.contains(&point) {
                points.push(point);
            }
        }
        for stop in points {
            let (f, mut p, prep) = seed();
            let install = f.install();
            let proof_path = f
                .path
                .join(".radishlex-upgrade-history-v1")
                .join(NEW)
                .join("preparation.json");
            let original = (
                fs::metadata(&proof_path).unwrap().ino(),
                fs::read(&proof_path).unwrap(),
            );
            let (task, control) = initial(&f, &prep, &install);
            let mut native = Native {
                mutation: Some((stop, Box::new(move || control.revoke()))),
                ..Native::default()
            };
            assert_eq!(
                execute(task, &mut p, &mut native, &mut DataPort(outcome)),
                Err(Error::TaskAuthorization),
                "{outcome:?}: {stop:?}"
            );
            assert_eq!(
                (
                    fs::metadata(&proof_path).unwrap().ino(),
                    fs::read(&proof_path).unwrap()
                ),
                original
            );
            assert!(
                install.acquire_guard().is_ok(),
                "outer lock released after failure"
            );
        }
    }
}

#[test]
fn progressed_grant_rejects_stale_receipts_expiry_wrong_root_and_both_lock_conflicts() {
    let (f, mut p, prep) = seed();
    let install = f.install();
    assert!(run_again(
        &f,
        &prep,
        &install,
        &mut p,
        &mut Native {
            stop: Some(Point::Programs(InstallState::TargetStaged)),
            ..Native::default()
        },
        Outcome::Completed
    )
    .is_err());
    let current = install.load_for_recovery_inspection().unwrap().unwrap();
    let data = {
        let g = install.acquire_guard().unwrap();
        let (_, _, r) = crate::rebind_prepared_upgrade(&install, &g, &prep).unwrap();
        PreparedUpgradeData::Active(Box::new(r))
    };
    for flag in 0..4 {
        let mut c = confirmation();
        match flag {
            0 => c.explicit_action_confirmed = false,
            1 => c.data_retention_acknowledged = false,
            2 => c.neutral_input_source_selected = false,
            _ => c.manager_closed = false,
        }
        assert!(authorize_prepared_upgrade_resume(
            &current,
            &prep,
            data.clone(),
            c,
            Duration::from_secs(300)
        )
        .is_err());
    }
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(authorize_prepared_upgrade_resume(
            &current,
            &prep,
            data.clone(),
            confirmation(),
            duration
        )
        .is_err());
    }
    let fresh = || {
        authorize_prepared_upgrade_resume(
            &current,
            &prep,
            data.clone(),
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap()
    };
    let (mut auth, _c) = fresh();
    auth.grant.expires = std::time::Instant::now();
    assert!(matches!(
        bind_resume(auth, &install),
        Err(Error::TaskAuthorization)
    ));
    let (auth, c) = fresh();
    drop(c);
    assert!(matches!(
        bind_resume(auth, &install),
        Err(Error::TaskAuthorization)
    ));
    let other = Fixture::new();
    let foreign = other.install();
    let _foreign_guard = foreign.acquire_guard().unwrap();
    let (auth, _c) = fresh();
    assert!(matches!(
        bind_resume(auth, &foreign),
        Err(Error::InvalidBinding)
    ));
    let guard = install.acquire_guard().unwrap();
    let (auth, _c) = fresh();
    assert!(bind_resume(auth, &install).is_err());
    drop(guard);
    let d = UpgradeReceiptStore::open_existing(root(&install).unwrap()).unwrap();
    let guard = d.acquire_guard().unwrap();
    let (auth, _c) = fresh();
    assert!(bind_resume(auth, &install).is_err());
    drop(guard);
    let (auth, _c) = fresh();
    run_again(
        &f,
        &prep,
        &install,
        &mut p,
        &mut Native {
            stop: Some(Point::Programs(InstallState::ProgramsCommitted)),
            ..Native::default()
        },
        Outcome::Completed,
    )
    .unwrap_err();
    assert!(matches!(
        bind_resume(auth, &install),
        Err(Error::InvalidBinding)
    ));
}

#[test]
fn unbound_release_record_creation_and_unknown_data_objects_do_not_get_adopted() {
    for stop in [
        Point::Release(TerminalReleaseCheckpoint::DirectoryCreated(
            TerminalReleaseDirectory::Data,
        )),
        Point::Release(TerminalReleaseCheckpoint::RecordCreated),
    ] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        assert!(run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native {
                stop: Some(stop),
                ..Native::default()
            },
            Outcome::Completed
        )
        .is_err());
        let before = fs::read(f.path.join(".radishlex-upgrade-v1/receipt.json")).unwrap();
        assert!(run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native::default(),
            Outcome::Completed
        )
        .is_err());
        assert_eq!(
            fs::read(f.path.join(".radishlex-upgrade-v1/receipt.json")).unwrap(),
            before
        );
    }
    let (f, mut p, prep) = seed();
    let install = f.install();
    fs::write(
        f.path.join(".radishlex-upgrade-v1/unknown.tmp"),
        "synthetic-unbound",
    )
    .unwrap();
    assert!(run_again(
        &f,
        &prep,
        &install,
        &mut p,
        &mut Native::default(),
        Outcome::Completed
    )
    .is_err());
    assert!(f.path.join(".radishlex-upgrade-v1/unknown.tmp").exists());
}
