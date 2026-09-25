#![cfg(unix)]
//! Real private files/SHA-256/SQLite with synthetic old receipts and authority.
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_upgrade::{
    inspect_startup_gate, PreparationArchiveSlot as Slot, PreparationBinding,
    PreparationJournalStore, PreparationPhase, PreparationReceipt, ProductRelease,
    SourcePreparationCheckpoint as Point, SourcePreparationPort, UpgradeArtifactIdentity,
    UpgradeArtifactSlot as Artifact, UpgradeFailureCode, UpgradeProcessGuard, UpgradeReceipt,
    UpgradeReceiptStore, UpgradeState as State, VerifiedDataRoot,
};
use radishlex_ime_userdb::{SelectionEventDraft, UserDb};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher as Hasher;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const NEW: &str = "11111111111111111111111111111111";
const OLD: &str = "22222222222222222222222222222222";
const OUTER: &str = "33333333333333333333333333333333";
const STATE: &str = ".radishlex-upgrade-v1";
const HISTORY: &str = ".radishlex-upgrade-history-v1";
const FILES: [(Slot, &str); 5] = [
    (Slot::Receipt, "receipt.json"),
    (Slot::Snapshot, "source-snapshot.sqlite3"),
    (Slot::Candidate, "migration-candidate.sqlite3"),
    (Slot::Backup, "source-backup.sqlite3"),
    (Slot::Settings, "source-settings.json"),
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-preparation-history-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let fixture = Self(path);
        let mut db = UserDb::open(fixture.source()).unwrap();
        for (code, text) in [("shi", "时"), ("shan", "删")] {
            db.record_selection(SelectionEventDraft::new(
                "synthetic-history",
                code,
                text,
                0,
                2,
            ))
            .unwrap();
        }
        db.delete_term("shan", "删", None).unwrap();
        drop(db);
        put(
            &fixture.0.join("manager-settings.json"),
            b"synthetic-settings",
        );
        fixture
    }
    fn source(&self) -> PathBuf {
        self.0.join("userdb.sqlite3")
    }
    fn state(&self, name: &str) -> PathBuf {
        self.0.join(STATE).join(name)
    }
    fn history(&self, name: &str) -> PathBuf {
        self.0.join(HISTORY).join(OLD).join("data").join(name)
    }
    fn verified(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.0, fs::metadata(&self.0).unwrap().uid()).unwrap()
    }
    fn reload(&self) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let store = PreparationJournalStore::open_existing(self.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (store, guard)
    }
    fn legacy(&self, state: Option<State>) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let v1 = UpgradeReceiptStore::open(self.verified()).unwrap();
        let guard = v1.acquire_guard().unwrap();
        if let Some(state) = state {
            put(
                &self.state("source-snapshot.sqlite3"),
                b"synthetic-old-snapshot",
            );
            put(&self.state("source-settings.json"), b"synthetic-settings");
            let source = if state == State::Completed {
                put(
                    &self.state("source-backup.sqlite3"),
                    b"synthetic-old-source",
                );
                self.state("source-backup.sqlite3")
            } else {
                self.source()
            };
            let candidate = if state == State::Completed {
                self.source()
            } else {
                put(
                    &self.state("migration-candidate.sqlite3"),
                    b"synthetic-old-candidate",
                );
                self.state("migration-candidate.sqlite3")
            };
            let mut receipt = UpgradeReceipt::new(
                OLD,
                None,
                release(if state == State::Completed { 38 } else { 39 }),
                release(if state == State::Completed { 39 } else { 40 }),
                Some(9),
                9,
                vec![
                    v1.data_root_identity().clone(),
                    artifact(Artifact::SourceDatabase, &source),
                    artifact(
                        Artifact::SourceSettings,
                        &self.0.join("manager-settings.json"),
                    ),
                ],
            )
            .unwrap();
            receipt
                .record_artifact(artifact(
                    Artifact::BackupSettings,
                    &self.state("source-settings.json"),
                ))
                .unwrap();
            receipt.advance(State::Quiesced).unwrap();
            receipt
                .record_artifact(artifact(
                    Artifact::SnapshotDatabase,
                    &self.state("source-snapshot.sqlite3"),
                ))
                .unwrap();
            receipt.advance(State::SnapshotReady).unwrap();
            receipt
                .record_artifact(artifact(Artifact::CandidateDatabase, &candidate))
                .unwrap();
            receipt.advance(State::CandidateMigrated).unwrap();
            receipt.advance(State::CandidateVerified).unwrap();
            if state == State::AbortedPreserved {
                receipt
                    .abort_preserved(UpgradeFailureCode::InputMethodValidationFailed, false)
                    .unwrap();
            } else if state != State::CandidateVerified {
                receipt
                    .record_artifact(artifact(Artifact::BackupDatabase, &source))
                    .unwrap();
                receipt.advance(State::SwitchPrepared).unwrap();
                receipt.advance(State::Switched).unwrap();
                if state == State::Completed {
                    receipt.advance(State::PostSwitchVerified).unwrap();
                    receipt.advance(State::Completed).unwrap();
                } else {
                    receipt
                        .require_rollback(UpgradeFailureCode::PostSwitchValidationFailed)
                        .unwrap();
                    receipt.mark_rolled_back().unwrap();
                }
            }
            put(&self.state("receipt.json"), &receipt.encode().unwrap());
            // Real normal learning after the old terminal: old active length is
            // deliberately stale, but immutable private artifacts still match.
            let mut db = UserDb::open(self.source()).unwrap();
            for i in 0..80 {
                db.record_selection(SelectionEventDraft::new(
                    "synthetic-new-learning",
                    "xin",
                    format!("合成新词{i}"),
                    0,
                    2,
                ))
                .unwrap();
            }
        }
        (PreparationJournalStore::attach(v1, &guard).unwrap(), guard)
    }
    fn reserve(
        &self,
        state: Option<State>,
    ) -> (PreparationJournalStore, UpgradeProcessGuard, Port) {
        let (store, guard) = self.legacy(state);
        let inventory = store
            .capture_previous_inventory(&guard, NEW, Some(OUTER), state.map(|_| OLD), &Hasher)
            .unwrap();
        let identity = store
            .previous_inventory_identity(&guard, &inventory, &Hasher)
            .unwrap();
        let binding = PreparationBinding {
            operation_id: NEW.to_owned(),
            previous_install_operation_id: Some(OUTER.to_owned()),
            previous_data_operation_id: state.map(|_| OLD.to_owned()),
            source_release: release(39),
            target_release: release(41),
            source_product_sha256: "a".repeat(64),
            target_product_sha256: "b".repeat(64),
            previous_install_receipt_sha256: Some("c".repeat(64)),
            previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
            previous_inventory_sha256: identity.as_ref().map(|value| value.sha256.clone()),
            target_schema_version: 9,
            data_root: store.data_root_identity(),
            state_directory: store.state_directory_identity(),
        };
        let mut receipt = PreparationReceipt::new(
            binding.clone(),
            store.observe_source_family(&guard, &Hasher).unwrap(),
        )
        .unwrap();
        if let Some(identity) = identity {
            receipt.bind_previous_inventory(identity).unwrap();
        }
        store.persist(&guard, None, &receipt).unwrap();
        let mut port = Port::new(binding);
        store
            .prepare_userdb_source(&guard, &mut port, &Hasher)
            .unwrap();
        (store, guard, port)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn release(build: u64) -> ProductRelease {
    ProductRelease::new("26.7.1", build).unwrap()
}
fn artifact(slot: Artifact, path: &Path) -> UpgradeArtifactIdentity {
    let metadata = fs::metadata(path).unwrap();
    UpgradeArtifactIdentity::private_file(
        slot,
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.len(),
    )
    .unwrap()
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
struct Port {
    binding: PreparationBinding,
    fail: Option<Point>,
    exit: Option<Point>,
    seen: Vec<Point>,
    mutation: Option<(Point, Box<dyn FnOnce()>)>,
}
impl Port {
    fn new(binding: PreparationBinding) -> Self {
        Self {
            binding,
            fail: None,
            exit: None,
            seen: Vec::new(),
            mutation: None,
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
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.exit == Some(point) {
            std::process::exit(74);
        }
        self.fail != Some(point)
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}

#[test]
fn all_terminal_predecessors_and_first_upgrade_preserve_private_and_runtime_data() {
    for state in [
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ] {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(state);
        let directory = store.state_directory_identity();
        let source = fs::read(fixture.source()).unwrap();
        let settings = fs::read(fixture.0.join("manager-settings.json")).unwrap();
        let source_inode = fs::metadata(fixture.source()).unwrap().ino();
        let private: Vec<_> = FILES
            .iter()
            .filter_map(|(slot, name)| {
                fs::read(fixture.state(name)).ok().map(|bytes| {
                    (
                        *slot,
                        *name,
                        fs::metadata(fixture.state(name)).unwrap().ino(),
                        bytes,
                    )
                })
            })
            .collect();
        let record = store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        assert_eq!(record.phase(), PreparationPhase::PreviousArchived);
        assert_eq!(
            record.archived_slots(),
            private.iter().map(|item| item.0).collect::<Vec<_>>()
        );
        assert_eq!(store.state_directory_identity(), directory);
        assert_eq!(fs::metadata(fixture.source()).unwrap().ino(), source_inode);
        assert_eq!(fs::read(fixture.source()).unwrap(), source);
        assert_eq!(
            fs::read(fixture.0.join("manager-settings.json")).unwrap(),
            settings
        );
        for (_, name, inode, bytes) in private {
            assert!(!fixture.state(name).exists());
            assert_eq!(fs::read(fixture.history(name)).unwrap(), bytes);
            assert_eq!(fs::metadata(fixture.history(name)).unwrap().ino(), inode);
        }
        assert!(fixture.state("source-preparation.json").exists());
        assert!(fixture.state("preparation-snapshot.sqlite3").exists());
        assert_eq!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .unwrap(),
            record
        );
        drop(guard);
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
        assert!(UpgradeReceiptStore::open_existing(fixture.verified()).is_err());
        let (store, guard) = fixture.reload();
        assert_eq!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .unwrap(),
            record
        );
        UserDb::verify_prepared_source(
            fixture.source(),
            fixture.state("preparation-snapshot.sqlite3"),
        )
        .unwrap();
    }
}

fn archive_points() -> Vec<Point> {
    let mut points = vec![Point::ArchiveBegin];
    for slot in [
        Slot::Receipt,
        Slot::Snapshot,
        Slot::Candidate,
        Slot::Settings,
    ] {
        points.extend([
            Point::ArchiveBeforeFileSync(slot),
            Point::ArchiveFileSynced(slot),
            Point::ArchiveBeforeRename(slot),
            Point::ArchiveRenamed(slot),
            Point::ArchiveTargetSynced(slot),
            Point::ArchiveSourceSynced(slot),
            Point::ArchiveSlotRecorded(slot),
        ]);
    }
    points.push(Point::ArchiveCompleted);
    points
}

#[test]
fn authority_loss_at_every_archive_boundary_preserves_and_reloads_exactly() {
    for point in archive_points() {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(Some(State::RolledBack));
        port.fail = Some(point);
        assert!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .is_err(),
            "{point:?}"
        );
        assert!(fixture.state("source-preparation.json").exists());
        drop(guard);
        let (store, guard) = fixture.reload();
        port.fail = None;
        assert_eq!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .unwrap()
                .phase(),
            PreparationPhase::PreviousArchived
        );
    }
}

