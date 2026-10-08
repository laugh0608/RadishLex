use super::*;
use radishlex_ime_product_upgrade::{inspect_startup_gate, UpgradeState};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[path = "preparation_execution_fixture.rs"]
mod fixture;
use fixture::*;

#[path = "preparation_lifecycle_tests.rs"]
mod lifecycle;
#[path = "preparation_rebinding_tests.rs"]
mod rebinding;

#[test]
fn first_and_three_old_terminals_reach_new_v1_without_switching_programs() {
    for previous in [
        None,
        Some(UpgradeState::Completed),
        Some(UpgradeState::AbortedPreserved),
        Some(UpgradeState::RolledBack),
    ] {
        let fixture = Fixture::new();
        if let Some(state) = previous {
            fixture.legacy(state);
        }
        let db = fixture.path.join("userdb.sqlite3");
        let inode = fs::metadata(&db).unwrap().ino();
        let mut observer = Observer::default();
        let receipt = fixture
            .run(&mut observer)
            .unwrap_or_else(|e| panic!("{previous:?}: {e:?}, {:?}", observer.seen));
        assert_eq!(receipt.state(), UpgradeState::Preflighted);
        assert_eq!(receipt.operation_id(), NEW);
        assert_eq!(receipt.previous_operation_id(), previous.map(|_| DATA));
        assert_eq!(fs::metadata(&db).unwrap().ino(), inode);
        assert_eq!(
            fs::read(fixture.path.join("manager-settings.json")).unwrap(),
            b"synthetic-settings"
        );
        let outer = fixture.install().load().unwrap().unwrap();
        assert_eq!(
            outer.state(),
            radishlex_ime_product_install::InstallState::Prepared
        );
        assert!(outer.artifacts().is_empty());
        assert_eq!(
            outer,
            expected_outer(&fixture.previous, &fixture.target, NEW).unwrap()
        );
        let prep = fixture.preparation();
        assert_eq!(prep.phase(), PreparationPhase::HandoffReady);
        outer::verify_preparation(&fixture.install(), &prep).unwrap();
        assert!(fixture
            .path
            .join(".radishlex-upgrade-history-v1")
            .join(NEW)
            .join("preparation-snapshot.sqlite3")
            .exists());
        assert!(!fixture
            .path
            .join(STATE)
            .join("source-preparation.json")
            .exists());
        assert!(observer.seen.contains(&ExecutionPoint::Source(
            SourcePreparationCheckpoint::HandoffComplete
        )));
        assert_eq!(receipt, fixture.resume(&mut Observer::default()).unwrap());
        let gate = inspect_startup_gate(&fixture.path, fixture.owner());
        assert!(!gate.is_allowed());
        rebinding::assert_product_binding(&fixture, &receipt);
    }
}

#[test]
fn durable_preparation_checkpoints_resume_exact_operation_with_new_authorization() {
    let points = [
        ExecutionPoint::ReservationPersisted,
        ExecutionPoint::OuterPersisted,
        ExecutionPoint::Source(SourcePreparationCheckpoint::SnapshotRecorded),
        ExecutionPoint::Source(SourcePreparationCheckpoint::MaintenanceIntentRecorded),
        ExecutionPoint::Source(SourcePreparationCheckpoint::SourceRecorded),
        ExecutionPoint::Source(SourcePreparationCheckpoint::ArchiveCompleted),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffIntentRecorded),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffReceiptRenamed),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffSnapshotRenamed),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffMarkerRenamed),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffComplete),
    ];
    for point in points {
        let fixture = Fixture::new();
        assert!(
            fixture
                .run(&mut Observer {
                    fail: Some(point),
                    ..Observer::default()
                })
                .is_err(),
            "{point:?}"
        );
        let prep = fixture.preparation();
        assert_eq!(prep.binding().operation_id, NEW);
        let previous = prep.previous_install_evidence().unwrap().clone();
        assert_eq!(
            fixture
                .resume(&mut Observer::default())
                .unwrap_or_else(|e| panic!("{point:?}: {e:?}"))
                .operation_id(),
            NEW
        );
        assert_eq!(
            fixture.preparation().previous_install_evidence(),
            Some(&previous)
        );
    }
}

