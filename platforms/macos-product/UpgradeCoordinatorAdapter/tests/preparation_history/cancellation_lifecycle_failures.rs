use super::*;

pub(super) fn tree(root: &Path) -> Vec<(PathBuf, u64, u32, Vec<u8>)> {
    let mut result = vec![];
    let mut paths = vec![root.to_owned()];
    while let Some(path) = paths.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(&path).unwrap()
        } else {
            vec![]
        };
        result.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.ino(),
            metadata.mode(),
            bytes,
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                paths.push(entry.unwrap().path());
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}
fn continue_release(fixture: &Fixture, port: &mut ReleaseAuthority) -> bool {
    let (store, guard) = reload(fixture);
    if fixture.state("preparation-cancellation.json").exists()
        && store
            .prepare_cancellation_release(&guard, NEW, port, &Hasher)
            .is_err()
    {
        return false;
    }
    store
        .finish_cancellation_release(&guard, NEW, port, &Hasher)
        .is_ok()
}
fn unbound(point: RP) -> bool {
    matches!(
        point,
        RP::ProofCreated
            | RP::ProofWritten
            | RP::ProofFileSynced
            | RP::IndexCreated
            | RP::IndexWritten
            | RP::IndexFileSynced
    )
}

#[test]
#[ignore = "entry exercised by cancellation_lifecycle_all_boundaries_recover_or_retain_unbound_evidence"]
fn cancellation_release_crash_child() {
    let Ok(root) = std::env::var("RADISHLEX_CANCEL_RELEASE_CHILD") else {
        return;
    };
    let fixture = Fixture(PathBuf::from(root));
    let target = std::env::var("RADISHLEX_CANCEL_RELEASE_POINT").unwrap();
    let point = points()
        .into_iter()
        .find(|p| format!("{p:?}") == target)
        .unwrap();
    let mut port = ReleaseAuthority::new();
    port.fail = Some(point);
    port.exit = true;
    continue_release(&fixture, &mut port);
    panic!("checkpoint was not visited");
}
fn points() -> Vec<RP> {
    vec![
        RP::Begin,
        RP::SourceVerified,
        RP::ProofCreated,
        RP::ProofWritten,
        RP::ProofFileSynced,
        RP::ProofPublished,
        RP::ProofDirectorySynced,
        RP::BeforeArchiveMove,
        RP::ArchiveFileSynced,
        RP::ArchiveMoved,
        RP::ArchiveTargetSynced,
        RP::ArchiveSourceSynced,
        RP::BeforeIndexWrite,
        RP::IndexCreated,
        RP::IndexWritten,
        RP::IndexFileSynced,
        RP::IndexPublished,
        RP::IndexDirectorySynced,
        RP::Ready,
        RP::BeforeMarkerMove,
        RP::MarkerFileSynced,
        RP::MarkerMoved,
        RP::MarkerTargetSynced,
        RP::MarkerSourceSynced,
        RP::Released,
    ]
}

#[test]
fn representative_cancellation_release_boundaries_recover_or_retain_unbound_evidence() {
    check_release_boundaries(vec![RP::ProofCreated, RP::MarkerMoved]);
}

#[cfg(feature = "reliability-matrix")]
#[test]
fn reliability_matrix_cancellation_lifecycle_all_boundaries_recover_or_retain_unbound_evidence() {
    check_release_boundaries(points());
}

