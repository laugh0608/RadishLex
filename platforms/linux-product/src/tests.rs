use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::{
    prepare_operation, resume_operation, ArtifactSlot, ArtifactVersionRelation,
    DataContractIdentity, DpkgPackageState, DpkgPortError, DpkgTransactionPort,
    LinuxArtifactIdentity, LinuxInstallState, LinuxInstallStore, LinuxInstallStoreErrorCode,
    LinuxOperationKind, LinuxOperationRequest, PackageSnapshot, TransactionError,
    TransactionOutcome,
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TestEnvironment {
    root: PathBuf,
    state_root: PathBuf,
    guard_path: PathBuf,
    artifacts_root: PathBuf,
    store: LinuxInstallStore,
}

impl TestEnvironment {
    fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root =
            Path::new("/tmp").join(format!("rlx-lp-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&root).expect("create test root");
        let root = fs::canonicalize(&root).expect("canonicalize test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("set test root mode");
        let metadata = fs::metadata(&root).expect("inspect test root");
        let state_root = root.join("install-v1");
        let guard_path = root.join("install-v1.lock");
        let artifacts_root = root.join("artifacts");
        fs::create_dir(&artifacts_root).expect("create artifacts root");
        fs::set_permissions(&artifacts_root, fs::Permissions::from_mode(0o700))
            .expect("set artifacts root mode");
        let store = LinuxInstallStore::bootstrap_at(
            &state_root,
            &guard_path,
            metadata.uid(),
            metadata.gid(),
        )
        .expect("bootstrap test store");
        Self {
            root,
            state_root,
            guard_path,
            artifacts_root,
            store,
        }
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ArtifactFixture {
    identity: LinuxArtifactIdentity,
    package_path: PathBuf,
    evidence_path: PathBuf,
}

fn artifact(environment: &TestEnvironment, version: &str, marker: &str) -> ArtifactFixture {
    let package = format!("synthetic deb payload: {marker}\n").into_bytes();
    let evidence = format!("{{\"fixture\":\"{marker}\"}}\n").into_bytes();
    let data_contract = DataContractIdentity::new(
        1,
        1,
        "linux-system-v1",
        "xdg-user-v1",
        1,
        1,
        "radishlex_pinyin",
        "11".repeat(32),
    )
    .expect("valid data contract");
    let identity = LinuxArtifactIdentity::new(
        version,
        package.len() as u64,
        evidence.len() as u64,
        sha256_bytes(&package),
        sha256_bytes(&evidence),
        "22".repeat(32),
        data_contract,
    )
    .expect("valid artifact identity");
    let directory = environment.artifacts_root.join(marker);
    fs::create_dir(&directory).expect("create artifact directory");
    let package_path = directory.join(identity.package_filename());
    let evidence_path = directory.join(identity.evidence_filename());
    write_fixture(&package_path, &package);
    write_fixture(&evidence_path, &evidence);
    ArtifactFixture {
        identity,
        package_path,
        evidence_path,
    }
}

fn write_fixture(path: &Path, value: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("create fixture");
    file.write_all(value).expect("write fixture");
    file.sync_all().expect("sync fixture");
}

fn sha256_bytes(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug)]
struct FakeDpkg {
    snapshot: PackageSnapshot,
    quiescent: bool,
    target_valid: bool,
    source_valid: bool,
    fail_apply_once: bool,
    fail_restore_once: bool,
    apply_calls: usize,
    remove_calls: usize,
    restore_calls: usize,
    user_data_touches: usize,
}

impl FakeDpkg {
    fn new(snapshot: PackageSnapshot) -> Self {
        Self {
            snapshot,
            quiescent: true,
            target_valid: true,
            source_valid: true,
            fail_apply_once: false,
            fail_restore_once: false,
            apply_calls: 0,
            remove_calls: 0,
            restore_calls: 0,
            user_data_touches: 0,
        }
    }
}

impl DpkgTransactionPort for FakeDpkg {
    fn validate_operation(
        &mut self,
        _request: &LinuxOperationRequest,
    ) -> Result<(), DpkgPortError> {
        Ok(())
    }

    fn prove_quiescent(&mut self) -> Result<(), DpkgPortError> {
        if self.quiescent {
            Ok(())
        } else {
            Err(DpkgPortError::new(
                "Manager or Fcitx process is still active",
            ))
        }
    }

    fn inspect_package(&mut self) -> Result<PackageSnapshot, DpkgPortError> {
        Ok(self.snapshot.clone())
    }

    fn apply_package(
        &mut self,
        _operation: LinuxOperationKind,
        artifact: &LinuxArtifactIdentity,
        package_path: &Path,
        evidence_path: &Path,
    ) -> Result<(), DpkgPortError> {
        assert!(package_path.is_file());
        assert!(evidence_path.is_file());
        self.apply_calls += 1;
        if self.fail_apply_once {
            self.fail_apply_once = false;
            self.snapshot =
                PackageSnapshot::new(DpkgPackageState::HalfConfigured, Some(artifact.clone()))
                    .expect("valid partial package state");
            return Err(DpkgPortError::new("synthetic dpkg interruption"));
        }
        self.snapshot = PackageSnapshot::exact_installed(artifact.clone());
        self.target_valid = true;
        Ok(())
    }

    fn remove_package(&mut self) -> Result<(), DpkgPortError> {
        self.remove_calls += 1;
        self.snapshot = PackageSnapshot::absent();
        Ok(())
    }

    fn validate_target(
        &mut self,
        operation: LinuxOperationKind,
        _snapshot: &PackageSnapshot,
    ) -> Result<bool, DpkgPortError> {
        Ok(operation == LinuxOperationKind::Remove || self.target_valid)
    }

    fn restore_source(
        &mut self,
        source: Option<&LinuxArtifactIdentity>,
        package_path: Option<&Path>,
        evidence_path: Option<&Path>,
    ) -> Result<(), DpkgPortError> {
        self.restore_calls += 1;
        if self.fail_restore_once {
            self.fail_restore_once = false;
            return Err(DpkgPortError::new("synthetic source restore interruption"));
        }
        match source {
            Some(artifact) => {
                assert!(package_path.is_some_and(Path::is_file));
                assert!(evidence_path.is_some_and(Path::is_file));
                self.snapshot = PackageSnapshot::exact_installed(artifact.clone());
            }
            None => {
                assert!(package_path.is_none());
                assert!(evidence_path.is_none());
                self.snapshot = PackageSnapshot::absent();
            }
        }
        self.source_valid = true;
        Ok(())
    }

    fn validate_source(&mut self, _snapshot: &PackageSnapshot) -> Result<bool, DpkgPortError> {
        Ok(self.source_valid)
    }
}

struct OperationCase<'a> {
    kind: LinuxOperationKind,
    relation: ArtifactVersionRelation,
    source: Option<&'a ArtifactFixture>,
    target: Option<&'a ArtifactFixture>,
}

fn request(case: &OperationCase<'_>, operation_id: &str) -> LinuxOperationRequest {
    LinuxOperationRequest::new(
        operation_id,
        case.kind,
        case.relation,
        case.source.map(|item| item.identity.clone()),
        case.target.map(|item| item.identity.clone()),
    )
    .expect("valid operation request")
}

fn stage_case(
    environment: &TestEnvironment,
    guard: &crate::LinuxInstallGuard,
    case: &OperationCase<'_>,
) {
    if matches!(
        case.kind,
        LinuxOperationKind::Upgrade | LinuxOperationKind::Remove | LinuxOperationKind::Rollback
    ) {
        let source = case.source.expect("source fixture");
        environment
            .store
            .stage_artifact(
                guard,
                ArtifactSlot::Source,
                &source.package_path,
                &source.evidence_path,
            )
            .expect("stage source artifact");
    }
    if case.kind != LinuxOperationKind::Remove {
        let target = case.target.expect("target fixture");
        environment
            .store
            .stage_artifact(
                guard,
                ArtifactSlot::Target,
                &target.package_path,
                &target.evidence_path,
            )
            .expect("stage target artifact");
    }
    environment
        .store
        .finish_staging(guard)
        .expect("finish artifact staging");
}

#[test]
fn five_operations_complete_without_touching_user_data() {
    for (index, kind) in [
        LinuxOperationKind::Install,
        LinuxOperationKind::Upgrade,
        LinuxOperationKind::Repair,
        LinuxOperationKind::Remove,
        LinuxOperationKind::Rollback,
    ]
    .into_iter()
    .enumerate()
    {
        let environment = TestEnvironment::new(&format!("five-{index}"));
        let old = artifact(&environment, "0.1.0-1", "old");
        let new = artifact(&environment, "0.2.0-1", "new");
        let case = match kind {
            LinuxOperationKind::Install => OperationCase {
                kind,
                relation: ArtifactVersionRelation::NotApplicable,
                source: None,
                target: Some(&new),
            },
            LinuxOperationKind::Upgrade => OperationCase {
                kind,
                relation: ArtifactVersionRelation::TargetNewer,
                source: Some(&old),
                target: Some(&new),
            },
            LinuxOperationKind::Repair => OperationCase {
                kind,
                relation: ArtifactVersionRelation::SameRelease,
                source: Some(&new),
                target: Some(&new),
            },
            LinuxOperationKind::Remove => OperationCase {
                kind,
                relation: ArtifactVersionRelation::NotApplicable,
                source: Some(&new),
                target: None,
            },
            LinuxOperationKind::Rollback => OperationCase {
                kind,
                relation: ArtifactVersionRelation::TargetOlder,
                source: Some(&new),
                target: Some(&old),
            },
        };
        let initial = case.source.map_or_else(PackageSnapshot::absent, |source| {
            PackageSnapshot::exact_installed(source.identity.clone())
        });
        let mut port = FakeDpkg::new(initial);
        if kind == LinuxOperationKind::Repair {
            port.target_valid = false;
        }
        let guard = environment.store.acquire_guard().expect("acquire guard");
        prepare_operation(
            &environment.store,
            &guard,
            request(&case, &format!("{index:032x}")),
            &mut port,
        )
        .expect("prepare operation");
        assert!(environment.state_root.join("receipt.json").is_file());
        stage_case(&environment, &guard, &case);
        assert_eq!(
            resume_operation(&environment.store, &guard, &mut port).expect("complete transaction"),
            TransactionOutcome::Completed
        );
        let receipt = environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists");
        assert_eq!(receipt.state(), LinuxInstallState::Completed);
        assert_eq!(port.user_data_touches, 0);
        assert_eq!(
            port.remove_calls,
            usize::from(kind == LinuxOperationKind::Remove)
        );
        assert_eq!(
            port.apply_calls,
            usize::from(kind != LinuxOperationKind::Remove)
        );
    }
}

#[test]
fn terminal_receipt_chain_appends_across_install_and_upgrade() {
    let environment = TestEnvironment::new("receipt-chain");
    let old = artifact(&environment, "0.1.0-1", "old");
    let new = artifact(&environment, "0.2.0-1", "new");
    let install = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&old),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&install, "11111111111111111111111111111111"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &install);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("complete install"),
        TransactionOutcome::Completed
    );

    let upgrade = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    prepare_operation(
        &environment.store,
        &guard,
        request(&upgrade, "22222222222222222222222222222222"),
        &mut port,
    )
    .expect("append upgrade");
    stage_case(&environment, &guard, &upgrade);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("complete upgrade"),
        TransactionOutcome::Completed
    );

    let receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    assert_eq!(
        receipt.operation_chain(),
        &[
            "11111111111111111111111111111111".to_owned(),
            "22222222222222222222222222222222".to_owned(),
        ]
    );
    assert!(environment
        .state_root
        .join("operations/11111111111111111111111111111111")
        .is_dir());
    assert!(environment
        .state_root
        .join("operations/22222222222222222222222222222222")
        .is_dir());
    assert_eq!(port.user_data_touches, 0);
}

