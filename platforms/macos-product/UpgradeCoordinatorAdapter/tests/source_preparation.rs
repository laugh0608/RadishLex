#![cfg(unix)]

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_upgrade::{
    inspect_startup_gate, PreparationBinding, PreparationHasher, PreparationJournalStore,
    PreparationPhase, PreparationReceipt, ProductRelease, SourcePreparationCheckpoint as Point,
    SourcePreparationError, SourcePreparationPort, SourcePreparationSpaceBudget,
    UpgradeProcessGuard, UpgradeReceiptStore, VerifiedDataRoot,
};
use radishlex_ime_userdb::{SelectionEventDraft, UserDb};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const SNAPSHOT: &str = ".radishlex-upgrade-v1/preparation-snapshot.sqlite3";
const POINTS: [Point; 14] = [
    Point::Begin,
    Point::SnapshotEstimated,
    Point::SnapshotCreated,
    Point::SnapshotCopied,
    Point::SnapshotSynced,
    Point::SnapshotRenamed,
    Point::SnapshotDirectorySynced,
    Point::SnapshotRecorded,
    Point::MaintenanceIntentRecorded,
    Point::BeforeMaintenance,
    Point::SourceMaintained,
    Point::SourceFileSynced,
    Point::SourceSynced,
    Point::SourceRecorded,
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-source-preparation-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
    fn source(&self) -> PathBuf {
        self.0.join("userdb.sqlite3")
    }
    fn verified(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.0, fs::metadata(&self.0).unwrap().uid()).unwrap()
    }
    fn populate(&self) {
        populate(&self.source());
    }
    fn reserve(&self) -> (PreparationJournalStore, UpgradeProcessGuard, Port) {
        let v1 = UpgradeReceiptStore::open(self.verified()).unwrap();
        let guard = v1.acquire_guard().unwrap();
        let store = PreparationJournalStore::attach(v1, &guard).unwrap();
        let family = store
            .observe_source_family(&guard, &MacOsPreparationHasher)
            .unwrap();
        let binding = PreparationBinding {
            operation_id: "11111111111111111111111111111111".to_owned(),
            previous_install_operation_id: None,
            previous_data_operation_id: None,
            source_release: ProductRelease::new("26.7.1", 39).unwrap(),
            target_release: ProductRelease::new("26.7.1", 41).unwrap(),
            source_product_sha256: "a".repeat(64),
            target_product_sha256: "b".repeat(64),
            previous_install_receipt_sha256: None,
            previous_data_receipt_sha256: None,
            previous_inventory_sha256: None,
            target_schema_version: 9,
            data_root: store.data_root_identity(),
            state_directory: store.state_directory_identity(),
        };
        store
            .persist(
                &guard,
                None,
                &PreparationReceipt::new(binding.clone(), family).unwrap(),
            )
            .unwrap();
        (store, guard, Port::new(binding))
    }
    fn reload(&self) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let store = PreparationJournalStore::open_existing(self.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (store, guard)
    }
    fn standalone(&self) {
        let backup = self.0.join("synthetic-before-preparation.sqlite3");
        put(&backup, b"");
        UserDb::create_consistent_snapshot(self.source(), &backup).unwrap();
        UserDb::prepare_source_for_upgrade(self.source(), &backup).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn populate(path: &Path) -> UserDb {
    let mut db = UserDb::open(path).unwrap();
    for (code, text) in [("shi", "时"), ("shan", "删")] {
        db.record_selection(SelectionEventDraft::new(
            "synthetic-preparation",
            code,
            text,
            0,
            2,
        ))
        .unwrap();
    }
    db.delete_term("shan", "删", None).unwrap();
    db
}
fn put(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

// A synthetic authority port, not real outer/program qualification. Real
// filesystem evidence, SHA-256 and SQLite operations are used by these tests.
struct Port {
    binding: PreparationBinding,
    fail: Option<Point>,
    exit: Option<Point>,
    capacity: u64,
    seen: Vec<Point>,
}
impl Port {
    fn new(binding: PreparationBinding) -> Self {
        Self {
            binding,
            fail: None,
            exit: None,
            capacity: u64::MAX,
            seen: Vec::new(),
        }
    }
}
impl SourcePreparationPort for Port {
    fn confirm_authority_and_quiescence(
        &mut self,
        record: &PreparationReceipt,
        point: Point,
    ) -> bool {
        assert_eq!(record.binding(), &self.binding);
        self.seen.push(point);
        if self.exit == Some(point) {
            std::process::exit(74);
        }
        self.fail != Some(point)
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(self.capacity)
    }
}

#[test]
fn sha256_uses_complete_stream_and_propagates_read_errors() {
    for (input, expected) in [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
    ] {
        let hash = MacOsPreparationHasher
            .sha256(&mut input.as_bytes())
            .unwrap();
        assert_eq!(
            hash.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    }
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Other.into())
        }
    }
    assert!(MacOsPreparationHasher.sha256(&mut Broken).is_err());
}