#[test]
fn process_exit_at_every_archive_boundary_reopens_with_new_guard() {
    for (index, point) in archive_points().into_iter().enumerate() {
        let fixture = Fixture::new();
        let (_, guard, mut port) = fixture.reserve(Some(State::RolledBack));
        drop(guard);
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "history_crash_child", "--ignored"])
            .env("RADISHLEX_HISTORY_TEST_ROOT", &fixture.0)
            .env("RADISHLEX_HISTORY_TEST_POINT", index.to_string())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(74), "{point:?}");
        let (store, guard) = fixture.reload();
        assert_eq!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .unwrap()
                .phase(),
            PreparationPhase::PreviousArchived
        );
        UserDb::verify_prepared_source(
            fixture.source(),
            fixture.state("preparation-snapshot.sqlite3"),
        )
        .unwrap();
    }
}
#[test]
#[ignore = "parent runs this in an isolated synthetic root"]
fn history_crash_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_HISTORY_TEST_ROOT").unwrap());
    let index: usize = std::env::var("RADISHLEX_HISTORY_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let owner = fs::metadata(&root).unwrap().uid();
    let store =
        PreparationJournalStore::open_existing(VerifiedDataRoot::verify(root, owner).unwrap())
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
    port.exit = Some(archive_points()[index]);
    store
        .archive_previous_upgrade(&guard, &mut port, &Hasher)
        .unwrap();
    panic!("exit point was not reached");
}

