//! Real files/SQLite with synthetic product observations; no native execution.
use super::*;
use radishlex_ime_product_install::InstallState;
use radishlex_ime_product_upgrade::{
    inspect_maintenance, MaintenancePhase, PreparationHasher, UpgradeFailureCode,
};
use radishlex_macos_installer_driver::{
    InstallerAction, InstallerProductSituation, InstallerStableError, InstallerViewPhase,
};
use std::cell::Cell;
use std::io::Read;

fn inspect(fixture: &Fixture) -> radishlex_ime_product_upgrade::MaintenanceInspection {
    let before = tree(&fixture.path);
    let value = inspect_maintenance(&fixture.path, fixture.owner(), &MacOsPreparationHasher)
        .unwrap()
        .unwrap();
    assert_eq!(tree(&fixture.path), before, "inspection must be zero-write");
    value
}

pub(super) fn assert_product_binding(fixture: &Fixture, expected: &UpgradeReceipt) {
    let before = tree(&fixture.path);
    let observation = inspect(fixture);
    assert_eq!(observation.phase(), MaintenancePhase::PreparationHandedOff);
    assert_eq!(observation.handed_off_receipt(), Some(expected));
    let view = crate::inspect_installer_view_with_recovery(
        &fixture.path,
        fixture.owner(),
        InstallerProductSituation::OlderReleaseInstalled,
    );
    assert_eq!(view.phase(), InstallerViewPhase::PreparingSource);
    assert_eq!(view.stable_error(), InstallerStableError::None);
    assert_eq!(view.primary_action(), InstallerAction::Refresh);
    assert_eq!(view.secondary_action(), InstallerAction::None);
    let install = fixture.install();
    let outer = install.acquire_guard().unwrap();
    assert!(crate::maintenance_inspection::require_no_maintenance(&install).is_err());
    let (store, inner, receipt) =
        crate::rebind_prepared_upgrade(&install, &outer, observation.preparation()).unwrap();
    assert_eq!(&receipt, expected);
    assert_eq!(store.load_guarded(&inner).unwrap().as_ref(), Some(expected));
    assert!(store.acquire_guard().is_err());
    drop(inner);
    drop(outer);
    assert_eq!(
        tree(&fixture.path),
        before,
        "binding must preserve persisted evidence"
    );
}

#[test]
fn partial_handoff_queries_preserve_moves_and_never_claim_a_completed_binding() {
    for point in [
        SourcePreparationCheckpoint::HandoffIntentRecorded,
        SourcePreparationCheckpoint::HandoffReceiptRenamed,
        SourcePreparationCheckpoint::HandoffReadyRecorded,
        SourcePreparationCheckpoint::HandoffSnapshotRenamed,
        SourcePreparationCheckpoint::HandoffMarkerRenamed,
        SourcePreparationCheckpoint::HandoffComplete,
    ] {
        let fixture = Fixture::new();
        fixture.legacy(UpgradeState::AbortedPreserved);
        fixture
            .run(&mut Observer {
                fail: Some(ExecutionPoint::Source(point)),
                ..Observer::default()
            })
            .unwrap_err();
        let observation = inspect(&fixture);
        let handed_off = matches!(
            point,
            SourcePreparationCheckpoint::HandoffMarkerRenamed
                | SourcePreparationCheckpoint::HandoffComplete
        );
        assert_eq!(
            observation.phase(),
            if handed_off {
                MaintenancePhase::PreparationHandedOff
            } else {
                MaintenancePhase::Preparing
            },
            "{point:?}"
        );
        let before = tree(&fixture.path);
        let install = fixture.install();
        let outer = install.acquire_guard().unwrap();
        let result = crate::rebind_prepared_upgrade(&install, &outer, observation.preparation());
        assert_eq!(result.is_ok(), handed_off, "{point:?}");
        drop(result);
        drop(outer);
        assert_eq!(tree(&fixture.path), before);
        let receipt = fixture.resume(&mut Observer::default()).unwrap();
        assert_product_binding(&fixture, &receipt);
    }
}

