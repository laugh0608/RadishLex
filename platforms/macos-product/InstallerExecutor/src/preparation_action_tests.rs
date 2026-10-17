use super::*;
use crate::{inspect_source_preparation, InstallerAction};

fn observe(f: &Fixture) -> Option<crate::SourcePreparationObservation> {
    inspect_source_preparation(&f.install(), &f.target).unwrap()
}

#[test]
fn preparation_selection_reaches_existing_executor_and_rejects_stale_actions() {
    let f = Fixture::new();
    let before = tree(&f.path);
    let start = observe(&f).unwrap();
    assert!(start.offers(InstallerAction::BeginSourcePreparation));
    assert!(!start.offers(InstallerAction::ContinuePreparedUpgrade));
    assert_eq!(tree(&f.path), before, "query cannot reserve or checkpoint");
    let (auth, _control) = authorize_source_preparation(
        start.previous().unwrap(),
        start.target(),
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap();
    execute(
        auth,
        &f.install(),
        &mut Observer {
            fail: Some(ExecutionPoint::OuterPersisted),
            ..Observer::default()
        },
        &mut Ids,
    )
    .unwrap_err();
    let before = tree(&f.path);
    let resumed = observe(&f).unwrap();
    assert!(!resumed.offers(InstallerAction::BeginSourcePreparation));
    assert!(resumed.offers(InstallerAction::ResumeSourcePreparation));
    assert!(resumed.offers(InstallerAction::PreservePreparationCancellation));
    assert_eq!(resumed.source(), &f.source);
    assert_eq!(tree(&f.path), before);
    let (auth, _control) = authorize_preparation_resume(
        resumed.maintenance().unwrap().preparation(),
        resumed.source(),
        resumed.target(),
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap();
    let result = execute(auth, &f.install(), &mut Observer::default(), &mut NoIds).unwrap();
    assert_eq!(result.state(), UpgradeState::Preflighted);
    assert_eq!(
        f.install()
            .load_for_recovery_inspection()
            .unwrap()
            .unwrap()
            .state(),
        radishlex_ime_product_install::InstallState::Prepared
    );
    assert_eq!(
        observe(&f),
        None,
        "handoff must use a separately confirmed action 10"
    );
}

#[test]
fn persisted_handoff_intent_removes_cancellation_before_marker_is_moved() {
    let f = Fixture::new();
    f.run(&mut Observer {
        fail: Some(ExecutionPoint::Source(
            SourcePreparationCheckpoint::HandoffIntentRecorded,
        )),
        ..Observer::default()
    })
    .unwrap_err();
    let before = tree(&f.path);
    let selected = observe(&f).unwrap();
    assert!(selected.offers(InstallerAction::ResumeSourcePreparation));
    assert!(!selected.offers(InstallerAction::PreservePreparationCancellation));
    assert!(crate::authorize_cancellation_preservation(
        selected.maintenance().unwrap(),
        selected.source(),
        selected.target(),
        confirmation(),
        Duration::from_secs(300)
    )
    .is_err());
    assert_eq!(tree(&f.path), before);
}

#[test]
fn preparation_selection_rejects_foreign_products_active_guards_and_unbound_materials() {
    let f = Fixture::new();
    let store = f.install();
    let guard = store.acquire_guard().unwrap();
    assert!(inspect_source_preparation(&store, &f.target).is_err());
    drop(guard);
    assert!(inspect_source_preparation(&store, &f.source).is_err());
    put(&f.path.join(".radishlex-upgrade-history-v1"), b"orphan");
    let before = tree(&f.path);
    assert!(inspect_source_preparation(&store, &f.target).is_err());
    assert_eq!(tree(&f.path), before);
}