#[test]
fn wal_and_delete_sources_keep_learning_tombstones_inode_and_protected_snapshot() {
    for standalone in [false, true] {
        let fixture = Fixture::new();
        fixture.populate();
        if standalone {
            fixture.standalone();
        }
        let inode = fs::metadata(fixture.source()).unwrap().ino();
        let (store, guard, mut port) = fixture.reserve();
        let state = store.state_directory_identity();
        let record = store
            .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
            .unwrap();
        assert_eq!(record.phase(), PreparationPhase::SourcePrepared);
        assert_eq!(fs::metadata(fixture.source()).unwrap().ino(), inode);
        assert_eq!(store.state_directory_identity(), state);
        for suffix in ["-wal", "-shm", "-journal"] {
            assert!(!fixture.0.join(format!("userdb.sqlite3{suffix}")).exists());
        }
        assert_eq!(
            UserDb::verify_prepared_source(fixture.source(), fixture.0.join(SNAPSHOT)).unwrap(),
            9
        );
        let protected = fs::read(fixture.0.join(SNAPSHOT)).unwrap();
        assert_eq!(
            store
                .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
                .unwrap(),
            record
        );
        assert_eq!(fs::read(fixture.0.join(SNAPSHOT)).unwrap(), protected);
        drop(guard);
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
        // Only the fixture reads via normal runtime; no coordinator startup
        // release is claimed. Normal learning must still enable WAL.
        let db = UserDb::open(fixture.source()).unwrap();
        assert_eq!(db.list_active_terms().unwrap().len(), 1);
        assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
        assert!(fixture.0.join("userdb.sqlite3-wal").exists());
    }
}

#[test]
fn each_quiescence_failure_reloads_or_preserves_an_unbound_snapshot() {
    for point in POINTS {
        let fixture = Fixture::new();
        fixture.populate();
        let original = fs::read(fixture.source()).unwrap();
        let (store, guard, mut port) = fixture.reserve();
        port.fail = Some(point);
        assert_eq!(
            store.prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher),
            Err(SourcePreparationError::AuthorityNotProven),
            "{point:?}"
        );
        if !matches!(
            point,
            Point::SourceMaintained
                | Point::SourceFileSynced
                | Point::SourceSynced
                | Point::SourceRecorded
        ) {
            assert_eq!(
                fs::read(fixture.source()).unwrap(),
                original,
                "source mutated before durable maintenance intent: {point:?}"
            );
        }
        drop(guard);
        drop(store);
        port.fail = None;
        let (store, guard) = fixture.reload();
        let retry = store.prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher);
        if matches!(
            point,
            Point::SnapshotCreated
                | Point::SnapshotCopied
                | Point::SnapshotSynced
                | Point::SnapshotRenamed
                | Point::SnapshotDirectorySynced
        ) {
            assert!(retry.is_err(), "orphan was adopted at {point:?}");
            assert!(
                fixture.0.join(SNAPSHOT).exists()
                    || fixture.0.join(format!("{SNAPSHOT}.tmp")).exists()
            );
        } else {
            assert_eq!(
                retry.unwrap().phase(),
                PreparationPhase::SourcePrepared,
                "{point:?}"
            );
        }
    }
}

#[test]
fn missing_authority_capacity_or_safe_source_never_starts_maintenance() {
    for kind in [
        "authority",
        "capacity",
        "replaced-source",
        "journal",
        "hardlink",
    ] {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, mut port) = fixture.reserve();
        let original = fs::read(fixture.source()).unwrap();
        match kind {
            "authority" => port.fail = Some(Point::Begin),
            "capacity" => port.capacity = 0,
            "replaced-source" => {
                fs::rename(fixture.source(), fixture.0.join("original")).unwrap();
                put(&fixture.source(), &original);
            }
            "journal" => put(
                &fixture.0.join("userdb.sqlite3-journal"),
                b"unqualified synthetic journal",
            ),
            "hardlink" => fs::hard_link(fixture.source(), fixture.0.join("alias")).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            store
                .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
                .is_err(),
            "{kind}"
        );
        assert_eq!(fs::read(fixture.source()).unwrap(), original);
        assert!(!fixture.0.join(SNAPSHOT).exists());
        assert_eq!(
            store.load_guarded(&guard).unwrap().unwrap().phase(),
            PreparationPhase::Reserved
        );
    }
    assert!(SourcePreparationSpaceBudget::evaluate(u64::MAX, 0, u64::MAX).is_err());
    assert!(SourcePreparationSpaceBudget::evaluate(0, u64::MAX, u64::MAX).is_err());
    let exact = 6 * 4096 + 1024 + 64 * 1024 * 1024;
    assert!(SourcePreparationSpaceBudget::evaluate(4096, 1024, exact - 1).is_err());
    assert_eq!(
        SourcePreparationSpaceBudget::evaluate(4096, 1024, exact)
            .unwrap()
            .required_available_bytes,
        exact
    );
}