#[test]
fn inventory_rejects_nonterminal_wrong_predecessor_duplicate_and_unowned_slots() {
    for case in 0..7 {
        let fixture = Fixture::new();
        let (store, guard) = fixture.legacy(Some(if case == 0 {
            State::CandidateVerified
        } else {
            State::RolledBack
        }));
        let operation = if case == 1 { OLD } else { NEW };
        let previous = if case == 2 {
            Some(OUTER)
        } else if case == 3 {
            None
        } else {
            Some(OLD)
        };
        match case {
            4 => {
                fs::remove_file(fixture.state("source-snapshot.sqlite3")).unwrap();
            }
            5 => {
                put(&fixture.state("source-backup.sqlite3"), b"unowned");
            }
            6 => {
                let path = fixture.state("source-settings.json");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("saved-old-settings")).unwrap();
                put(&path, &bytes);
            }
            _ => {}
        }
        assert!(
            store
                .capture_previous_inventory(&guard, operation, Some(OUTER), previous, &Hasher)
                .is_err(),
            "case {case}"
        );
        assert!(!fixture.0.join(HISTORY).exists());
        assert!(fixture.state("receipt.json").exists());
    }
}

#[test]
fn archive_rejects_drift_conflicts_missing_slots_and_history_replacement() {
    for case in 0..13 {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(Some(State::RolledBack));
        let marker = fs::read(fixture.state("source-preparation.json")).unwrap();
        let source = fixture.state("source-snapshot.sqlite3");
        let target = fixture.history("source-snapshot.sqlite3");
        match case {
            0 => {
                let bytes = fs::read(&source).unwrap();
                fs::rename(&source, fixture.0.join("old-snapshot")).unwrap();
                put(&source, &bytes);
            }
            1 => {
                fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
            }
            2 => {
                fs::hard_link(&source, fixture.0.join("snapshot-link")).unwrap();
            }
            3 => {
                fs::write(&source, b"synthetic-old-SNAPSHOT").unwrap();
            }
            4 => {
                put(&target, b"target-conflict");
            }
            5 => {
                fs::remove_file(&source).unwrap();
            }
            6 => {
                fs::rename(&source, &target).unwrap();
            } // out-of-order movement
            7 => {
                fs::rename(&source, fixture.0.join("old-snapshot")).unwrap();
                symlink(fixture.0.join("old-snapshot"), &source).unwrap();
            }
            8 => {
                let path = fixture.0.join(HISTORY).join(OLD).join("inventory.json");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("old-inventory")).unwrap();
                put(&path, &bytes);
            }
            9 => {
                let path = fixture.0.join(HISTORY).join(OLD).join("data");
                fs::rename(&path, fixture.0.join("old-history-dir")).unwrap();
                DirBuilder::new().mode(0o700).create(&path).unwrap();
            }
            10 => {
                put(&fixture.history("unknown"), b"unknown");
            }
            11 => {
                fs::write(fixture.source(), b"invalid-source").unwrap();
            }
            12 => {
                let path = fixture.state("receipt.json");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("old-receipt")).unwrap();
                put(&path, &bytes);
            }
            _ => unreachable!(),
        }
        assert!(
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .is_err(),
            "case {case}"
        );
        assert_eq!(
            fs::read(fixture.state("source-preparation.json")).unwrap(),
            marker
        );
        assert!(fixture.state("source-preparation.json").exists());
    }
}