fn core_binding(fixture: &Fixture, expected: &PreparationReceipt) -> UpgradeReceipt {
    let before = tree(&fixture.path);
    let journal = PreparationJournalStore::open_existing(
        VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
    )
    .unwrap();
    let guard = journal.acquire_guard().unwrap();
    let (store, receipt) = journal
        .rebind_handed_off_upgrade(&guard, expected, &MacOsPreparationHasher)
        .unwrap();
    assert_eq!(store.load_guarded(&guard).unwrap(), Some(receipt.clone()));
    drop(guard);
    assert_eq!(tree(&fixture.path), before);
    receipt
}

#[test]
fn evolving_v1_receipt_preserves_preparation_lineage_and_distinct_protection_snapshot() {
    for abort in [false, true] {
        let fixture = Fixture::new();
        fixture.legacy(UpgradeState::AbortedPreserved);
        let mut current = fixture.run(&mut Observer::default()).unwrap();
        let prep = fixture.preparation();
        let history = fixture.path.join(".radishlex-upgrade-history-v1");
        let sealed = tree(&history);
        for step in 0..5 {
            let store = UpgradeReceiptStore::open_existing(
                VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
            )
            .unwrap();
            let guard = store.acquire_guard().unwrap();
            match step {
                0 => {
                    current.advance(UpgradeState::Quiesced).unwrap();
                    store.persist(&guard, &current).unwrap();
                }
                1 => {
                    store.create_settings_backup(&guard, &mut current).unwrap();
                    store
                        .create_userdb_snapshot(&guard, &mut current, u64::MAX)
                        .unwrap();
                }
                2 => {
                    store.create_userdb_candidate(&guard, &mut current).unwrap();
                }
                3 => {
                    current.advance(UpgradeState::CandidateVerified).unwrap();
                    store.persist(&guard, &current).unwrap();
                }
                4 if abort => {
                    current
                        .abort_preserved(UpgradeFailureCode::SwitchFailed, false)
                        .unwrap();
                    store.persist(&guard, &current).unwrap();
                }
                4 => {
                    store.switch_userdb_candidate(&guard, &mut current).unwrap();
                }
                _ => unreachable!(),
            }
            drop(guard);
            assert_eq!(core_binding(&fixture, &prep), current);
            assert_eq!(inspect(&fixture).handed_off_receipt(), Some(&current));
            assert_eq!(tree(&history), sealed);
            // The product must reject an impossible pair even though the data
            // chain itself is valid: its outer is still Prepared in this test.
            let install = fixture.install();
            let outer = install.acquire_guard().unwrap();
            assert!(crate::rebind_prepared_upgrade(&install, &outer, &prep).is_err());
        }
    }
}

#[test]
fn missing_replaced_or_changed_handoff_material_never_falls_back_to_bootstrap() {
    for mode in 0..8 {
        let fixture = Fixture::new();
        fixture.legacy(UpgradeState::AbortedPreserved);
        fixture.run(&mut Observer::default()).unwrap();
        let prep = fixture.preparation();
        let history = fixture.path.join(".radishlex-upgrade-history-v1").join(NEW);
        let active = fixture.path.join(STATE).join("receipt.json");
        match mode {
            0 => {
                fs::rename(
                    history.join("preparation.json"),
                    fixture.path.join("saved-proof"),
                )
                .unwrap();
            }
            1 => {
                let path = history.join("preparation-snapshot.sqlite3");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.path.join("saved-snapshot")).unwrap();
                put(&path, &bytes);
            }
            2 => {
                let path = fixture
                    .path
                    .join(".radishlex-install-history-v1")
                    .join(OLD)
                    .join("receipt.json");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.path.join("saved-outer")).unwrap();
                put(&path, &bytes);
            }
            3 => {
                put(&history.join("unknown"), b"unexpected");
            }
            4 => {
                let bytes = fs::read(&active).unwrap();
                fs::rename(&active, fixture.path.join("saved-data")).unwrap();
                put(&active, &bytes);
            }
            5 => {
                let path = fixture.path.join("userdb.sqlite3");
                let mut bytes = fs::read(&path).unwrap();
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                fs::write(path, bytes).unwrap();
            }
            6 => {
                put(
                    &fixture.path.join(STATE).join("receipt.json.tmp"),
                    b"interrupted",
                );
            }
            7 => {
                fs::rename(&history, fixture.path.join("saved-history")).unwrap();
            }
            _ => unreachable!(),
        }
        let before = tree(&fixture.path);
        let view = crate::inspect_installer_view_with_recovery(
            &fixture.path,
            fixture.owner(),
            InstallerProductSituation::OlderReleaseInstalled,
        );
        assert_eq!(view.phase(), InstallerViewPhase::Blocked, "mode {mode}");
        assert!(!view.offers(InstallerAction::ResumeOperation));
        let install = fixture.install();
        let outer = install.acquire_guard().unwrap();
        assert!(
            crate::rebind_prepared_upgrade(&install, &outer, &prep).is_err(),
            "mode {mode}"
        );
        drop(outer);
        assert_eq!(tree(&fixture.path), before, "mode {mode}");
    }
}

