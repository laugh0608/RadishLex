use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_upgrade::{
    ProductRelease, UpgradeCandidateValidationReport, UpgradeCoordinatorCheckpoint,
    UpgradeCoordinatorPort, UpgradePostSwitchValidationReport,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::runner::{HostMode, HostOutput, ProductHostRunner};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    target: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "radishlex-macos-upgrade-adapter-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create fixture root");
        let source = root.join("source");
        let target = root.join("target");
        write_product(&source, "0.0.9", "34", 8, false);
        write_product(&target, "0.1.0", "35", 9, true);
        Self {
            root,
            source,
            target,
        }
    }

    fn target_manager_validation(&self) -> PathBuf {
        self.target
            .join("Components/radishlex_manager.app")
            .join("Contents/Helpers/RadishLexUpgradeValidationHost")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct FakeRunner {
    state: Rc<RefCell<FakeRunnerState>>,
}

#[derive(Default)]
struct FakeRunnerState {
    responses: VecDeque<HostOutput>,
    calls: Vec<(String, HostMode)>,
}

impl FakeRunner {
    fn new(
        responses: impl IntoIterator<Item = HostOutput>,
    ) -> (Self, Rc<RefCell<FakeRunnerState>>) {
        let state = Rc::new(RefCell::new(FakeRunnerState {
            responses: responses.into_iter().collect(),
            calls: Vec::new(),
        }));
        (
            Self {
                state: state.clone(),
            },
            state,
        )
    }
}

impl ProductHostRunner for FakeRunner {
    fn run(&mut self, executable: &VerifiedExecutable, mode: HostMode) -> HostOutput {
        let name = executable
            .path()
            .file_name()
            .expect("host file name")
            .to_string_lossy()
            .into_owned();
        let mut state = self.state.borrow_mut();
        state.calls.push((name, mode));
        state.responses.pop_front().unwrap_or_else(failed)
    }
}

#[test]
fn preflight_accepts_only_strict_ready_summary() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([ready_preflight(987_654)]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");

    let summary = adapter.inspect_preflight().expect("preflight");

    assert_eq!(summary.available_bytes(), 987_654);
    assert_eq!(
        state.borrow().calls,
        vec![(
            "RadishLexUpgradePreflightHost".to_owned(),
            HostMode::Preflight
        )]
    );
}

#[test]
fn target_only_preflight_uses_the_same_manifest_bound_host() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([ready_preflight(456_789)]);
    let mut adapter = MacOsProductPreflightAdapter::with_runner(&fixture.target, Box::new(runner))
        .expect("load target-only preflight adapter");

    let summary = adapter.inspect_preflight().expect("target-only preflight");

    assert_eq!(summary.available_bytes(), 456_789);
    assert!(adapter.matches_release("0.1.0", 35));
    assert!(!adapter.matches_release("0.1.1", 36));
    assert_eq!(
        state.borrow().calls,
        vec![(
            "RadishLexUpgradePreflightHost".to_owned(),
            HostMode::Preflight
        )]
    );
    assert_eq!(
        MacOsProductPreflightAdapter::load(&fixture.source)
            .expect_err("source product has no preflight host"),
        MacOsUpgradeAdapterError::InvalidManifest
    );
}

