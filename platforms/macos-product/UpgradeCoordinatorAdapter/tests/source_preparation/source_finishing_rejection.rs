use super::*;

fn paused() -> (
    Fixture,
    PreparationCancellationStore,
    UpgradeProcessGuard,
    FinishPort,
) {
    let fixture = Fixture::new();
    fixture.populate();
    let (store, guard, mut port) = requested(&fixture, PreparationPhase::MaintenanceIntent);
    port.fail = Some(FinishPoint::Begin);
    assert_eq!(
        store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
        Err(SourcePreparationError::AuthorityNotProven)
    );
    port.fail = None;
    (fixture, store, guard, port)
}

#[test]
fn source_progress_strict_encoding_and_binding_rejects_unsafe_records() {
    let (fixture, store, guard, mut port) = paused();
    let ready = store
        .finish_source(&guard, &mut port, &MacOsPreparationHasher)
        .unwrap();
    let bytes = ready.encode().unwrap();
    assert_eq!(CancellationSourceReceipt::decode(&bytes).unwrap(), ready);
    let text = String::from_utf8(bytes).unwrap();
    for value in [
        text.replace(
            "radishlex-cancellation-source-v1",
            "radishlex-cancellation-source-v2",
        ),
        text.replacen(
            "\"phase\":\"source_ready\"",
            "\"phase\":\"cancel_ready\"",
            1,
        ),
        text.replacen("\"phase\":\"source_ready\"", "\"phase\":\"finishing\"", 1),
        text.replacen('{', "{\"unknown\":true,", 1),
        text.replacen("\"format\":", "\"format\":\"duplicate\",\"format\":", 1),
        text.trim_end().into(),
        " ".repeat(256 * 1024 + 1),
    ] {
        assert!(CancellationSourceReceipt::decode(value.as_bytes()).is_err());
    }
    let modified = text.replace(&"a".repeat(64), &"f".repeat(64));
    // Well-formed canonical receipt, but cannot change the immutable admission.
    assert!(CancellationSourceReceipt::decode(modified.as_bytes()).is_ok());
    fs::write(fixture.0.join(PROGRESS), modified).unwrap();
    assert!(store
        .load_source_guarded(&guard, &MacOsPreparationHasher)
        .is_err());
    assert!(store
        .finish_source(&guard, &mut port, &MacOsPreparationHasher)
        .is_err());
    blocked(&fixture);
}

#[test]
fn source_finishing_rejects_drift_in_source_request_snapshot_and_progress() {
    for case in 0..12 {
        let (fixture, store, guard, mut port) = paused();
        let path = fixture.0.join(match case {
            0 => REQUEST,
            1 => JOURNAL,
            2 => SNAPSHOT,
            3 => "userdb.sqlite3",
            _ => PROGRESS,
        });
        match case {
            0..=3 => {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("held-original")).unwrap();
                put(&path, &bytes);
            }
            4 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            5 => fs::hard_link(&path, fixture.0.join("hardlink")).unwrap(),
            6 => {
                fs::rename(&path, fixture.0.join("held-original")).unwrap();
                symlink(fixture.0.join("held-original"), &path).unwrap();
            }
            7 => fs::write(&path, b"invalid record").unwrap(),
            8 => put(&fixture.0.join(PROGRESS_TEMP), b"incomplete progress"),
            9 => put(
                &fixture
                    .0
                    .join(".radishlex-upgrade-v1/unknown-cancellation-object"),
                b"unknown",
            ),
            10 => fs::write(fixture.0.join(SNAPSHOT), b"changed protected snapshot").unwrap(),
            11 => put(
                &fixture.0.join("userdb.sqlite3-journal"),
                b"unqualified journal",
            ),
            _ => unreachable!(),
        }
        let before = fs::read(fixture.source()).unwrap();
        assert!(
            store
                .finish_source(&guard, &mut port, &MacOsPreparationHasher)
                .is_err(),
            "{case}"
        );
        assert_eq!(fs::read(fixture.source()).unwrap(), before);
        blocked(&fixture);
    }
}

