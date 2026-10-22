use super::*;
use crate::preparation_execution::tests::fixture::{confirmation, Fixture, Observer};
use std::fs;
use std::time::Instant;

fn grant(fixture: &Fixture) -> (PreparedContinuationAuthorization, PreparationTaskControl) {
    let install = fixture.install();
    let guard = install.acquire_guard().unwrap();
    let current = install.load_guarded(&guard).unwrap().unwrap();
    drop(guard);
    authorize_prepared_continuation(
        &current,
        &fixture.preparation(),
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap()
}

#[test]
fn admission_keeps_both_guards_and_does_not_advance_or_replace_receipts() {
    let fixture = Fixture::new();
    let receipt = fixture.run(&mut Observer::default()).unwrap();
    let (authorization, control) = grant(&fixture);
    let install = fixture.install();
    let task = bind(authorization, &install).unwrap();
    let identity = outer::active_identity(&install).unwrap();
    let mut seen = Vec::new();
    task.revalidate_with(&mut |point| {
        seen.push(point);
        Ok(())
    })
    .unwrap();
    assert!(seen.contains(&Checkpoint::Handoff(
        SourcePreparationCheckpoint::HandoffReceiptTargetSynced
    )));
    assert!(seen.contains(&Checkpoint::Handoff(
        SourcePreparationCheckpoint::HandoffSnapshotSourceSynced
    )));
    assert!(seen.contains(&Checkpoint::Handoff(
        SourcePreparationCheckpoint::HandoffMarkerSourceSynced
    )));
    assert_eq!(seen.last(), Some(&Checkpoint::Ready));
    assert_eq!(task.install_receipt().state(), InstallState::Prepared);
    assert_eq!(task.data_receipt(), &receipt);
    assert_eq!(outer::active_identity(&install).unwrap(), identity);
    assert!(install.acquire_guard().is_err());
    assert!(task.data.acquire_guard().is_err());
    task.revalidate_with(&mut |_| Ok(())).unwrap();
    control.revoke();
    assert_eq!(
        task.revalidate_with(&mut |_| panic!("revoked task observed")),
        Err(Error::TaskAuthorization)
    );
    drop(task);
    assert!(install.acquire_guard().is_ok());
}

#[test]
fn every_durability_checkpoint_revokes_without_advancing_and_fresh_task_replays() {
    let fixture = Fixture::new();
    fixture.run(&mut Observer::default()).unwrap();
    let install = fixture.install();
    let (authorization, _control) = grant(&fixture);
    let task = bind(authorization, &install).unwrap();
    let mut points = Vec::new();
    task.revalidate_with(&mut |point| {
        if !points.contains(&point) {
            points.push(point);
        }
        Ok(())
    })
    .unwrap();
    drop(task);
    for stop in points {
        let (authorization, control) = grant(&fixture);
        let task = bind(authorization, &install).unwrap();
        assert_eq!(
            task.revalidate_with(&mut |point| {
                if point == stop {
                    control.revoke();
                }
                Ok(())
            }),
            Err(Error::TaskAuthorization),
            "{stop:?}"
        );
        assert_eq!(task.data_receipt().state(), UpgradeState::Preflighted);
        drop(task);
        let (authorization, _fresh_control) = grant(&fixture);
        bind(authorization, &install)
            .unwrap()
            .revalidate_with(&mut |_| Ok(()))
            .unwrap();
    }
}

#[test]
fn admission_rejects_missing_confirmations_expiry_stale_outer_and_foreign_root() {
    let fixture = Fixture::new();
    fixture.run(&mut Observer::default()).unwrap();
    let current = fixture.install().load().unwrap().unwrap();
    let preparation = fixture.preparation();
    for index in 0..4 {
        let mut input = confirmation();
        match index {
            0 => input.explicit_action_confirmed = false,
            1 => input.data_retention_acknowledged = false,
            2 => input.neutral_input_source_selected = false,
            _ => input.manager_closed = false,
        }
        assert!(authorize_prepared_continuation(
            &current,
            &preparation,
            input,
            Duration::from_secs(300)
        )
        .is_err());
    }
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(
            authorize_prepared_continuation(&current, &preparation, confirmation(), duration)
                .is_err()
        );
    }
    let install = fixture.install();
    let (mut authorization, _control) = grant(&fixture);
    authorization.grant.expires = Instant::now();
    assert!(matches!(
        bind(authorization, &install),
        Err(Error::TaskAuthorization)
    ));
    let foreign = Fixture::new();
    let (authorization, _control) = grant(&fixture);
    let foreign_install = foreign.install();
    let foreign_guard = foreign_install.acquire_guard().unwrap();
    assert!(
        matches!(
            bind(authorization, &foreign_install),
            Err(Error::InvalidBinding)
        ),
        "wrong-root authorization must fail before trying its occupied guard"
    );
    drop(foreign_guard);
    let (authorization, _control) = grant(&fixture);
    let guard = install.acquire_guard().unwrap();
    let mut advanced = current;
    advanced.advance(InstallState::Quiesced).unwrap();
    install.persist(&guard, &advanced).unwrap();
    drop(guard);
    assert!(bind(authorization, &install).is_err());
    assert!(authorize_prepared_continuation(
        &advanced,
        &preparation,
        confirmation(),
        Duration::from_secs(300)
    )
    .is_err());
}