#[test]
fn preflight_rejects_extra_fields_and_nonempty_stderr() {
    let fixture = Fixture::new();
    let extra = HostOutput {
        success: true,
        stdout: br#"{"format":"radishlex-upgrade-preflight-v1","result":"ready","available_bytes":1,"quiescent":true,"path":"/forbidden"}"#.to_vec(),
        stderr: Vec::new(),
    };
    let noisy = HostOutput {
        success: true,
        stdout: ready_preflight(1).stdout,
        stderr: b"unexpected".to_vec(),
    };
    let (runner, _) = FakeRunner::new([extra, noisy]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");

    assert_eq!(
        adapter.inspect_preflight(),
        Err(MacOsUpgradeAdapterError::PreflightRejected)
    );
    assert_eq!(
        adapter.inspect_preflight(),
        Err(MacOsUpgradeAdapterError::PreflightRejected)
    );
}

#[test]
fn candidate_and_post_switch_are_bound_to_target_hosts() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([succeeded(), succeeded(), succeeded(), succeeded()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");
    let target = ProductRelease::new("0.1.0", 35).expect("target release");

    let candidate = adapter.validate_candidate(&target, 9);
    let post_switch = adapter.validate_post_switch(&target, 9);

    assert!(matches!(
        candidate,
        UpgradeCandidateValidationReport::Passed { .. }
    ));
    assert!(matches!(
        post_switch,
        UpgradePostSwitchValidationReport::Passed { .. }
    ));
    let modes: Vec<_> = state.borrow().calls.iter().map(|(_, mode)| *mode).collect();
    assert_eq!(
        modes,
        vec![
            HostMode::Candidate,
            HostMode::Candidate,
            HostMode::PostSwitch,
            HostMode::PostSwitch,
        ]
    );
}

#[test]
fn target_release_drift_fails_before_launching_hosts() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");
    let wrong = ProductRelease::new("0.1.0", 36).expect("wrong release");

    assert_eq!(
        adapter.validate_candidate(&wrong, 9),
        UpgradeCandidateValidationReport::manager_failed()
    );
    assert_eq!(
        adapter.validate_post_switch(&wrong, 9),
        UpgradePostSwitchValidationReport::manager_failed()
    );
    assert!(state.borrow().calls.is_empty());
}

#[test]
fn endpoint_failures_preserve_ordered_validation_semantics() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([failed(), succeeded(), failed()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");
    let target = ProductRelease::new("0.1.0", 35).expect("target release");

    assert_eq!(
        adapter.validate_candidate(&target, 9),
        UpgradeCandidateValidationReport::manager_failed()
    );
    assert!(matches!(
        adapter.validate_post_switch(&target, 9),
        UpgradePostSwitchValidationReport::InputMethodFailed { .. }
    ));
    assert_eq!(state.borrow().calls.len(), 3);
}

#[test]
fn restored_source_requires_both_source_hosts() {
    let fixture = Fixture::new();
    let source = ProductRelease::new("0.0.9", 34).expect("source release");
    let (runner, state) = FakeRunner::new([succeeded(), succeeded(), succeeded(), failed()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");

    assert!(adapter.validate_restored_source(&source, 8).is_some());
    assert!(adapter.validate_restored_source(&source, 8).is_none());
    assert_eq!(
        state
            .borrow()
            .calls
            .iter()
            .map(|(_, mode)| *mode)
            .collect::<Vec<_>>(),
        vec![
            HostMode::PostSwitch,
            HostMode::PostSwitch,
            HostMode::PostSwitch,
            HostMode::PostSwitch,
        ]
    );
}

#[test]
fn manifest_bound_host_change_is_rejected_before_execution() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([succeeded()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");
    let target = ProductRelease::new("0.1.0", 35).expect("target release");
    fs::write(fixture.target_manager_validation(), b"changed-host")
        .expect("replace target helper bytes");
    set_executable(&fixture.target_manager_validation());

    assert_eq!(
        adapter.validate_candidate(&target, 9),
        UpgradeCandidateValidationReport::manager_failed()
    );
    assert!(state.borrow().calls.is_empty());
}

#[test]
fn quiescence_rechecks_preflight_for_each_checkpoint() {
    let fixture = Fixture::new();
    let (runner, state) = FakeRunner::new([ready_preflight(10), failed()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");

    assert!(adapter.confirm_quiescence(UpgradeCoordinatorCheckpoint::BeforeSnapshot));
    assert!(!adapter.confirm_quiescence(UpgradeCoordinatorCheckpoint::BeforeCandidateMigration));
    assert_eq!(state.borrow().calls.len(), 2);
}

#[test]
fn target_manifest_must_record_preflight_host() {
    let fixture = Fixture::new();
    let manifest_path = fixture.target.join("ProductManifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["components"][0]["files"]
        .as_array_mut()
        .expect("manager files")
        .retain(|record| {
            record["path"].as_str() != Some("Contents/Helpers/RadishLexUpgradePreflightHost")
        });
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    assert_eq!(
        MacOsUpgradeCoordinatorAdapter::load(&fixture.source, &fixture.target)
            .expect_err("missing preflight must fail"),
        MacOsUpgradeAdapterError::InvalidManifest
    );
}

#[test]
fn helper_parent_symlink_and_added_hardlink_are_rejected() {
    let fixture = Fixture::new();
    let helpers = fixture
        .target
        .join("Components/radishlex_manager.app/Contents/Helpers");
    let real_helpers = helpers.with_file_name("RealHelpers");
    fs::rename(&helpers, &real_helpers).expect("move helpers");
    symlink("RealHelpers", &helpers).expect("symlink helpers");
    assert_eq!(
        MacOsUpgradeCoordinatorAdapter::load(&fixture.source, &fixture.target)
            .expect_err("symlinked helper parent must fail"),
        MacOsUpgradeAdapterError::ProductChanged
    );

    fs::remove_file(&helpers).expect("remove test symlink");
    fs::rename(&real_helpers, &helpers).expect("restore helpers");
    let (runner, state) = FakeRunner::new([succeeded()]);
    let mut adapter = MacOsUpgradeCoordinatorAdapter::with_runner(
        &fixture.source,
        &fixture.target,
        Box::new(runner),
    )
    .expect("load adapter");
    let hardlink = helpers.join("UnexpectedHardlink");
    fs::hard_link(fixture.target_manager_validation(), hardlink).expect("add hardlink");
    let target = ProductRelease::new("0.1.0", 35).expect("target release");
    assert_eq!(
        adapter.validate_candidate(&target, 9),
        UpgradeCandidateValidationReport::manager_failed()
    );
    assert!(state.borrow().calls.is_empty());
}

#[cfg(feature = "qualification-harness")]
#[test]
fn qualification_requires_private_temp_root_and_exact_marker() {
    let fixture = Fixture::new();
    set_directory_mode(&fixture.root, 0o700);
    set_directory_mode(&fixture.source, 0o700);
    set_directory_mode(&fixture.target, 0o700);
    let home = fixture.root.join("synthetic-home");
    fs::create_dir(&home).expect("create synthetic home");
    set_directory_mode(&home, 0o700);
    let marker = fixture.root.join("radishlex-upgrade-qualification.marker");
    fs::write(&marker, b"radishlex-upgrade-qualification-v1\n").expect("write marker");
    let mut marker_permissions = fs::metadata(&marker)
        .expect("marker metadata")
        .permissions();
    marker_permissions.set_mode(0o600);
    fs::set_permissions(&marker, marker_permissions).expect("set marker mode");

    MacOsUpgradeCoordinatorAdapter::load_for_qualification(
        &fixture.source,
        &fixture.target,
        &fixture.root,
        &home,
    )
    .expect("load qualification adapter");

    fs::write(&marker, b"wrong-marker\n").expect("replace marker");
    assert_eq!(
        MacOsUpgradeCoordinatorAdapter::load_for_qualification(
            &fixture.source,
            &fixture.target,
            &fixture.root,
            &home,
        )
        .expect_err("wrong marker must fail"),
        MacOsUpgradeAdapterError::UnsafeQualification
    );

    fs::write(&marker, b"radishlex-upgrade-qualification-v1\n").expect("restore marker");
    let linked_home = fixture.root.join("linked-home");
    symlink(&home, &linked_home).expect("link synthetic home");
    assert_eq!(
        MacOsUpgradeCoordinatorAdapter::load_for_qualification(
            &fixture.source,
            &fixture.target,
            &fixture.root,
            &linked_home,
        )
        .expect_err("symlinked home must fail"),
        MacOsUpgradeAdapterError::UnsafeQualification
    );
}

fn write_product(root: &Path, version: &str, build: &str, schema: u32, include_preflight: bool) {
    let manager = root.join("Components/radishlex_manager.app");
    let input_method = root.join("Components/RadishLexInputMethod.app");
    let manager_validation = manager.join("Contents/Helpers/RadishLexUpgradeValidationHost");
    let input_validation = input_method.join("Contents/Helpers/RadishLexUpgradeValidationHost");
    let preflight = manager.join("Contents/Helpers/RadishLexUpgradePreflightHost");
    fs::create_dir_all(manager_validation.parent().expect("manager helpers"))
        .expect("create manager helpers");
    fs::create_dir_all(input_validation.parent().expect("input helpers"))
        .expect("create input helpers");
    fs::write(&manager_validation, format!("manager-validation-{version}"))
        .expect("write manager validation");
    fs::write(&input_validation, format!("input-validation-{version}"))
        .expect("write input validation");
    set_executable(&manager_validation);
    set_executable(&input_validation);
    if include_preflight {
        fs::write(&preflight, format!("preflight-{version}")).expect("write preflight");
        set_executable(&preflight);
    }
    let mut manager_files = vec![file_record(
        &manager_validation,
        "Contents/Helpers/RadishLexUpgradeValidationHost",
    )];
    if include_preflight {
        manager_files.push(file_record(
            &preflight,
            "Contents/Helpers/RadishLexUpgradePreflightHost",
        ));
    }
    let manifest = json!({
        "format_version": 3,
        "product_id": "radishlex-macos",
        "product_version": version,
        "build_number": build,
        "minimum_macos": "13.0",
        "ffi_abi_version": 9,
        "userdb_schema_version": schema,
        "rime_data_manifest_version": 2,
        "native_libraries_manifest_version": 1,
        "data_layout": "application-support-v1",
        "distribution_identity": "community-adhoc-v1",
        "rime_schema_id": "radishlex_pinyin",
        "components": [
            {
                "component": "manager",
                "bundle_id": "dev.radishlex.radishlexManager",
                "product_version": version,
                "build_number": build,
                "minimum_macos": "13.0",
                "files": manager_files,
            },
            {
                "component": "input_method",
                "bundle_id": "org.radishlex.inputmethod.macos",
                "product_version": version,
                "build_number": build,
                "minimum_macos": "13.0",
                "files": [
                    file_record(
                        &input_validation,
                        "Contents/Helpers/RadishLexUpgradeValidationHost",
                    )
                ],
            }
        ],
        "licenses": [{
            "path": "LICENSE",
            "size": 1,
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        }],
    });
    fs::write(
        root.join("ProductManifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize product manifest"),
    )
    .expect("write product manifest");
}

fn file_record(path: &Path, relative: &str) -> serde_json::Value {
    let bytes = fs::read(path).expect("read host");
    json!({
        "path": relative,
        "type": "file",
        "size": bytes.len(),
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
    })
}

fn set_executable(path: &Path) {
    let mut permissions = fs::metadata(path).expect("host metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable");
}

#[cfg(feature = "qualification-harness")]
fn set_directory_mode(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path)
        .expect("directory metadata")
        .permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set directory mode");
}

fn ready_preflight(available_bytes: u64) -> HostOutput {
    HostOutput {
        success: true,
        stdout: format!(
            "{{\"format\":\"radishlex-upgrade-preflight-v1\",\"result\":\"ready\",\"available_bytes\":{available_bytes},\"quiescent\":true}}\n"
        )
        .into_bytes(),
        stderr: Vec::new(),
    }
}

fn succeeded() -> HostOutput {
    HostOutput {
        success: true,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn failed() -> HostOutput {
    HostOutput {
        success: false,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}
