//! Real files/SQLite/SHA-256; outer authorization is deliberately synthetic.
use super::*;
use radishlex_ime_product_upgrade::{
    PreparationCancellationCheckpoint as CancelPoint, PreparationCancellationPort,
    PreparationCancellationRequest, PreparationCancellationStore, PreparationJournalError,
};
use std::os::unix::fs::{symlink, PermissionsExt};

const REQUEST: &str = ".radishlex-upgrade-v1/preparation-cancellation.json";
const TEMP: &str = ".radishlex-upgrade-v1/preparation-cancellation.json.tmp";
const JOURNAL: &str = ".radishlex-upgrade-v1/source-preparation.json";
const PHASES: [PreparationPhase; 5] = [
    PreparationPhase::Reserved,
    PreparationPhase::SnapshotReady,
    PreparationPhase::MaintenanceIntent,
    PreparationPhase::SourcePrepared,
    PreparationPhase::PreviousArchived,
];
const CANCEL_POINTS: [CancelPoint; 11] = [
    CancelPoint::Begin,
    CancelPoint::BeforeCreate,
    CancelPoint::Created,
    CancelPoint::Written,
    CancelPoint::BeforeFileSync,
    CancelPoint::FileSynced,
    CancelPoint::BeforeRename,
    CancelPoint::Renamed,
    CancelPoint::BeforeDirectorySync,
    CancelPoint::DirectorySynced,
    CancelPoint::Recorded,
];

struct CancelPort {
    preparation: PreparationReceipt,
    fail: Option<CancelPoint>,
    exit: Option<CancelPoint>,
    mutate: Option<(CancelPoint, Box<dyn FnMut()>)>,
}
impl CancelPort {
    fn new(preparation: PreparationReceipt) -> Self {
        Self {
            preparation,
            fail: None,
            exit: None,
            mutate: None,
        }
    }
}
impl PreparationCancellationPort for CancelPort {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        point: CancelPoint,
    ) -> bool {
        assert_eq!(request.preparation(), &self.preparation);
        if self.exit == Some(point) {
            std::process::exit(76);
        }
        if self.mutate.as_ref().is_some_and(|(at, _)| *at == point) {
            let (_, mut mutate) = self.mutate.take().unwrap();
            mutate();
        }
        self.fail != Some(point)
    }
}

fn at_phase(
    fixture: &Fixture,
    phase: PreparationPhase,
) -> (PreparationJournalStore, UpgradeProcessGuard, Port) {
    let (store, guard, mut port) = fixture.reserve_with_previous(true);
    match phase {
        PreparationPhase::Reserved => {}
        PreparationPhase::SnapshotReady | PreparationPhase::MaintenanceIntent => {
            port.fail = Some(if phase == PreparationPhase::SnapshotReady {
                Point::SnapshotRecorded
            } else {
                Point::MaintenanceIntentRecorded
            });
            assert!(store
                .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
                .is_err());
        }
        PreparationPhase::SourcePrepared | PreparationPhase::PreviousArchived => {
            store
                .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
                .unwrap();
            if phase == PreparationPhase::PreviousArchived {
                store
                    .archive_previous_upgrade(&guard, &mut port, &MacOsPreparationHasher)
                    .unwrap();
            }
        }
        _ => unreachable!(),
    }
    assert_eq!(store.load_guarded(&guard).unwrap().unwrap().phase(), phase);
    port.fail = None;
    (store, guard, port)
}

