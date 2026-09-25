use super::*;

fn points() -> Vec<RP> {
    let mut points = vec![RP::Begin];
    for dir in [RD::HistoryRoot, RD::Operation, RD::Data] {
        points.extend([
            RP::DirectoryCreated(dir),
            RP::DirectorySynced(dir),
            RP::ParentSynced(dir),
        ]);
    }
    points.extend([
        RP::RecordCreated,
        RP::RecordWritten,
        RP::RecordFileSynced,
        RP::RecordRenamed,
        RP::RecordDirectorySynced,
        RP::IntentRecorded,
    ]);
    for slot in [
        Slot::Receipt,
        Slot::Snapshot,
        Slot::Candidate,
        Slot::Settings,
    ] {
        points.extend([
            RP::BeforeMove(slot),
            RP::FileSynced(slot),
            RP::Moved(slot),
            RP::TargetSynced(slot),
            RP::SourceSynced(slot),
            RP::SlotRecorded(slot),
        ]);
    }
    points.extend([
        RP::ReadyRecorded,
        RP::BeforeIndexWrite,
        RP::IndexCreated,
        RP::IndexWritten,
        RP::IndexFileSynced,
        RP::IndexRenamed,
        RP::IndexDirectorySynced,
        RP::IndexRecorded,
        RP::BeforeMarkerMove,
        RP::MarkerMoved,
        RP::MarkerTargetSynced,
        RP::MarkerSourceSynced,
        RP::Released,
    ]);
    points
}
fn blocked(point: RP) -> bool {
    matches!(
        point,
        RP::DirectoryCreated(RD::Operation | RD::Data)
            | RP::DirectorySynced(RD::Operation | RD::Data)
            | RP::ParentSynced(RD::Operation | RD::Data)
            | RP::RecordCreated
            | RP::RecordWritten
            | RP::RecordFileSynced
            | RP::IndexCreated
            | RP::IndexWritten
            | RP::IndexFileSynced
    )
}
fn continue_release(fixture: &Fixture, mut port: ReleasePort) -> bool {
    let Ok(store) = Store::open_existing(fixture.verified()) else {
        return false;
    };
    let guard = store.acquire_guard().unwrap();
    if !history(fixture, "terminal-release.json").exists()
        && store
            .prepare_terminal_release(&guard, &port.binding.clone(), &mut port, &Hasher)
            .is_err()
    {
        return false;
    }
    port.finalize(&fixture.0);
    store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .is_ok()
}
pub(super) fn tree(root: &Path) -> Vec<(PathBuf, u64, u32, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, out: &mut Vec<(PathBuf, u64, u32, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(path).unwrap()
        } else if metadata.is_symlink() {
            fs::read_link(path)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        out.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.ino(),
            metadata.mode(),
            bytes,
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn authorization_loss_at_each_release_boundary_preserves_or_resumes_exact_objects() {
    for point in points() {
        let fixture = Fixture::new();
        let (store, guard, binding, mut port) = terminal(&fixture, false, State::AbortedPreserved);
        let source = fs::read(fixture.source()).unwrap();
        port.fail = Some(point);
        let first = store.prepare_terminal_release(&guard, &binding, &mut port, &Hasher);
        if first.is_ok() {
            port.finalize(&fixture.0);
            assert!(
                store
                    .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
                    .is_err(),
                "{point:?}"
            );
        }
        assert_eq!(fs::read(fixture.source()).unwrap(), source);
        drop(guard);
        let before = tree(&fixture.0);
        let success = continue_release(&fixture, ReleasePort::new(binding));
        assert_eq!(success, !blocked(point), "{point:?}");
        if !success {
            assert_eq!(tree(&fixture.0), before);
        }
    }
}

#[test]
fn process_exit_at_each_release_boundary_reloads_from_disk() {
    for (index, point) in points().into_iter().enumerate() {
        let fixture = Fixture::new();
        let (_, guard, binding, _) = terminal(&fixture, false, State::AbortedPreserved);
        drop(guard);
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal_release::failures::release_crash_child",
                "--ignored",
            ])
            .env("RADISHLEX_RELEASE_ROOT", &fixture.0)
            .env("RADISHLEX_RELEASE_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(75),
            "{point:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let before = tree(&fixture.0);
        let success = continue_release(&fixture, ReleasePort::new(binding));
        assert_eq!(success, !blocked(point), "{point:?}");
        if !success {
            assert_eq!(tree(&fixture.0), before);
        }
    }
}
#[test]
#[ignore = "subprocess entry invoked by release matrix"]
fn release_crash_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_RELEASE_ROOT").unwrap());
    let index: usize = std::env::var("RADISHLEX_RELEASE_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let store = Store::open_existing(
        VerifiedDataRoot::verify(&root, fs::metadata(&root).unwrap().uid()).unwrap(),
    )
    .unwrap();
    let guard = store.acquire_guard().unwrap();
    let binding = Binding {
        operation_id: NEW.to_owned(),
        installed_release: release(39),
        installed_product_sha256: "a".repeat(64),
    };
    let mut port = ReleasePort::new(binding.clone());
    port.exit = Some(points()[index]);
    store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    port.finalize(&root);
    store
        .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
        .unwrap();
    panic!("checkpoint not reached");
}

