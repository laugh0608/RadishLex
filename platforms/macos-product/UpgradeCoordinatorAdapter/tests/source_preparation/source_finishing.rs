//! Synthetic outer authority; real SQLite, private files and child-process exits.
use super::*;
use radishlex_ime_product_upgrade::{
    CancellationSourceCheckpoint as FinishPoint, CancellationSourcePhase as FinishPhase,
    CancellationSourcePort, CancellationSourceReceipt,
};

const PROGRESS: &str = ".radishlex-upgrade-v1/cancellation-source.json";
const PROGRESS_TEMP: &str = ".radishlex-upgrade-v1/cancellation-source.json.tmp";

struct FinishPort {
    request: PreparationCancellationRequest,
    fail: Option<FinishPoint>,
    exit: Option<FinishPoint>,
    mutate: Option<(FinishPoint, Box<dyn FnMut()>)>,
    capacity: Option<u64>,
    seen: Vec<FinishPoint>,
    recovery_only: bool,
}
impl FinishPort {
    fn new(request: PreparationCancellationRequest) -> Self {
        Self {
            request,
            fail: None,
            exit: None,
            mutate: None,
            capacity: Some(u64::MAX),
            seen: Vec::new(),
            recovery_only: false,
        }
    }
}
impl CancellationSourcePort for FinishPort {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        progress: &CancellationSourceReceipt,
        point: FinishPoint,
    ) -> bool {
        assert_eq!(request, &self.request);
        assert_eq!(
            progress.preparation().binding(),
            request.preparation().binding()
        );
        self.seen.push(point);
        let eligible = !self.recovery_only || progress.preparation().journal_recovery().is_some();
        if eligible && self.exit == Some(point) {
            std::process::exit(77);
        }
        if self.mutate.as_ref().is_some_and(|(at, _)| *at == point) {
            let (_, mut f) = self.mutate.take().unwrap();
            f();
        }
        !eligible || self.fail != Some(point)
    }
    fn available_bytes(&mut self) -> Option<u64> {
        self.capacity
    }
}

