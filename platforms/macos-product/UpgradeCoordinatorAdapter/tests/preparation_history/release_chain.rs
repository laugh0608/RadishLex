//! Ancestors remain authoritative after the latest locator advances.
use super::*;

const SECOND: &str = "44444444444444444444444444444444";
const THIRD: &str = "55555555555555555555555555555555";

fn next_terminal(
    fixture: &Fixture,
    operation: &str,
    previous: &str,
    source: u64,
) -> (Store, UpgradeProcessGuard, Binding, ReleasePort) {
    let (store, guard, mut receipt) = standalone(
        fixture,
        operation,
        Some(previous.to_owned()),
        source,
        source + 2,
    );
    store
        .resume_userdb_upgrade(
            &guard,
            &mut receipt,
            u64::MAX,
            &mut ProductPort(State::Completed),
        )
        .unwrap();
    let binding = Binding {
        operation_id: operation.to_owned(),
        installed_release: release(source + 2),
        installed_product_sha256: "c".repeat(64),
    };
    let port = ReleasePort::new(binding.clone());
    (Store::attach(store, &guard).unwrap(), guard, binding, port)
}

fn finish(
    fixture: &Fixture,
    (store, guard, binding, mut port): (Store, UpgradeProcessGuard, Binding, ReleasePort),
) -> Proof {
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    port.finalize(&fixture.0);
    store
        .finish_terminal_release(&guard, &binding.operation_id, &mut port, &Hasher)
        .unwrap()
        .1
}

fn twice_released(fixture: &Fixture) -> Proof {
    finish(fixture, terminal(fixture, false, State::Completed));
    finish(fixture, next_terminal(fixture, SECOND, NEW, 41))
}

#[test]
fn latest_lookup_rejects_drift_in_older_released_materials_without_writing() {
    for case in 0..7 {
        let fixture = Fixture::new();
        twice_released(&fixture);
        let latest = finish(&fixture, next_terminal(&fixture, THIRD, SECOND, 43));
        let (store, guard) = reload(&fixture);
        assert_eq!(
            store.load_latest_release(&guard, &Hasher).unwrap(),
            Some(latest)
        );
        let ancestor = history(&fixture, "data/source-snapshot.sqlite3");
        match case {
            0 => fs::write(&ancestor, b"ancestor drift").unwrap(),
            1 => fs::rename(
                history(&fixture, "terminal-release.json"),
                fixture.0.join("retained-proof"),
            )
            .unwrap(),
            2 => {
                let bytes = fs::read(&ancestor).unwrap();
                fs::rename(&ancestor, fixture.0.join("retained-snapshot")).unwrap();
                put(&ancestor, &bytes);
            }
            3 => fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o644)).unwrap(),
            4 => fs::hard_link(&ancestor, fixture.0.join("extra-link")).unwrap(),
            5 => put(
                &history(&fixture, "data/unknown"),
                b"unbound ancestor object",
            ),
            6 => {
                fs::rename(&ancestor, fixture.0.join("retained-snapshot")).unwrap();
                std::os::unix::fs::symlink(fixture.0.join("retained-snapshot"), &ancestor).unwrap();
            }
            _ => unreachable!(),
        }
        let before = failures::tree(&fixture.0);
        assert!(
            store.load_latest_release(&guard, &Hasher).is_err(),
            "case {case}"
        );
        assert_eq!(failures::tree(&fixture.0), before, "case {case}");
    }
}

#[test]
fn history_chain_allows_later_learning_but_blocks_fresh_preparation_on_ancestor_drift() {
    let fixture = Fixture::new();
    let latest = twice_released(&fixture);
    let mut db = UserDb::open(fixture.source()).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-chain",
        "xin",
        "新",
        0,
        8,
    ))
    .unwrap();
    db.delete_term("xin", "新", None).unwrap();
    let before = failures::tree(&fixture.0);
    let (store, guard) = reload(&fixture);
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(latest)
    );
    assert_eq!(failures::tree(&fixture.0), before);
    drop(guard);
    drop(db);
    let (store, guard) = fixture.reload();
    assert!(store
        .capture_previous_inventory(&guard, THIRD, Some(OUTER), Some(SECOND), &Hasher)
        .is_ok());
    fs::write(
        history(&fixture, "data/source-snapshot.sqlite3"),
        b"old drift",
    )
    .unwrap();
    let before = failures::tree(&fixture.0);
    assert!(store
        .capture_previous_inventory(&guard, THIRD, Some(OUTER), Some(SECOND), &Hasher)
        .is_err());
    assert_eq!(failures::tree(&fixture.0), before);
}

#[test]
fn ancestor_drift_in_release_callbacks_stops_before_the_next_mutation() {
    for point in [
        RP::Begin,
        RP::RecordWritten,
        RP::BeforeMove(Slot::Receipt),
        RP::BeforeIndexWrite,
        RP::IndexWritten,
        RP::IndexRecorded,
        RP::BeforeMarkerMove,
    ] {
        let fixture = Fixture::new();
        twice_released(&fixture);
        let (store, guard, binding, mut port) = next_terminal(&fixture, THIRD, SECOND, 43);
        let ancestor = history(&fixture, "data/source-snapshot.sqlite3");
        let root = fixture.0.clone();
        let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
        let observed = captured.clone();
        port.mutation = Some((
            point,
            Box::new(move || {
                fs::write(ancestor, b"drift inside callback").unwrap();
                *observed.borrow_mut() = Some(failures::tree(&root));
            }),
        ));
        if point == RP::BeforeMarkerMove {
            store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .unwrap();
            port.finalize(&fixture.0);
            assert!(store
                .finish_terminal_release(&guard, THIRD, &mut port, &Hasher)
                .is_err());
            assert!(fixture.state("terminal-release.json").exists());
        } else {
            assert!(
                store
                    .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                    .is_err(),
                "{point:?}"
            );
        }
        assert_eq!(
            &failures::tree(&fixture.0),
            captured.borrow().as_ref().unwrap(),
            "{point:?}"
        );
    }
}