#[test]
fn drift_unknown_objects_and_conflicts_remain_untouched() {
    for case in 0..16 {
        let fixture = Fixture::new();
        let (store, guard, binding, mut port) = terminal(&fixture, true, State::Completed);
        port.fail = Some(RP::IntentRecorded);
        assert!(store
            .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
            .is_err());
        drop(guard);
        let replace = |path: PathBuf| {
            let bytes = fs::read(&path).unwrap();
            fs::rename(
                &path,
                fixture
                    .0
                    .join(format!("retained-{}", fs::metadata(&path).unwrap().ino())),
            )
            .unwrap();
            put(&path, &bytes);
        };
        match case {
            0 => replace(fixture.source()),
            1 => replace(fixture.state("receipt.json")),
            2 => replace(fixture.state("source-snapshot.sqlite3")),
            3 => replace(history(&fixture, "preparation.json")),
            4 => replace(history(&fixture, "preparation-snapshot.sqlite3")),
            5 => put(&history(&fixture, "data/receipt.json"), b"target-conflict"),
            6 => put(&fixture.state("terminal-release.json.tmp"), b"interrupted"),
            7 => put(
                &fixture.0.join(HISTORY).join("latest-release.json.tmp"),
                b"interrupted",
            ),
            8 => put(&fixture.state("unknown"), b"unknown"),
            9 => put(&history(&fixture, "data/unknown"), b"unknown"),
            10 => fs::set_permissions(history(&fixture, "data"), fs::Permissions::from_mode(0o755))
                .unwrap(),
            11 => {
                let path = history(&fixture, "data");
                fs::rename(&path, fixture.0.join("retained-data")).unwrap();
                DirBuilder::new().mode(0o700).create(path).unwrap();
            }
            12 => {
                let path = fixture.state("terminal-release.json");
                let bytes = String::from_utf8(fs::read(&path).unwrap())
                    .unwrap()
                    .replace(
                        "radishlex-terminal-release-v1",
                        "radishlex-terminal-release-v9",
                    );
                fs::write(path, bytes).unwrap();
            }
            13 => put(&fixture.0.join("userdb.sqlite3-wal"), b"unqualified"),
            14 => fs::write(fixture.0.join("manager-settings.json"), b"CHANGED").unwrap(),
            15 => put(&fixture.0.join(HISTORY).join("unknown"), b"unknown-history"),
            _ => unreachable!(),
        }
        let before = tree(&fixture.0);
        assert!(
            !continue_release(&fixture, ReleasePort::new(binding)),
            "case {case}"
        );
        assert_eq!(tree(&fixture.0), before, "case {case}");
    }
}