fn requested(
    fixture: &Fixture,
    phase: PreparationPhase,
) -> (
    PreparationCancellationStore,
    UpgradeProcessGuard,
    FinishPort,
) {
    let (store, guard, _) = at_phase(fixture, phase);
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
    (cancel, guard, FinishPort::new(request))
}
fn frozen(fixture: &Fixture) -> Vec<(PathBuf, u64, Vec<u8>)> {
    [
        JOURNAL,
        REQUEST,
        SNAPSHOT,
        "manager-settings.json",
        "synthetic-rime-userdb",
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
fn blocked(fixture: &Fixture) {
    assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
    assert!(
        !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id()).is_allowed()
    );
}
fn points(phase: PreparationPhase) -> Vec<FinishPoint> {
    let mut points = Vec::new();
    for state in [FinishPhase::Finishing, FinishPhase::SourceReady] {
        points.extend(CANCEL_POINTS.map(|point| FinishPoint::Write(state, point)));
    }
    points.extend([
        FinishPoint::Begin,
        FinishPoint::BeforeVerify,
        FinishPoint::SourceVerified,
        FinishPoint::SourceReady,
    ]);
    if phase == PreparationPhase::MaintenanceIntent {
        points.extend([
            FinishPoint::BeforeMaintenance,
            FinishPoint::SourceMaintained,
            FinishPoint::BeforeSourceSync,
            FinishPoint::SourceFileSynced,
            FinishPoint::SourceDirectorySynced,
        ]);
    }
    points
}

#[test]
fn source_finishing_preserves_all_five_phase_contracts_and_never_releases() {
    for phase in PHASES {
        let fixture = Fixture::new();
        child(&fixture.0, "wal");
        put(
            &fixture.0.join("manager-settings.json"),
            b"{\"synthetic\":true}\n",
        );
        put(
            &fixture.0.join("synthetic-rime-userdb"),
            b"synthetic untouched input data",
        );
        let (cancel, guard, mut port) = requested(&fixture, phase);
        let before = protected(&fixture);
        let immutable = frozen(&fixture);
        let original_inode = fs::metadata(fixture.source()).unwrap().ino();
        let ready = cancel
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .unwrap();
        assert_eq!(ready.phase(), FinishPhase::SourceReady);
        assert_eq!(ready.source_ready().unwrap().database.inode, original_inode);
        assert_preserved(&immutable);
        if phase < PreparationPhase::MaintenanceIntent {
            assert_preserved(&before);
            assert!(!port.seen.contains(&FinishPoint::BeforeMaintenance));
            assert_eq!(
                fixture.0.join(SNAPSHOT).exists(),
                phase != PreparationPhase::Reserved
            );
        } else {
            for suffix in ["-wal", "-shm", "-journal"] {
                assert!(!fixture.0.join(format!("userdb.sqlite3{suffix}")).exists());
            }
            assert_eq!(
                UserDb::verify_prepared_source(fixture.source(), fixture.0.join(SNAPSHOT)).unwrap(),
                9
            );
        }
        let progress_inode = fs::metadata(fixture.0.join(PROGRESS)).unwrap().ino();
        assert_eq!(
            cancel
                .finish_source(&guard, &mut port, &MacOsPreparationHasher)
                .unwrap(),
            ready
        );
        assert_eq!(
            fs::metadata(fixture.0.join(PROGRESS)).unwrap().ino(),
            progress_inode
        );
        assert!(cancel
            .load_guarded(&guard, &MacOsPreparationHasher)
            .is_err());
        drop(guard);
        let reopened = PreparationCancellationStore::open_existing(fixture.verified()).unwrap();
        let guard = reopened.acquire_guard().unwrap();
        assert_eq!(
            reopened
                .load_source_guarded(&guard, &MacOsPreparationHasher)
                .unwrap(),
            Some(ready)
        );
        blocked(&fixture);
        drop(guard);
        let db = UserDb::open(fixture.source()).unwrap();
        assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
        assert_eq!(db.list_active_terms().unwrap().len(), 1);
    }
}

fn resume(fixture: &Fixture, request: PreparationCancellationRequest) {
    let store = PreparationCancellationStore::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    let mut port = FinishPort::new(request);
    if fixture.0.join(PROGRESS_TEMP).exists() {
        let bytes = fs::read(fixture.0.join(PROGRESS_TEMP)).unwrap();
        let inode = fs::metadata(fixture.0.join(PROGRESS_TEMP)).unwrap().ino();
        assert!(store
            .load_source_guarded(&guard, &MacOsPreparationHasher)
            .is_err());
        assert!(store
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .is_err());
        assert_eq!(fs::read(fixture.0.join(PROGRESS_TEMP)).unwrap(), bytes);
        assert_eq!(
            fs::metadata(fixture.0.join(PROGRESS_TEMP)).unwrap().ino(),
            inode
        );
    } else {
        store
            .load_source_guarded(&guard, &MacOsPreparationHasher)
            .unwrap();
        let ready = store
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .unwrap();
        assert_eq!(ready.phase(), FinishPhase::SourceReady);
    }
    blocked(fixture);
}

#[test]
fn source_finishing_authority_loss_at_each_phase_and_checkpoint_preserves_or_resumes() {
    for phase in PHASES {
        for point in points(phase) {
            let fixture = Fixture::new();
            child(&fixture.0, "wal");
            let (store, guard, mut port) = requested(&fixture, phase);
            let immutable = frozen(&fixture);
            port.fail = Some(point);
            assert_eq!(
                store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
                Err(SourcePreparationError::AuthorityNotProven),
                "{phase:?} {point:?}"
            );
            assert!(port.seen.contains(&point));
            assert_preserved(&immutable);
            drop(guard);
            resume(&fixture, port.request);
            assert_preserved(&immutable);
        }
    }
}

fn exit_at(fixture: &Fixture, point: FinishPoint) {
    exit_with_recovery(fixture, point, false);
}
fn exit_with_recovery(fixture: &Fixture, point: FinishPoint, recovery_only: bool) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cancellation::source_finishing::source_finishing_child",
        ])
        .env("RADISHLEX_FINISH_SOURCE_ROOT", &fixture.0)
        .env("RADISHLEX_FINISH_SOURCE_POINT", format!("{point:?}"))
        .env(
            "RADISHLEX_FINISH_SOURCE_RECOVERY",
            if recovery_only { "yes" } else { "no" },
        )
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(77),
        "{point:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn source_finishing_real_process_exit_matrix_reacquires_guard_and_resumes() {
    for phase in PHASES {
        for point in points(phase) {
            let fixture = Fixture::new();
            child(&fixture.0, "wal");
            let (store, guard, port) = requested(&fixture, phase);
            let immutable = frozen(&fixture);
            drop(guard);
            drop(store);
            exit_at(&fixture, point);
            assert_preserved(&immutable);
            resume(&fixture, port.request);
            assert_preserved(&immutable);
        }
    }
}

