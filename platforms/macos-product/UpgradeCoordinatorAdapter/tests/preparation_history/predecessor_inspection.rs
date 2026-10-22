use super::*;

fn tree(root: &Path) -> Vec<(PathBuf, u64, u32, Vec<u8>)> {
    let mut result = vec![];
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(&path).unwrap()
        } else {
            vec![]
        };
        result.push((path.clone(), metadata.ino(), metadata.mode(), bytes));
        if metadata.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

#[test]
fn predecessor_inspection_is_readonly_and_rejects_live_guard() {
    for state in [
        None,
        Some(State::Completed),
        Some(State::AbortedPreserved),
        Some(State::RolledBack),
    ] {
        let fixture = Fixture::new();
        let (journal, guard) = fixture.legacy(state);
        assert!(journal.inspect_preparation_predecessor(&Hasher).is_err());
        drop(guard);
        let before = tree(&fixture.0);
        let observation = journal.inspect_preparation_predecessor(&Hasher).unwrap();
        assert_eq!(observation.0.as_ref().map(UpgradeReceipt::state), state);
        assert!(observation.1.is_none());
        assert_eq!(tree(&fixture.0), before);
        let guard = journal.acquire_guard().unwrap();
        assert_eq!(
            journal
                .observe_preparation_predecessor(&guard, NEW, &Hasher)
                .unwrap(),
            observation
        );
    }
}

#[test]
fn predecessor_inspection_rejects_unbound_partial_and_drifted_material() {
    for mutation in 0..7 {
        let fixture = Fixture::new();
        let (journal, guard) = fixture.legacy(Some(if mutation == 6 {
            State::CandidateVerified
        } else {
            State::Completed
        }));
        drop(guard);
        match mutation {
            0 => put(&fixture.state("source-preparation.json.tmp"), b"partial"),
            1 => put(&fixture.state("unexpected"), b"unknown"),
            2 => {
                fs::rename(
                    fixture.state("source-snapshot.sqlite3"),
                    fixture.0.join("saved-snapshot"),
                )
                .unwrap();
            }
            3 => {
                let path = fixture.state("source-snapshot.sqlite3");
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("saved-snapshot")).unwrap();
                put(&path, &bytes);
            }
            4 => {
                let history = fixture.0.join(HISTORY);
                DirBuilder::new().mode(0o700).create(&history).unwrap();
                put(&history.join("unindexed-proof"), b"orphan");
            }
            5 => {
                fs::rename(
                    fixture.state("receipt.json"),
                    fixture.0.join("saved-receipt"),
                )
                .unwrap();
            }
            _ => {}
        }
        let before = tree(&fixture.0);
        assert!(
            journal.inspect_preparation_predecessor(&Hasher).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(tree(&fixture.0), before);
    }
}