fn protected(fixture: &Fixture) -> Vec<(PathBuf, u64, Vec<u8>)> {
    [
        "userdb.sqlite3",
        "userdb.sqlite3-wal",
        "userdb.sqlite3-shm",
        JOURNAL,
        SNAPSHOT,
    ]
    .iter()
    .filter_map(|name| {
        let path = fixture.0.join(name);
        path.exists().then(|| {
            (
                path.clone(),
                fs::metadata(&path).unwrap().ino(),
                fs::read(&path).unwrap(),
            )
        })
    })
    .collect()
}
fn assert_preserved(files: &[(PathBuf, u64, Vec<u8>)]) {
    for (path, inode, bytes) in files {
        assert_eq!(fs::metadata(path).unwrap().ino(), *inode);
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
}

#[test]
fn cancellation_request_is_immutable_reloadable_and_blocks_every_normal_entry() {
    for phase in PHASES {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, mut normal_port) = at_phase(&fixture, phase);
        let alias = PreparationJournalStore::open_existing(fixture.verified()).unwrap();
        let handoff_alias = PreparationJournalStore::open_existing(fixture.verified()).unwrap();
        let prep = store.load_guarded(&guard).unwrap().unwrap();
        let family = store
            .observe_source_family(&guard, &MacOsPreparationHasher)
            .unwrap();
        let state = store.state_directory_identity();
        let before = protected(&fixture);
        let mut port = CancelPort::new(prep.clone());
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        let request = cancel
            .request_cancellation(
                &guard,
                &prep.binding().operation_id,
                &mut port,
                &MacOsPreparationHasher,
            )
            .unwrap();
        assert_eq!(request.observed_source(), &family);
        assert_eq!(request.preparation(), &prep);
        assert_eq!(
            request.preparation_identity().inode,
            fs::metadata(fixture.0.join(JOURNAL)).unwrap().ino()
        );
        assert!(!fixture.0.join(TEMP).exists());
        let inode = fs::metadata(fixture.0.join(REQUEST)).unwrap().ino();
        assert!(alias.load_guarded(&guard).is_err());
        assert!(alias.persist(&guard, Some(&prep), &prep).is_err());
        assert!(alias
            .prepare_userdb_source(&guard, &mut normal_port, &MacOsPreparationHasher)
            .is_err());
        assert!(alias
            .archive_previous_upgrade(&guard, &mut normal_port, &MacOsPreparationHasher)
            .is_err());
        assert!(handoff_alias
            .handoff_userdb_source(
                &guard,
                &prep.binding().operation_id,
                &mut normal_port,
                &MacOsPreparationHasher
            )
            .is_err());
        assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
        assert!(UpgradeReceiptStore::open_existing(fixture.verified()).is_err());
        drop(guard);
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
        let cancel = PreparationCancellationStore::open_existing(fixture.verified()).unwrap();
        let guard = cancel.acquire_guard().unwrap();
        assert_eq!(
            cancel
                .load_guarded(&guard, &MacOsPreparationHasher)
                .unwrap(),
            Some(request.clone())
        );
        assert_eq!(
            cancel
                .request_cancellation(
                    &guard,
                    &prep.binding().operation_id,
                    &mut port,
                    &MacOsPreparationHasher
                )
                .unwrap(),
            request
        );
        assert!(cancel
            .request_cancellation(
                &guard,
                "44444444444444444444444444444444",
                &mut port,
                &MacOsPreparationHasher
            )
            .is_err());
        assert_eq!(fs::metadata(fixture.0.join(REQUEST)).unwrap().ino(), inode);
        assert_eq!(
            fs::metadata(fixture.0.join(".radishlex-upgrade-v1"))
                .unwrap()
                .ino(),
            state.inode
        );
        assert_preserved(&before);
    }
}

fn assert_restart(fixture: &Fixture, prep: &PreparationReceipt, point: CancelPoint) {
    let cancel = PreparationCancellationStore::open_existing(fixture.verified()).unwrap();
    let guard = cancel.acquire_guard().unwrap();
    let mut port = CancelPort::new(prep.clone());
    let result = cancel.request_cancellation(
        &guard,
        &prep.binding().operation_id,
        &mut port,
        &MacOsPreparationHasher,
    );
    if matches!(
        point,
        CancelPoint::Created
            | CancelPoint::Written
            | CancelPoint::BeforeFileSync
            | CancelPoint::FileSynced
            | CancelPoint::BeforeRename
    ) {
        assert_eq!(
            result,
            Err(SourcePreparationError::Journal(
                PreparationJournalError::InterruptedWrite
            ))
        );
        assert!(fixture.0.join(TEMP).exists());
        assert!(!fixture.0.join(REQUEST).exists());
        assert!(cancel
            .load_guarded(&guard, &MacOsPreparationHasher)
            .is_err());
    } else {
        let request = result.unwrap();
        assert_eq!(request.preparation(), prep);
        assert_eq!(
            cancel
                .load_guarded(&guard, &MacOsPreparationHasher)
                .unwrap(),
            Some(request)
        );
    }
    assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
    drop(guard);
    assert!(
        !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id()).is_allowed()
    );
}

