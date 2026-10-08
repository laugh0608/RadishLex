//! Exact released predecessors use real SQLite/files and explicit synthetic
//! qualification only. These tests never execute an old binary or native host.
use super::*;
use radishlex_ime_product_upgrade::*;
use radishlex_ime_userdb::SelectionEventDraft;

const PRIOR: &str = "11111111111111111111111111111111";
const NEXT: &str = "cccccccccccccccccccccccccccccccc";

struct NextId(&'static str);
impl InstallerOperationIdSource for NextId {
    fn next_operation_id(&mut self) -> std::result::Result<String, crate::InstallerExecutionError> {
        Ok(self.0.into())
    }
}
struct Synthetic;
impl UpgradeCoordinatorPort for Synthetic {
    fn confirm_quiescence(&mut self, _: UpgradeCoordinatorCheckpoint) -> bool {
        true
    }
    fn validate_candidate(
        &mut self,
        _: &ProductRelease,
        _: i64,
    ) -> UpgradeCandidateValidationReport {
        UpgradeCandidateValidationReport::manager_failed()
    }
    fn validate_post_switch(
        &mut self,
        _: &ProductRelease,
        _: i64,
    ) -> UpgradePostSwitchValidationReport {
        panic!("pre-switch abort")
    }
    fn validate_restored_source(
        &mut self,
        _: &ProductRelease,
        _: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        panic!("pre-switch abort")
    }
}
struct TerminalPort {
    finalized: bool,
}
impl TerminalReleasePort for TerminalPort {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &TerminalReleaseReceipt,
        _: TerminalReleaseCheckpoint,
        requirement: TerminalReleaseOuterRequirement,
    ) -> bool {
        self.finalized == (requirement == TerminalReleaseOuterRequirement::MatchingTerminal)
    }
}
impl PreparationCancellationPort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: PreparationCancellationCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationSourcePort for Synthetic {
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
impl CancellationArchivePort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &CancellationArchiveReceipt,
        _: CancellationArchiveCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationReleasePort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &CancellationReleaseReceipt,
        _: CancellationReleaseCheckpoint,
        _: CancellationReleaseRequirement,
    ) -> bool {
        true
    }
}

fn release_data(fixture: &Fixture, operation: &str, legacy: bool) {
    let install = fixture.install();
    let _outer = install.acquire_guard().unwrap();
    let root = || VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap();
    if legacy {
        UserDb::migrate_and_validate(fixture.path.join("userdb.sqlite3")).unwrap();
    }
    let store = UpgradeReceiptStore::open(root()).unwrap();
    let guard = store.acquire_guard().unwrap();
    let mut receipt = if legacy {
        let schema = UserDb::supported_schema_version();
        let receipt = UpgradeReceipt::new(
            operation,
            None,
            ProductRelease::new("26.7.1", 39).unwrap(),
            ProductRelease::new("26.7.1", 41).unwrap(),
            Some(schema),
            schema,
            vec![
                store.data_root_identity().clone(),
                artifact(
                    UpgradeArtifactSlot::SourceDatabase,
                    &fixture.path.join("userdb.sqlite3"),
                ),
                artifact(
                    UpgradeArtifactSlot::SourceSettings,
                    &fixture.path.join("manager-settings.json"),
                ),
            ],
        )
        .unwrap();
        store.persist(&guard, &receipt).unwrap();
        receipt
    } else {
        store.load_guarded(&guard).unwrap().unwrap()
    };
    store
        .resume_userdb_upgrade(&guard, &mut receipt, u64::MAX, &mut Synthetic)
        .unwrap();
    assert_eq!(receipt.state(), UpgradeState::AbortedPreserved);
    let store = TerminalReleaseStore::attach(store, &guard).unwrap();
    let binding = TerminalReleaseBinding {
        operation_id: operation.into(),
        installed_release: ProductRelease::new("26.7.1", 39).unwrap(),
        installed_product_sha256: fixture.source.product_manifest_sha256().into(),
    };
    let mut port = TerminalPort { finalized: false };
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &MacOsPreparationHasher)
        .unwrap();
    port.finalized = true;
    store
        .finish_terminal_release(&guard, operation, &mut port, &MacOsPreparationHasher)
        .unwrap();
}

fn learn(fixture: &Fixture) {
    let mut db = UserDb::open(fixture.path.join("userdb.sqlite3")).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-next-preparation",
        "xin",
        "新",
        0,
        9,
    ))
    .unwrap();
    db.delete_term("xin", "新", None).unwrap();
}

