//! Continuous synthetic release -> normal use -> fresh preparation -> v1 chain.
use super::*;
use radishlex_ime_product_upgrade::PreparationPhase;

const NEXT: &str = "44444444444444444444444444444444";

#[test]
fn cancellation_binds_released_history_without_moving_or_rewriting_it() {
    use radishlex_ime_product_upgrade::{
        CancellationSourceCheckpoint, CancellationSourcePort, CancellationSourceReceipt,
        PreparationCancellationCheckpoint, PreparationCancellationPort,
        PreparationCancellationRequest, PreparationCancellationStore,
    };
    struct Admit(PreparationBinding);
    impl PreparationCancellationPort for Admit {
        fn confirm_authority_and_quiescence(
            &mut self,
            request: &PreparationCancellationRequest,
            _: PreparationCancellationCheckpoint,
        ) -> bool {
            assert_eq!(request.preparation().binding(), &self.0);
            true
        }
    }
    impl CancellationSourcePort for Admit {
        fn confirm_authority_and_quiescence(
            &mut self,
            request: &PreparationCancellationRequest,
            _: &CancellationSourceReceipt,
            _: CancellationSourceCheckpoint,
        ) -> bool {
            assert_eq!(request.preparation().binding(), &self.0);
            true
        }
        fn available_bytes(&mut self) -> Option<u64> {
            Some(u64::MAX)
        }
    }
    for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
        let fixture = Fixture::new();
        let prior = released(&fixture, state);
        let old_root = fixture.0.join(HISTORY);
        let before = failures::tree(&old_root);
        let (store, guard, binding) = reserve(&fixture, &prior, NEXT);
        let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
        let request = cancel
            .request_cancellation(&guard, NEXT, &mut Admit(binding.clone()), &Hasher)
            .unwrap();
        assert_eq!(cancel.load_guarded(&guard, &Hasher).unwrap(), Some(request));
        let ready = cancel
            .finish_source(&guard, &mut Admit(binding.clone()), &Hasher)
            .unwrap();
        assert_eq!(
            cancel.load_source_guarded(&guard, &Hasher).unwrap(),
            Some(ready)
        );
        assert_eq!(failures::tree(&old_root), before);
        let index = old_root.join("latest-release.json");
        let bytes = fs::read(&index).unwrap();
        fs::rename(&index, fixture.0.join("retained-index")).unwrap();
        put(&index, &bytes);
        assert!(cancel.load_source_guarded(&guard, &Hasher).is_err());
        assert!(cancel
            .finish_source(&guard, &mut Admit(binding), &Hasher)
            .is_err());
    }
}

fn released(fixture: &Fixture, state: State) -> Binding {
    let (store, guard, binding, mut port) = terminal(fixture, true, state);
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    port.finalize(&fixture.0);
    store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    drop(guard);
    let mut db = UserDb::open(fixture.source()).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-next",
        "xin",
        "新",
        0,
        5,
    ))
    .unwrap();
    db.delete_term("xin", "新", None).unwrap();
    drop(db);
    binding
}

fn reserve(
    fixture: &Fixture,
    prior: &Binding,
    operation: &str,
) -> (
    PreparationJournalStore,
    UpgradeProcessGuard,
    PreparationBinding,
) {
    reserve_with_outer_digest(fixture, prior, operation, "d".repeat(64))
}

fn reserve_with_outer_digest(
    fixture: &Fixture,
    prior: &Binding,
    operation: &str,
    outer_digest: String,
) -> (
    PreparationJournalStore,
    UpgradeProcessGuard,
    PreparationBinding,
) {
    let (store, guard) = fixture.reload();
    let inventory = store
        .capture_previous_inventory(
            &guard,
            operation,
            Some(OUTER),
            Some(&prior.operation_id),
            &Hasher,
        )
        .unwrap();
    let proof = store
        .previous_inventory_identity(&guard, &inventory, &Hasher)
        .unwrap()
        .unwrap();
    let binding = PreparationBinding {
        operation_id: operation.to_owned(),
        previous_install_operation_id: Some(OUTER.to_owned()),
        previous_data_operation_id: Some(prior.operation_id.clone()),
        source_release: prior.installed_release.clone(),
        target_release: release(prior.installed_release.build_number() + 2),
        source_product_sha256: prior.installed_product_sha256.clone(),
        target_product_sha256: "c".repeat(64),
        previous_install_receipt_sha256: Some(outer_digest),
        previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
        previous_inventory_sha256: Some(proof.sha256.clone()),
        target_schema_version: 9,
        data_root: store.data_root_identity(),
        state_directory: store.state_directory_identity(),
    };
    let mut record = PreparationReceipt::new(
        binding.clone(),
        store.observe_source_family(&guard, &Hasher).unwrap(),
    )
    .unwrap();
    record
        .bind_released_predecessor(proof, inventory.release_index_identity().unwrap().clone())
        .unwrap();
    store.persist(&guard, None, &record).unwrap();
    (store, guard, binding)
}

