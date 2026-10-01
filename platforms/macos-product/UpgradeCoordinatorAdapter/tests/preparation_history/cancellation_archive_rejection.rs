use super::*;

#[test]
fn rejects_drift_conflicts_and_unbound_objects_without_releasing_request() {
    for case in 0..10 {
        let fixture = Fixture::new();
        let (store, guard, outer) = prepare(&fixture, Some(State::AbortedPreserved), 3, true);
        let root = fixture.0.clone();
        let mut port = Authority::new();
        port.mutation = Some((
            AP::IntentRecorded,
            Box::new(move || {
                let history = root.join(HISTORY).join(NEW);
                let request = root.join(STATE).join("preparation-cancellation.json");
                match case {
                    0 => {
                        let bytes = fs::read(&request).unwrap();
                        fs::rename(&request, root.join("preserved-request")).unwrap();
                        put(&request, &bytes);
                    }
                    1 => fs::set_permissions(&request, fs::Permissions::from_mode(0o644)).unwrap(),
                    2 => fs::hard_link(&request, root.join("request-link")).unwrap(),
                    3 => put(&history.join("unknown.json"), b"unknown"),
                    4 => put(
                        &history.join("preparation.json"),
                        &fs::read(root.join(STATE).join("source-preparation.json")).unwrap(),
                    ),
                    5 => {
                        let old = root
                            .join(".radishlex-install-history-v1")
                            .join(OUTER)
                            .join("receipt.json");
                        fs::write(old, b"changed").unwrap();
                    }
                    6 => fs::write(root.join("manager-settings.json"), b"changed").unwrap(),
                    7 => {
                        let original = root.join(STATE).join("preparation-snapshot.sqlite3");
                        fs::rename(&original, root.join("preserved-snapshot")).unwrap();
                        symlink(root.join("preserved-snapshot"), original).unwrap();
                    }
                    8 => {
                        fs::rename(&history, root.join("preserved-history")).unwrap();
                        DirBuilder::new().mode(0o700).create(&history).unwrap();
                    }
                    9 => put(&root.join(HISTORY).join("unbound-root-object"), b"unknown"),
                    _ => unreachable!(),
                }
            }),
        ));
        assert!(
            store
                .preserve_cancellation(&guard, &outer, &mut port, &Hasher)
                .is_err(),
            "case {case}"
        );
        assert!(fixture.state("preparation-cancellation.json").exists());
        assert!(store.load_guarded(&guard, &Hasher).is_err());
    }
}

#[test]
fn compatibility_publish_rejects_active_slot_conflict_and_fresh_authority_loss() {
    for conflict in [false, true] {
        let fixture = Fixture::new();
        let (store, guard, outer) = prepare(&fixture, None, 0, true);
        let current = fixture.0.join(".radishlex-install-v1/receipt.json");
        let mut port = Authority::new();
        port.mutation = Some((
            AP::BeforeCompatibilityPublish,
            Box::new(move || {
                if conflict {
                    put(&current, OLD_BYTES);
                } else {
                    fs::set_permissions(
                        current.parent().unwrap(),
                        fs::Permissions::from_mode(0o755),
                    )
                    .unwrap();
                }
            }),
        ));
        assert!(store
            .preserve_cancellation(&guard, &outer, &mut port, &Hasher)
            .is_err());
        assert!(store.load_guarded(&guard, &Hasher).is_err());
        assert!(fixture.state("preparation-cancellation.json").exists());
    }
}

#[test]
fn strict_archive_record_rejects_unknown_fields_and_invalid_progress() {
    let fixture = Fixture::new();
    let (store, guard, outer) = prepare(&fixture, None, 0, false);
    let record = store
        .preserve_cancellation(&guard, &outer, &mut Authority::new(), &Hasher)
        .unwrap();
    let text = String::from_utf8(record.encode().unwrap()).unwrap();
    for changed in [
        text.replacen("archive-v1", "archive-v2", 1),
        text.replacen('{', "{\"unknown\":true,", 1),
        text.replacen("source_progress", "cancel_ready", 1),
        text.replacen(
            "\"moved\":[\"source_progress\",\"preparation\"]",
            "\"moved\":[]",
            1,
        ),
        format!("{text} "),
    ] {
        assert_ne!(changed, text);
        assert!(CancellationArchiveReceipt::decode(changed.as_bytes()).is_err());
    }
}
