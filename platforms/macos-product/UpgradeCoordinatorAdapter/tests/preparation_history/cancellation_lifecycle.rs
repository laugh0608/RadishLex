//! Qualification below is deliberately synthetic; no installed programs run.
use super::*;
use radishlex_ime_product_upgrade::{
    CancellationReleaseCheckpoint as RP, CancellationReleasePort,
    CancellationReleaseReceipt as Proof, CancellationReleaseRequirement as Requirement,
    CancellationReleaseStore as Store, LifecycleReleaseReceipt, TerminalReleaseStore,
};

const SECOND: &str = "44444444444444444444444444444444";
const THIRD: &str = "55555555555555555555555555555555";
struct ReleaseAuthority {
    qualified: bool,
    fail: Option<RP>,
    exit: bool,
    points: Vec<RP>,
    mutation: Option<(RP, Box<dyn FnOnce()>)>,
}
impl ReleaseAuthority {
    fn new() -> Self {
        Self {
            qualified: true,
            fail: None,
            exit: false,
            points: vec![],
            mutation: None,
        }
    }
}
impl CancellationReleasePort for ReleaseAuthority {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &Proof,
        point: RP,
        requirement: Requirement,
    ) -> bool {
        assert!(proof.archive().preserved());
        assert_eq!(proof.archive().outer().previous_bytes(), OLD_BYTES);
        self.points.push(point);
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.fail == Some(point) {
            if self.exit {
                std::process::exit(76);
            }
            return false;
        }
        requirement == Requirement::Preserve || self.qualified
    }
}
fn preserved(fixture: &Fixture, state: Option<State>, stage: u8, new_outer: bool) {
    let (archive, guard, outer) = prepare(fixture, state, stage, new_outer);
    archive
        .preserve_cancellation(&guard, &outer, &mut Authority::new(), &Hasher)
        .unwrap();
}
fn reload(fixture: &Fixture) -> (Store, UpgradeProcessGuard) {
    let store = Store::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    (store, guard)
}
fn finish(fixture: &Fixture, operation: &str) -> Proof {
    let (store, guard) = reload(fixture);
    let mut port = ReleaseAuthority::new();
    let ready = store
        .prepare_cancellation_release(&guard, operation, &mut port, &Hasher)
        .unwrap();
    let (_, result) = store
        .finish_cancellation_release(&guard, operation, &mut port, &Hasher)
        .unwrap();
    assert_eq!(ready, result);
    result
}
fn query(fixture: &Fixture) -> LifecycleReleaseReceipt {
    let store = TerminalReleaseStore::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    store
        .load_latest_lifecycle(&guard, &Hasher)
        .unwrap()
        .unwrap()
}
fn reserve_next(
    fixture: &Fixture,
    operation: &str,
    previous_data: Option<&str>,
) -> (
    PreparationJournalStore,
    UpgradeProcessGuard,
    PreparationBinding,
) {
    let (store, guard) = fixture.reload();
    let inventory = store
        .capture_previous_inventory(&guard, operation, Some(OUTER), previous_data, &Hasher)
        .unwrap();
    let identity = store
        .previous_inventory_identity(&guard, &inventory, &Hasher)
        .unwrap()
        .unwrap();
    let binding = PreparationBinding {
        operation_id: operation.into(),
        previous_install_operation_id: Some(OUTER.into()),
        previous_data_operation_id: previous_data.map(str::to_owned),
        source_release: release(39),
        target_release: release(41),
        source_product_sha256: "a".repeat(64),
        target_product_sha256: "b".repeat(64),
        previous_install_receipt_sha256: Some(format!("{:x}", Sha256::digest(OLD_BYTES))),
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
    record
        .bind_released_predecessor(
            identity,
            inventory.release_index_identity().unwrap().clone(),
        )
        .unwrap();
    store.persist(&guard, None, &record).unwrap();
    (store, guard, binding)
}
fn cancel_next(fixture: &Fixture, operation: &str, previous_data: Option<&str>) -> Proof {
    let (store, guard, binding) = reserve_next(fixture, operation, previous_data);
    store
        .prepare_userdb_source(&guard, &mut Port::new(binding), &Hasher)
        .unwrap();
    let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
    let mut authority = Authority::new();
    cancel
        .request_cancellation(&guard, operation, &mut authority, &Hasher)
        .unwrap();
    cancel
        .finish_source(&guard, &mut authority, &Hasher)
        .unwrap();
    drop(guard);
    let archive = CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
    let guard = archive.acquire_guard().unwrap();
    let outer = archive.observe_outer(&guard, &Hasher).unwrap();
    archive
        .preserve_cancellation(&guard, &outer, &mut authority, &Hasher)
        .unwrap();
    drop(guard);
    finish(fixture, operation)
}

#[test]
fn cancellation_release_requires_qualification_and_preserves_originals() {
    check_qualification_cases(&[None, Some(State::RolledBack)], &[0, 5], &[true]);
}

#[cfg(feature = "reliability-matrix")]
#[test]
fn reliability_matrix_cancellation_lifecycle_requires_qualification_and_preserves_originals() {
    check_qualification_cases(
        &[
            None,
            Some(State::Completed),
            Some(State::AbortedPreserved),
            Some(State::RolledBack),
        ],
        &[0, 2, 5],
        &[false, true],
    );
}

fn check_qualification_cases(states: &[Option<State>], stages: &[u8], outers: &[bool]) {
    for &state in states {
        for &stage in stages {
            for &new_outer in outers {
                let fixture = Fixture::new();
                preserved(&fixture, state, stage, new_outer);
                let request = fs::read(fixture.state("preparation-cancellation.json")).unwrap();
                let request_inode = fs::metadata(fixture.state("preparation-cancellation.json"))
                    .unwrap()
                    .ino();
                let archive = fs::read(fixture.state("cancellation-archive.json")).unwrap();
                let source = fs::read(fixture.source()).unwrap();
                let (store, guard) = reload(&fixture);
                let mut authority = ReleaseAuthority::new();
                authority.qualified = false;
                let ready = store
                    .prepare_cancellation_release(&guard, NEW, &mut authority, &Hasher)
                    .unwrap();
                assert_eq!(Proof::decode(&ready.encode().unwrap()).unwrap(), ready);
                assert!(store
                    .finish_cancellation_release(&guard, NEW, &mut authority, &Hasher)
                    .is_err());
                assert!(fixture.state("preparation-cancellation.json").exists());
                assert!(TerminalReleaseStore::open_existing(fixture.verified()).is_err());
                assert!(PreparationJournalStore::open_existing(fixture.verified()).is_err());
                drop(guard);
                assert!(!inspect_startup_gate(
                    &fixture.0,
                    fixture.verified().identity().owner_id()
                )
                .is_allowed());
                assert_eq!(finish(&fixture, NEW), ready);
                assert!(
                    inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                        .is_allowed()
                );
                let directory = fixture.0.join(HISTORY).join(NEW);
                assert_eq!(
                    fs::read(directory.join("preparation-cancellation.json")).unwrap(),
                    request
                );
                assert_eq!(
                    fs::metadata(directory.join("preparation-cancellation.json"))
                        .unwrap()
                        .ino(),
                    request_inode
                );
                assert_eq!(
                    fs::read(directory.join("cancellation-archive.json")).unwrap(),
                    archive
                );
                assert_eq!(fs::read(fixture.source()).unwrap(), source);
                assert!(
                    matches!(query(&fixture), LifecycleReleaseReceipt::Cancellation(value) if *value == ready)
                );
            }
        }
    }
}

#[test]
fn representative_cancellation_lifecycle_continuous_cancellations_keep_null_and_legacy_data_predecessors(
) {
    check_continuous_cancellations(&[None]);
}

#[cfg(feature = "reliability-matrix")]
#[test]
fn reliability_matrix_cancellation_lifecycle_continuous_cancellations_keep_null_and_legacy_data_predecessors(
) {
    check_continuous_cancellations(&[
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ]);
}

fn check_continuous_cancellations(states: &[Option<State>]) {
    for &state in states {
        let fixture = Fixture::new();
        preserved(&fixture, state, 3, true);
        finish(&fixture, NEW);
        let first = fs::read(
            fixture
                .0
                .join(HISTORY)
                .join(NEW)
                .join("cancellation-release.json"),
        )
        .unwrap();
        let mut db = UserDb::open(fixture.source()).unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-cancel",
            "hou",
            "后",
            0,
            3,
        ))
        .unwrap();
        drop(db);
        assert_eq!(query(&fixture).operation_id(), NEW);
        cancel_next(&fixture, SECOND, state.map(|_| OLD));
        cancel_next(&fixture, THIRD, state.map(|_| OLD));
        let result = query(&fixture);
        assert_eq!(result.operation_id(), THIRD);
        assert!(matches!(
            &result,
            LifecycleReleaseReceipt::Cancellation(proof)
                if proof.predecessor_kind()
                    == radishlex_ime_product_upgrade::CancellationPredecessorKind::CancellationV2
        ));
        assert_eq!(result.legacy_outer_operation_id(), OUTER);
        assert_eq!(
            result.data_receipt().map(UpgradeReceipt::operation_id),
            state.map(|_| OLD)
        );
        assert_eq!(
            fs::read(
                fixture
                    .0
                    .join(HISTORY)
                    .join(NEW)
                    .join("cancellation-release.json")
            )
            .unwrap(),
            first
        );
        let (store, guard, binding) = reserve_next(
            &fixture,
            "66666666666666666666666666666666",
            state.map(|_| OLD),
        );
        let upgraded = super::super::terminal_release::upgrade_after_cancellation(
            &fixture, store, &guard, binding,
        );
        assert_eq!(
            upgraded.data_receipt().previous_operation_id(),
            state.map(|_| OLD)
        );
        drop(guard);
        assert_eq!(
            query(&fixture).operation_id(),
            "66666666666666666666666666666666"
        );
    }
}

#[path = "cancellation_lifecycle_failures.rs"]
mod failures;

#[path = "maintenance_inspection.rs"]
mod inspection;
