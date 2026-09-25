//! Conflicts and identity changes must remain untouched on rejection.
use super::*;

fn replace_same_bytes(path: &Path) {
    let bytes = fs::read(path).unwrap();
    let root = path
        .ancestors()
        .find(|parent| {
            parent.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("radishlex-preparation-history-")
            })
        })
        .unwrap();
    fs::rename(
        path,
        root.join(format!("retained-{}", fs::metadata(path).unwrap().ino())),
    )
    .unwrap();
    put(path, &bytes);
}

fn tree(root: &Path) -> Vec<(PathBuf, u64, u32, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, items: &mut Vec<(PathBuf, u64, u32, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let bytes = if metadata.is_symlink() {
            fs::read_link(path)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else if metadata.is_file() {
            fs::read(path).unwrap()
        } else {
            Vec::new()
        };
        items.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.ino(),
            metadata.mode(),
            bytes,
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), items);
            }
        }
    }
    let mut items = Vec::new();
    walk(root, root, &mut items);
    items.sort();
    items
}

#[test]
fn changed_objects_and_unknown_material_are_preserved_on_fresh_guard_retry() {
    for case in 0..24 {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(Some(State::Completed));
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        port.fail = Some(Point::HandoffIntentRecorded);
        assert!(store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .is_err());
        drop(guard);
        let new_receipt = new_history(&fixture, "receipt.json");
        let snapshot = fixture.state("preparation-snapshot.sqlite3");
        let marker = fixture.state("source-preparation.json");
        match case {
            0 => replace_same_bytes(&fixture.source()),
            1 => replace_same_bytes(&new_receipt),
            2 => fs::set_permissions(&new_receipt, fs::Permissions::from_mode(0o640)).unwrap(),
            3 => {
                let directory = new_history(&fixture, "");
                fs::rename(&directory, fixture.0.join("retained-new-history")).unwrap();
                DirBuilder::new().mode(0o700).create(&directory).unwrap();
                fs::rename(
                    fixture.0.join("retained-new-history/receipt.json"),
                    &new_receipt,
                )
                .unwrap();
            }
            4 => {
                let root = fixture.0.join(HISTORY);
                fs::rename(&root, fixture.0.join("retained-history")).unwrap();
                DirBuilder::new().mode(0o700).create(&root).unwrap();
                for operation in [OLD, NEW] {
                    fs::rename(
                        fixture.0.join("retained-history").join(operation),
                        root.join(operation),
                    )
                    .unwrap();
                }
            }
            5 => replace_same_bytes(&snapshot),
            6 => replace_same_bytes(&fixture.0.join(HISTORY).join(OLD).join("inventory.json")),
            7 => put(
                &fixture.state("receipt.json"),
                &fs::read(&new_receipt).unwrap(),
            ),
            8 => put(&fixture.state("unknown"), b"preserve"),
            9 => put(
                &fixture.state("source-preparation.json.tmp"),
                b"interrupted-record",
            ),
            10 => put(&fixture.state("receipt.json.tmp"), b"interrupted-receipt"),
            11 => put(&fixture.state("source-backup.sqlite3"), b"unrelated"),
            12 => put(&new_history(&fixture, "unknown"), b"preserve"),
            13 => replace_same_bytes(&fixture.0.join("manager-settings.json")),
            14 => fs::write(
                fixture.0.join("manager-settings.json"),
                b"SYNTHETIC-SETTINGS",
            )
            .unwrap(),
            15 => DirBuilder::new()
                .mode(0o700)
                .create(fixture.0.join("Rime"))
                .unwrap(),
            16 => {
                let text = String::from_utf8(fs::read(&marker).unwrap())
                    .unwrap()
                    .replace(
                        "radishlex-source-preparation-v1",
                        "radishlex-source-preparation-v9",
                    );
                fs::write(&marker, text).unwrap();
            }
            17 => fs::remove_file(fixture.history("receipt.json")).unwrap(),
            18 => fs::rename(
                &snapshot,
                new_history(&fixture, "preparation-snapshot.sqlite3"),
            )
            .unwrap(),
            19 => fs::rename(&marker, new_history(&fixture, "preparation.json")).unwrap(),
            20 => {
                let original = fixture.0.join("retained-receipt");
                fs::rename(&new_receipt, &original).unwrap();
                symlink(original, &new_receipt).unwrap();
            }
            21 => fs::hard_link(&new_receipt, fixture.0.join("receipt-link")).unwrap(),
            22 => fs::remove_file(fixture.0.join("manager-settings.json")).unwrap(),
            23 => put(&fixture.0.join("userdb.sqlite3-wal"), b"unqualified-wal"),
            _ => unreachable!(),
        }
        let before = tree(&fixture.0);
        port.fail = None;
        if let Ok(store) = PreparationJournalStore::open_existing(fixture.verified()) {
            let guard = store.acquire_guard().unwrap();
            assert!(
                store
                    .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
                    .is_err(),
                "case {case}"
            );
        }
        assert_eq!(tree(&fixture.0), before, "case {case}");
    }
}

