//! Filesystem cancellation protocol with synthetic product authority. Real
//! source binaries and the Installer port remain separate qualifications.
use super::*;
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint as AP, CancellationArchivePort, CancellationArchiveReceipt,
    CancellationArchiveStore, CancellationOuterBinding, CancellationSourceCheckpoint,
    CancellationSourcePort, CancellationSourceReceipt, PreparationCancellationCheckpoint as CP,
    PreparationCancellationPort, PreparationCancellationRequest, PreparationCancellationStore,
};
use sha2::{Digest, Sha256};

const OLD_BYTES: &[u8] = b"{\"synthetic_outer\":\"old_terminal\"}\n";
const NEW_BYTES: &[u8] = b"{\"synthetic_outer\":\"new_prepared\"}\n";

struct Authority {
    fail: Option<usize>,
    exit: bool,
    points: Vec<AP>,
    mutation: Option<(AP, Box<dyn FnOnce()>)>,
}
impl Authority {
    fn new() -> Self {
        Self {
            fail: None,
            exit: false,
            points: Vec::new(),
            mutation: None,
        }
    }
}
impl PreparationCancellationPort for Authority {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: CP,
    ) -> bool {
        true
    }
}
impl CancellationSourcePort for Authority {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: &CancellationSourceReceipt,
        _: CancellationSourceCheckpoint,
    ) -> bool {
        true
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
impl CancellationArchivePort for Authority {
    fn confirm_authority_and_quiescence(
        &mut self,
        record: &CancellationArchiveReceipt,
        point: AP,
    ) -> bool {
        assert_eq!(record.outer().previous_bytes(), OLD_BYTES);
        assert!(record
            .outer()
            .new_bytes()
            .map_or(true, |bytes| bytes == NEW_BYTES));
        self.points.push(point);
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.fail == Some(self.points.len() - 1) {
            if self.exit {
                std::process::exit(77)
            }
            return false;
        }
        true
    }
}

fn prepare(
    fixture: &Fixture,
    state: Option<State>,
    stage: u8,
    new_outer: bool,
) -> (
    CancellationArchiveStore,
    UpgradeProcessGuard,
    CancellationOuterBinding,
) {
    let (store, guard) = fixture.legacy_with_learning(state, 1);
    let outer_history = fixture.0.join(".radishlex-install-history-v1");
    let old = outer_history.join(OUTER);
    let active = fixture.0.join(".radishlex-install-v1");
    for path in [&outer_history, &old, &active] {
        DirBuilder::new().mode(0o700).create(path).unwrap();
    }
    put(&old.join("receipt.json"), OLD_BYTES);
    put(
        &active.join("receipt.json"),
        if new_outer { NEW_BYTES } else { OLD_BYTES },
    );
    let inventory = store
        .capture_previous_inventory(&guard, NEW, Some(OUTER), state.map(|_| OLD), &Hasher)
        .unwrap();
    let identity = store
        .previous_inventory_identity(&guard, &inventory, &Hasher)
        .unwrap();
    let binding = PreparationBinding {
        operation_id: NEW.into(),
        previous_install_operation_id: Some(OUTER.into()),
        previous_data_operation_id: state.map(|_| OLD.into()),
        source_release: release(39),
        target_release: release(41),
        source_product_sha256: "a".repeat(64),
        target_product_sha256: "b".repeat(64),
        previous_install_receipt_sha256: Some(format!("{:x}", Sha256::digest(OLD_BYTES))),
        previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
        previous_inventory_sha256: identity.as_ref().map(|v| v.sha256.clone()),
        target_schema_version: 9,
        data_root: store.data_root_identity(),
        state_directory: store.state_directory_identity(),
    };
    let mut prep = PreparationReceipt::new(
        binding.clone(),
        store.observe_source_family(&guard, &Hasher).unwrap(),
    )
    .unwrap();
    if let Some(identity) = identity {
        prep.bind_previous_inventory(identity).unwrap();
    }
    store.persist(&guard, None, &prep).unwrap();
    let mut port = Port::new(binding);
    if stage > 0 {
        if stage == 1 {
            port.fail = Some(Point::SnapshotRecorded);
        }
        if stage == 2 {
            port.fail = Some(Point::MaintenanceIntentRecorded);
        }
        let result = store.prepare_userdb_source(&guard, &mut port, &Hasher);
        if stage < 3 {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
    }
    if stage == 4 {
        port.fail = Some(Point::ArchiveSlotRecorded(Slot::Receipt));
        assert!(store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .is_err());
    } else if stage == 5 {
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
    }
    let cancellation = PreparationCancellationStore::attach(store, &guard).unwrap();
    let mut authority = Authority::new();
    cancellation
        .request_cancellation(&guard, NEW, &mut authority, &Hasher)
        .unwrap();
    cancellation
        .finish_source(&guard, &mut authority, &Hasher)
        .unwrap();
    drop(guard);
    let archive = CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
    let guard = archive.acquire_guard().unwrap();
    let outer = archive.observe_outer(&guard, &Hasher).unwrap();
    (archive, guard, outer)
}

fn assert_preserved(fixture: &Fixture, record: &CancellationArchiveReceipt) {
    assert!(record.preserved());
    assert_eq!(
        fs::read(fixture.0.join(".radishlex-install-v1/receipt.json")).unwrap(),
        OLD_BYTES
    );
    assert_eq!(
        fs::read(
            fixture
                .0
                .join(".radishlex-install-history-v1")
                .join(OUTER)
                .join("receipt.json")
        )
        .unwrap(),
        OLD_BYTES
    );
    assert!(fixture.state("preparation-cancellation.json").exists());
    assert!(fixture.state("cancellation-archive.json").exists());
    assert!(!fixture.0.join(HISTORY).join("latest-release.json").exists());
    assert!(PreparationCancellationStore::open_existing(fixture.verified()).is_err());
    assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
}

#[test]
fn preserves_cancellation_materials_and_outer_without_releasing_gate() {
    for state in [
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ] {
        for stage in 0..=5 {
            if stage == 4 && state.is_none() {
                continue;
            }
            for new_outer in [false, true] {
                let fixture = Fixture::new();
                let (store, guard, outer) = prepare(&fixture, state, stage, new_outer);
                let db = fs::read(fixture.source()).unwrap();
                let db_inode = fs::metadata(fixture.source()).unwrap().ino();
                let request = fs::read(fixture.state("preparation-cancellation.json")).unwrap();
                let record = store
                    .preserve_cancellation(&guard, &outer, &mut Authority::new(), &Hasher)
                    .unwrap();
                assert_preserved(&fixture, &record);
                assert_eq!(fs::read(fixture.source()).unwrap(), db);
                assert_eq!(fs::metadata(fixture.source()).unwrap().ino(), db_inode);
                assert_eq!(
                    fs::read(fixture.state("preparation-cancellation.json")).unwrap(),
                    request
                );
                assert_eq!(
                    fs::read(fixture.0.join("manager-settings.json")).unwrap(),
                    b"synthetic-settings"
                );
                let encoded = record.encode().unwrap();
                assert_eq!(
                    CancellationArchiveReceipt::decode(&encoded).unwrap(),
                    record
                );
                drop(guard);
                let reloaded = CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
                let guard = reloaded.acquire_guard().unwrap();
                assert_eq!(
                    reloaded.load_guarded(&guard, &Hasher).unwrap(),
                    Some(record.clone())
                );
                assert_eq!(
                    reloaded
                        .preserve_cancellation(&guard, &outer, &mut Authority::new(), &Hasher)
                        .unwrap(),
                    record
                );
            }
        }
    }
}

#[test]
#[ignore = "subprocess entry exercised by every_archive_boundary_revokes_authority_and_survives_process_exit"]
fn archive_crash_child() {
    let Ok(root) = std::env::var("RADISHLEX_CANCEL_ARCHIVE_CHILD") else {
        return;
    };
    let fail: usize = std::env::var("RADISHLEX_CANCEL_ARCHIVE_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let fixture = Fixture(PathBuf::from(root));
    let store = CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    let outer = store.observe_outer(&guard, &Hasher).unwrap();
    let mut authority = Authority::new();
    authority.fail = Some(fail);
    authority.exit = true;
    store
        .preserve_cancellation(&guard, &outer, &mut authority, &Hasher)
        .unwrap();
    panic!("requested exit checkpoint was not reached");
}

#[test]
fn every_archive_boundary_revokes_authority_and_survives_process_exit() {
    let fixture = Fixture::new();
    let (store, guard, outer) = prepare(&fixture, Some(State::RolledBack), 3, true);
    let mut trace = Authority::new();
    store
        .preserve_cancellation(&guard, &outer, &mut trace, &Hasher)
        .unwrap();
    drop(guard);
    // Exercise every distinct boundary (including each concrete move slot).
    // Repeated visits to the same writer boundary use identical persistence
    // machinery; additionally exercise its last visit at final proof binding.
    let mut selected = Vec::new();
    let mut seen = Vec::new();
    for (index, point) in trace.points.iter().copied().enumerate() {
        if !seen.contains(&point) {
            selected.push((index, point));
            seen.push(point);
        }
    }
    for point in [AP::Record(CP::BeforeRename), AP::Record(CP::Renamed)] {
        let index = trace
            .points
            .iter()
            .rposition(|value| *value == point)
            .unwrap();
        if !selected.contains(&(index, point)) {
            selected.push((index, point));
        }
    }
    std::thread::scope(|scope| {
        for chunk in selected.chunks(selected.len().div_ceil(4)) {
            scope.spawn(move || {
                for (index, point) in chunk.iter().copied() {
                    for crash in [false, true] {
                        let fixture = Fixture::new();
                        let (store, guard, outer) =
                            prepare(&fixture, Some(State::RolledBack), 3, true);
                        if crash {
                            drop(guard);
                            let output = Command::new(std::env::current_exe().unwrap())
                                .args([
                                    "--exact",
                                    "cancellation_archive::archive_crash_child",
                                    "--nocapture",
                                    "--ignored",
                                ])
                                .env("RADISHLEX_CANCEL_ARCHIVE_CHILD", &fixture.0)
                                .env("RADISHLEX_CANCEL_ARCHIVE_POINT", index.to_string())
                                .output()
                                .unwrap();
                            assert_eq!(
                                output.status.code(),
                                Some(77),
                                "{index}: {point:?}: {}",
                                String::from_utf8_lossy(&output.stdout)
                            );
                        } else {
                            let mut authority = Authority::new();
                            authority.fail = Some(index);
                            assert!(
                                store
                                    .preserve_cancellation(&guard, &outer, &mut authority, &Hasher)
                                    .is_err(),
                                "{point:?}"
                            );
                            drop(guard);
                        }
                        assert!(fixture.state("preparation-cancellation.json").exists());
                        // Unbound directory/temp interruption must remain blocked. Every
                        // durably bound move/publication must recover with a fresh guard.
                        let unbound = fixture.state("cancellation-archive.json.tmp").exists()
                            || (!fixture.state("cancellation-archive.json").exists()
                                && fixture.0.join(HISTORY).join(NEW).exists());
                        if unbound {
                            if let Ok(store) =
                                CancellationArchiveStore::open_existing(fixture.verified())
                            {
                                let guard = store.acquire_guard().unwrap();
                                assert!(store
                                    .preserve_cancellation(
                                        &guard,
                                        &outer,
                                        &mut Authority::new(),
                                        &Hasher
                                    )
                                    .is_err());
                            }
                            continue;
                        }
                        let store =
                            CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
                        let guard = store.acquire_guard().unwrap();
                        let loaded = store.load_guarded(&guard, &Hasher);
                        if loaded.is_err() {
                            assert!(
                                fixture
                                    .0
                                    .join(HISTORY)
                                    .join(NEW)
                                    .join("compatibility-outer.json")
                                    .exists(),
                                "{index}: {point:?}: {loaded:?}"
                            );
                            assert!(store
                                .preserve_cancellation(
                                    &guard,
                                    &outer,
                                    &mut Authority::new(),
                                    &Hasher
                                )
                                .is_err());
                            continue;
                        }
                        let record = store
                            .preserve_cancellation(&guard, &outer, &mut Authority::new(), &Hasher)
                            .unwrap_or_else(|e| panic!("{index}: {point:?}: {e:?}"));
                        assert_preserved(&fixture, &record);
                    }
                }
            });
        }
    });
}

#[path = "cancellation_archive_rejection.rs"]
mod rejection;