#[test]
fn intent_rejects_changed_snapshot_or_business_content_without_restoring_source() {
    for snapshot_changed in [false, true] {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, mut port) = fixture.reserve();
        port.fail = Some(Point::BeforeMaintenance);
        assert!(store
            .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
            .is_err());
        assert_eq!(
            store.load_guarded(&guard).unwrap().unwrap().phase(),
            PreparationPhase::MaintenanceIntent
        );
        if snapshot_changed {
            let path = fixture.0.join(SNAPSHOT);
            let mut bytes = fs::read(&path).unwrap();
            bytes[100] ^= 1;
            fs::write(path, bytes).unwrap();
        } else {
            let mut db = UserDb::open(fixture.source()).unwrap();
            db.record_selection(SelectionEventDraft::new(
                "synthetic-change",
                "xin",
                "新",
                0,
                2,
            ))
            .unwrap();
        }
        let source_before_retry = fs::read(fixture.source()).unwrap();
        port.fail = None;
        assert!(store
            .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
            .is_err());
        assert_eq!(fs::read(fixture.source()).unwrap(), source_before_retry);
        assert_eq!(
            store.load_guarded(&guard).unwrap().unwrap().phase(),
            PreparationPhase::MaintenanceIntent
        );
    }
}

#[test]
fn prepared_replay_refuses_normal_runtime_wal_or_new_learning() {
    let fixture = Fixture::new();
    fixture.populate();
    let (store, guard, mut port) = fixture.reserve();
    store
        .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
        .unwrap();
    let mut db = UserDb::open(fixture.source()).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-change",
        "xin",
        "新",
        0,
        2,
    ))
    .unwrap();
    assert!(store
        .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
        .is_err());
    assert_eq!(db.list_active_terms().unwrap().len(), 2);
}

#[test]
fn recorded_snapshot_schema_must_match_before_source_write() {
    let fixture = Fixture::new();
    fixture.populate();
    let (store, guard, mut port) = fixture.reserve();
    port.fail = Some(Point::BeforeMaintenance);
    assert!(store
        .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
        .is_err());
    let before = fs::read(fixture.source()).unwrap();
    let record = store.load_guarded(&guard).unwrap().unwrap();
    let mut value = serde_json::to_value(record).unwrap();
    value["snapshot"]["schema_version"] = serde_json::json!(8);
    let forged: PreparationReceipt = serde_json::from_value(value).unwrap();
    fs::write(
        fixture
            .0
            .join(".radishlex-upgrade-v1/source-preparation.json"),
        forged.encode().unwrap(),
    )
    .unwrap();
    port.fail = None;
    assert_eq!(
        store.prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher),
        Err(SourcePreparationError::EvidenceChanged)
    );
    assert_eq!(fs::read(fixture.source()).unwrap(), before);
}

#[test]
fn empty_schema_zero_is_prepared_without_creating_or_migrating_business_tables() {
    let fixture = Fixture::new();
    put(&fixture.source(), b"");
    let (store, guard, mut port) = fixture.reserve();
    let record = store
        .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
        .unwrap();
    assert_eq!(record.snapshot_schema_version(), Some(0));
    assert_eq!(
        UserDb::verify_prepared_source(fixture.source(), fixture.0.join(SNAPSHOT)).unwrap(),
        0
    );
    assert_eq!(record.initial_source().database.byte_len, 0);
    assert_eq!(
        record.prepared_source().unwrap().inode,
        record.initial_source().database.inode
    );
}