#[test]
fn index_tampering_or_wrong_outer_never_releases_marker() {
    for case in 0..6 {
        let fixture = Fixture::new();
        let (store, guard, binding, mut port) = terminal(&fixture, true, State::RolledBack);
        store
            .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
            .unwrap();
        let index = fixture.0.join(HISTORY).join("latest-release.json");
        match case {
            0 => {}
            1 => fs::remove_file(index).unwrap(),
            2 => {
                let bytes = fs::read(&index).unwrap();
                fs::write(index, String::from_utf8(bytes).unwrap().replace(NEW, OLD)).unwrap();
            }
            3 => port.binding.installed_product_sha256 = "d".repeat(64),
            4 => {}
            5 => {
                put(
                    &history(&fixture, "terminal-release.json"),
                    &fs::read(fixture.state("terminal-release.json")).unwrap(),
                );
                assert!(store.load_latest_release(&guard, &Hasher).is_err());
            }
            _ => unreachable!(),
        }
        let before = tree(&fixture.0);
        if case != 0 {
            port.finalize(&fixture.0);
        }
        if case == 4 {
            port.outer_proof = Some(b"other-release-proof".to_vec());
        }
        assert!(store
            .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
            .is_err());
        assert!(fixture.state("terminal-release.json").exists());
        assert_eq!(tree(&fixture.0), before);
    }
}