#[test]
fn unbound_outer_or_snapshot_materials_are_preserved_and_never_adopted() {
    for point in [
        ExecutionPoint::HistoryDirectoryCreated,
        ExecutionPoint::OriginalCreated,
        ExecutionPoint::OriginalWritten,
        ExecutionPoint::OriginalSynced,
        ExecutionPoint::OriginalRenamed,
        ExecutionPoint::OriginalDirectorySynced,
        ExecutionPoint::BeforeInventory,
        ExecutionPoint::BeforeReservation,
        ExecutionPoint::Source(SourcePreparationCheckpoint::SnapshotCreated),
        ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffHistoryCreated),
    ] {
        let fixture = Fixture::new();
        assert!(fixture
            .run(&mut Observer {
                fail: Some(point),
                ..Observer::default()
            })
            .is_err());
        let root = fixture.path.clone();
        let before = tree(&root);
        let result = if root.join(STATE).join("source-preparation.json").exists() {
            let preparation = PreparationReceipt::decode(
                &fs::read(root.join(STATE).join("source-preparation.json")).unwrap(),
            )
            .unwrap();
            let (grant, _control) = authorize_preparation_resume(
                &preparation,
                &fixture.source,
                &fixture.target,
                confirmation(),
                Duration::from_secs(300),
            )
            .unwrap();
            execute(
                grant,
                &fixture.install(),
                &mut Observer::default(),
                &mut NoIds,
            )
        } else {
            fixture.run(&mut Observer::default())
        };
        assert!(result.is_err(), "{point:?}");
        assert_eq!(tree(&root), before, "{point:?}");
    }
}

#[test]
fn revoked_expired_or_incomplete_confirmation_never_reserves() {
    let fixture = Fixture::new();
    for field in 0..4 {
        let mut c = confirmation();
        match field {
            0 => c.explicit_action_confirmed = false,
            1 => c.data_retention_acknowledged = false,
            2 => c.neutral_input_source_selected = false,
            _ => c.manager_closed = false,
        }
        assert!(authorize_source_preparation(
            &fixture.previous,
            &fixture.target,
            c,
            Duration::from_secs(300)
        )
        .is_err());
    }
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(authorize_source_preparation(
            &fixture.previous,
            &fixture.target,
            confirmation(),
            duration
        )
        .is_err());
    }
    let before = tree(&fixture.path);
    for expired in [false, true] {
        let (mut auth, control) = fixture.grant();
        if expired {
            auth.grant.expires = std::time::Instant::now();
        } else {
            control.revoke();
        }
        assert_eq!(
            execute(auth, &fixture.install(), &mut Observer::default(), &mut Ids),
            Err(Error::TaskAuthorization)
        );
        assert_eq!(tree(&fixture.path), before);
    }
    let (auth, control) = fixture.grant();
    let mut observer = Observer {
        mutation: Some((
            ExecutionPoint::BeforeReservation,
            Box::new(move || control.revoke()),
        )),
        ..Observer::default()
    };
    assert_eq!(
        execute(auth, &fixture.install(), &mut observer, &mut Ids),
        Err(Error::TaskAuthorization)
    );
    assert!(!fixture
        .path
        .join(STATE)
        .join("source-preparation.json")
        .exists());
}