#[test]
fn callback_identity_drift_and_native_failure_stop_admission() {
    for which in 0..4 {
        let fixture = Fixture::new();
        fixture.run(&mut Observer::default()).unwrap();
        let (authorization, _control) = grant(&fixture);
        let install = fixture.install();
        let task = bind(authorization, &install).unwrap();
        let result = task.revalidate_with(&mut |point| {
            if point != Checkpoint::Admission {
                return Ok(());
            }
            if which == 3 {
                return Err(Error::NativeObservation);
            }
            let path = match which {
                0 => fixture.path.join(".radishlex-install-v1/receipt.json"),
                1 => fixture
                    .path
                    .join(".radishlex-upgrade-history-v1")
                    .join(task.receipt.operation_id())
                    .join("preparation.json"),
                _ => fixture.path.join("userdb.sqlite3"),
            };
            let bytes = fs::read(&path).unwrap();
            let retained = fixture.path.join("retained-synthetic-original");
            fs::rename(&path, retained).unwrap();
            fs::write(&path, bytes).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            Ok(())
        });
        assert!(result.is_err(), "drift {which}");
    }
}

const EXIT_POINTS: [Checkpoint; 5] = [
    Checkpoint::OuterHistory,
    Checkpoint::OuterSynced,
    Checkpoint::Handoff(SourcePreparationCheckpoint::HandoffReceiptTargetSynced),
    Checkpoint::Handoff(SourcePreparationCheckpoint::HandoffMarkerSourceSynced),
    Checkpoint::Ready,
];

#[test]
fn admission_process_exit_requires_fresh_grant_and_preserves_replay() {
    for (index, point) in EXIT_POINTS.iter().enumerate() {
        let fixture = Fixture::new();
        fixture.run(&mut Observer::default()).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "preparation_continuation::tests::continuation_exit_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RADISHLEX_CONTINUATION_TEST_ROOT", &fixture.path)
            .env("RADISHLEX_CONTINUATION_TEST_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(81),
            "{point:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let (authorization, _control) = grant(&fixture);
        let install = fixture.install();
        let task = bind(authorization, &install).unwrap();
        task.revalidate_with(&mut |_| Ok(())).unwrap();
        assert_eq!(task.install_receipt().state(), InstallState::Prepared);
        assert_eq!(task.data_receipt().state(), UpgradeState::Preflighted);
    }
}

#[test]
#[ignore = "invoked by the continuation process-exit matrix"]
fn continuation_exit_child() {
    let fixture = Fixture::reopen(
        std::env::var_os("RADISHLEX_CONTINUATION_TEST_ROOT")
            .unwrap()
            .into(),
    );
    let index: usize = std::env::var("RADISHLEX_CONTINUATION_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let (authorization, _control) = grant(&fixture);
    let install = fixture.install();
    bind(authorization, &install)
        .unwrap()
        .revalidate_with(&mut |point| {
            if point == EXIT_POINTS[index] {
                std::process::exit(81);
            }
            Ok(())
        })
        .unwrap();
    panic!("exit checkpoint not reached");
}

#[test]
fn dropped_control_and_both_lock_conflicts_fail_before_observation() {
    let fixture = Fixture::new();
    fixture.run(&mut Observer::default()).unwrap();
    let install = fixture.install();
    let (authorization, control) = grant(&fixture);
    drop(control);
    assert!(matches!(
        bind(authorization, &install),
        Err(Error::TaskAuthorization)
    ));
    let (authorization, _control) = grant(&fixture);
    let outer_guard = install.acquire_guard().unwrap();
    assert!(bind(authorization, &install).is_err());
    drop(outer_guard);
    let (authorization, _control) = grant(&fixture);
    let data = UpgradeReceiptStore::open_existing(
        VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
    )
    .unwrap();
    let inner_guard = data.acquire_guard().unwrap();
    assert!(bind(authorization, &install).is_err());
    assert!(
        install.acquire_guard().is_ok(),
        "failed inner admission must release outer guard"
    );
    drop(inner_guard);
}