#[path = "cancellation_released.rs"]
mod cancellation_released;

fn advance(
    store: PreparationJournalStore,
    guard: &UpgradeProcessGuard,
    binding: &PreparationBinding,
    port: &mut Port,
) -> Result<
    (UpgradeReceiptStore, UpgradeReceipt),
    radishlex_ime_product_upgrade::SourcePreparationError,
> {
    if let Some(record) = store.load_guarded(guard)? {
        if record.phase() <= PreparationPhase::SourcePrepared {
            store.prepare_userdb_source(guard, port, &Hasher)?;
        }
        if record.phase() <= PreparationPhase::SourcePrepared {
            store.archive_previous_upgrade(guard, port, &Hasher)?;
        }
    }
    store.handoff_userdb_source(guard, &binding.operation_id, port, &Hasher)
}

#[test]
fn every_released_terminal_allows_fresh_preparation_after_learning_and_deletion() {
    for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
        let fixture = Fixture::new();
        let prior = released(&fixture, state);
        let old_root = fixture.0.join(HISTORY).join(NEW);
        let old = failures::tree(&old_root);
        let (store, guard, binding) = reserve(&fixture, &prior, NEXT);
        let initial = store.load_guarded(&guard).unwrap().unwrap();
        assert!(initial.previous_release_index_identity().is_some());
        let mut port = Port::new(binding.clone());
        let prepared = store
            .prepare_userdb_source(&guard, &mut port, &Hasher)
            .unwrap();
        let archived = store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        assert!(archived.archived_slots().is_empty());
        assert_eq!(failures::tree(&old_root), old);
        assert!(!fixture.state("receipt.json").exists());
        let (v1, mut receipt) = store
            .handoff_userdb_source(&guard, NEXT, &mut port, &Hasher)
            .unwrap();
        assert_eq!(receipt.previous_operation_id(), Some(NEW));
        assert_eq!(receipt.source_release(), &prior.installed_release);
        let snapshot = fixture
            .0
            .join(HISTORY)
            .join(NEXT)
            .join("preparation-snapshot.sqlite3");
        assert_eq!(
            fs::metadata(&snapshot).unwrap().ino(),
            prepared.snapshot_identity().unwrap().inode
        );
        UserDb::verify_prepared_source(fixture.source(), &snapshot).unwrap();
        let protected = UserDb::open_read_only_current(&snapshot).unwrap();
        assert_eq!(protected.list_active_terms().unwrap().len(), 1);
        assert_eq!(protected.list_deleted_term_tombstones().unwrap().len(), 2);
        drop(protected);
        v1.resume_userdb_upgrade(
            &guard,
            &mut receipt,
            u64::MAX,
            &mut ProductPort(State::Completed),
        )
        .unwrap();
        let next = Binding {
            operation_id: NEXT.to_owned(),
            installed_release: binding.target_release.clone(),
            installed_product_sha256: binding.target_product_sha256.clone(),
        };
        let store = Store::attach(v1, &guard).unwrap();
        let mut finalizer = ReleasePort::new(next.clone());
        store
            .prepare_terminal_release(&guard, &next, &mut finalizer, &Hasher)
            .unwrap();
        finalizer.finalize(&fixture.0);
        store
            .finish_terminal_release(&guard, NEXT, &mut finalizer, &Hasher)
            .unwrap();
        assert_eq!(failures::tree(&old_root), old);
        drop(guard);
        // Third preparation proves the new locator and multiple retained ancestors.
        let (store, guard, binding) = reserve(&fixture, &next, "55555555555555555555555555555555");
        advance(store, &guard, &binding, &mut Port::new(binding.clone())).unwrap();
        assert_eq!(failures::tree(&old_root), old);
    }
}