fn cancel(fixture: &Fixture, operation: &str) -> CancellationReleaseReceipt {
    let install = fixture.install();
    let _outer = install.acquire_guard().unwrap();
    let root = || VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap();
    let store = PreparationCancellationStore::open_existing(root()).unwrap();
    let guard = store.acquire_guard().unwrap();
    store
        .request_cancellation(&guard, operation, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap();
    store
        .finish_source(&guard, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap();
    let archive = CancellationArchiveStore::open_existing(root()).unwrap();
    let binding = archive
        .observe_outer(&guard, &MacOsPreparationHasher)
        .unwrap();
    archive
        .preserve_cancellation(&guard, &binding, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap();
    let release = CancellationReleaseStore::open_existing(root()).unwrap();
    release
        .prepare_cancellation_release(&guard, operation, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap();
    release
        .finish_cancellation_release(&guard, operation, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap()
        .1
}

#[test]
fn released_v1_and_v2_predecessors_preserve_history_after_new_learning() {
    for legacy in [true, false] {
        let mut fixture = Fixture::new();
        if !legacy {
            let (grant, _control) = fixture.grant();
            execute(
                grant,
                &fixture.install(),
                &mut Observer::default(),
                &mut NextId(PRIOR),
            )
            .unwrap();
        }
        release_data(&fixture, PRIOR, legacy);
        if !legacy {
            let install = fixture.install();
            let guard = install.acquire_guard().unwrap();
            let mut current = install.load_guarded(&guard).unwrap().unwrap();
            current
                .abort_preserved(
                    radishlex_ime_product_install::InstallFailureCode::SourceArtifactInvalid,
                )
                .unwrap();
            install.persist(&guard, &current).unwrap();
            fixture.previous = current;
        }
        learn(&fixture);
        let history_root = fixture.path.join(".radishlex-upgrade-history-v1");
        let before = tree(&history_root.join(PRIOR));
        let old_index = fs::read(history_root.join("latest-release.json")).unwrap();
        let receipt = fixture.run(&mut Observer::default()).unwrap();
        assert_eq!(receipt.previous_operation_id(), Some(PRIOR));
        assert_eq!(tree(&history_root.join(PRIOR)), before);
        assert_eq!(
            fs::read(history_root.join("latest-release.json")).unwrap(),
            old_index
        );
        assert!(fixture
            .preparation()
            .previous_release_index_identity()
            .is_some());
        assert!(fixture.preparation().archived_slots().is_empty());
    }
}

#[test]
fn cancelled_predecessors_and_repeated_cancellation_reuse_only_proven_outer_original() {
    for data in [
        None,
        Some(UpgradeState::Completed),
        Some(UpgradeState::AbortedPreserved),
        Some(UpgradeState::RolledBack),
    ] {
        let fixture = Fixture::new();
        if let Some(state) = data {
            fixture.legacy(state);
        }
        assert!(fixture
            .run(&mut Observer {
                fail: Some(ExecutionPoint::Source(
                    SourcePreparationCheckpoint::SourceRecorded
                )),
                ..Observer::default()
            })
            .is_err());
        let proof = cancel(&fixture, NEW);
        learn(&fixture);
        let old_original = fixture.path.join(".radishlex-install-history-v1").join(OLD);
        let before = tree(&old_original);
        let (grant, _control) = fixture.grant();
        execute(
            grant,
            &fixture.install(),
            &mut Observer {
                fail: Some(ExecutionPoint::Source(
                    SourcePreparationCheckpoint::SourceRecorded,
                )),
                ..Observer::default()
            },
            &mut NextId(NEXT),
        )
        .unwrap_err();
        let proof2 = cancel(&fixture, NEXT);
        assert_eq!(proof2.legacy_outer_operation_id(), OLD);
        assert_eq!(
            proof2.data_receipt().map(UpgradeReceipt::operation_id),
            data.map(|_| DATA)
        );
        assert_eq!(
            proof2.archive().outer().previous_install_evidence(),
            proof.archive().outer().previous_install_evidence()
        );
        assert_eq!(tree(&old_original), before);
        let (grant, _control) = fixture.grant();
        let receipt = execute(
            grant,
            &fixture.install(),
            &mut Observer::default(),
            &mut NextId(PRIOR),
        )
        .unwrap();
        assert_eq!(receipt.previous_operation_id(), data.map(|_| DATA));
        assert_eq!(tree(&old_original), before);
    }
}

#[test]
fn missing_or_drifted_released_history_and_active_cancellation_never_become_first_preparation() {
    for corrupt in [false, true] {
        let fixture = Fixture::new();
        release_data(&fixture, PRIOR, true);
        let index = fixture
            .path
            .join(".radishlex-upgrade-history-v1/latest-release.json");
        if corrupt {
            fs::write(&index, b"invalid\n").unwrap();
        } else {
            fs::rename(&index, index.with_extension("saved")).unwrap();
        }
        let before = tree(&fixture.path);
        assert!(fixture.run(&mut Observer::default()).is_err());
        assert_eq!(tree(&fixture.path), before);
    }
    let fixture = Fixture::new();
    assert!(fixture
        .run(&mut Observer {
            fail: Some(ExecutionPoint::Source(
                SourcePreparationCheckpoint::SourceRecorded
            )),
            ..Observer::default()
        })
        .is_err());
    let preparation = fixture.preparation();
    let install = fixture.install();
    let outer = install.acquire_guard().unwrap();
    let store = PreparationCancellationStore::open_existing(
        VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
    )
    .unwrap();
    let inner = store.acquire_guard().unwrap();
    store
        .request_cancellation(&inner, NEW, &mut Synthetic, &MacOsPreparationHasher)
        .unwrap();
    drop(inner);
    drop(outer);
    let (grant, _control) = authorize_preparation_resume(
        &preparation,
        &fixture.source,
        &fixture.target,
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap();
    let before = tree(&fixture.path);
    assert!(execute(grant, &install, &mut Observer::default(), &mut NoIds).is_err());
    assert_eq!(tree(&fixture.path), before);
}