#[test]
fn root_guards_capacity_and_replaced_history_fail_closed() {
    let fixture = Fixture::new();
    let foreign = Fixture::new();
    let (auth, _control) = fixture.grant();
    assert_eq!(
        execute(auth, &foreign.install(), &mut Observer::default(), &mut Ids),
        Err(Error::InvalidBinding)
    );
    let install = fixture.install();
    let guard = install.acquire_guard().unwrap();
    assert!(fixture.run(&mut Observer::default()).is_err());
    drop(guard);
    assert!(fixture
        .run(&mut Observer {
            available: Some(1),
            ..Observer::default()
        })
        .is_err());
    assert!(fixture
        .path
        .join(STATE)
        .join("source-preparation.json")
        .exists());
    let path = fixture
        .path
        .join(".radishlex-install-history-v1")
        .join(OLD)
        .join("receipt.json");
    let bytes = fs::read(&path).unwrap();
    fs::rename(&path, path.with_extension("saved")).unwrap();
    put(&path, &bytes);
    assert!(fixture.resume(&mut Observer::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn outer_or_products_changed_during_observation_block_before_sqlite() {
    for replacement in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.path.join(".radishlex-install-v1/receipt.json");
        let bytes = fs::read(&path).unwrap();
        let mutation = Box::new(move || {
            if replacement {
                fs::rename(&path, path.with_extension("saved")).unwrap();
                put(&path, &bytes);
            } else {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
        });
        assert!(fixture
            .run(&mut Observer {
                mutation: Some((ExecutionPoint::OriginalWritten, mutation)),
                ..Observer::default()
            })
            .is_err());
        assert!(!fixture
            .path
            .join(STATE)
            .join("source-preparation.json")
            .exists());
    }
}

#[test]
fn immutable_outer_evidence_is_optional_for_legacy_encoding_but_cannot_be_added_by_replacement() {
    let fixture = Fixture::new();
    fixture
        .run(&mut Observer {
            fail: Some(ExecutionPoint::ReservationPersisted),
            ..Observer::default()
        })
        .unwrap_err();
    let prep = fixture.preparation();
    let legacy =
        PreparationReceipt::new(prep.binding().clone(), prep.initial_source().clone()).unwrap();
    let legacy_bytes = legacy.encode().unwrap();
    assert!(!String::from_utf8_lossy(&legacy_bytes).contains("previous_install_evidence"));
    assert_eq!(PreparationReceipt::decode(&legacy_bytes).unwrap(), legacy);
    assert!(!prep.can_replace(&legacy));
    assert!(!legacy.can_replace(&prep));
    for drift in 0..4 {
        let mut changed = legacy.clone();
        let mut evidence = prep.previous_install_evidence().unwrap().clone();
        match drift {
            0 => evidence.previous_receipt.sha256 = "0".repeat(64),
            1 => evidence.history_root.inode = evidence.previous_directory.inode,
            2 => evidence.previous_receipt.mode = 0o644,
            _ => evidence.previous_receipt.owner_id += 1,
        }
        assert!(changed.bind_previous_install(evidence).is_err());
        assert_eq!(changed, legacy);
    }
}

#[test]
fn inner_guard_nonterminal_predecessor_and_stale_task_are_rejected() {
    let fixture = Fixture::new();
    let store = UpgradeReceiptStore::open(
        VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
    )
    .unwrap();
    let inner = store.acquire_guard().unwrap();
    assert!(fixture.run(&mut Observer::default()).is_err());
    drop(inner);
    fixture
        .run(&mut Observer {
            fail: Some(ExecutionPoint::ReservationPersisted),
            ..Observer::default()
        })
        .unwrap_err();
    let (stale, _control) = fixture.resume_grant();
    fixture
        .resume(&mut Observer {
            fail: Some(ExecutionPoint::Source(
                SourcePreparationCheckpoint::SourceRecorded,
            )),
            ..Observer::default()
        })
        .unwrap_err();
    let before = tree(&fixture.path);
    assert_eq!(
        execute(
            stale,
            &fixture.install(),
            &mut Observer::default(),
            &mut NoIds
        ),
        Err(Error::InvalidBinding)
    );
    assert_eq!(tree(&fixture.path), before);
    let other = Fixture::new();
    other.legacy(UpgradeState::CandidateVerified);
    let before = tree(&other.path);
    assert!(other.run(&mut Observer::default()).is_err());
    assert_eq!(tree(&other.path), before);
}

#[test]
fn callback_cannot_replace_an_active_outer_with_identical_bytes() {
    for point in [
        ExecutionPoint::ReservationPersisted,
        ExecutionPoint::Source(SourcePreparationCheckpoint::Begin),
    ] {
        let fixture = Fixture::new();
        let path = fixture.path.join(".radishlex-install-v1/receipt.json");
        let mutation = Box::new(move || {
            let bytes = fs::read(&path).unwrap();
            let saved = path.with_extension("saved");
            fs::rename(&path, &saved).unwrap();
            put(&path, &bytes);
            fs::remove_file(saved).unwrap();
        });
        assert_eq!(
            fixture.run(&mut Observer {
                mutation: Some((point, mutation)),
                ..Observer::default()
            }),
            Err(Error::InvalidBinding)
        );
        assert_eq!(fixture.preparation().phase(), PreparationPhase::Reserved);
    }
}

#[test]
fn inner_directory_drift_during_outer_sealing_or_before_publication_stops_the_outer_write() {
    use std::os::unix::fs::DirBuilderExt;
    for point in [
        ExecutionPoint::HistoryDirectoryCreated,
        ExecutionPoint::ReservationPersisted,
    ] {
        let fixture = Fixture::new();
        let root = fixture.path.clone();
        let mut observer = Observer {
            mutation: Some((
                point,
                Box::new(move || {
                    let state = root.join(STATE);
                    let saved = root.join("synthetic-replaced-state");
                    fs::rename(&state, &saved).unwrap();
                    fs::DirBuilder::new().mode(0o700).create(&state).unwrap();
                    for entry in fs::read_dir(saved).unwrap() {
                        let entry = entry.unwrap();
                        fs::copy(entry.path(), state.join(entry.file_name())).unwrap();
                    }
                }),
            )),
            ..Observer::default()
        };
        assert!(fixture.run(&mut observer).is_err());
        assert_eq!(
            fixture.install().load().unwrap().as_ref(),
            Some(&fixture.previous)
        );
        assert!(!observer.seen.contains(&ExecutionPoint::OuterPersisted));
        if point == ExecutionPoint::HistoryDirectoryCreated {
            assert!(!observer.seen.contains(&ExecutionPoint::OriginalCreated));
        }
    }
}

fn tree(root: &std::path::Path) -> Vec<(std::path::PathBuf, u64, u32, Vec<u8>)> {
    let mut nodes = vec![root.to_owned()];
    let mut result = vec![];
    while let Some(path) = nodes.pop() {
        let m = fs::symlink_metadata(&path).unwrap();
        result.push((
            path.strip_prefix(root).unwrap().to_owned(),
            m.ino(),
            m.mode(),
            if m.is_file() {
                fs::read(&path).unwrap()
            } else {
                vec![]
            },
        ));
        if m.is_dir() {
            for entry in fs::read_dir(&path).unwrap() {
                nodes.push(entry.unwrap().path());
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

const CRASH_POINTS: [ExecutionPoint; 8] = [
    ExecutionPoint::OriginalRenamed,
    ExecutionPoint::ReservationPersisted,
    ExecutionPoint::OuterPersisted,
    ExecutionPoint::Source(SourcePreparationCheckpoint::SourceRecorded),
    ExecutionPoint::Source(SourcePreparationCheckpoint::ArchiveCompleted),
    ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffIntentRecorded),
    ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffReceiptRenamed),
    ExecutionPoint::Source(SourcePreparationCheckpoint::HandoffMarkerRenamed),
];

#[test]
fn real_child_exits_reacquire_both_guards_and_resume_only_bound_operations() {
    for (index, point) in CRASH_POINTS.iter().enumerate() {
        let fixture = Fixture::new();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "preparation_execution::tests::preparation_task_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RADISHLEX_PREPARATION_TASK_TEST_ROOT", &fixture.path)
            .env("RADISHLEX_PREPARATION_TASK_TEST_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            status.status.code(),
            Some(79),
            "{point:?}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        if index == 0 {
            let before = tree(&fixture.path);
            assert!(fixture.run(&mut Observer::default()).is_err());
            assert_eq!(tree(&fixture.path), before);
        } else {
            assert_eq!(
                fixture
                    .resume(&mut Observer::default())
                    .unwrap_or_else(|e| panic!("{point:?}: {e:?}"))
                    .operation_id(),
                NEW
            );
        }
    }
}

#[test]
#[ignore = "invoked only by the parent crash matrix"]
fn preparation_task_crash_child() {
    let root = std::env::var_os("RADISHLEX_PREPARATION_TASK_TEST_ROOT").expect("parent root");
    let point: usize = std::env::var("RADISHLEX_PREPARATION_TASK_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let fixture = Fixture::reopen(root.into());
    let _ = fixture.run(&mut Observer {
        fail: Some(CRASH_POINTS[point]),
        exit: true,
        ..Observer::default()
    });
    panic!("child checkpoint not reached");
}
