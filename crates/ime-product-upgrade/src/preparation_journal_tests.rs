use std::cell::RefCell;
use std::os::unix::fs::symlink;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{PreparationBinding, PreparationFamily, PreparationFileIdentity, ProductRelease};

use super::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const POINTS: [FaultPoint; 9] = [
    FaultPoint::Created,
    FaultPoint::Written,
    FaultPoint::BeforeFileSync,
    FaultPoint::FileSynced,
    FaultPoint::BeforeRename,
    FaultPoint::Renamed,
    FaultPoint::BeforeDirectorySync,
    FaultPoint::DirectorySynced,
    FaultPoint::ReadBack,
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-preparation-journal-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
    fn verified(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.0, fs::metadata(&self.0).unwrap().uid()).unwrap()
    }
    fn start(&self) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let store = UpgradeReceiptStore::open(self.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (
            PreparationJournalStore::attach(store, &guard).unwrap(),
            guard,
        )
    }
    fn reload(&self) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let store = PreparationJournalStore::open_existing(self.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (store, guard)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// Synthetic metadata exercises journal contracts; it does not certify a real
// database, product identity, outer guard or archive relationship.
fn record(store: &PreparationJournalStore) -> PreparationReceipt {
    let root = store.data_root_identity();
    PreparationReceipt::new(
        PreparationBinding {
            operation_id: "11111111111111111111111111111111".to_owned(),
            previous_install_operation_id: Some("22222222222222222222222222222222".to_owned()),
            previous_data_operation_id: Some("33333333333333333333333333333333".to_owned()),
            source_release: ProductRelease::new("26.7.1", 39).unwrap(),
            target_release: ProductRelease::new("26.7.1", 41).unwrap(),
            source_product_sha256: "a".repeat(64),
            target_product_sha256: "b".repeat(64),
            previous_install_receipt_sha256: Some("c".repeat(64)),
            previous_data_receipt_sha256: Some("d".repeat(64)),
            previous_inventory_sha256: Some("e".repeat(64)),
            target_schema_version: 9,
            data_root: root.clone(),
            state_directory: store.state_directory_identity(),
        },
        PreparationFamily {
            database: PreparationFileIdentity {
                device_id: root.device_id,
                inode: 10,
                owner_id: root.owner_id,
                mode: 0o600,
                link_count: 1,
                byte_len: 4096,
                sha256: "a".repeat(64),
            },
            wal: None,
            shm: None,
            journal: None,
        },
    )
    .unwrap()
}

fn ready(reserved: &PreparationReceipt) -> PreparationReceipt {
    let mut next = reserved.clone();
    let mut snapshot = next.initial_source().database.clone();
    snapshot.inode = 11;
    next.record_snapshot(snapshot, next.initial_source().clone(), 9)
        .unwrap();
    next
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

struct StopAt(FaultPoint);
impl FaultInjector for StopAt {
    fn checkpoint(&self, point: FaultPoint) -> Result<()> {
        if point == self.0 {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }
}

#[test]
fn record_roundtrip_keeps_guard_directory_and_old_reader_blocking() {
    let fixture = Fixture::new();
    let (store, guard) = fixture.start();
    let directory = store.state_directory_identity();
    let reserved = record(&store);
    assert_eq!(store.load_guarded(&guard).unwrap(), None);
    store.persist(&guard, None, &reserved).unwrap();
    let inode = fs::metadata(store.path(JOURNAL)).unwrap().ino();
    store.persist(&guard, None, &reserved).unwrap();
    assert_eq!(fs::metadata(store.path(JOURNAL)).unwrap().ino(), inode);
    let snapshot = ready(&reserved);
    store.persist(&guard, Some(&reserved), &snapshot).unwrap();
    let mut intent = snapshot.clone();
    intent.begin_maintenance().unwrap();
    store.persist(&guard, Some(&snapshot), &intent).unwrap();
    let mut prepared = intent.clone();
    prepared
        .record_prepared_source(prepared.initial_source().database.clone())
        .unwrap();
    store.persist(&guard, Some(&intent), &prepared).unwrap();
    let mut archived = prepared.clone();
    archived.record_previous_archive("e".repeat(64)).unwrap();
    store.persist(&guard, Some(&prepared), &archived).unwrap();
    let mut handed = archived.clone();
    let source = handed.prepared_source().unwrap();
    let data_receipt = UpgradeReceipt::new(
        handed.binding().operation_id.clone(),
        handed.binding().previous_data_operation_id.clone(),
        handed.binding().source_release.clone(),
        handed.binding().target_release.clone(),
        Some(9),
        9,
        vec![
            store.store.data_root_identity().clone(),
            UpgradeArtifactIdentity::private_file(
                UpgradeArtifactSlot::SourceDatabase,
                source.device_id,
                source.inode,
                source.owner_id,
                source.byte_len,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    handed
        .record_handoff(&data_receipt, "f".repeat(64))
        .unwrap();
    store.persist(&guard, Some(&archived), &handed).unwrap();
    assert_eq!(store.state_directory_identity(), directory);
    assert_eq!(store.load_guarded(&guard).unwrap(), Some(handed.clone()));
    assert!(store.acquire_guard().is_err());
    // Existing v1 APIs retain their whitelist, even with the matching guard.
    assert_eq!(
        store.store.load_guarded(&guard).unwrap_err().code(),
        UpgradeFilesystemErrorCode::UnexpectedStateObject
    );
    drop(guard);
    assert!(!inspect_startup_gate(&fixture.0, fixture.verified().expected_owner_id).is_allowed());
    assert!(UpgradeReceiptStore::open_existing(fixture.verified()).is_err());
    let (reloaded, guard) = fixture.reload();
    assert_eq!(reloaded.state_directory_identity(), directory);
    assert_eq!(reloaded.load_guarded(&guard).unwrap(), Some(handed));
}

#[test]
fn journal_never_rewrites_existing_v1_artifacts_or_unblocks_a_handoff() {
    let fixture = Fixture::new();
    let (store, guard) = fixture.start();
    let reserved = record(&store);
    // The journal is metadata storage, not an archive executor or v1 reader.
    // Keep synthetic payloads byte-for-byte, including sizes above 64 KiB.
    let mut originals = Vec::new();
    for name in [
        RECEIPT_FILE_NAME,
        SNAPSHOT_FILE_NAME,
        CANDIDATE_FILE_NAME,
        SOURCE_BACKUP_FILE_NAME,
        SETTINGS_BACKUP_FILE_NAME,
    ] {
        let bytes = vec![b'x'; 70 * 1024];
        put(&store.path(name), &bytes);
        originals.push((
            name,
            bytes,
            FileIdentity::from_metadata(&fs::metadata(store.path(name)).unwrap()),
        ));
    }
    store.persist(&guard, None, &reserved).unwrap();
    store
        .persist(&guard, Some(&reserved), &ready(&reserved))
        .unwrap();
    for (name, bytes, identity) in originals {
        assert_eq!(fs::read(store.path(name)).unwrap(), bytes);
        assert_eq!(
            FileIdentity::from_metadata(&fs::metadata(store.path(name)).unwrap()),
            identity
        );
    }
    assert!(store.path(JOURNAL).exists());
}

#[test]
fn existing_interrupted_v1_or_snapshot_write_blocks_journal_progress() {
    for name in [
        STAGED_RECEIPT_FILE_NAME,
        STAGED_SNAPSHOT_FILE_NAME,
        STAGED_CANDIDATE_FILE_NAME,
        STAGED_SETTINGS_BACKUP_FILE_NAME,
        STAGED_PREPARATION_SNAPSHOT,
    ] {
        let fixture = Fixture::new();
        let (store, guard) = fixture.start();
        let reserved = record(&store);
        store.persist(&guard, None, &reserved).unwrap();
        put(&store.path(name), b"interrupted synthetic write");
        assert_eq!(store.load_guarded(&guard), Err(Error::InterruptedWrite));
        assert_eq!(
            store.persist(&guard, Some(&reserved), &ready(&reserved)),
            Err(Error::InterruptedWrite)
        );
        assert_eq!(
            fs::read(store.path(JOURNAL)).unwrap(),
            reserved.encode().unwrap()
        );
        assert_eq!(
            fs::read(store.path(name)).unwrap(),
            b"interrupted synthetic write"
        );
    }
}

#[test]
fn every_write_boundary_reloads_exact_successor_or_preserves_blocking_temp() {
    for update in [false, true] {
        for (index, point) in POINTS.into_iter().enumerate() {
            let fixture = Fixture::new();
            let (store, guard) = fixture.start();
            let reserved = record(&store);
            let next = if update {
                store.persist(&guard, None, &reserved).unwrap();
                ready(&reserved)
            } else {
                reserved.clone()
            };
            let previous = update.then_some(&reserved);
            assert_eq!(
                store.persist_with_faults(&guard, previous, &next, &StopAt(point)),
                Err(Error::Io),
                "{point:?}"
            );
            let marker = store.path(JOURNAL);
            let staged = store.path(STAGED_JOURNAL);
            let preserved = fs::read(&staged).ok();
            drop(guard);
            drop(store);
            let (reloaded, guard) = fixture.reload();
            if index < 5 {
                assert_eq!(reloaded.load_guarded(&guard), Err(Error::InterruptedWrite));
                assert_eq!(
                    reloaded.persist(&guard, previous, &next),
                    Err(Error::InterruptedWrite)
                );
                assert_eq!(fs::read(&staged).ok(), preserved);
                assert_eq!(
                    fs::read(&marker).ok(),
                    update.then(|| reserved.encode().unwrap())
                );
            } else {
                assert_eq!(reloaded.load_guarded(&guard).unwrap(), Some(next.clone()));
                let identity = fs::metadata(&marker).unwrap().ino();
                reloaded.persist(&guard, previous, &next).unwrap();
                assert_eq!(fs::metadata(marker).unwrap().ino(), identity);
                assert!(!staged.exists());
            }
        }
    }
}

#[test]
fn replay_requires_file_and_directory_sync_even_when_bytes_already_match() {
    let fixture = Fixture::new();
    let (store, guard) = fixture.start();
    let reserved = record(&store);
    store.persist(&guard, None, &reserved).unwrap();
    for point in [FaultPoint::BeforeFileSync, FaultPoint::BeforeDirectorySync] {
        assert_eq!(
            store.persist_with_faults(&guard, None, &reserved, &StopAt(point)),
            Err(Error::Io)
        );
        assert_eq!(store.load_guarded(&guard).unwrap(), Some(reserved.clone()));
    }
}

#[test]
fn rejects_wrong_guard_stale_predecessor_skipped_phase_and_cross_operation() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let (store, guard) = fixture.start();
    let (_, other_guard) = other.start();
    let reserved = record(&store);
    assert_eq!(
        store.persist(&other_guard, None, &reserved),
        Err(Error::IdentityChanged)
    );
    let snapshot = ready(&reserved);
    assert_eq!(
        store.persist(&guard, None, &snapshot),
        Err(Error::InvalidReplacement)
    );
    store.persist(&guard, None, &reserved).unwrap();
    let mut intent = snapshot.clone();
    intent.begin_maintenance().unwrap();
    assert_eq!(
        store.persist(&guard, Some(&reserved), &intent),
        Err(Error::InvalidReplacement)
    );
    assert_eq!(
        store.persist(&guard, Some(&snapshot), &intent),
        Err(Error::InvalidReplacement)
    );
    let mut binding = reserved.binding().clone();
    binding.operation_id = "44444444444444444444444444444444".to_owned();
    let changed = PreparationReceipt::new(binding, reserved.initial_source().clone()).unwrap();
    assert_eq!(
        store.persist(&guard, None, &changed),
        Err(Error::InvalidReplacement)
    );
    assert_eq!(store.load_guarded(&guard).unwrap(), Some(reserved));
}

#[test]
fn strict_parsing_bounds_unknown_files_and_orphans_are_preserved() {
    let fixture = Fixture::new();
    let (store, guard) = fixture.start();
    let reserved = record(&store);
    let valid = reserved.encode().unwrap();
    let invalid = [
        b"{}\n".to_vec(),
        [b" ".as_slice(), &valid].concat(),
        String::from_utf8(valid.clone())
            .unwrap()
            .replacen("\"phase\":", "\"unknown\":true,\"phase\":", 1)
            .into_bytes(),
        String::from_utf8(valid.clone())
            .unwrap()
            .replacen("\"phase\":", "\"phase\":\"reserved\",\"phase\":", 1)
            .into_bytes(),
        String::from_utf8(valid.clone())
            .unwrap()
            .replace(
                "radishlex-source-preparation-v1",
                "radishlex-source-preparation-v2",
            )
            .into_bytes(),
        vec![b' '; MAX_PREPARATION_RECEIPT_BYTES + 1],
    ];
    for bytes in invalid {
        put(&store.path(JOURNAL), &bytes);
        assert_eq!(store.load_guarded(&guard), Err(Error::InvalidRecord));
        assert!(store.persist(&guard, None, &reserved).is_err());
        assert_eq!(fs::read(store.path(JOURNAL)).unwrap(), bytes);
        fs::remove_file(store.path(JOURNAL)).unwrap();
    }
    for name in [
        "terminal-release.json",
        "unknown",
        "source-preparation.json.bak",
    ] {
        put(&store.path(name), b"synthetic\n");
        assert_eq!(store.load_guarded(&guard), Err(Error::UnexpectedObject));
        fs::remove_file(store.path(name)).unwrap();
    }
    put(
        &store.path(PREPARATION_SNAPSHOT),
        b"unbound synthetic snapshot",
    );
    assert_eq!(store.load_guarded(&guard), Err(Error::UnexpectedObject));
    assert!(store.persist(&guard, None, &reserved).is_err());
}

#[test]
fn unsafe_marker_and_directory_identity_changes_fail_closed() {
    for kind in [
        "mode",
        "hardlink",
        "symlink",
        "directory",
        "root-binding",
        "state-binding",
    ] {
        let fixture = Fixture::new();
        let (store, guard) = fixture.start();
        let reserved = record(&store);
        store.persist(&guard, None, &reserved).unwrap();
        let path = store.path(JOURNAL);
        match kind {
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            "hardlink" => fs::hard_link(&path, fixture.0.join("alias")).unwrap(),
            "symlink" => {
                fs::rename(&path, fixture.0.join("target")).unwrap();
                symlink(fixture.0.join("target"), &path).unwrap();
            }
            "directory" => {
                fs::rename(&store.store.state_directory, fixture.0.join("old-state")).unwrap();
                DirBuilder::new()
                    .mode(0o700)
                    .create(&store.store.state_directory)
                    .unwrap();
            }
            "root-binding" | "state-binding" => {
                let mut binding = reserved.binding().clone();
                if kind == "root-binding" {
                    binding.data_root.inode += 100;
                } else {
                    binding.state_directory.inode += 100;
                }
                let changed =
                    PreparationReceipt::new(binding, reserved.initial_source().clone()).unwrap();
                fs::write(&path, changed.encode().unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(store.load_guarded(&guard).is_err(), "{kind}");
        assert!(store.persist(&guard, None, &reserved).is_err(), "{kind}");
    }
}

#[test]
fn replacement_of_staged_or_current_file_before_rename_is_rejected() {
    struct Replace<'a> {
        store: &'a PreparationJournalStore,
        staged: bool,
        fired: RefCell<bool>,
    }
    impl FaultInjector for Replace<'_> {
        fn checkpoint(&self, point: FaultPoint) -> Result<()> {
            if point == FaultPoint::BeforeRename && !*self.fired.borrow() {
                *self.fired.borrow_mut() = true;
                let path = self
                    .store
                    .path(if self.staged { STAGED_JOURNAL } else { JOURNAL });
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, self.store.store.root.path.join("preserved-original")).unwrap();
                put(&path, &bytes);
            }
            Ok(())
        }
    }
    for staged in [false, true] {
        let fixture = Fixture::new();
        let (store, guard) = fixture.start();
        let reserved = record(&store);
        store.persist(&guard, None, &reserved).unwrap();
        let faults = Replace {
            store: &store,
            staged,
            fired: RefCell::new(false),
        };
        assert_eq!(
            store.persist_with_faults(&guard, Some(&reserved), &ready(&reserved), &faults),
            Err(Error::IdentityChanged)
        );
        assert_eq!(
            fs::read(store.path(JOURNAL)).unwrap(),
            reserved.encode().unwrap()
        );
        assert!(store.path(STAGED_JOURNAL).exists());
    }
}

#[test]
fn actual_process_exit_at_each_boundary_preserves_the_recovery_decision() {
    for (index, _) in POINTS.iter().enumerate() {
        let fixture = Fixture::new();
        let (store, guard) = fixture.start();
        let reserved = record(&store);
        drop(guard);
        drop(store);
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "filesystem::preparation_journal::tests::journal_crash_child",
                "--ignored",
            ])
            .env("RADISHLEX_JOURNAL_TEST_ROOT", &fixture.0)
            .env("RADISHLEX_JOURNAL_TEST_POINT", index.to_string())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73));
        // New acquisition must safely reclaim only this exited child's socket.
        let (reloaded, guard) = fixture.reload();
        if index < 5 {
            assert_eq!(reloaded.load_guarded(&guard), Err(Error::InterruptedWrite));
        } else {
            assert_eq!(
                reloaded.load_guarded(&guard).unwrap(),
                Some(reserved.clone())
            );
            reloaded.persist(&guard, None, &reserved).unwrap();
        }
    }
}

#[test]
#[ignore = "child process entered only by the boundary matrix"]
fn journal_crash_child() {
    struct ExitAt(FaultPoint);
    impl FaultInjector for ExitAt {
        fn checkpoint(&self, point: FaultPoint) -> Result<()> {
            if point == self.0 {
                std::process::exit(73);
            }
            Ok(())
        }
    }
    let root = PathBuf::from(std::env::var_os("RADISHLEX_JOURNAL_TEST_ROOT").unwrap());
    // Never accept a real product root through this subprocess-only fixture.
    assert_eq!(
        root.parent().unwrap(),
        fs::canonicalize(std::env::temp_dir()).unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-preparation-journal-"));
    let owner = fs::metadata(&root).unwrap().uid();
    let store =
        PreparationJournalStore::open_existing(VerifiedDataRoot::verify(root, owner).unwrap())
            .unwrap();
    let guard = store.acquire_guard().unwrap();
    let index: usize = std::env::var("RADISHLEX_JOURNAL_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    store
        .persist_with_faults(&guard, None, &record(&store), &ExitAt(POINTS[index]))
        .unwrap();
    panic!("fault point was not reached");
}
