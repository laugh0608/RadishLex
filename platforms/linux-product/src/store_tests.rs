use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use super::*;
use crate::model::{
    ArtifactVersionRelation, DataContractIdentity, LinuxArtifactIdentity, LinuxOperationKind,
    LinuxOperationRequest, PackageSnapshot,
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const OPERATION_ID: &str = "99999999999999999999999999999999";

struct TestSite {
    root: PathBuf,
    state_root: PathBuf,
    artifacts_root: PathBuf,
    store: LinuxInstallStore,
}

impl TestSite {
    fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = Path::new("/tmp").join(format!(
            "rlx-linux-store-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("set test root mode");
        let root = fs::canonicalize(root).expect("canonicalize test root");
        let metadata = fs::metadata(&root).expect("inspect test root");
        let state_root = root.join("install-v1");
        let artifacts_root = root.join("artifacts");
        fs::create_dir(&artifacts_root).expect("create artifact root");
        fs::set_permissions(&artifacts_root, fs::Permissions::from_mode(0o700))
            .expect("set artifact root mode");
        let store = LinuxInstallStore::bootstrap_at(
            &state_root,
            &root.join("install-v1.lock"),
            metadata.uid(),
            metadata.gid(),
        )
        .expect("bootstrap store");
        Self {
            root,
            state_root,
            artifacts_root,
            store,
        }
    }

    fn target_fixture(&self) -> ArtifactFixture {
        let package = b"complete synthetic Debian carrier\n".to_vec();
        let evidence = b"{\"synthetic\":\"evidence\"}\n".to_vec();
        let identity = LinuxArtifactIdentity::new(
            "26.8.1+39-1",
            package.len() as u64,
            evidence.len() as u64,
            sha256_bytes(&package),
            sha256_bytes(&evidence),
            "33".repeat(32),
            DataContractIdentity::new(
                9,
                1,
                "debian-system-v1",
                "xdg-user-v1",
                1,
                1,
                "radishlex_pinyin",
                "44".repeat(32),
            )
            .expect("valid data contract"),
        )
        .expect("valid artifact identity");
        let fixture_root = self.artifacts_root.join("target");
        fs::create_dir(&fixture_root).expect("create fixture root");
        fs::set_permissions(&fixture_root, fs::Permissions::from_mode(0o700))
            .expect("set fixture root mode");
        let package_path = fixture_root.join(identity.package_filename());
        let evidence_path = fixture_root.join(identity.evidence_filename());
        write_file(&package_path, &package, 0o600);
        write_file(&evidence_path, &evidence, 0o600);
        ArtifactFixture {
            identity,
            package,
            evidence,
            package_path,
            evidence_path,
        }
    }

    fn persist_prepared(
        &self,
        guard: &LinuxInstallGuard,
        target: &LinuxArtifactIdentity,
    ) -> LinuxInstallReceipt {
        let request = LinuxOperationRequest::new(
            OPERATION_ID,
            LinuxOperationKind::Install,
            ArtifactVersionRelation::NotApplicable,
            None,
            Some(target.clone()),
        )
        .expect("valid install request");
        let receipt = LinuxInstallReceipt::from_request(
            request,
            vec![OPERATION_ID.to_owned()],
            self.store.root_identity().clone(),
            PackageSnapshot::absent(),
        )
        .expect("valid prepared receipt");
        self.store
            .persist_receipt(guard, &receipt)
            .expect("persist prepared receipt");
        receipt
    }
}

impl Drop for TestSite {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ArtifactFixture {
    identity: LinuxArtifactIdentity,
    package: Vec<u8>,
    evidence: Vec<u8>,
    package_path: PathBuf,
    evidence_path: PathBuf,
}

#[test]
fn partial_and_complete_staged_temporaries_recover_under_the_guard() {
    let site = TestSite::new("stage-recovery");
    let target = site.target_fixture();
    let guard = site.store.acquire_guard().expect("acquire guard");
    site.persist_prepared(&guard, &target.identity);
    let operation_root = site
        .state_root
        .join(OPERATIONS_DIRECTORY)
        .join(OPERATION_ID);
    fs::create_dir(&operation_root).expect("create operation root");
    fs::set_permissions(&operation_root, fs::Permissions::from_mode(0o700))
        .expect("set operation mode");
    write_file(&operation_root.join("target.deb.tmp"), b"partial", 0o000);
    write_file(
        &operation_root.join("target.evidence.json.tmp"),
        &target.evidence,
        0o600,
    );

    site.store
        .stage_artifact(
            &guard,
            ArtifactSlot::Target,
            &target.package_path,
            &target.evidence_path,
        )
        .expect("recover interrupted staged files");

    assert_eq!(
        fs::read(operation_root.join("target.deb")).expect("read staged package"),
        target.package
    );
    assert_eq!(
        fs::read(operation_root.join("target.evidence.json")).expect("read staged evidence"),
        target.evidence
    );
    assert!(!operation_root.join("target.deb.tmp").exists());
    assert!(!operation_root.join("target.evidence.json.tmp").exists());
    site.store
        .finish_staging(&guard)
        .expect("finish staging after recovery");
}

#[test]
fn complete_unrecorded_final_pair_is_reproved_and_adopted() {
    let site = TestSite::new("final-pair-recovery");
    let target = site.target_fixture();
    let guard = site.store.acquire_guard().expect("acquire guard");
    site.persist_prepared(&guard, &target.identity);
    let operation_root = site
        .state_root
        .join(OPERATIONS_DIRECTORY)
        .join(OPERATION_ID);
    ensure_directory(&operation_root, 0o700).expect("create operation root");
    fs::copy(&target.package_path, operation_root.join("target.deb"))
        .expect("copy unrecorded package");
    fs::copy(
        &target.evidence_path,
        operation_root.join("target.evidence.json"),
    )
    .expect("copy unrecorded evidence");

    let receipt = site
        .store
        .stage_artifact(
            &guard,
            ArtifactSlot::Target,
            &target.package_path,
            &target.evidence_path,
        )
        .expect("reprove and adopt complete final pair");
    assert!(receipt.staged_artifact(ArtifactSlot::Target).is_some());
    site.store
        .finish_staging(&guard)
        .expect("finish recovered staging");
}

#[test]
fn single_unrecorded_final_file_is_resumed_after_a_staging_crash() {
    for committed_name in ["target.deb", "target.evidence.json"] {
        let site = TestSite::new(committed_name);
        let target = site.target_fixture();
        let guard = site.store.acquire_guard().expect("acquire guard");
        site.persist_prepared(&guard, &target.identity);
        let operation_root = site
            .state_root
            .join(OPERATIONS_DIRECTORY)
            .join(OPERATION_ID);
        ensure_directory(&operation_root, 0o700).expect("create operation root");
        let source = if committed_name == "target.deb" {
            &target.package_path
        } else {
            &target.evidence_path
        };
        fs::copy(source, operation_root.join(committed_name))
            .expect("commit one staging file before simulated crash");
        sync_directory(&operation_root).expect("sync simulated committed file");
        drop(guard);

        let recovered_guard = site.store.acquire_guard().expect("reacquire after crash");
        let receipt = site
            .store
            .stage_artifact(
                &recovered_guard,
                ArtifactSlot::Target,
                &target.package_path,
                &target.evidence_path,
            )
            .expect("resume the missing half of the staged artifact pair");
        assert!(receipt.staged_artifact(ArtifactSlot::Target).is_some());
        site.store
            .finish_staging(&recovered_guard)
            .expect("finish staging after single-file recovery");
    }
}

#[test]
fn complete_interrupted_receipt_rolls_forward_and_duplicate_is_removed() {
    let site = TestSite::new("receipt-recovery");
    let target = site.target_fixture();
    let guard = site.store.acquire_guard().expect("acquire guard");
    let prepared = site.persist_prepared(&guard, &target.identity);
    let prepared_bytes = prepared.encode().expect("encode prepared receipt");
    site.store
        .stage_artifact(
            &guard,
            ArtifactSlot::Target,
            &target.package_path,
            &target.evidence_path,
        )
        .expect("stage target");
    let advanced = site
        .store
        .load_receipt()
        .expect("load advanced receipt")
        .expect("advanced receipt exists");
    let advanced_bytes = advanced.encode().expect("encode advanced receipt");
    let receipt_path = site.state_root.join(RECEIPT_FILENAME);
    let replacement = site.state_root.join("receipt.old");
    write_file(&replacement, &prepared_bytes, 0o644);
    fs::rename(&replacement, &receipt_path).expect("restore old receipt");
    write_file(
        &site.state_root.join(RECEIPT_TMP_FILENAME),
        &advanced_bytes,
        0o600,
    );
    drop(guard);

    let recovered_guard = site.store.acquire_guard().expect("recover valid receipt");
    assert_eq!(
        site.store
            .load_receipt()
            .expect("load recovered receipt")
            .expect("recovered receipt exists"),
        advanced
    );
    assert!(!site.state_root.join(RECEIPT_TMP_FILENAME).exists());
    drop(recovered_guard);

    write_file(
        &site.state_root.join(RECEIPT_TMP_FILENAME),
        &advanced_bytes,
        0o644,
    );
    let duplicate_guard = site
        .store
        .acquire_guard()
        .expect("remove duplicate receipt");
    assert!(!site.state_root.join(RECEIPT_TMP_FILENAME).exists());
    drop(duplicate_guard);

    write_file(
        &site.state_root.join(RECEIPT_TMP_FILENAME),
        &advanced_bytes,
        0o000,
    );
    let pre_mode_guard = site
        .store
        .acquire_guard()
        .expect("recover receipt created before its final mode was committed");
    assert!(!site.state_root.join(RECEIPT_TMP_FILENAME).exists());
    drop(pre_mode_guard);
}

#[test]
fn incomplete_receipt_is_preserved_and_blocks_recovery() {
    let site = TestSite::new("receipt-incomplete");
    let target = site.target_fixture();
    let guard = site.store.acquire_guard().expect("acquire guard");
    site.persist_prepared(&guard, &target.identity);
    drop(guard);
    let temporary = site.state_root.join(RECEIPT_TMP_FILENAME);
    write_file(&temporary, b"partial", 0o600);

    let error = site
        .store
        .acquire_guard()
        .expect_err("incomplete receipt must fail closed");
    assert_eq!(error.code(), LinuxInstallStoreErrorCode::InterruptedWrite);
    assert_eq!(
        fs::read(&temporary).expect("preserved temporary"),
        b"partial"
    );
    assert!(!site.root.join("install-v1.lock").exists());
}

#[test]
fn fixed_state_parent_can_be_created_before_the_first_receipt() {
    let site = TestSite::new("state-parent");
    let parent_root = site.root.join("var-lib");
    fs::create_dir(&parent_root).expect("create synthetic var-lib");
    fs::set_permissions(&parent_root, fs::Permissions::from_mode(0o755))
        .expect("set synthetic var-lib mode");
    let metadata = fs::metadata(&parent_root).expect("inspect synthetic var-lib");
    let state_parent = parent_root.join("radishlex");

    ensure_state_parent(&state_parent, metadata.uid(), metadata.gid())
        .expect("create fixed product state parent");

    let created = fs::symlink_metadata(&state_parent).expect("inspect created state parent");
    assert!(created.file_type().is_dir());
    assert_eq!(created.mode() & 0o7777, 0o755);
    assert_eq!(created.uid(), metadata.uid());
    assert_eq!(created.gid(), metadata.gid());
}

#[test]
fn shared_lock_parent_accepts_only_exact_sticky_world_writable_mode() {
    let site = TestSite::new("shared-lock-parent");
    let metadata = fs::metadata(&site.root).expect("inspect test root");
    let lock_parent = site.root.join("run-lock");
    fs::create_dir(&lock_parent).expect("create synthetic lock parent");

    for mode in [0o755, 0o775, 0o1777] {
        fs::set_permissions(&lock_parent, fs::Permissions::from_mode(mode))
            .expect("set accepted lock parent mode");
        validate_secure_parent(&lock_parent, metadata.uid(), metadata.gid(), true)
            .expect("accept supported root-owned lock parent mode");
    }

    for mode in [0o777, 0o1703, 0o1733, 0o1757] {
        fs::set_permissions(&lock_parent, fs::Permissions::from_mode(mode))
            .expect("set rejected lock parent mode");
        let error = validate_secure_parent(&lock_parent, metadata.uid(), metadata.gid(), true)
            .expect_err("reject unsafe shared lock parent mode");
        assert_eq!(error.code(), LinuxInstallStoreErrorCode::PermissionDenied);
    }

    fs::set_permissions(&lock_parent, fs::Permissions::from_mode(0o1777))
        .expect("restore Debian lock parent mode");
    let error = validate_secure_parent(&lock_parent, metadata.uid(), metadata.gid(), false)
        .expect_err("private state parents must reject sticky world-writable mode");
    assert_eq!(error.code(), LinuxInstallStoreErrorCode::PermissionDenied);
}

#[test]
fn directory_creation_modes_are_exact_under_restrictive_umask() {
    const CHILD_ENV: &str = "RADISHLEX_STORE_UMASK_CHILD";
    if std::env::var_os(CHILD_ENV).is_some() {
        let site = TestSite::new("restrictive-umask");
        let state = fs::symlink_metadata(&site.state_root).expect("inspect state root");
        let operations = fs::symlink_metadata(site.state_root.join(OPERATIONS_DIRECTORY))
            .expect("inspect operations root");
        assert_eq!(state.mode() & 0o7777, 0o755);
        assert_eq!(operations.mode() & 0o7777, 0o755);
        let parent = fs::symlink_metadata(&site.root).expect("inspect test parent");
        let reopened = LinuxInstallStore::bootstrap_at(
            &site.state_root,
            &site.root.join("install-v1.lock"),
            parent.uid(),
            parent.gid(),
        )
        .expect("reopen exact existing directories");
        assert_eq!(reopened.root_identity(), site.store.root_identity());
        let private = site.root.join("private-operation");
        ensure_directory(&private, 0o700).expect("create exact private directory");
        assert_eq!(
            fs::symlink_metadata(private)
                .expect("inspect private directory")
                .mode()
                & 0o7777,
            0o700
        );
        let target = site.target_fixture();
        let guard = site
            .store
            .acquire_guard()
            .expect("acquire exact-mode guard");
        site.persist_prepared(&guard, &target.identity);
        site.store
            .stage_artifact(
                &guard,
                ArtifactSlot::Target,
                &target.package_path,
                &target.evidence_path,
            )
            .expect("stage exact-mode artifact");
        for path in [
            site.state_root.join(RECEIPT_FILENAME),
            site.state_root
                .join(OPERATIONS_DIRECTORY)
                .join(OPERATION_ID)
                .join("target.deb"),
            site.state_root
                .join(OPERATIONS_DIRECTORY)
                .join(OPERATION_ID)
                .join("target.evidence.json"),
            site.root.join("install-v1.lock"),
        ] {
            let expected = if path.ends_with(RECEIPT_FILENAME) {
                0o644
            } else {
                0o600
            };
            assert_eq!(
                fs::symlink_metadata(path)
                    .expect("inspect exact-mode regular file")
                    .mode()
                    & 0o7777,
                expected
            );
        }
        return;
    }

    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg(
            "umask 0777; exec \"$1\" --exact store::tests::directory_creation_modes_are_exact_under_restrictive_umask --nocapture",
        )
        .arg("radishlex-store-umask")
        .arg(std::env::current_exe().expect("locate current test executable"))
        .env(CHILD_ENV, "1")
        .status()
        .expect("run restrictive-umask child test");
    assert!(status.success(), "restrictive-umask child failed");
}

#[test]
fn durable_directory_entry_order_is_source_bound() {
    let source = include_str!("store.rs");
    let state_create = source
        .find("ensure_directory(state_root, 0o755)")
        .expect("state root creation");
    let state_parent_sync = source[state_create..]
        .find("sync_directory(state_root.parent()")
        .map(|offset| state_create + offset)
        .expect("state parent sync");
    let operations_create = source[state_parent_sync..]
        .find("ensure_directory(&operations, 0o755)")
        .map(|offset| state_parent_sync + offset)
        .expect("operations creation");
    let state_sync = source[operations_create..]
        .find("sync_directory(state_root)")
        .map(|offset| operations_create + offset)
        .expect("state root sync");
    assert!(state_create < state_parent_sync);
    assert!(state_parent_sync < operations_create);
    assert!(operations_create < state_sync);

    let operation_create = source
        .find("ensure_directory(&operation_directory, 0o700)")
        .expect("operation directory creation");
    let operations_sync = source[operation_create..]
        .find("sync_directory(&self.state_root.join(OPERATIONS_DIRECTORY))")
        .map(|offset| operation_create + offset)
        .expect("operations parent sync");
    let first_stage = source[operations_sync..]
        .find("stage_file(")
        .map(|offset| operations_sync + offset)
        .expect("artifact staging");
    assert!(operation_create < operations_sync);
    assert!(operations_sync < first_stage);

    let receipt_temporary = source
        .find("create receipt temporary file")
        .expect("receipt temporary creation");
    let content_sync = source[receipt_temporary..]
        .find("temporary.sync_all()")
        .map(|offset| receipt_temporary + offset)
        .expect("receipt content sync");
    let metadata_sync = source[content_sync..]
        .find("set_regular_file_mode_and_sync(")
        .map(|offset| content_sync + offset)
        .expect("receipt metadata sync");
    let receipt_rename = source[metadata_sync..]
        .find("fs::rename(&temporary_path, &receipt_path)")
        .map(|offset| metadata_sync + offset)
        .expect("receipt rename");
    let receipt_parent_sync = source[receipt_rename..]
        .find("sync_directory(&self.state_root)")
        .map(|offset| receipt_rename + offset)
        .expect("receipt parent sync");
    assert!(receipt_temporary < content_sync);
    assert!(content_sync < metadata_sync);
    assert!(metadata_sync < receipt_rename);
    assert!(receipt_rename < receipt_parent_sync);
}

fn write_file(path: &Path, value: &[u8], mode: u32) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .expect("create test file");
    file.write_all(value).expect("write test file");
    file.sync_all().expect("sync test file");
    file.set_permissions(fs::Permissions::from_mode(mode))
        .expect("set exact test file mode");
}

fn sha256_bytes(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
