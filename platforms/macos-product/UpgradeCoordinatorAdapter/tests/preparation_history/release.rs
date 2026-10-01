//! Real SQLite / files and synthetic product validation and outer finalization.
use super::*;
use radishlex_ime_product_upgrade::{
    TerminalReleaseBinding as Binding, TerminalReleaseCheckpoint as RP,
    TerminalReleaseDirectory as RD, TerminalReleaseOuterRequirement as Outer,
    TerminalReleasePhase as Phase, TerminalReleasePort, TerminalReleaseReceipt as Proof,
    TerminalReleaseStore as Store, UpgradeCandidateValidationReport, UpgradeCoordinatorCheckpoint,
    UpgradeCoordinatorPort, UpgradeInputMethodValidationEvidence, UpgradeManagerValidationEvidence,
    UpgradePostSwitchValidationReport, UpgradeRollbackValidationEvidence,
};

#[path = "release_chain.rs"]
mod chain;
#[path = "release_failures.rs"]
mod failures;
#[path = "release_predecessor.rs"]
mod predecessor;

struct ProductPort(State);
impl UpgradeCoordinatorPort for ProductPort {
    fn confirm_quiescence(&mut self, _: UpgradeCoordinatorCheckpoint) -> bool {
        true
    }
    fn validate_candidate(
        &mut self,
        _: &ProductRelease,
        schema: i64,
    ) -> UpgradeCandidateValidationReport {
        if self.0 == State::AbortedPreserved {
            UpgradeCandidateValidationReport::manager_failed()
        } else {
            UpgradeCandidateValidationReport::passed(
                UpgradeManagerValidationEvidence::new(1, schema, 1, 1),
                UpgradeInputMethodValidationEvidence::new(1, schema, 1, 1),
            )
        }
    }
    fn validate_post_switch(
        &mut self,
        _: &ProductRelease,
        schema: i64,
    ) -> UpgradePostSwitchValidationReport {
        if self.0 == State::RolledBack {
            UpgradePostSwitchValidationReport::manager_failed()
        } else {
            UpgradePostSwitchValidationReport::passed(
                UpgradeManagerValidationEvidence::new(1, schema, 1, 1),
                UpgradeInputMethodValidationEvidence::new(1, schema, 1, 1),
            )
        }
    }
    fn validate_restored_source(
        &mut self,
        _: &ProductRelease,
        schema: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        Some(UpgradeRollbackValidationEvidence::new(1, schema, 1, 1))
    }
}
struct ReleasePort {
    binding: Binding,
    terminal: bool,
    outer_proof: Option<Vec<u8>>,
    fail: Option<RP>,
    exit: Option<RP>,
    skip: usize,
    mutation: Option<(RP, Box<dyn FnOnce()>)>,
}
impl ReleasePort {
    fn new(binding: Binding) -> Self {
        Self {
            binding,
            terminal: false,
            outer_proof: None,
            fail: None,
            exit: None,
            skip: 0,
            mutation: None,
        }
    }
    fn finalize(&mut self, root: &Path) {
        let active = root.join(STATE).join("terminal-release.json");
        let path = if active.exists() {
            active
        } else {
            root.join(HISTORY)
                .join(&self.binding.operation_id)
                .join("terminal-release.json")
        };
        let proof = Proof::decode(&fs::read(path).unwrap()).unwrap();
        assert_eq!(proof.phase(), Phase::ReleaseReady);
        self.outer_proof = Some(proof.encode().unwrap());
        self.terminal = true;
    }
}
impl TerminalReleasePort for ReleasePort {
    fn confirm_authority_and_quiescence(&mut self, proof: &Proof, point: RP, outer: Outer) -> bool {
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.exit == Some(point) {
            std::process::exit(75);
        }
        let reject = if self.fail == Some(point) {
            if self.skip == 0 {
                true
            } else {
                self.skip -= 1;
                false
            }
        } else {
            false
        };
        proof.binding() == &self.binding
            && !reject
            && self.terminal == (outer == Outer::MatchingTerminal)
            && (outer == Outer::Nonterminal || proof.encode().ok() == self.outer_proof)
    }
}
fn terminal(
    fixture: &Fixture,
    prepared: bool,
    state: State,
) -> (Store, UpgradeProcessGuard, Binding, ReleasePort) {
    let (store, guard, mut receipt) = if prepared {
        let (store, guard, mut port) = fixture.reserve(None);
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        let (store, receipt) = store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        (store, guard, receipt)
    } else {
        standalone(fixture, NEW, None, 39, 41)
    };
    store
        .resume_userdb_upgrade(&guard, &mut receipt, u64::MAX, &mut ProductPort(state))
        .unwrap();
    assert_eq!(receipt.state(), state);
    let binding = Binding {
        operation_id: NEW.to_owned(),
        installed_release: release(if state == State::Completed { 41 } else { 39 }),
        installed_product_sha256: if state == State::Completed { "b" } else { "a" }.repeat(64),
    };
    let port = ReleasePort::new(binding.clone());
    (Store::attach(store, &guard).unwrap(), guard, binding, port)
}
fn standalone(
    fixture: &Fixture,
    operation: &str,
    previous: Option<String>,
    source: u64,
    target: u64,
) -> (UpgradeReceiptStore, UpgradeProcessGuard, UpgradeReceipt) {
    UserDb::migrate_and_validate(fixture.source()).unwrap();
    let store = UpgradeReceiptStore::open(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    let receipt = UpgradeReceipt::new(
        operation,
        previous,
        release(source),
        release(target),
        Some(9),
        9,
        vec![
            store.data_root_identity().clone(),
            artifact(Artifact::SourceDatabase, &fixture.source()),
            artifact(
                Artifact::SourceSettings,
                &fixture.0.join("manager-settings.json"),
            ),
        ],
    )
    .unwrap();
    store.persist(&guard, &receipt).unwrap();
    (store, guard, receipt)
}
fn history(fixture: &Fixture, name: &str) -> PathBuf {
    fixture.0.join(HISTORY).join(NEW).join(name)
}
fn reload(fixture: &Fixture) -> (Store, UpgradeProcessGuard) {
    let store = Store::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    (store, guard)
}

#[test]
fn all_real_data_terminals_release_only_after_outer_finalization_and_allow_later_wal() {
    for prepared in [false, true] {
        for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
            let fixture = Fixture::new();
            let (store, guard, binding, mut port) = terminal(&fixture, prepared, state);
            let source = fs::read(fixture.source()).unwrap();
            let state_inode = fs::metadata(fixture.0.join(STATE)).unwrap().ino();
            let files: Vec<_> = FILES
                .iter()
                .filter_map(|(_, name)| {
                    fs::read(fixture.state(name)).ok().map(|bytes| {
                        (
                            *name,
                            fs::metadata(fixture.state(name)).unwrap().ino(),
                            bytes,
                        )
                    })
                })
                .collect();
            let ready = store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .unwrap();
            assert_eq!(ready.phase(), Phase::ReleaseReady);
            assert_eq!(ready.data_receipt().state(), state);
            assert!(fixture.state("terminal-release.json").exists());
            assert!(UpgradeReceiptStore::open_existing(fixture.verified()).is_err());
            assert!(
                store.load_latest_release(&guard, &Hasher).is_err(),
                "index alone is not release"
            );
            assert!(store
                .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
                .is_err());
            drop(guard);
            assert!(
                !inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                    .is_allowed()
            );
            let (store, guard) = reload(&fixture);
            port.finalize(&fixture.0);
            let (v1, released) = store
                .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
                .unwrap();
            assert_eq!(released, ready);
            assert_eq!(v1.load_guarded(&guard).unwrap(), None);
            assert_eq!(
                fs::metadata(fixture.0.join(STATE)).unwrap().ino(),
                state_inode
            );
            assert_eq!(fs::read(fixture.source()).unwrap(), source);
            for (name, inode, bytes) in files {
                let path = history(&fixture, "data").join(name);
                assert_eq!(fs::metadata(path.clone()).unwrap().ino(), inode);
                assert_eq!(fs::read(path).unwrap(), bytes);
                assert!(!fixture.state(name).exists());
            }
            let record_bytes = fs::read(history(&fixture, "terminal-release.json")).unwrap();
            drop(guard);
            let (store, guard) = reload(&fixture);
            let (v1, replayed) = store
                .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
                .unwrap();
            assert_eq!(replayed, released);
            assert_eq!(
                fs::read(history(&fixture, "terminal-release.json")).unwrap(),
                record_bytes
            );
            drop(guard);
            let mut db = UserDb::open(fixture.source()).unwrap();
            db.record_selection(SelectionEventDraft::new(
                "synthetic-release",
                "xin",
                "新",
                0,
                1,
            ))
            .unwrap();
            db.delete_term("xin", "新", None).unwrap();
            let source_after_learning = fs::read(fixture.source()).unwrap();
            let wal = fs::read(fixture.0.join("userdb.sqlite3-wal")).unwrap();
            let store = Store::open_existing(fixture.verified()).unwrap();
            let guard = store.acquire_guard().unwrap();
            assert_eq!(
                store.load_latest_release(&guard, &Hasher).unwrap(),
                Some(released)
            );
            assert_eq!(fs::read(fixture.source()).unwrap(), source_after_learning);
            assert_eq!(fs::read(fixture.0.join("userdb.sqlite3-wal")).unwrap(), wal);
            drop(guard);
            assert!(
                inspect_startup_gate(&fixture.0, fixture.verified().identity().owner_id())
                    .is_allowed()
            );
            drop(db);
            drop(v1);
        }
    }
}