#[test]
fn source_finishing_detects_callback_replacements_and_collisions_before_mutation() {
    for case in 0..10 {
        let fixture = Fixture::new();
        fixture.populate();
        let (store, guard, mut port) = requested(&fixture, PreparationPhase::MaintenanceIntent);
        let root = fixture.0.clone();
        let point = match case {
            0 => FinishPoint::Write(FinishPhase::Finishing, CancelPoint::Created),
            1 | 2 => FinishPoint::Write(FinishPhase::Finishing, CancelPoint::BeforeRename),
            3 => FinishPoint::Write(FinishPhase::Finishing, CancelPoint::Renamed),
            4..=7 => FinishPoint::BeforeMaintenance,
            8 => FinishPoint::Write(FinishPhase::SourceReady, CancelPoint::BeforeRename),
            9 => FinishPoint::SourceReady,
            _ => unreachable!(),
        };
        port.mutate = Some((
            point,
            Box::new(move || {
                if case == 9 {
                    put(
                        &root.join(PROGRESS_TEMP),
                        b"unexpected final temporary file",
                    );
                    return;
                }
                let name = match case {
                    0 | 1 | 8 => PROGRESS_TEMP,
                    2 | 3 => PROGRESS,
                    4 => REQUEST,
                    5 => JOURNAL,
                    6 => SNAPSHOT,
                    7 => "userdb.sqlite3",
                    _ => unreachable!(),
                };
                let path = root.join(name);
                if case == 1 {
                    fs::write(&path, b"changed temporary bytes").unwrap();
                } else if case == 2 {
                    put(&path, &fs::read(root.join(PROGRESS_TEMP)).unwrap());
                } else {
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, root.join("retained-conflicting-file")).unwrap();
                    put(&path, &bytes);
                }
            }),
        ));
        assert!(
            store
                .finish_source(&guard, &mut port, &MacOsPreparationHasher)
                .is_err(),
            "{case}"
        );
        assert!(port.seen.contains(&point));
        blocked(&fixture);
    }
}

#[test]
fn source_finishing_checks_full_content_not_just_main_inode_and_schema() {
    let (fixture, store, guard, mut port) = paused();
    let original_inode = fs::metadata(fixture.source()).unwrap().ino();
    // Same inode and supported schema, different persisted learning data.
    let mut db = UserDb::open(fixture.source()).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-divergence",
        "xin",
        "新",
        0,
        3,
    ))
    .unwrap();
    drop(db);
    assert_eq!(
        fs::metadata(fixture.source()).unwrap().ino(),
        original_inode
    );
    let bytes = fs::read(fixture.source()).unwrap();
    let result = store.finish_source(&guard, &mut port, &MacOsPreparationHasher);
    assert!(
        matches!(result, Err(SourcePreparationError::Maintenance(_))),
        "{result:?}"
    );
    assert_eq!(fs::read(fixture.source()).unwrap(), bytes);
    blocked(&fixture);
}

#[test]
fn source_finishing_rejects_changed_or_foreign_recovery_journal_and_busy_source() {
    for replace in [false, true] {
        let (fixture, store, guard, mut port) = hot_fixture();
        port.fail = Some(FinishPoint::JournalRecoveryIntentRecorded);
        assert_eq!(
            store.finish_source(&guard, &mut port, &MacOsPreparationHasher),
            Err(SourcePreparationError::AuthorityNotProven)
        );
        let path = fixture.0.join("userdb.sqlite3-journal");
        let bytes = fs::read(&path).unwrap();
        if replace {
            fs::rename(&path, fixture.0.join("held-journal")).unwrap();
            put(&path, &bytes);
        } else {
            let mut changed = bytes;
            changed[1024] ^= 1;
            fs::write(&path, changed).unwrap();
        }
        let source = fs::read(fixture.source()).unwrap();
        port.fail = None;
        assert!(store
            .finish_source(&guard, &mut port, &MacOsPreparationHasher)
            .is_err());
        assert_eq!(fs::read(fixture.source()).unwrap(), source);
        assert!(path.exists());
        blocked(&fixture);
    }
    let (fixture, store, guard, mut port) = paused();
    let _reader = UserDb::open(fixture.source()).unwrap();
    let before = fs::read(fixture.source()).unwrap();
    assert!(store
        .finish_source(&guard, &mut port, &MacOsPreparationHasher)
        .is_err());
    assert_eq!(fs::read(fixture.source()).unwrap(), before);
    blocked(&fixture);
}