#[test]
fn rebinding_rejects_stale_preparation_foreign_guards_and_changed_outer_predecessor() {
    let fixture = Fixture::new();
    fixture.run(&mut Observer::default()).unwrap();
    let prep = fixture.preparation();
    let foreign = Fixture::new();
    foreign.run(&mut Observer::default()).unwrap();
    let foreign_prep = foreign.preparation();
    let install = fixture.install();
    let foreign_store = foreign.install();
    let foreign_guard = foreign_store.acquire_guard().unwrap();
    assert!(crate::rebind_prepared_upgrade(&install, &foreign_guard, &prep).is_err());
    let guard = install.acquire_guard().unwrap();
    assert!(crate::rebind_prepared_upgrade(&install, &guard, &foreign_prep).is_err());
    let original = install.load_guarded(&guard).unwrap().unwrap();
    let changed = InstallReceipt::new(
        NEW,
        Some(DATA.into()),
        InstallOperationKind::Upgrade,
        original.root_identity().clone(),
        original.source_product().cloned(),
        original.target_product().cloned(),
    )
    .unwrap();
    assert_eq!(changed.state(), InstallState::Prepared);
    fs::write(
        fixture.path.join(".radishlex-install-v1/receipt.json"),
        changed.encode().unwrap(),
    )
    .unwrap();
    let before = tree(&fixture.path);
    assert!(crate::rebind_prepared_upgrade(&install, &guard, &prep).is_err());
    drop(guard);
    assert_eq!(tree(&fixture.path), before);
    assert_eq!(
        crate::inspect_installer_view_with_recovery(
            &fixture.path,
            fixture.owner(),
            InstallerProductSituation::OlderReleaseInstalled
        )
        .phase(),
        InstallerViewPhase::Blocked
    );
}

struct ReplacingHasher {
    path: std::path::PathBuf,
    proof: Vec<u8>,
    hits: Cell<usize>,
}
impl PreparationHasher for ReplacingHasher {
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
        let mut bytes = vec![];
        source.read_to_end(&mut bytes)?;
        if bytes == self.proof {
            self.hits.set(self.hits.get() + 1);
            if self.hits.get() == 2 {
                let old = self
                    .path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("replaced-proof");
                fs::rename(&self.path, old)?;
                put(&self.path, &bytes);
            }
        }
        MacOsPreparationHasher.sha256(&mut bytes.as_slice())
    }
}

#[test]
fn query_detects_same_byte_proof_replacement_during_observation() {
    let fixture = Fixture::new();
    fixture.run(&mut Observer::default()).unwrap();
    let path = fixture
        .path
        .join(".radishlex-upgrade-history-v1")
        .join(NEW)
        .join("preparation.json");
    let hasher = ReplacingHasher {
        proof: fs::read(&path).unwrap(),
        path,
        hits: Cell::new(0),
    };
    assert!(inspect_maintenance(&fixture.path, fixture.owner(), &hasher).is_err());
    assert!(hasher.hits.get() >= 2);
}

