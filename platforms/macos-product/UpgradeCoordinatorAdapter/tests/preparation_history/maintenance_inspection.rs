use super::*;
use radishlex_ime_product_upgrade::{inspect_maintenance, MaintenancePhase as Phase};

fn observe(
    fixture: &Fixture,
) -> Result<Option<Phase>, radishlex_ime_product_upgrade::SourcePreparationError> {
    let before = failures::tree(&fixture.0);
    let result = inspect_maintenance(
        &fixture.0,
        fixture.verified().identity().owner_id(),
        &Hasher,
    )
    .map(|value| value.map(|value| value.phase()));
    assert_eq!(
        failures::tree(&fixture.0),
        before,
        "query must not change any file or directory"
    );
    result
}

struct StopSource;
impl CancellationSourcePort for StopSource {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: &CancellationSourceReceipt,
        point: CancellationSourceCheckpoint,
    ) -> bool {
        point != CancellationSourceCheckpoint::Begin
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}

#[test]
fn maintenance_inspection_preparation_request_and_source_progress_are_read_only() {
    let fixture = Fixture::new();
    let (journal, guard, _) = fixture.reserve(None);
    assert!(
        observe(&fixture).is_err(),
        "active guard must block inspection"
    );
    drop(guard);
    assert_eq!(observe(&fixture).unwrap(), Some(Phase::Preparing));
    let guard = journal.acquire_guard().unwrap();
    let cancel = PreparationCancellationStore::attach(journal, &guard).unwrap();
    cancel
        .request_cancellation(&guard, NEW, &mut Authority::new(), &Hasher)
        .unwrap();
    drop(guard);
    assert_eq!(
        observe(&fixture).unwrap(),
        Some(Phase::CancellationRequested)
    );
    let guard = cancel.acquire_guard().unwrap();
    assert!(cancel
        .finish_source(&guard, &mut StopSource, &Hasher)
        .is_err());
    drop(guard);
    assert_eq!(
        observe(&fixture).unwrap(),
        Some(Phase::CancellationFinishingSource)
    );
    let guard = cancel.acquire_guard().unwrap();
    cancel
        .finish_source(&guard, &mut Authority::new(), &Hasher)
        .unwrap();
    drop(guard);
    assert_eq!(
        observe(&fixture).unwrap(),
        Some(Phase::CancellationPreserving)
    );
}

#[test]
fn maintenance_inspection_requires_final_marker_and_preserves_later_learning() {
    for prior in [
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ] {
        for stage in [0, 3] {
            let fixture = Fixture::new();
            preserved(&fixture, prior, stage, true);
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationPreserving)
            );
            let (store, guard) = reload(&fixture);
            store
                .prepare_cancellation_release(&guard, NEW, &mut ReleaseAuthority::new(), &Hasher)
                .unwrap();
            drop(guard);
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationAwaitingQualification)
            );
            let observation = inspect_maintenance(
                &fixture.0,
                fixture.verified().identity().owner_id(),
                &Hasher,
            )
            .unwrap()
            .unwrap();
            assert!(observation.cancellation_outer().is_some());
            let guard = store.acquire_guard().unwrap();
            let mut unqualified = ReleaseAuthority::new();
            unqualified.qualified = false;
            assert!(store
                .finish_cancellation_release(&guard, NEW, &mut unqualified, &Hasher)
                .is_err());
            drop(guard);
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationAwaitingQualification)
            );
            let (store, guard) = reload(&fixture);
            store
                .finish_cancellation_release(&guard, NEW, &mut ReleaseAuthority::new(), &Hasher)
                .unwrap();
            drop(guard);
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationReleased)
            );
            let journal = PreparationJournalStore::open_existing(fixture.verified()).unwrap();
            let before = failures::tree(&fixture.0);
            let (data, lifecycle) = journal.inspect_preparation_predecessor(&Hasher).unwrap();
            assert_eq!(data.as_ref().map(UpgradeReceipt::state), prior);
            assert!(matches!(
                lifecycle,
                Some(radishlex_ime_product_upgrade::LifecycleReleaseReceipt::Cancellation(_))
            ));
            assert_eq!(failures::tree(&fixture.0), before);
            let mut db = UserDb::open(fixture.source()).unwrap();
            db.record_selection(SelectionEventDraft::new(
                "synthetic-after-cancel",
                "xin",
                "新",
                0,
                1,
            ))
            .unwrap();
            drop(db);
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationReleased)
            );
            let (journal, guard, _) = reserve_next(&fixture, SECOND, prior.map(|_| OLD));
            drop(guard);
            assert_eq!(observe(&fixture).unwrap(), Some(Phase::Preparing));
            drop(journal);
        }
    }
}

#[test]
fn maintenance_inspection_rejects_missing_changed_and_orphaned_materials() {
    for mutation in 0..6 {
        let fixture = Fixture::new();
        preserved(&fixture, None, 0, false);
        finish(&fixture, NEW);
        assert_eq!(
            observe(&fixture).unwrap(),
            Some(Phase::CancellationReleased)
        );
        let history = fixture.0.join(HISTORY);
        let operation = history.join(NEW);
        let index = history.join("latest-release.json");
        match mutation {
            0 => fs::rename(&index, fixture.0.join("saved-index")).unwrap(),
            1 => fs::write(operation.join("cancellation-release.json"), b"{}\n").unwrap(),
            2 => fs::rename(
                operation.join("preparation-cancellation.json"),
                fixture.state("preparation-cancellation.json"),
            )
            .unwrap(),
            3 => put(&fixture.state("unexpected"), b"unexpected"),
            4 => {
                let proof = operation.join("cancellation-release.json");
                let bytes = fs::read(&proof).unwrap();
                fs::rename(&proof, fixture.0.join("saved-proof")).unwrap();
                put(&proof, &bytes);
            }
            5 => fs::rename(
                operation.join("cancellation-archive.json"),
                fixture.0.join("saved-archive"),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        if mutation == 2 {
            assert_eq!(
                observe(&fixture).unwrap(),
                Some(Phase::CancellationAwaitingQualification)
            );
        } else {
            assert!(observe(&fixture).is_err(), "mutation {mutation}");
        }
        let before = failures::tree(&fixture.0);
        if let Ok(journal) = PreparationJournalStore::open_existing(fixture.verified()) {
            assert!(
                journal.inspect_preparation_predecessor(&Hasher).is_err(),
                "new preparation must reject incomplete lifecycle {mutation}"
            );
        }
        assert_eq!(failures::tree(&fixture.0), before);
    }
}

#[test]
fn maintenance_inspection_absent_roots_and_state_are_zero_creation() {
    let fixture = Fixture::new();
    let owner = fs::metadata(&fixture.0).unwrap().uid();
    let before = failures::tree(&fixture.0);
    assert!(
        inspect_maintenance(fixture.0.join("absent"), owner, &Hasher)
            .unwrap()
            .is_none()
    );
    assert!(inspect_maintenance(&fixture.0, owner, &Hasher)
        .unwrap()
        .is_none());
    assert_eq!(failures::tree(&fixture.0), before);
    let history = fixture.0.join(HISTORY);
    DirBuilder::new().mode(0o700).create(&history).unwrap();
    assert!(inspect_maintenance(&fixture.0, owner, &Hasher).is_err());
    assert!(!fixture.0.join(STATE).exists());
}