#[test]
fn released_material_drift_blocks_before_any_new_sqlite_write() {
    for case in 0..16 {
        let fixture = Fixture::new();
        let prior = released(&fixture, State::AbortedPreserved);
        let (store, guard, _) = reserve(&fixture, &prior, NEXT);
        let index = fixture.0.join(HISTORY).join("latest-release.json");
        let proof = history(&fixture, "terminal-release.json");
        let replace = |path: &Path| {
            let bytes = fs::read(path).unwrap();
            fs::rename(path, fixture.0.join("retained-object")).unwrap();
            put(path, &bytes);
        };
        match case {
            0 => replace(&index),
            1 => replace(&proof),
            2 => fs::rename(&index, fixture.0.join("retained-index")).unwrap(),
            3 => put(
                &fixture.0.join(HISTORY).join("latest-release.json.tmp"),
                b"unknown",
            ),
            4 => fs::write(&proof, b"unknown-release").unwrap(),
            5 => fs::write(
                history(&fixture, "data/source-snapshot.sqlite3"),
                b"changed",
            )
            .unwrap(),
            6 => replace(&history(&fixture, "preparation.json")),
            7 => replace(&history(&fixture, "preparation-snapshot.sqlite3")),
            8 => fs::set_permissions(history(&fixture, "data"), fs::Permissions::from_mode(0o755))
                .unwrap(),
            9 => put(&fixture.0.join(HISTORY).join("unknown"), b"unknown"),
            10 => put(
                &fixture.state("receipt.json"),
                &fs::read(history(&fixture, "data/receipt.json")).unwrap(),
            ),
            11..=14 => {
                let path = fixture.state("source-preparation.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                match case {
                    11 => {
                        value
                            .as_object_mut()
                            .unwrap()
                            .remove("previous_release_index_identity");
                    }
                    12 => {
                        value["binding"]["source_product_sha256"] =
                            serde_json::json!("e".repeat(64))
                    }
                    13 => {
                        value["binding"]["source_release"]["build_number"] = serde_json::json!(38)
                    }
                    14 => value["binding"]["previous_data_operation_id"] = serde_json::json!(OLD),
                    _ => unreachable!(),
                }
                // Re-encode through the strict contract; a legacy-looking record
                // cannot downgrade a released proof to a movable old inventory.
                let mut bytes = serde_json::to_vec(&value).unwrap();
                bytes.push(b'\n');
                let typed: PreparationReceipt = serde_json::from_slice(&bytes).unwrap();
                fs::write(path, typed.encode().unwrap()).unwrap();
            }
            15 => DirBuilder::new()
                .mode(0o700)
                .create(fixture.0.join(HISTORY).join(NEXT))
                .unwrap(),
            _ => unreachable!(),
        }
        let before = failures::tree(&fixture.0);
        let observed = store
            .load_guarded(&guard)
            .unwrap()
            .unwrap()
            .binding()
            .clone();
        assert!(
            store
                .prepare_userdb_source(&guard, &mut Port::new(observed), &Hasher)
                .is_err(),
            "case {case}"
        );
        assert_eq!(failures::tree(&fixture.0), before, "case {case}");
    }
}

#[test]
fn capture_requires_explicit_matching_predecessor_and_preserves_history() {
    let fixture = Fixture::new();
    let prior = released(&fixture, State::Completed);
    let (store, guard) = fixture.reload();
    let before = failures::tree(&fixture.0);
    for (operation, previous) in [(NEXT, None), (NEXT, Some(OLD)), (NEW, Some(NEW))] {
        assert!(store
            .capture_previous_inventory(&guard, operation, Some(OUTER), previous, &Hasher)
            .is_err());
        assert_eq!(failures::tree(&fixture.0), before);
    }
    let inventory = store
        .capture_previous_inventory(
            &guard,
            NEXT,
            Some(OUTER),
            Some(&prior.operation_id),
            &Hasher,
        )
        .unwrap();
    assert_eq!(failures::tree(&fixture.0), before);
    let index = fixture.0.join(HISTORY).join("latest-release.json");
    let bytes = fs::read(&index).unwrap();
    fs::rename(&index, fixture.0.join("retained-index")).unwrap();
    put(&index, &bytes);
    let before = failures::tree(&fixture.0);
    assert!(store
        .previous_inventory_identity(&guard, &inventory, &Hasher)
        .is_err());
    assert_eq!(failures::tree(&fixture.0), before);
}

