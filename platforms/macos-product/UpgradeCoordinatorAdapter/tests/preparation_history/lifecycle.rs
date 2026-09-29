//! Old-format interruption replay and mixed-version lifecycle continuation.
use super::*;

pub(super) fn legacy_release(fixture: &Fixture, state: State) -> (Binding, Proof) {
    let (store, guard, binding, mut port) = terminal(fixture, true, state);
    port.fail = Some(RP::IntentRecorded);
    assert!(store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .is_err());
    // Seed the exact pre-v2 layout at a durable reserved boundary. No v2 index
    // exists yet and no private v1 slot has moved. This is synthetic old-writer
    // input, not a migration/retagging operation offered by production code.
    let path = fixture.state("terminal-release.json");
    let old = fs::read_to_string(&path).unwrap().replacen(
        "radishlex-terminal-release-v2",
        "radishlex-terminal-release-v1",
        1,
    );
    let old = Proof::decode(old.as_bytes()).unwrap();
    fs::write(&path, old.encode().unwrap()).unwrap();
    port.fail = None;
    let ready = store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    assert!(String::from_utf8(ready.encode().unwrap())
        .unwrap()
        .contains("radishlex-terminal-release-v1"));
    port.finalize(&fixture.0);
    let (v1, proof) = store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    let store = Store::attach(v1, &guard).unwrap();
    let before = failures::tree(&fixture.0);
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(proof.clone())
    );
    assert_eq!(failures::tree(&fixture.0), before);
    (binding, proof)
}

fn next_release(fixture: &Fixture, prior: &Binding, operation: &str) -> (Binding, Proof) {
    let (store, guard, binding) = reserve(fixture, prior, operation);
    let (v1, mut receipt) =
        advance(store, &guard, &binding, &mut Port::new(binding.clone())).unwrap();
    v1.resume_userdb_upgrade(
        &guard,
        &mut receipt,
        u64::MAX,
        &mut ProductPort(State::Completed),
    )
    .unwrap();
    let next = Binding {
        operation_id: operation.to_owned(),
        installed_release: binding.target_release,
        installed_product_sha256: binding.target_product_sha256,
    };
    let store = Store::attach(v1, &guard).unwrap();
    let mut port = ReleasePort::new(next.clone());
    store
        .prepare_terminal_release(&guard, &next, &mut port, &Hasher)
        .unwrap();
    assert!(store.load_latest_release(&guard, &Hasher).is_err());
    port.finalize(&fixture.0);
    let (v1, proof) = store
        .finish_terminal_release(&guard, operation, &mut port, &Hasher)
        .unwrap();
    let store = Store::attach(v1, &guard).unwrap();
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(proof.clone())
    );
    (next, proof)
}

#[test]
fn legacy_replay_then_v2_preparation_and_release_preserve_all_three_terminal_histories() {
    for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
        let fixture = Fixture::new();
        let (prior, _) = legacy_release(&fixture, state);
        let old_tree = failures::tree(&fixture.0.join(HISTORY).join(NEW));
        let index = fixture.0.join(HISTORY).join("latest-release.json");
        let old_bytes = fs::read(&index).unwrap();
        let old_value: serde_json::Value = serde_json::from_slice(&old_bytes).unwrap();
        assert_eq!(old_value["format"], "radishlex-latest-release-v1");
        let old_inode = fs::metadata(&index).unwrap().ino();
        let mut db = UserDb::open(fixture.source()).unwrap();
        db.record_selection(SelectionEventDraft::new("synthetic-v2", "xin", "新", 0, 9))
            .unwrap();
        db.delete_term("xin", "新", None).unwrap();
        drop(db);
        let (next, proof) = next_release(&fixture, &prior, NEXT);
        let value: serde_json::Value = serde_json::from_slice(&proof.encode().unwrap()).unwrap();
        assert_eq!(value["format"], "radishlex-terminal-release-v2");
        assert_eq!(value["previous_index"]["index"], old_value);
        assert_eq!(value["previous_index"]["identity"]["inode"], old_inode);
        let new_index: serde_json::Value =
            serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
        assert_eq!(new_index["format"], "radishlex-latest-release-v2");
        assert_eq!(new_index["operation_id"], NEXT);
        assert_eq!(new_index["legacy_outer_operation_id"], NEXT);
        assert_eq!(new_index["data_operation_id"], NEXT);
        assert_eq!(new_index["proof"]["kind"], "terminal_release");
        assert_eq!(failures::tree(&fixture.0.join(HISTORY).join(NEW)), old_tree);
        let retained = failures::tree(&fixture.0.join(HISTORY).join(NEXT));
        let (_, third) = next_release(&fixture, &next, "55555555555555555555555555555555");
        let third_value: serde_json::Value =
            serde_json::from_slice(&third.encode().unwrap()).unwrap();
        assert_eq!(third_value["previous_index"]["index"], new_index);
        assert_eq!(failures::tree(&fixture.0.join(HISTORY).join(NEW)), old_tree);
        assert_eq!(
            failures::tree(&fixture.0.join(HISTORY).join(NEXT)),
            retained
        );
        // A v1 proof must never accept a v2 predecessor or rewrite the chain.
        let downgraded = String::from_utf8(third.encode().unwrap())
            .unwrap()
            .replacen(
                "radishlex-terminal-release-v2",
                "radishlex-terminal-release-v1",
                1,
            );
        assert!(Proof::decode(downgraded.as_bytes()).is_err());
    }
}