#[test]
fn cancellation_each_authority_loss_keeps_request_or_blocks_unbound_temp() {
    for phase in PHASES {
        for point in CANCEL_POINTS {
            let fixture = Fixture::new();
            fixture.populate();
            let (store, guard, _) = at_phase(&fixture, phase);
            let prep = store.load_guarded(&guard).unwrap().unwrap();
            let before = protected(&fixture);
            let mut port = CancelPort::new(prep.clone());
            port.fail = Some(point);
            let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
            assert_eq!(
                cancel.request_cancellation(
                    &guard,
                    &prep.binding().operation_id,
                    &mut port,
                    &MacOsPreparationHasher
                ),
                Err(SourcePreparationError::AuthorityNotProven),
                "{phase:?} {point:?}"
            );
            drop(guard);
            assert_restart(&fixture, &prep, point);
            assert_preserved(&before);
        }
    }
}

#[test]
fn cancellation_each_process_exit_reloads_exactly_or_preserves_interrupted_files() {
    for phase in PHASES {
        for point in CANCEL_POINTS {
            let fixture = Fixture::new();
            fixture.populate();
            let (store, guard, _) = at_phase(&fixture, phase);
            let prep = store.load_guarded(&guard).unwrap().unwrap();
            let before = protected(&fixture);
            drop(guard);
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "cancellation::cancellation_child", "--ignored"])
                .env("RADISHLEX_CANCELLATION_TEST_ROOT", &fixture.0)
                .env("RADISHLEX_CANCELLATION_TEST_POINT", format!("{point:?}"))
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(76), "{phase:?} {point:?}");
            assert_restart(&fixture, &prep, point);
            assert_preserved(&before);
        }
    }
}

#[test]
#[ignore = "invoked in an isolated synthetic root by cancellation process-exit tests"]
fn cancellation_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_CANCELLATION_TEST_ROOT").unwrap());
    assert_eq!(
        root.parent().unwrap(),
        fs::canonicalize(std::env::temp_dir()).unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-source-preparation-"));
    let verified = VerifiedDataRoot::verify(&root, fs::metadata(&root).unwrap().uid()).unwrap();
    let cancel = PreparationCancellationStore::open_existing(verified).unwrap();
    let guard = cancel.acquire_guard().unwrap();
    let prep = PreparationReceipt::decode(&fs::read(root.join(JOURNAL)).unwrap()).unwrap();
    let point = std::env::var("RADISHLEX_CANCELLATION_TEST_POINT").unwrap();
    let mut port = CancelPort::new(prep.clone());
    port.exit = CANCEL_POINTS
        .into_iter()
        .find(|value| format!("{value:?}") == point);
    assert!(port.exit.is_some());
    cancel
        .request_cancellation(
            &guard,
            &prep.binding().operation_id,
            &mut port,
            &MacOsPreparationHasher,
        )
        .unwrap();
    panic!("exit point was not reached");
}

#[test]
fn cancellation_before_maintenance_keeps_unclosed_nonempty_wal_byte_for_byte() {
    for phase in [
        PreparationPhase::Reserved,
        PreparationPhase::SnapshotReady,
        PreparationPhase::MaintenanceIntent,
    ] {
        let fixture = Fixture::new();
        assert_eq!(child(&fixture.0, "wal").code(), Some(75));
        let (store, guard, _) = at_phase(&fixture, phase);
        let prep = store.load_guarded(&guard).unwrap().unwrap();
        let before = protected(&fixture);
        let mut port = CancelPort::new(prep.clone());
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        let request = cancel
            .request_cancellation(
                &guard,
                &prep.binding().operation_id,
                &mut port,
                &MacOsPreparationHasher,
            )
            .unwrap();
        assert!(request.observed_source().wal.as_ref().unwrap().byte_len > 0);
        assert_eq!(
            cancel
                .load_guarded(&guard, &MacOsPreparationHasher)
                .unwrap(),
            Some(request)
        );
        assert_preserved(&before);
    }
}