#[test]
#[ignore = "called only by the parent with an isolated synthetic directory"]
fn source_finishing_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_FINISH_SOURCE_ROOT").unwrap());
    assert_eq!(
        root.parent().unwrap(),
        fs::canonicalize(std::env::temp_dir()).unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("radishlex-source-preparation-"));
    let owner = fs::metadata(&root).unwrap().uid();
    let store = PreparationCancellationStore::open_existing(
        VerifiedDataRoot::verify(&root, owner).unwrap(),
    )
    .unwrap();
    let guard = store.acquire_guard().unwrap();
    let request =
        PreparationCancellationRequest::decode(&fs::read(root.join(REQUEST)).unwrap()).unwrap();
    let mut port = FinishPort::new(request.clone());
    let mut allowed = points(request.preparation().phase());
    allowed.extend([
        FinishPoint::BeforeJournalRecovery,
        FinishPoint::JournalRecoveryIntentRecorded,
        FinishPoint::JournalRecovered,
    ]);
    port.recovery_only = std::env::var("RADISHLEX_FINISH_SOURCE_RECOVERY").unwrap() == "yes";
    let name = std::env::var("RADISHLEX_FINISH_SOURCE_POINT").unwrap();
    port.exit = allowed
        .into_iter()
        .find(|point| format!("{point:?}") == name);
    assert!(port.exit.is_some());
    store
        .finish_source(&guard, &mut port, &MacOsPreparationHasher)
        .unwrap();
    panic!("requested child checkpoint not reached");
}

#[test]
fn source_finishing_rejects_capacity_loss_without_touching_source() {
    for capacity in [None, Some(0)] {
        let fixture = Fixture::new();
        child(&fixture.0, "wal");
        let (store, guard, mut port) = requested(&fixture, PreparationPhase::MaintenanceIntent);
        let before = protected(&fixture);
        port.capacity = capacity;
        assert_eq!(
            store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
            Err(SourcePreparationError::InsufficientSpace)
        );
        assert_preserved(&before);
        port.capacity = Some(u64::MAX);
        store
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .unwrap();
        blocked(&fixture);
    }
}

fn hot_fixture() -> (
    Fixture,
    PreparationCancellationStore,
    UpgradeProcessGuard,
    FinishPort,
) {
    let fixture = Fixture::new();
    put(
        &fixture.source(),
        include_bytes!("../../../../../crates/ime-userdb/tests/fixtures/journal-before.sqlite3"),
    );
    let (store, guard, mut port) = requested(&fixture, PreparationPhase::MaintenanceIntent);
    port.fail = Some(FinishPoint::Begin);
    assert_eq!(
        store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
        Err(SourcePreparationError::AuthorityNotProven)
    );
    // Existing userdb pager fixtures, deliberately not a product power-loss claim.
    fs::write(
        fixture.source(),
        include_bytes!("../../../../../crates/ime-userdb/tests/fixtures/journal-spilled.sqlite3"),
    )
    .unwrap();
    put(
        &fixture.0.join("userdb.sqlite3-journal"),
        include_bytes!(
            "../../../../../crates/ime-userdb/tests/fixtures/journal-spilled.sqlite3-journal"
        ),
    );
    port.fail = None;
    (fixture, store, guard, port)
}

#[test]
fn source_finishing_qualified_hot_journal_resumes_after_durable_recovery_and_real_exit() {
    for point in [
        FinishPoint::BeforeJournalRecovery,
        FinishPoint::JournalRecoveryIntentRecorded,
        FinishPoint::JournalRecovered,
    ] {
        let (fixture, store, guard, mut port) = hot_fixture();
        let immutable = frozen(&fixture);
        port.fail = Some(point);
        assert_eq!(
            store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
            Err(SourcePreparationError::AuthorityNotProven)
        );
        port.fail = None;
        let ready = store
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .unwrap();
        assert!(ready.preparation().journal_recovery().is_some());
        assert_preserved(&immutable);
        assert!(!fixture.0.join("userdb.sqlite3-journal").exists());
        let (fixture, store, guard, port) = hot_fixture();
        let immutable = frozen(&fixture);
        drop(guard);
        drop(store);
        exit_at(&fixture, point);
        resume(&fixture, port.request);
        assert_preserved(&immutable);
    }
}

#[test]
fn recovery_progress_persistence_preserves_interrupted_slots_at_every_checkpoint() {
    for point in CANCEL_POINTS {
        let point = FinishPoint::Write(FinishPhase::Finishing, point);
        for process_exit in [false, true] {
            let (fixture, store, guard, mut port) = hot_fixture();
            let immutable = frozen(&fixture);
            if process_exit {
                drop(guard);
                drop(store);
                exit_with_recovery(&fixture, point, true);
            } else {
                port.fail = Some(point);
                port.recovery_only = true;
                assert_eq!(
                    store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
                    Err(SourcePreparationError::AuthorityNotProven),
                    "{point:?}"
                );
                drop(guard);
            }
            assert_preserved(&immutable);
            resume(&fixture, port.request);
            assert_preserved(&immutable);
        }
    }
}

#[path = "source_finishing_rejection.rs"]
mod rejection;