#[test]
fn v2_locator_rejects_wrong_kind_null_data_cross_links_and_version_confusion() {
    let fixture = Fixture::new();
    let (store, guard, binding, mut port) = terminal(&fixture, false, State::Completed);
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    port.finalize(&fixture.0);
    let (v1, proof) = store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    let store = Store::attach(v1, &guard).unwrap();
    let path = fixture.0.join(HISTORY).join("latest-release.json");
    let valid = fs::read_to_string(&path).unwrap();
    for invalid in [
        valid.replace("radishlex-latest-release-v2", "radishlex-latest-release-v9"),
        valid.replace("radishlex-latest-release-v2", "radishlex-latest-release-v1"),
        valid.replace("terminal_release", "preparation_cancellation"),
        valid.replace(
            &format!("\"legacy_outer_operation_id\":\"{NEW}\""),
            &format!("\"legacy_outer_operation_id\":\"{OLD}\""),
        ),
        valid.replace(
            &format!("\"data_operation_id\":\"{NEW}\""),
            "\"data_operation_id\":null",
        ),
        valid.replace(
            &format!("\"data_operation_id\":\"{NEW}\""),
            &format!("\"data_operation_id\":\"{OLD}\""),
        ),
        valid.replacen("{", "{\"unknown\":true,", 1),
        valid.replace("\"kind\":", "\"kind\":\"terminal_release\",\"kind\":"),
        valid.trim_end().to_owned(),
    ] {
        assert_ne!(invalid, valid);
        fs::write(&path, invalid).unwrap();
        let before = failures::tree(&fixture.0);
        assert!(store.load_latest_release(&guard, &Hasher).is_err());
        assert_eq!(failures::tree(&fixture.0), before);
    }
    fs::write(path, valid).unwrap();
    assert_eq!(
        store.load_latest_release(&guard, &Hasher).unwrap(),
        Some(proof)
    );
}

#[test]
fn missing_locator_with_existing_history_is_not_a_first_v2_release() {
    for legacy in [false, true] {
        let fixture = Fixture::new();
        if legacy {
            legacy_release(&fixture, State::Completed);
        } else {
            released(&fixture, State::Completed);
        }
        let index = fixture.0.join(HISTORY).join("latest-release.json");
        fs::rename(&index, fixture.0.join("retained-index")).unwrap();
        let (store, guard, mut receipt) = standalone(&fixture, NEXT, Some(NEW.to_owned()), 41, 43);
        store
            .resume_userdb_upgrade(
                &guard,
                &mut receipt,
                u64::MAX,
                &mut ProductPort(State::Completed),
            )
            .unwrap();
        let store = Store::attach(store, &guard).unwrap();
        let binding = Binding {
            operation_id: NEXT.to_owned(),
            installed_release: release(43),
            installed_product_sha256: "c".repeat(64),
        };
        let before = failures::tree(&fixture.0);
        assert!(store
            .prepare_terminal_release(
                &guard,
                &binding,
                &mut ReleasePort::new(binding.clone()),
                &Hasher
            )
            .is_err());
        assert_eq!(failures::tree(&fixture.0), before);
        assert!(!fixture.0.join(HISTORY).join(NEXT).exists());
    }
}

#[test]
fn first_v2_release_preserves_and_verifies_unreleased_legacy_inventory() {
    for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(Some(state));
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        let old = failures::tree(&fixture.0.join(HISTORY).join(OLD));
        let (v1, mut receipt) = store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        v1.resume_userdb_upgrade(
            &guard,
            &mut receipt,
            u64::MAX,
            &mut ProductPort(State::Completed),
        )
        .unwrap();
        let store = Store::attach(v1, &guard).unwrap();
        let binding = Binding {
            operation_id: NEW.to_owned(),
            installed_release: release(41),
            installed_product_sha256: "b".repeat(64),
        };
        let mut port = ReleasePort::new(binding.clone());
        store
            .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
            .unwrap();
        port.finalize(&fixture.0);
        let (v1, proof) = store
            .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        assert_eq!(failures::tree(&fixture.0.join(HISTORY).join(OLD)), old);
        let store = Store::attach(v1, &guard).unwrap();
        assert_eq!(
            store.load_latest_release(&guard, &Hasher).unwrap(),
            Some(proof)
        );
        fs::write(fixture.history("receipt.json"), b"old inventory drift").unwrap();
        let before = failures::tree(&fixture.0);
        assert!(store.load_latest_release(&guard, &Hasher).is_err());
        assert_eq!(failures::tree(&fixture.0), before);
    }
}