#[test]
fn interrupted_mutation_retries_source_restore_and_rolls_back() {
    let environment = TestEnvironment::new("retry-rollback");
    let old = artifact(&environment, "0.1.0-1", "old");
    let new = artifact(&environment, "0.2.0-1", "new");
    let case = OperationCase {
        kind: LinuxOperationKind::Upgrade,
        relation: ArtifactVersionRelation::TargetNewer,
        source: Some(&old),
        target: Some(&new),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::exact_installed(old.identity.clone()));
    port.fail_apply_once = true;
    port.fail_restore_once = true;
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        &mut port,
    )
    .expect("prepare upgrade");
    stage_case(&environment, &guard, &case);

    let error = resume_operation(&environment.store, &guard, &mut port)
        .expect_err("first restore attempt must remain blocked");
    assert!(matches!(error, TransactionError::RecoveryBlocked { .. }));
    assert_eq!(
        environment
            .store
            .load_receipt()
            .expect("load receipt")
            .expect("receipt exists")
            .state(),
        LinuxInstallState::SourceRestoring
    );
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("retry source restore"),
        TransactionOutcome::RolledBack
    );
    assert_eq!(port.apply_calls, 1);
    assert_eq!(port.restore_calls, 2);
    assert_eq!(port.user_data_touches, 0);
}