#[test]
fn strict_contract_rejects_unknown_fields_invalid_phase_and_changed_source_binding() {
    let fixture = Fixture::new();
    let (store, guard, binding, mut port) = terminal(&fixture, false, State::Completed);
    let ready = store
        .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
        .unwrap();
    let bytes = ready.encode().unwrap();
    assert_eq!(Proof::decode(&bytes).unwrap(), ready);
    let text = String::from_utf8(bytes).unwrap();
    for bad in [
        text.replacen("{", "{\"unknown\":true,", 1),
        text.replace("release_ready", "released"),
        text.replace(
            "radishlex-terminal-release-v1",
            "radishlex-terminal-release-v2",
        ),
        text.replacen("\"mode\":448", "\"mode\":493", 1),
        text.trim_end().to_owned(),
    ] {
        assert!(Proof::decode(bad.as_bytes()).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["archived_slots"] = serde_json::json!([]);
    let wrong = serde_json::to_vec(&value).unwrap();
    assert!(Proof::decode(&wrong).is_err());
}

#[test]
fn nonterminal_manual_and_wrong_product_results_are_rejected_before_history_creation() {
    for case in 0..4 {
        let fixture = Fixture::new();
        let (store, guard, mut receipt) = standalone(&fixture, NEW, None, 39, 41);
        if case == 1 {
            receipt.advance(State::Quiesced).unwrap();
            store.persist(&guard, &receipt).unwrap();
            store.create_settings_backup(&guard, &mut receipt).unwrap();
            store
                .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
                .unwrap();
            store.create_userdb_candidate(&guard, &mut receipt).unwrap();
            receipt
                .abort_preserved(UpgradeFailureCode::ManagerValidationFailed, true)
                .unwrap();
            store.persist(&guard, &receipt).unwrap();
        } else if case >= 2 {
            store
                .resume_userdb_upgrade(
                    &guard,
                    &mut receipt,
                    u64::MAX,
                    &mut ProductPort(State::Completed),
                )
                .unwrap();
        }
        let store = Store::attach(store, &guard).unwrap();
        let binding = Binding {
            operation_id: if case == 3 { OLD } else { NEW }.to_owned(),
            installed_release: release(39),
            installed_product_sha256: "a".repeat(64),
        };
        let before = tree(&fixture.0);
        let mut port = ReleasePort::new(binding.clone());
        assert!(store
            .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
            .is_err());
        assert_eq!(tree(&fixture.0), before);
        assert!(!fixture.0.join(HISTORY).exists());
    }
}

#[test]
fn callback_conflicts_are_rechecked_before_mutation() {
    for case in 0..3 {
        let fixture = Fixture::new();
        let (store, guard, binding, mut port) = terminal(&fixture, true, State::AbortedPreserved);
        let (point, path) = match case {
            0 => (
                RP::BeforeMove(Slot::Receipt),
                history(&fixture, "data/receipt.json"),
            ),
            1 => (
                RP::BeforeIndexWrite,
                fixture.0.join(HISTORY).join("latest-release.json"),
            ),
            _ => (
                RP::BeforeMarkerMove,
                history(&fixture, "terminal-release.json"),
            ),
        };
        let copy = path.clone();
        port.mutation = Some((point, Box::new(move || put(&copy, b"preserve-conflict"))));
        if case == 2 {
            store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .unwrap();
            port.finalize(&fixture.0);
            assert!(store
                .finish_terminal_release(&guard, NEW, &mut port, &Hasher)
                .is_err());
        } else {
            assert!(store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .is_err());
        }
        assert_eq!(fs::read(path).unwrap(), b"preserve-conflict");
        assert!(fixture.state("terminal-release.json").exists());
    }
}

#[test]
fn interrupted_progress_records_never_adopt_temps_or_lose_moved_slots() {
    for point in [RP::RecordCreated, RP::RecordFileSynced, RP::RecordRenamed] {
        for skip in [1, 5] {
            let fixture = Fixture::new();
            let (store, guard, binding, mut port) =
                terminal(&fixture, false, State::AbortedPreserved);
            port.fail = Some(point);
            port.skip = skip;
            assert!(store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .is_err());
            assert!(history(&fixture, "data/receipt.json").exists());
            assert!(!fixture.state("receipt.json").exists());
            drop(guard);
            let before = tree(&fixture.0);
            let success = continue_release(&fixture, ReleasePort::new(binding));
            assert_eq!(success, point == RP::RecordRenamed, "{point:?} skip {skip}");
            if !success {
                assert_eq!(tree(&fixture.0), before);
            }
        }
    }
}

#[test]
fn initialization_callback_cannot_create_or_replace_unbound_directories() {
    for case in 0..5 {
        let fixture = Fixture::new();
        let (store, guard, binding, mut port) = terminal(&fixture, false, State::Completed);
        let root = fixture.0.join(HISTORY);
        if case == 1 {
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let root_copy = root.clone();
        let retained = fixture.0.join("retained-history");
        let point = match case {
            2 => RP::DirectoryCreated(RD::HistoryRoot),
            3 => RP::DirectoryCreated(RD::Operation),
            4 => RP::ParentSynced(RD::HistoryRoot),
            _ => RP::Begin,
        };
        let injected = match case {
            2 | 4 => root.join(NEW),
            3 => root.join(NEW).join("data"),
            _ => root.clone(),
        };
        let injected_copy = injected.clone();
        port.mutation = Some((
            point,
            Box::new(move || {
                if case == 1 {
                    fs::rename(&root_copy, &retained).unwrap();
                }
                fs::create_dir(&injected_copy).unwrap();
                fs::set_permissions(&injected_copy, fs::Permissions::from_mode(0o700)).unwrap();
            }),
        ));
        let receipt = fs::read(fixture.state("receipt.json")).unwrap();
        assert!(
            store
                .prepare_terminal_release(&guard, &binding, &mut port, &Hasher)
                .is_err(),
            "case {case}"
        );
        assert_eq!(fs::read(fixture.state("receipt.json")).unwrap(), receipt);
        assert!(!fixture.state("terminal-release.json").exists());
        assert!(injected.is_dir());
        assert_eq!(fs::read_dir(injected).unwrap().count(), 0);
    }
}