fn replace_latest_proof(fixture: &Fixture, bytes: &[u8]) {
    use radishlex_ime_product_upgrade::PreparationHasher;
    let index = fixture.0.join(HISTORY).join("latest-release.json");
    let original = fs::read_to_string(&index).unwrap();
    let json: serde_json::Value = serde_json::from_str(&original).unwrap();
    let old_hash = json["release_sha256"].as_str().unwrap();
    let mut input = bytes;
    let new_hash: String = Hasher
        .sha256(&mut input)
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(
        fixture
            .0
            .join(HISTORY)
            .join(SECOND)
            .join("terminal-release.json"),
        bytes,
    )
    .unwrap();
    fs::write(index, original.replace(old_hash, &new_hash)).unwrap();
}

#[test]
fn exact_latest_digest_cannot_waive_invalid_predecessor_bindings() {
    let fixture = Fixture::new();
    let latest = twice_released(&fixture);
    let valid = latest.encode().unwrap();
    let original: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    for case in 0..9 {
        let mut json = original.clone();
        match case {
            0 => json["previous_index"]["identity"]["sha256"] = serde_json::json!("f".repeat(64)),
            1 => json["previous_index"]["identity"]["byte_len"] = serde_json::json!(1),
            2 => {
                json["previous_index"]["index"]["format"] =
                    serde_json::json!("radishlex-latest-release-v2")
            }
            3 => json["previous_index"]["index"]["data_operation_id"] = serde_json::json!(OLD),
            4 => json["previous_index"]["index"]["operation_id"] = serde_json::json!(SECOND),
            5 => {
                json["previous_index"]["index"]["installed_release"]["build_number"] =
                    serde_json::json!(37)
            }
            6 => json["receipt"]["previous_operation_id"] = serde_json::json!(OLD),
            7 => {
                json["previous_index"]["index"]["release_sha256"] =
                    serde_json::json!("f".repeat(64))
            }
            8 => json["previous_index"]["identity"]["mode"] = serde_json::json!(0o644),
            _ => unreachable!(),
        }
        // Serialize through the real field ordering, deliberately bypassing
        // encode validation to simulate an invalid on-disk proof. Rebind the
        // latest digest so rejection must inspect the predecessor contract.
        let typed: Proof = serde_json::from_value(json).unwrap();
        let mut bytes = serde_json::to_vec(&typed).unwrap();
        bytes.push(b'\n');
        replace_latest_proof(&fixture, &bytes);
        let (store, guard) = reload(&fixture);
        let before = failures::tree(&fixture.0);
        assert!(
            store.load_latest_release(&guard, &Hasher).is_err(),
            "case {case}"
        );
        assert_eq!(failures::tree(&fixture.0), before);
    }
    replace_latest_proof(&fixture, &valid);
    let (store, guard) = reload(&fixture);
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(latest)
    );
}

#[test]
fn new_release_rejects_wrong_data_or_source_predecessor_before_creating_history() {
    for (previous, source) in [(OLD, 43), (SECOND, 41)] {
        let fixture = Fixture::new();
        twice_released(&fixture);
        let (store, guard, binding, mut port) = next_terminal(&fixture, THIRD, previous, source);
        let before = failures::tree(&fixture.0);
        assert!(store
            .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
            .is_err());
        assert_eq!(failures::tree(&fixture.0), before);
        assert!(!fixture.0.join(HISTORY).join(THIRD).exists());
    }
}

#[test]
fn lookup_rechecks_latest_index_identity_after_reading_ancestors() {
    use radishlex_ime_product_upgrade::PreparationHasher;
    struct ReplaceIndex {
        root: PathBuf,
        ancestor_bytes: Vec<u8>,
        changed: std::cell::Cell<bool>,
    }
    impl PreparationHasher for ReplaceIndex {
        fn sha256(&self, source: &mut dyn std::io::Read) -> std::io::Result<[u8; 32]> {
            let mut bytes = Vec::new();
            source.read_to_end(&mut bytes)?;
            let digest = Hasher.sha256(&mut bytes.as_slice())?;
            if bytes == self.ancestor_bytes && !self.changed.replace(true) {
                let index = self.root.join(HISTORY).join("latest-release.json");
                let original = fs::read(&index)?;
                fs::rename(&index, self.root.join("retained-index"))?;
                put(&index, &original);
            }
            Ok(digest)
        }
    }
    let fixture = Fixture::new();
    twice_released(&fixture);
    let hasher = ReplaceIndex {
        root: fixture.0.clone(),
        ancestor_bytes: fs::read(history(&fixture, "terminal-release.json")).unwrap(),
        changed: std::cell::Cell::new(false),
    };
    let (store, guard) = reload(&fixture);
    assert!(store.load_latest_release(&guard, &hasher).is_err());
    assert!(hasher.changed.get());
}