#[test]
fn product_quiesced_outer_keeps_preflighted_data_bound_and_both_guards_block_query() {
    let fixture = Fixture::new();
    let receipt = fixture.run(&mut Observer::default()).unwrap();
    let prep = fixture.preparation();
    let install = fixture.install();
    let guard = install.acquire_guard().unwrap();
    let mut outer = install.load_guarded(&guard).unwrap().unwrap();
    outer.advance(InstallState::Quiesced).unwrap();
    install.persist(&guard, &outer).unwrap();
    assert_eq!(
        crate::inspect_installer_view_with_recovery(
            &fixture.path,
            fixture.owner(),
            InstallerProductSituation::OlderReleaseInstalled
        )
        .stable_error(),
        InstallerStableError::OperationActive
    );
    drop(guard);
    assert_product_binding(&fixture, &receipt);
    let journal = PreparationJournalStore::open_existing(
        VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
    )
    .unwrap();
    let inner = journal.acquire_guard().unwrap();
    let before = tree(&fixture.path);
    assert_eq!(
        crate::inspect_installer_view_with_recovery(
            &fixture.path,
            fixture.owner(),
            InstallerProductSituation::OlderReleaseInstalled
        )
        .stable_error(),
        InstallerStableError::OperationActive
    );
    let outer = install.acquire_guard().unwrap();
    assert!(crate::rebind_prepared_upgrade(&install, &outer, &prep).is_err());
    drop(outer);
    drop(inner);
    assert_eq!(tree(&fixture.path), before);
}

#[test]
fn interrupted_switch_scenes_use_v1_locations_without_reopening_sqlite() {
    for scene in 0..3 {
        let fixture = Fixture::new();
        let mut current = fixture.run(&mut Observer::default()).unwrap();
        let prep = fixture.preparation();
        let store = UpgradeReceiptStore::open_existing(
            VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
        )
        .unwrap();
        let guard = store.acquire_guard().unwrap();
        current.advance(UpgradeState::Quiesced).unwrap();
        store.persist(&guard, &current).unwrap();
        store.create_settings_backup(&guard, &mut current).unwrap();
        store
            .create_userdb_snapshot(&guard, &mut current, u64::MAX)
            .unwrap();
        store.create_userdb_candidate(&guard, &mut current).unwrap();
        current.advance(UpgradeState::CandidateVerified).unwrap();
        store.persist(&guard, &current).unwrap();
        store.switch_userdb_candidate(&guard, &mut current).unwrap();
        drop(guard);
        // Reconstruct each exact on-disk rename scene using genuine generated
        // artifacts; this is not a real power-loss or old-program qualification.
        let bytes = String::from_utf8(current.encode().unwrap())
            .unwrap()
            .replace("\"state\":\"switched\"", "\"state\":\"switch_prepared\"");
        current = UpgradeReceipt::decode(bytes.as_bytes()).unwrap();
        fs::write(fixture.path.join(STATE).join("receipt.json"), bytes).unwrap();
        if scene < 2 {
            fs::rename(
                fixture.path.join("userdb.sqlite3"),
                fixture.path.join(STATE).join("migration-candidate.sqlite3"),
            )
            .unwrap();
        }
        if scene == 0 {
            fs::rename(
                fixture.path.join(STATE).join("source-backup.sqlite3"),
                fixture.path.join("userdb.sqlite3"),
            )
            .unwrap();
        }
        assert_eq!(core_binding(&fixture, &prep), current);
        assert_eq!(inspect(&fixture).handed_off_receipt(), Some(&current));
    }
}

#[test]
fn advanced_data_receipt_cannot_change_immutable_predecessor_release_or_schema() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        fixture.legacy(UpgradeState::AbortedPreserved);
        let original = fixture.run(&mut Observer::default()).unwrap();
        let prep = fixture.preparation();
        let mut changed = UpgradeReceipt::new(
            NEW,
            if mode == 0 { None } else { Some(DATA.into()) },
            if mode == 1 {
                ProductRelease::new("26.7.1", 38).unwrap()
            } else {
                original.source_release().clone()
            },
            original.target_release().clone(),
            if mode == 2 {
                original.source_schema_version().map(|v| v - 1)
            } else {
                original.source_schema_version()
            },
            original.target_schema_version(),
            original.artifacts().to_vec(),
        )
        .unwrap();
        changed.advance(UpgradeState::Quiesced).unwrap();
        fs::write(
            fixture.path.join(STATE).join("receipt.json"),
            changed.encode().unwrap(),
        )
        .unwrap();
        let before = tree(&fixture.path);
        assert!(
            inspect_maintenance(&fixture.path, fixture.owner(), &MacOsPreparationHasher).is_err()
        );
        let journal = PreparationJournalStore::open_existing(
            VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
        )
        .unwrap();
        let guard = journal.acquire_guard().unwrap();
        assert!(journal
            .rebind_handed_off_upgrade(&guard, &prep, &MacOsPreparationHasher)
            .is_err());
        drop(guard);
        assert_eq!(tree(&fixture.path), before);
    }
}