#[test]
fn drift_during_authority_checks_never_advances_to_prepared() {
    struct Drift {
        inner: Port,
        point: Point,
        path: PathBuf,
        fired: bool,
    }
    impl SourcePreparationPort for Drift {
        fn confirm_authority_and_quiescence(
            &mut self,
            record: &PreparationReceipt,
            point: Point,
        ) -> bool {
            let allowed = self.inner.confirm_authority_and_quiescence(record, point);
            if point == self.point && !self.fired {
                let mut bytes = fs::read(&self.path).unwrap();
                if bytes.is_empty() {
                    bytes.push(1);
                } else {
                    bytes[0] ^= 1;
                }
                fs::write(&self.path, bytes).unwrap();
                self.fired = true;
            }
            allowed
        }
        fn available_bytes(&mut self) -> Option<u64> {
            self.inner.available_bytes()
        }
    }
    for (point, relative) in [
        (
            Point::SnapshotCreated,
            ".radishlex-upgrade-v1/preparation-snapshot.sqlite3.tmp",
        ),
        (
            Point::SnapshotSynced,
            ".radishlex-upgrade-v1/preparation-snapshot.sqlite3.tmp",
        ),
        (Point::SnapshotRenamed, SNAPSHOT),
        (Point::BeforeMaintenance, SNAPSHOT),
        (Point::SourceMaintained, "userdb.sqlite3"),
        (Point::SourceFileSynced, "userdb.sqlite3"),
        (Point::SourceSynced, "userdb.sqlite3"),
    ] {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, inner) = fixture.reserve();
        let mut port = Drift {
            inner,
            point,
            path: fixture.0.join(relative),
            fired: false,
        };
        assert!(
            store
                .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
                .is_err(),
            "{point:?}"
        );
        assert!(port.fired);
        if let Ok(Some(record)) = store.load_guarded(&guard) {
            assert!(record.phase() < PreparationPhase::SourcePrepared);
        }
    }
}

#[test]
fn unclosed_wal_and_process_exit_boundaries_use_new_guards_and_exact_records() {
    for point in [
        Point::SnapshotCreated,
        Point::SnapshotRenamed,
        Point::SnapshotDirectorySynced,
        Point::SnapshotRecorded,
        Point::MaintenanceIntentRecorded,
        Point::SourceMaintained,
        Point::SourceFileSynced,
        Point::SourceSynced,
        Point::SourceRecorded,
    ] {
        let fixture = Fixture::new();
        let status = child(&fixture.0, "wal");
        assert_eq!(status.code(), Some(75));
        assert!(
            fs::metadata(fixture.0.join("userdb.sqlite3-wal"))
                .unwrap()
                .len()
                > 0
        );
        let (store, guard, mut port) = fixture.reserve();
        drop(guard);
        drop(store);
        let status = child(&fixture.0, &format!("{point:?}"));
        assert_eq!(status.code(), Some(74));
        let (store, guard) = fixture.reload();
        let result = store.prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher);
        if matches!(
            point,
            Point::SnapshotCreated | Point::SnapshotRenamed | Point::SnapshotDirectorySynced
        ) {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().phase(), PreparationPhase::SourcePrepared);
            let db = UserDb::open(fixture.source()).unwrap();
            assert_eq!(db.list_active_terms().unwrap().len(), 1);
            assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
        }
    }
}

fn child(root: &Path, point: &str) -> std::process::ExitStatus {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "source_preparation_child", "--ignored"])
        .env("RADISHLEX_SOURCE_PREPARATION_TEST_ROOT", root)
        .env("RADISHLEX_SOURCE_PREPARATION_TEST_POINT", point)
        .status()
        .unwrap()
}

#[test]
#[ignore = "entered only by the parent process-exit matrix"]
fn source_preparation_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_SOURCE_PREPARATION_TEST_ROOT").unwrap());
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
    let point = std::env::var("RADISHLEX_SOURCE_PREPARATION_TEST_POINT").unwrap();
    if point == "wal" {
        let _db = populate(&root.join("userdb.sqlite3"));
        std::process::exit(75);
    }
    let owner = fs::metadata(&root).unwrap().uid();
    let store =
        PreparationJournalStore::open_existing(VerifiedDataRoot::verify(&root, owner).unwrap())
            .unwrap();
    let guard = store.acquire_guard().unwrap();
    let mut port = Port::new(
        store
            .load_guarded(&guard)
            .unwrap()
            .unwrap()
            .binding()
            .clone(),
    );
    port.exit = POINTS
        .into_iter()
        .find(|value| format!("{value:?}") == point);
    assert!(port.exit.is_some());
    store
        .prepare_userdb_source(&guard, &mut port, &MacOsPreparationHasher)
        .unwrap();
    panic!("exit point not reached");
}
