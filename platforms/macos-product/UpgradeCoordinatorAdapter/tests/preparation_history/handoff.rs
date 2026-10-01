//! Handoff tests reuse the real SQLite / terminal predecessor fixture.
use super::*;

#[path = "handoff_mutations.rs"]
mod mutations;

fn new_history(fixture: &Fixture, name: &str) -> PathBuf {
    fixture.0.join(HISTORY).join(NEW).join(name)
}

#[test]
fn first_and_three_terminal_predecessors_handoff_without_opening_startup() {
    for previous in [
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ] {
        let fixture = Fixture::new();
        DirBuilder::new()
            .mode(0o700)
            .create(fixture.0.join("Rime"))
            .unwrap();
        let (store, guard, mut port) = fixture.reserve(previous);
        let prepared = store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        let directory = fs::metadata(fixture.0.join(STATE)).unwrap().ino();
        let snapshot = fs::read(fixture.state("preparation-snapshot.sqlite3")).unwrap();
        let old_receipt = previous.map(|_| fs::read(fixture.history("receipt.json")).unwrap());
        let source = fs::read(fixture.source()).unwrap();
        assert!(UpgradeReceiptStore::open_existing(fixture.verified()).is_err());
        let (v1, receipt) = store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        assert_eq!(v1.load_guarded(&guard).unwrap(), Some(receipt.clone()));
        assert_eq!(receipt.state(), State::Preflighted);
        assert_eq!(receipt.operation_id(), NEW);
        assert_eq!(receipt.previous_operation_id(), previous.map(|_| OLD));
        assert_ne!(receipt.previous_operation_id(), Some(OUTER));
        assert_eq!(receipt.source_release(), &release(39));
        assert_eq!(receipt.target_release(), &release(41));
        assert_eq!(receipt.source_schema_version(), Some(9));
        assert!(receipt
            .artifacts()
            .iter()
            .any(|a| a.slot() == Artifact::RimeRoot));
        let database = receipt
            .artifacts()
            .iter()
            .find(|a| a.slot() == Artifact::SourceDatabase)
            .unwrap();
        assert_eq!(database.inode(), prepared.prepared_source().unwrap().inode);
        assert_eq!(
            database.byte_len(),
            prepared.prepared_source().unwrap().byte_len
        );
        assert_eq!(
            fs::metadata(fixture.0.join(STATE)).unwrap().ino(),
            directory
        );
        assert_eq!(fs::read(fixture.source()).unwrap(), source);
        assert!(!fixture.state("source-preparation.json").exists());
        assert!(!fixture.state("preparation-snapshot.sqlite3").exists());
        assert_eq!(
            fs::read(new_history(&fixture, "preparation-snapshot.sqlite3")).unwrap(),
            snapshot
        );
        assert_eq!(
            fs::metadata(new_history(&fixture, "preparation-snapshot.sqlite3"))
                .unwrap()
                .ino(),
            prepared.snapshot_identity().unwrap().inode
        );
        let proof_bytes = fs::read(new_history(&fixture, "preparation.json")).unwrap();
        let proof = PreparationReceipt::decode(&proof_bytes).unwrap();
        assert_eq!(proof.phase(), PreparationPhase::HandoffReady);
        assert_eq!(proof.binding(), prepared.binding());
        if let Some(bytes) = old_receipt {
            assert_eq!(fs::read(fixture.history("receipt.json")).unwrap(), bytes);
            let old = UpgradeReceipt::decode(&bytes).unwrap();
            assert!(v1.persist(&guard, &old).is_err());
        }
        drop(guard);
        assert!(
            !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                .is_allowed()
        );
        let (store, guard) = fixture.reload();
        let (_, repeated) = store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        assert_eq!(repeated, receipt);
        assert_eq!(
            fs::read(new_history(&fixture, "preparation.json")).unwrap(),
            proof_bytes
        );
        UserDb::verify_prepared_source(
            fixture.source(),
            new_history(&fixture, "preparation-snapshot.sqlite3"),
        )
        .unwrap();
    }
}

#[test]
fn handed_off_store_continues_normal_v1_snapshot_without_reusing_protection_snapshot() {
    let fixture = Fixture::new();
    let (store, guard, mut port) = fixture.reserve(Some(State::AbortedPreserved));
    store
        .archive_previous_upgrade(&guard, &mut port, &Hasher)
        .unwrap();
    let (store, mut receipt) = store
        .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    let protection = new_history(&fixture, "preparation-snapshot.sqlite3");
    let protected_bytes = fs::read(&protection).unwrap();
    let proof = fs::read(new_history(&fixture, "preparation.json")).unwrap();
    receipt.advance(State::Quiesced).unwrap();
    store.persist(&guard, &receipt).unwrap();
    store.create_settings_backup(&guard, &mut receipt).unwrap();
    store
        .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
        .unwrap();
    assert_eq!(receipt.state(), State::SnapshotReady);
    assert_eq!(store.load_guarded(&guard).unwrap(), Some(receipt));
    assert_ne!(
        fs::metadata(fixture.state("source-snapshot.sqlite3"))
            .unwrap()
            .ino(),
        fs::metadata(&protection).unwrap().ino()
    );
    assert_eq!(fs::read(protection).unwrap(), protected_bytes);
    assert_eq!(
        fs::read(new_history(&fixture, "preparation.json")).unwrap(),
        proof
    );
}