#[test]
fn retry_after_target_proof_does_not_repeat_package_mutation() {
    let environment = TestEnvironment::new("target-proof-retry");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "abababababababababababababababab"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    environment
        .store
        .finish_staging(&guard)
        .expect("finishing staging is idempotent");

    let mut receipt = environment
        .store
        .load_receipt()
        .expect("load receipt")
        .expect("receipt exists");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("record quiescence");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("enter package mutation");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist package mutation");

    let target_snapshot = PackageSnapshot::exact_installed(target.identity.clone());
    port.snapshot = target_snapshot.clone();
    receipt
        .record_target_proof(target_snapshot)
        .expect("record target proof before simulated crash");
    environment
        .store
        .persist_receipt(&guard, &receipt)
        .expect("persist target proof before simulated crash");

    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port)
            .expect("resume from persisted target proof"),
        TransactionOutcome::Completed
    );
    assert_eq!(port.apply_calls, 0);
    assert_eq!(port.remove_calls, 0);
}

#[test]
fn running_programs_abort_before_package_mutation() {
    let environment = TestEnvironment::new("quiescence");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    port.quiescent = false;
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        &mut port,
    )
    .expect("prepare install");
    stage_case(&environment, &guard, &case);
    assert_eq!(
        resume_operation(&environment.store, &guard, &mut port).expect("abort safely"),
        TransactionOutcome::AbortedPreserved
    );
    assert_eq!(port.apply_calls, 0);
    assert_eq!(port.remove_calls, 0);
}