#[test]
fn second_release_preserves_previous_locator_and_immutable_history() {
    let fixture = Fixture::new();
    let (store, guard, binding, mut port) = terminal(&fixture, true, State::Completed);
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    port.finalize(&fixture.0);
    store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    drop(guard);
    let old_index = fs::read(fixture.0.join(HISTORY).join("latest-release.json")).unwrap();
    let old_proof = fs::read(history(&fixture, "terminal-release.json")).unwrap();
    let next = "44444444444444444444444444444444";
    let (v1, guard, mut receipt) = standalone(&fixture, next, Some(NEW.to_owned()), 41, 43);
    v1.resume_userdb_upgrade(
        &guard,
        &mut receipt,
        u64::MAX,
        &mut ProductPort(State::Completed),
    )
    .unwrap();
    let store = Store::attach(v1, &guard).unwrap();
    let binding = Binding {
        operation_id: next.to_owned(),
        installed_release: release(43),
        installed_product_sha256: "c".repeat(64),
    };
    let mut port = ReleasePort::new(binding.clone());
    let ready = store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&ready.encode().unwrap()).unwrap();
    let previous: serde_json::Value = serde_json::from_slice(&old_index).unwrap();
    assert_eq!(json["previous_index"]["index"], previous);
    port.finalize(&fixture.0);
    let (store, proof) = store
        .finish_terminal_release(&guard, next, &mut port, &Hasher)
        .unwrap();
    let store = Store::attach(store, &guard).unwrap();
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(proof)
    );
    assert_eq!(
        fs::read(history(&fixture, "terminal-release.json")).unwrap(),
        old_proof
    );
}