#[test]
fn mutations_during_authority_callback_are_rechecked_before_rename() {
    let fixture = Fixture::new();
    let (store, guard, mut port) = fixture.reserve(Some(State::RolledBack));
    let target = fixture.history("receipt.json");
    let copy = target.clone();
    port.mutation = Some((
        Point::ArchiveBeforeRename(Slot::Receipt),
        Box::new(move || put(&copy, b"do-not-overwrite")),
    ));
    assert!(store
        .archive_previous_upgrade(&guard, &mut port, &Hasher)
        .is_err());
    assert_eq!(fs::read(target).unwrap(), b"do-not-overwrite");
    assert!(fixture.state("receipt.json").exists());
}

#[test]
fn interrupted_progress_write_keeps_moved_receipt_blocked_and_unmodified() {
    let fixture = Fixture::new();
    let (store, guard, mut port) = fixture.reserve(Some(State::RolledBack));
    let old_receipt = fs::read(fixture.state("receipt.json")).unwrap();
    let marker = fs::read(fixture.state("source-preparation.json")).unwrap();
    let temporary = fixture.state("source-preparation.json.tmp");
    let copy = temporary.clone();
    port.mutation = Some((
        Point::ArchiveSourceSynced(Slot::Receipt),
        Box::new(move || put(&copy, b"interrupted-record")),
    ));
    assert!(store
        .archive_previous_upgrade(&guard, &mut port, &Hasher)
        .is_err());
    assert!(!fixture.state("receipt.json").exists());
    assert_eq!(
        fs::read(fixture.history("receipt.json")).unwrap(),
        old_receipt
    );
    assert_eq!(
        fs::read(fixture.state("source-preparation.json")).unwrap(),
        marker
    );
    drop(guard);
    let (store, guard) = fixture.reload();
    assert!(store
        .archive_previous_upgrade(&guard, &mut port, &Hasher)
        .is_err());
    assert_eq!(fs::read(temporary).unwrap(), b"interrupted-record");
    assert_eq!(
        fs::read(fixture.history("receipt.json")).unwrap(),
        old_receipt
    );
}