#[test]
fn unknown_package_state_refuses_to_create_receipt() {
    let environment = TestEnvironment::new("unknown-state");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(
        PackageSnapshot::new(DpkgPackageState::Unknown, None).expect("valid unknown snapshot"),
    );
    let guard = environment.store.acquire_guard().expect("acquire guard");
    let error = prepare_operation(
        &environment.store,
        &guard,
        request(&case, "cccccccccccccccccccccccccccccccc"),
        &mut port,
    )
    .expect_err("unknown dpkg state must fail closed");
    assert_eq!(
        error.failure_code(),
        Some(crate::LinuxFailureCode::PackageStateUnknown)
    );
    assert!(environment
        .store
        .load_receipt()
        .expect("load receipt")
        .is_none());
}

#[test]
fn guard_is_exclusive_and_released_by_exact_inode() {
    let environment = TestEnvironment::new("guard");
    let first = environment
        .store
        .acquire_guard()
        .expect("acquire first guard");
    let error = environment
        .store
        .acquire_guard()
        .expect_err("second guard must be rejected");
    assert_eq!(error.code(), LinuxInstallStoreErrorCode::GuardActive);
    drop(first);
    assert!(!environment.guard_path.exists());
    environment
        .store
        .acquire_guard()
        .expect("guard can be reacquired after release");
}

#[test]
fn staged_artifact_tamper_and_hardlinks_fail_closed() {
    let environment = TestEnvironment::new("artifact-fail-closed");
    let target = artifact(&environment, "0.2.0-1", "target");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&target),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("acquire guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "dddddddddddddddddddddddddddddddd"),
        &mut port,
    )
    .expect("prepare install");
    environment
        .store
        .stage_artifact(
            &guard,
            ArtifactSlot::Target,
            &target.package_path,
            &target.evidence_path,
        )
        .expect("stage target");
    let staged = environment
        .store
        .staged_artifact_paths("dddddddddddddddddddddddddddddddd", ArtifactSlot::Target)
        .expect("resolve staged target")
        .package_path()
        .to_owned();
    OpenOptions::new()
        .append(true)
        .open(&staged)
        .and_then(|mut file| file.write_all(b"tamper"))
        .expect("tamper staged file");
    let error = environment
        .store
        .finish_staging(&guard)
        .expect_err("tampered staged artifact must fail closed");
    assert_eq!(error.code(), LinuxInstallStoreErrorCode::ArtifactInvalid);

    let second_environment = TestEnvironment::new("hardlink");
    let linked = artifact(&second_environment, "0.2.0-1", "linked");
    let alias_directory = second_environment.artifacts_root.join("alias");
    fs::create_dir(&alias_directory).expect("create alias directory");
    let alias = alias_directory.join(linked.identity.package_filename());
    fs::hard_link(&linked.package_path, &alias).expect("create hardlink");
    let second_case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&linked),
    };
    let mut second_port = FakeDpkg::new(PackageSnapshot::absent());
    let second_guard = second_environment
        .store
        .acquire_guard()
        .expect("acquire second guard");
    prepare_operation(
        &second_environment.store,
        &second_guard,
        request(&second_case, "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        &mut second_port,
    )
    .expect("prepare second install");
    let hardlink_error = second_environment
        .store
        .stage_artifact(
            &second_guard,
            ArtifactSlot::Target,
            &linked.package_path,
            &linked.evidence_path,
        )
        .expect_err("hardlinked source must fail closed");
    assert_eq!(
        hardlink_error.code(),
        LinuxInstallStoreErrorCode::ArtifactInvalid
    );
}