#[test]
fn cancellation_strict_record_rejects_unknowns_duplicates_and_noncanonical_bytes() {
    let fixture = Fixture::new();
    fixture.populate();
    let (store, guard, _) = at_phase(&fixture, PreparationPhase::Reserved);
    let prep = store.load_guarded(&guard).unwrap().unwrap();
    let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
    let request = cancel
        .request_cancellation(
            &guard,
            &prep.binding().operation_id,
            &mut CancelPort::new(prep.clone()),
            &MacOsPreparationHasher,
        )
        .unwrap();
    let text = String::from_utf8(request.encode().unwrap()).unwrap();
    assert_eq!(
        PreparationCancellationRequest::decode(text.as_bytes()).unwrap(),
        request
    );
    for changed in [
        text.replace(
            "radishlex-preparation-cancellation-v1",
            "radishlex-preparation-cancellation-v2",
        ),
        text.replacen("\"requested\"", "\"cancel_ready\"", 1),
        text.replacen("\"user_requested\"", "\"snapshot_failed\"", 1),
        text.replacen("{", "{\"unknown\":true,", 1),
        text.replacen("{", "{\"reason\":\"user_requested\",", 1),
        text.replacen(
            "\"previous_install_operation_id\":\"22222222222222222222222222222222\"",
            "\"previous_install_operation_id\":null",
            1,
        ),
        format!(" {text}"),
        text.trim_end().to_owned(),
    ] {
        assert_ne!(changed, text);
        assert!(PreparationCancellationRequest::decode(changed.as_bytes()).is_err());
    }
    assert!(PreparationCancellationRequest::decode(&vec![b' '; 256 * 1024 + 1]).is_err());
}

#[test]
fn cancellation_requires_previous_outer_and_rejects_handoff_intent() {
    for handoff in [false, true] {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, mut normal_port) = if handoff {
            at_phase(&fixture, PreparationPhase::PreviousArchived)
        } else {
            fixture.reserve()
        };
        let alias = PreparationJournalStore::open_existing(fixture.verified()).unwrap();
        if handoff {
            normal_port.fail = Some(Point::HandoffIntentRecorded);
            assert!(store
                .handoff_userdb_source(
                    &guard,
                    &normal_port.binding.operation_id.clone(),
                    &mut normal_port,
                    &MacOsPreparationHasher
                )
                .is_err());
        }
        let prep = alias.load_guarded(&guard).unwrap().unwrap();
        let before = protected(&fixture);
        let cancel = PreparationCancellationStore::attach(alias, &guard).unwrap();
        assert!(cancel
            .request_cancellation(
                &guard,
                &prep.binding().operation_id,
                &mut CancelPort::new(prep.clone()),
                &MacOsPreparationHasher
            )
            .is_err());
        assert!(!fixture.0.join(REQUEST).exists());
        assert!(!fixture.0.join(TEMP).exists());
        assert_preserved(&before);
    }
}