#[test]
fn callbacks_cannot_waive_pinned_history_at_preparation_or_handoff() {
    for point in [
        Point::Begin,
        Point::BeforeMaintenance,
        Point::SourceRecorded,
        Point::ArchiveCompleted,
        Point::HandoffBeforeReceiptRename,
    ] {
        let fixture = Fixture::new();
        let prior = released(&fixture, State::Completed);
        let (store, guard, binding) = reserve(&fixture, &prior, NEXT);
        let mut port = Port::new(binding.clone());
        let path = fixture.0.join(HISTORY).join("latest-release.json");
        let root = fixture.0.clone();
        let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
        let copy = captured.clone();
        port.mutation = Some((
            point,
            Box::new(move || {
                fs::write(path, b"changed-in-callback").unwrap();
                *copy.borrow_mut() = Some(failures::tree(&root));
            }),
        ));
        assert!(
            advance(store, &guard, &binding, &mut port).is_err(),
            "{point:?}"
        );
        assert_eq!(
            &failures::tree(&fixture.0),
            captured.borrow().as_ref().unwrap()
        );
    }
}

fn retry_points() -> Vec<Point> {
    vec![
        Point::Begin,
        Point::SnapshotCreated,
        Point::SnapshotRecorded,
        Point::MaintenanceIntentRecorded,
        Point::SourceMaintained,
        Point::SourceRecorded,
        Point::ArchiveBegin,
        Point::ArchiveCompleted,
        Point::HandoffHistoryCreated,
        Point::HandoffIntentRecorded,
        Point::HandoffReceiptRenamed,
        Point::HandoffReadyRecorded,
        Point::HandoffSnapshotRenamed,
        Point::HandoffMarkerRenamed,
    ]
}
fn retry_blocked(point: Point) -> bool {
    matches!(point, Point::SnapshotCreated | Point::HandoffHistoryCreated)
}

#[test]
fn released_predecessor_authority_loss_reloads_the_same_index_and_proof() {
    for point in retry_points() {
        let fixture = Fixture::new();
        let prior = released(&fixture, State::RolledBack);
        let old = failures::tree(&fixture.0.join(HISTORY).join(NEW));
        let (store, guard, binding) = reserve(&fixture, &prior, NEXT);
        let mut port = Port::new(binding.clone());
        port.fail = Some(point);
        assert!(
            advance(store, &guard, &binding, &mut port).is_err(),
            "{point:?}"
        );
        drop(guard);
        let before = failures::tree(&fixture.0);
        let (store, guard) = fixture.reload();
        let result = advance(store, &guard, &binding, &mut Port::new(binding.clone()));
        assert_eq!(result.is_err(), retry_blocked(point), "{point:?}");
        drop(guard);
        if result.is_err() {
            assert_eq!(failures::tree(&fixture.0), before);
        }
        assert_eq!(failures::tree(&fixture.0.join(HISTORY).join(NEW)), old);
    }
}

#[test]
fn released_predecessor_process_exit_recovers_from_disk_without_rearchiving() {
    for (index, point) in retry_points().into_iter().enumerate() {
        let fixture = Fixture::new();
        let prior = released(&fixture, State::Completed);
        let (store, guard, binding) = reserve(&fixture, &prior, NEXT);
        drop(store);
        drop(guard);
        let old = failures::tree(&fixture.0.join(HISTORY).join(NEW));
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal_release::predecessor::predecessor_crash_child",
                "--ignored",
            ])
            .env("RADISHLEX_PREDECESSOR_ROOT", &fixture.0)
            .env("RADISHLEX_PREDECESSOR_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(74),
            "{point:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let before = failures::tree(&fixture.0);
        let (store, guard) = fixture.reload();
        let result = advance(store, &guard, &binding, &mut Port::new(binding.clone()));
        assert_eq!(result.is_err(), retry_blocked(point), "{point:?}");
        drop(guard);
        if result.is_err() {
            assert_eq!(failures::tree(&fixture.0), before);
        }
        assert_eq!(failures::tree(&fixture.0.join(HISTORY).join(NEW)), old);
    }
}

#[test]
#[ignore = "entry point invoked by released predecessor process exit matrix"]
fn predecessor_crash_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_PREDECESSOR_ROOT").unwrap());
    let index: usize = std::env::var("RADISHLEX_PREDECESSOR_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let store = PreparationJournalStore::open_existing(
        VerifiedDataRoot::verify(&root, fs::metadata(&root).unwrap().uid()).unwrap(),
    )
    .unwrap();
    let guard = store.acquire_guard().unwrap();
    let binding = store
        .load_guarded(&guard)
        .unwrap()
        .unwrap()
        .binding()
        .clone();
    let mut port = Port::new(binding.clone());
    port.exit = Some(retry_points()[index]);
    advance(store, &guard, &binding, &mut port).unwrap();
    panic!("checkpoint not reached");
}
