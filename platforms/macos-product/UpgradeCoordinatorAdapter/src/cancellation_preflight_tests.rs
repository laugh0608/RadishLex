use super::*;
use radishlex_ime_product_upgrade::PreparationDirectoryIdentity;

fn cancellation_response(root: &PreparationDirectoryIdentity) -> serde_json::Value {
    json!({
        "format": "radishlex-cancellation-preflight-v1", "result": "ready",
        "available_bytes": 100, "quiescent": true, "data_root": root,
        "manager_bundle_id": "dev.radishlex.radishlexManager",
        "input_method_bundle_id": "org.radishlex.inputmethod.macos"
    })
}
fn root() -> PreparationDirectoryIdentity {
    PreparationDirectoryIdentity {
        device_id: 1,
        inode: 2,
        owner_id: 501,
        mode: 0o700,
    }
}
fn output(value: &serde_json::Value) -> HostOutput {
    HostOutput {
        success: true,
        stdout: serde_json::to_vec(value).unwrap(),
        stderr: vec![],
    }
}
fn digest(fixture: &Fixture) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(fixture.target.join("ProductManifest.json")).unwrap())
    )
}

#[test]
fn cancellation_observation_is_fresh_and_bound_to_root_and_exact_manifest() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([output(&cancellation_response(&root())), failed()]);
    let mut adapter =
        MacOsProductPreflightAdapter::with_runner(&fixture.target, Box::new(runner)).unwrap();
    assert_eq!(
        adapter
            .inspect_cancellation_quiescence(&root(), &digest(&fixture))
            .unwrap()
            .available_bytes(),
        100
    );
    assert!(adapter
        .inspect_cancellation_quiescence(&root(), &digest(&fixture))
        .is_err());
    assert_eq!(state.borrow().calls.len(), 2);
    assert!(state
        .borrow()
        .calls
        .iter()
        .all(|(_, mode)| *mode == HostMode::CancellationPreflight));
    assert!(adapter
        .inspect_cancellation_quiescence(&root(), &"f".repeat(64))
        .is_err());
    assert_eq!(state.borrow().calls.len(), 2);
}

#[test]
fn cancellation_observation_rejects_legacy_unknown_duplicate_and_foreign_fields() {
    let fixture = Fixture::new();
    let correct = cancellation_response(&root());
    let changes = [
        ("format", json!("radishlex-upgrade-preflight-v1")),
        ("result", json!("blocked")),
        ("quiescent", json!(false)),
        ("available_bytes", json!(0)),
        ("manager_bundle_id", json!("foreign")),
        ("input_method_bundle_id", json!("foreign")),
        ("unknown", json!(true)),
        (
            "data_root",
            json!({"device_id":1,"inode":3,"owner_id":501,"mode":448}),
        ),
    ];
    let mut responses = vec![ready_preflight(100)];
    for (field, value) in changes {
        let mut changed = correct.clone();
        changed[field] = value;
        responses.push(output(&changed));
    }
    let mut duplicate = output(&correct);
    duplicate.stdout = String::from_utf8(duplicate.stdout)
        .unwrap()
        .replacen('{', "{\"quiescent\":true,", 1)
        .into_bytes();
    responses.push(duplicate);
    let mut stderr = output(&correct);
    stderr.stderr = b"warning".to_vec();
    responses.push(stderr);
    let count = responses.len();
    let (runner, _) = FakeRunner::new(responses);
    let mut adapter =
        MacOsProductPreflightAdapter::with_runner(&fixture.target, Box::new(runner)).unwrap();
    for _ in 0..count {
        assert_eq!(
            adapter.inspect_cancellation_quiescence(&root(), &digest(&fixture)),
            Err(MacOsUpgradeAdapterError::PreflightRejected)
        );
    }
}

struct DriftingRunner {
    path: PathBuf,
    bytes: Vec<u8>,
}
impl ProductHostRunner for DriftingRunner {
    fn run(&mut self, _: &VerifiedExecutable, _: HostMode) -> HostOutput {
        fs::write(&self.path, &self.bytes).unwrap();
        output(&cancellation_response(&root()))
    }
}

#[test]
fn cancellation_observation_rechecks_manifest_and_host_after_native_return() {
    for helper in [false, true] {
        let fixture = Fixture::new();
        let expected = digest(&fixture);
        let path = if helper {
            fixture.target.join(
                "Components/radishlex_manager.app/Contents/Helpers/RadishLexUpgradePreflightHost",
            )
        } else {
            fixture.target.join("ProductManifest.json")
        };
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        let runner = DriftingRunner { path, bytes };
        let mut adapter =
            MacOsProductPreflightAdapter::with_runner(&fixture.target, Box::new(runner)).unwrap();
        assert_eq!(
            adapter.inspect_cancellation_quiescence(&root(), &expected),
            Err(MacOsUpgradeAdapterError::ProductChanged)
        );
    }
}