#[test]
fn cancellation_rejects_bound_evidence_and_private_object_drift() {
    for case in 0..13 {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, _) = at_phase(&fixture, PreparationPhase::SourcePrepared);
        let prep = store.load_guarded(&guard).unwrap().unwrap();
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        let mut port = CancelPort::new(prep.clone());
        let request = cancel
            .request_cancellation(
                &guard,
                &prep.binding().operation_id,
                &mut port,
                &MacOsPreparationHasher,
            )
            .unwrap();
        let marker = fixture.0.join(REQUEST);
        match case {
            0 => {
                fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap();
            }
            1 => {
                fs::hard_link(&marker, fixture.0.join("retained-link")).unwrap();
            }
            2 => {
                fs::rename(&marker, fixture.0.join("retained-request")).unwrap();
                symlink(fixture.0.join("retained-request"), &marker).unwrap();
            }
            3 => {
                fs::write(&marker, b"{}\n").unwrap();
            }
            4 => {
                put(&fixture.0.join(TEMP), &request.encode().unwrap());
            }
            5 => {
                put(
                    &fixture.0.join(".radishlex-upgrade-v1/unknown"),
                    b"unexpected",
                );
            }
            6 | 7 => {
                let path = fixture.0.join(if case == 6 { JOURNAL } else { SNAPSHOT });
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("retained-original")).unwrap();
                put(&path, &bytes);
            }
            8 => {
                fs::write(fixture.0.join(SNAPSHOT), b"changed snapshot").unwrap();
            }
            9 => {
                fs::write(fixture.source(), b"changed database").unwrap();
            }
            10 => {
                put(
                    &fixture.0.join("userdb.sqlite3-journal"),
                    b"unqualified journal",
                );
            }
            11 => {
                put(
                    &fixture.0.join(".radishlex-upgrade-v1/receipt.json"),
                    b"unexpected data operation",
                );
            }
            12 => {
                fs::rename(
                    fixture.0.join(".radishlex-upgrade-v1"),
                    fixture.0.join("retained-state"),
                )
                .unwrap();
                DirBuilder::new()
                    .mode(0o700)
                    .create(fixture.0.join(".radishlex-upgrade-v1"))
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            cancel
                .load_guarded(&guard, &MacOsPreparationHasher)
                .is_err(),
            "{case}"
        );
        assert!(
            cancel
                .request_cancellation(
                    &guard,
                    &prep.binding().operation_id,
                    &mut port,
                    &MacOsPreparationHasher
                )
                .is_err(),
            "{case}"
        );
    }
}

#[test]
fn cancellation_callback_races_are_rechecked_before_acknowledgement() {
    for point in [
        CancelPoint::Begin,
        CancelPoint::BeforeCreate,
        CancelPoint::Created,
        CancelPoint::BeforeRename,
        CancelPoint::Renamed,
        CancelPoint::Recorded,
    ] {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, _) = at_phase(&fixture, PreparationPhase::Reserved);
        let prep = store.load_guarded(&guard).unwrap().unwrap();
        let path = fixture.0.join(JOURNAL);
        let retained = fixture.0.join("retained-preparation");
        let mut port = CancelPort::new(prep.clone());
        port.mutate = Some((
            point,
            Box::new(move || {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, &retained).unwrap();
                put(&path, &bytes);
            }),
        ));
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        assert!(
            cancel
                .request_cancellation(
                    &guard,
                    &prep.binding().operation_id,
                    &mut port,
                    &MacOsPreparationHasher
                )
                .is_err(),
            "{point:?}"
        );
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
    }
}

#[test]
fn cancellation_rejects_temp_replacement_collisions_and_marker_races() {
    for case in 0..5 {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, _) = at_phase(&fixture, PreparationPhase::Reserved);
        let prep = store.load_guarded(&guard).unwrap().unwrap();
        let mut port = CancelPort::new(prep.clone());
        let root = fixture.0.clone();
        let point = match case {
            0 => CancelPoint::Created,
            1 | 2 => CancelPoint::BeforeRename,
            3 => CancelPoint::Renamed,
            4 => CancelPoint::Recorded,
            _ => unreachable!(),
        };
        port.mutate = Some((
            point,
            Box::new(move || match case {
                0 | 3 => {
                    let path = root.join(if case == 0 { TEMP } else { REQUEST });
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, root.join("retained-cancellation-file")).unwrap();
                    put(&path, &bytes);
                }
                1 => {
                    fs::write(root.join(TEMP), b"changed staged record").unwrap();
                }
                2 => {
                    put(&root.join(REQUEST), &fs::read(root.join(TEMP)).unwrap());
                }
                4 => {
                    fs::write(root.join("userdb.sqlite3"), b"changed source").unwrap();
                }
                _ => unreachable!(),
            }),
        ));
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        assert!(
            cancel
                .request_cancellation(
                    &guard,
                    &prep.binding().operation_id,
                    &mut port,
                    &MacOsPreparationHasher
                )
                .is_err(),
            "{case}"
        );
        if case == 2 {
            assert_eq!(
                fs::read(fixture.0.join(REQUEST)).unwrap(),
                fs::read(fixture.0.join(TEMP)).unwrap()
            );
        }
        drop(guard);
        assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
    }
}

#[path = "source_finishing.rs"]
mod source_finishing;