fn check_release_boundaries(checkpoints: Vec<RP>) {
    // Internal serial loop: never rebuild the test executable beside children.
    for maintained in [false, true] {
        for point in checkpoints.iter().copied() {
            for child in [false, true] {
                let fixture = Fixture::new();
                preserved(
                    &fixture,
                    maintained.then_some(State::Completed),
                    if maintained { 2 } else { 0 },
                    true,
                );
                if child {
                    let output = Command::new(std::env::current_exe().unwrap())
                        .args(["--exact", "cancellation_archive::lifecycle::failures::cancellation_release_crash_child", "--ignored", "--nocapture"])
                        .env("RADISHLEX_CANCEL_RELEASE_CHILD", &fixture.0)
                        .env("RADISHLEX_CANCEL_RELEASE_POINT", format!("{point:?}"))
                        .output().unwrap();
                    assert_eq!(
                        output.status.code(),
                        Some(76),
                        "{point:?}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                } else {
                    let mut port = ReleaseAuthority::new();
                    port.fail = Some(point);
                    assert!(!continue_release(&fixture, &mut port), "{point:?}");
                }
                let before = tree(&fixture.0);
                let observed = radishlex_ime_product_upgrade::inspect_maintenance(
                    &fixture.0,
                    fs::metadata(&fixture.0).unwrap().uid(),
                    &Hasher,
                );
                assert_eq!(
                    tree(&fixture.0),
                    before,
                    "inspection after {point:?} must preserve interruption evidence"
                );
                let released = observed
                    .as_ref()
                    .ok()
                    .and_then(|value| value.as_ref())
                    .is_some_and(|value| {
                        value.phase()
                            == radishlex_ime_product_upgrade::MaintenancePhase::CancellationReleased
                    });
                assert_eq!(
                    released,
                    matches!(
                        point,
                        RP::MarkerMoved
                            | RP::MarkerTargetSynced
                            | RP::MarkerSourceSynced
                            | RP::Released
                    ),
                    "{point:?}, child {child}, maintained {maintained}: {observed:?}"
                );
                let continued = continue_release(&fixture, &mut ReleaseAuthority::new());
                assert_eq!(
                    continued,
                    !unbound(point),
                    "{point:?}, child {child}, maintained {maintained}"
                );
                if continued {
                    assert_eq!(query(&fixture).operation_id(), NEW);
                } else {
                    assert_eq!(tree(&fixture.0), before);
                    assert!(fixture.state("preparation-cancellation.json").exists());
                }
            }
        }
    }
}

#[test]
fn cancellation_lifecycle_strict_proof_and_locator_reject_invalid_bindings() {
    let fixture = Fixture::new();
    preserved(&fixture, None, 0, true);
    let proof = finish(&fixture, NEW);
    let original = String::from_utf8(proof.encode().unwrap()).unwrap();
    for bad in [
        original.replacen(
            "radishlex-cancellation-release-v1",
            "radishlex-cancellation-release-v9",
            1,
        ),
        original.replacen("\"preserved\":true", "\"preserved\":false", 1),
        original.replacen("{", "{\"unknown\":0,", 1),
        original.replacen("\"format\":", "\"format\":\"duplicate\",\"format\":", 1),
        original.replacen("\"data_receipt\":null", "\"data_receipt\":{}", 1),
    ] {
        assert_ne!(bad, original);
        assert!(Proof::decode(bad.as_bytes()).is_err());
    }
    assert!(Proof::decode(&vec![b' '; 256 * 1024 + 1]).is_err());
    let path = fixture.0.join(HISTORY).join("latest-release.json");
    let index = fs::read_to_string(&path).unwrap();
    for bad in [
        index.replacen("preparation_cancellation", "terminal_release", 1),
        index.replacen(
            "radishlex-latest-release-v2",
            "radishlex-latest-release-v1",
            1,
        ),
        index.replacen(OUTER, NEW, 1),
        index.replacen(
            "\"data_operation_id\":null",
            &format!("\"data_operation_id\":\"{NEW}\""),
            1,
        ),
        index.replacen(
            "\"data_operation_id\":null",
            &format!("\"data_operation_id\":\"{OLD}\""),
            1,
        ),
        index.replacen("preparation_cancellation", "unknown", 1),
    ] {
        assert_ne!(bad, index);
        fs::write(&path, bad).unwrap();
        let store = TerminalReleaseStore::open_existing(fixture.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        assert!(store.load_latest_lifecycle(&guard, &Hasher).is_err());
    }
    fs::rename(&path, fixture.0.join("retained-index")).unwrap();
    let store = TerminalReleaseStore::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    assert!(store.load_latest_lifecycle(&guard, &Hasher).is_err());
}

#[test]
fn representative_cancellation_lifecycle_ancestor_drift_blocks_query_and_new_preparation_without_writes(
) {
    check_ancestor_drift(2..3);
}

#[cfg(feature = "reliability-matrix")]
#[test]
fn reliability_matrix_cancellation_lifecycle_ancestor_drift_blocks_query_and_new_preparation_without_writes(
) {
    check_ancestor_drift(0..6);
}

fn check_ancestor_drift(kinds: std::ops::Range<u8>) {
    for kind in kinds {
        let fixture = Fixture::new();
        preserved(&fixture, Some(State::Completed), 3, true);
        finish(&fixture, NEW);
        cancel_next(&fixture, SECOND, Some(OLD));
        let path = fixture
            .0
            .join(HISTORY)
            .join(NEW)
            .join("preparation-cancellation.json");
        match kind {
            0 => {
                fs::rename(&path, fixture.0.join("retained-request")).unwrap();
            }
            1 => {
                fs::write(&path, b"changed").unwrap();
            }
            2 => {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, fixture.0.join("retained-request")).unwrap();
                put(&path, &bytes);
            }
            3 => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
            4 => {
                fs::hard_link(&path, fixture.0.join("linked-request")).unwrap();
            }
            _ => {
                put(&fixture.0.join(HISTORY).join(NEW).join("unknown"), b"keep");
            }
        }
        let before = tree(&fixture.0);
        let store = TerminalReleaseStore::open_existing(fixture.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        assert!(store.load_latest_lifecycle(&guard, &Hasher).is_err());
        drop(guard);
        let (store, guard) = fixture.reload();
        assert!(store
            .capture_previous_inventory(&guard, THIRD, Some(OUTER), Some(OLD), &Hasher)
            .is_err());
        assert_eq!(tree(&fixture.0), before);
    }
}

#[test]
fn cancellation_lifecycle_callback_drift_cannot_release_or_overwrite_conflicts() {
    for (point, name) in [
        (RP::ProofPublished, "cancellation-archive.json"),
        (RP::BeforeMarkerMove, "preparation-cancellation.json"),
        (RP::Ready, "preparation-cancellation.json"),
    ] {
        let fixture = Fixture::new();
        preserved(&fixture, None, 0, true);
        let path = fixture.state(name);
        let retained = fixture.0.join("retained-marker");
        let mut port = ReleaseAuthority::new();
        port.mutation = Some((
            point,
            Box::new(move || {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, retained).unwrap();
                put(&path, &bytes);
            }),
        ));
        assert!(!continue_release(&fixture, &mut port));
        assert!(fixture.state("preparation-cancellation.json").exists());
    }
}

#[test]
fn cancellation_lifecycle_locator_pins_latest_proof_inode_and_bytes() {
    let fixture = Fixture::new();
    preserved(&fixture, None, 0, false);
    finish(&fixture, NEW);
    let path = fixture
        .0
        .join(HISTORY)
        .join(NEW)
        .join("cancellation-release.json");
    let original = fs::read(&path).unwrap();
    fs::rename(&path, fixture.0.join("retained-proof")).unwrap();
    put(&path, &original);
    let before = tree(&fixture.0);
    let store = TerminalReleaseStore::open_existing(fixture.verified()).unwrap();
    let guard = store.acquire_guard().unwrap();
    assert!(store.load_latest_lifecycle(&guard, &Hasher).is_err());
    assert_eq!(tree(&fixture.0), before);
}

#[test]
fn cancellation_proof_codec_preserves_nested_validation_and_canonical_bytes() {
    let fixture = Fixture::new();
    preserved(&fixture, None, 0, true);
    let proof = finish(&fixture, NEW);
    let canonical = proof.encode().unwrap();
    let mut direct = serde_json::to_vec(&proof).unwrap();
    direct.push(b'\n');
    assert_eq!(direct, canonical);
    assert_eq!(Proof::decode(&canonical).unwrap(), proof);
    for (checked, mut direct) in [
        (
            proof.archive().encode().unwrap(),
            serde_json::to_vec(proof.archive()).unwrap(),
        ),
        (
            proof.archive().request().encode().unwrap(),
            serde_json::to_vec(proof.archive().request()).unwrap(),
        ),
        (
            proof.archive().source().encode().unwrap(),
            serde_json::to_vec(proof.archive().source()).unwrap(),
        ),
        (
            proof.preparation().encode().unwrap(),
            serde_json::to_vec(proof.preparation()).unwrap(),
        ),
    ] {
        direct.push(b'\n');
        assert_eq!(checked, direct);
    }

    // A caller can deserialize into the public model without using its strict
    // decoder. encode() must still reject invalid nested values/relationships.
    let json = serde_json::to_value(&proof).unwrap();
    for (pointer, changed) in [
        ("/format", serde_json::json!("invalid")),
        ("/archive/format", serde_json::json!("invalid")),
        ("/archive/request/format", serde_json::json!("invalid")),
        (
            "/archive/request/preparation/format",
            serde_json::json!("invalid"),
        ),
        ("/archive/source/format", serde_json::json!("invalid")),
        (
            "/archive/source/preparation/format",
            serde_json::json!("invalid"),
        ),
        (
            "/archive/source/request_identity/byte_len",
            serde_json::json!(0),
        ),
        ("/archive/source_identity/byte_len", serde_json::json!(0)),
        ("/archive_identity/byte_len", serde_json::json!(0)),
        (
            "/archive/request/preparation_identity/mode",
            serde_json::json!(0o640),
        ),
        (
            "/archive/source/source_ready/database/link_count",
            serde_json::json!(2),
        ),
        (
            "/archive/outer/previous_identity/byte_len",
            serde_json::json!(0),
        ),
    ] {
        let mut invalid = json.clone();
        *invalid.pointer_mut(pointer).unwrap() = changed;
        let invalid: Proof = serde_json::from_value(invalid).unwrap();
        assert!(invalid.encode().is_err(), "{pointer}");
        let mut bytes = serde_json::to_vec(&invalid).unwrap();
        bytes.push(b'\n');
        assert!(Proof::decode(&bytes).is_err(), "{pointer}");
    }
    for bad in [
        [canonical.as_slice(), b" "].concat(),
        canonical[..canonical.len() - 1].to_vec(),
        vec![b' '; 256 * 1024 + 1],
    ] {
        assert!(Proof::decode(&bad).is_err());
    }
}