#[test]
fn conflicts_during_authority_callbacks_never_overwrite_targets_or_release_marker() {
    for point in [
        Point::HandoffBeforeReceiptRename,
        Point::HandoffBeforeSnapshotRename,
        Point::HandoffBeforeMarkerRename,
    ] {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(None);
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        let target = match point {
            Point::HandoffBeforeReceiptRename => fixture.state("receipt.json"),
            Point::HandoffBeforeSnapshotRename => {
                new_history(&fixture, "preparation-snapshot.sqlite3")
            }
            Point::HandoffBeforeMarkerRename => new_history(&fixture, "preparation.json"),
            _ => unreachable!(),
        };
        let copy = target.clone();
        port.mutation = Some((point, Box::new(move || put(&copy, b"preserve-conflict"))));
        assert!(store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .is_err());
        assert_eq!(fs::read(target).unwrap(), b"preserve-conflict");
        assert!(fixture.state("source-preparation.json").exists());
    }
}

#[test]
fn completed_handoff_requires_exact_operation_and_unchanged_proof_relationships() {
    for case in 0..5 {
        let fixture = Fixture::new();
        let (store, guard, mut port) = fixture.reserve(None);
        store
            .archive_previous_upgrade(&guard, &mut port, &Hasher)
            .unwrap();
        store
            .handoff_userdb_source(&guard, NEW, &mut port, &Hasher)
            .unwrap();
        drop(guard);
        match case {
            0 => {}
            1 => replace_same_bytes(&fixture.state("receipt.json")),
            2 => replace_same_bytes(&new_history(&fixture, "preparation-snapshot.sqlite3")),
            3 => {
                let path = new_history(&fixture, "preparation.json");
                let text = String::from_utf8(fs::read(&path).unwrap())
                    .unwrap()
                    .replace(OUTER, OLD);
                fs::write(path, text).unwrap();
            }
            4 => put(
                &fixture.state("source-preparation.json"),
                &fs::read(new_history(&fixture, "preparation.json")).unwrap(),
            ),
            _ => unreachable!(),
        }
        let before = tree(&fixture.0);
        let (store, guard) = fixture.reload();
        let operation = if case == 0 { OLD } else { NEW };
        let mut port = BindingPort(port.binding);
        assert!(
            store
                .handoff_userdb_source(&guard, operation, &mut port, &Hasher)
                .is_err(),
            "case {case}"
        );
        assert_eq!(tree(&fixture.0), before);
    }
}

struct BindingPort(PreparationBinding);
impl SourcePreparationPort for BindingPort {
    fn confirm_authority_and_quiescence(&mut self, record: &PreparationReceipt, _: Point) -> bool {
        record.binding() == &self.0
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