#[test]
fn existing_history_or_orphan_artifacts_are_never_treated_as_first_upgrade() {
    for with_receipt in [false, true] {
        let fixture = Fixture::new();
        let (store, guard) = fixture.legacy(with_receipt.then_some(State::Completed));
        let history = fixture.0.join(HISTORY);
        DirBuilder::new().mode(0o700).create(&history).unwrap();
        if with_receipt {
            DirBuilder::new()
                .mode(0o700)
                .create(history.join(OLD))
                .unwrap();
            put(&history.join(OLD).join("keep"), b"existing-history");
        }
        assert!(store
            .capture_previous_inventory(
                &guard,
                NEW,
                Some(OUTER),
                with_receipt.then_some(OLD),
                &Hasher
            )
            .is_err());
        if with_receipt {
            assert_eq!(
                fs::read(history.join(OLD).join("keep")).unwrap(),
                b"existing-history"
            );
        }
    }
}

#[test]
fn captured_inventory_cannot_be_rebound_to_another_outer_or_source() {
    for wrong_outer in [false, true] {
        let fixture = Fixture::new();
        let (store, guard) = fixture.legacy(Some(State::Completed));
        let inventory = store
            .capture_previous_inventory(&guard, NEW, Some(OUTER), Some(OLD), &Hasher)
            .unwrap();
        let identity = store
            .previous_inventory_identity(&guard, &inventory, &Hasher)
            .unwrap()
            .unwrap();
        let binding = PreparationBinding {
            operation_id: NEW.to_owned(),
            previous_install_operation_id: Some(if wrong_outer { OLD } else { OUTER }.to_owned()),
            previous_data_operation_id: Some(OLD.to_owned()),
            source_release: release(if wrong_outer { 39 } else { 38 }),
            target_release: release(41),
            source_product_sha256: "a".repeat(64),
            target_product_sha256: "b".repeat(64),
            previous_install_receipt_sha256: Some("c".repeat(64)),
            previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
            previous_inventory_sha256: Some(identity.sha256.clone()),
            target_schema_version: 9,
            data_root: store.data_root_identity(),
            state_directory: store.state_directory_identity(),
        };
        let mut record = PreparationReceipt::new(
            binding.clone(),
            store.observe_source_family(&guard, &Hasher).unwrap(),
        )
        .unwrap();
        record.bind_previous_inventory(identity).unwrap();
        store.persist(&guard, None, &record).unwrap();
        let mut port = Port::new(binding);
        assert!(store
            .prepare_userdb_source(&guard, &mut port, &Hasher)
            .is_err());
        assert!(store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .is_err());
        assert!(fixture.state("receipt.json").exists());
    }
}