fn points() -> Vec<Point> {
    vec![
        Point::HandoffBegin,
        Point::HandoffHistoryRootReady,
        Point::HandoffHistoryRootSynced,
        Point::HandoffHistoryRootParentSynced,
        Point::HandoffHistoryCreated,
        Point::HandoffOperationSynced,
        Point::HandoffOperationParentSynced,
        Point::HandoffReceiptCreated,
        Point::HandoffReceiptWritten,
        Point::HandoffReceiptFileSynced,
        Point::HandoffHistorySynced,
        Point::HandoffIntentRecorded,
        Point::HandoffBeforeReceiptRename,
        Point::HandoffReceiptRenamed,
        Point::HandoffReceiptTargetSynced,
        Point::HandoffReceiptSourceSynced,
        Point::HandoffReadyRecorded,
        Point::HandoffBeforeSnapshotRename,
        Point::HandoffSnapshotRenamed,
        Point::HandoffSnapshotTargetSynced,
        Point::HandoffSnapshotSourceSynced,
        Point::HandoffBeforeMarkerRename,
        Point::HandoffMarkerRenamed,
        Point::HandoffMarkerTargetSynced,
        Point::HandoffMarkerSourceSynced,
        Point::HandoffComplete,
    ]
}

fn unbound_creation(point: Point) -> bool {
    matches!(
        point,
        Point::HandoffHistoryCreated
            | Point::HandoffOperationSynced
            | Point::HandoffOperationParentSynced
            | Point::HandoffHistoryRootReady
            | Point::HandoffHistoryRootSynced
            | Point::HandoffHistoryRootParentSynced
            | Point::HandoffReceiptCreated
            | Point::HandoffReceiptWritten
            | Point::HandoffReceiptFileSynced
            | Point::HandoffHistorySynced
    )
}

fn root_checkpoint(point: Point) -> bool {
    matches!(
        point,
        Point::HandoffHistoryRootReady
            | Point::HandoffHistoryRootSynced
            | Point::HandoffHistoryRootParentSynced
    )
}

#[test]
fn authority_loss_at_every_boundary_preserves_evidence_and_only_resumes_bound_objects() {
    for previous in [None, Some(State::AbortedPreserved)] {
        for point in points() {
            let fixture = Fixture::new();
            let (store, guard, mut port) = fixture.reserve(previous);
            store
                .archive_previous_upgrade(&guard, &mut port, &Hasher)
                .unwrap();
            let source = fs::read(fixture.source()).unwrap();
            port.fail = Some(point);
            assert!(
                store
                    .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
                    .is_err(),
                "{point:?}"
            );
            assert_eq!(fs::read(fixture.source()).unwrap(), source);
            assert!(
                !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                    .is_allowed()
            );
            drop(guard);
            let (store, guard) = fixture.reload();
            port.fail = None;
            let result = store.handoff_userdb_source(&guard, NEW, &mut port, &Hasher);
            // With a predecessor, the first directory callback observes the
            // existing history root before the new operation is created.
            let blocked =
                unbound_creation(point) && !(previous.is_some() && root_checkpoint(point));
            assert_eq!(result.is_err(), blocked, "{previous:?} {point:?}");
            if blocked {
                assert!(fixture.state("source-preparation.json").exists());
                assert!(!fixture.state("receipt.json").exists());
            }
        }
    }
}

#[test]
fn subprocess_exit_at_each_boundary_reloads_under_a_fresh_guard() {
    for (index, point) in points().into_iter().enumerate() {
        let fixture = Fixture::new();
        let (store, guard, port) = fixture.reserve(Some(State::RolledBack));
        let mut port = port;
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        drop(guard);
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "handoff::handoff_crash_child", "--ignored"])
            .env("RADISHLEX_HANDOFF_ROOT", &fixture.0)
            .env("RADISHLEX_HANDOFF_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(74),
            "{point:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let (store, guard) = fixture.reload();
        let result = store.handoff_userdb_source(&guard, NEW, &mut port, &Hasher);
        assert_eq!(
            result.is_err(),
            unbound_creation(point) && !root_checkpoint(point),
            "{point:?}"
        );
    }
}

#[test]
#[ignore = "subprocess selected by parent test"]
fn handoff_crash_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_HANDOFF_ROOT").unwrap());
    let index: usize = std::env::var("RADISHLEX_HANDOFF_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let verified = VerifiedDataRoot::verify(&root, fs::metadata(&root).unwrap().uid()).unwrap();
    let store = PreparationJournalStore::open_existing(verified).unwrap();
    let guard = store.acquire_guard().unwrap();
    let mut port = Port::new(
        store
            .load_guarded(&guard)
            .unwrap()
            .unwrap()
            .binding()
            .clone(),
    );
    port.exit = Some(points()[index]);
    store
        .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    panic!("crash checkpoint was not reached");
}
