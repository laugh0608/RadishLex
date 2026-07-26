use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{ProductRelease, UpgradeState};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TestDataRoot {
    container: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
}

impl TestDataRoot {
    fn new() -> Self {
        let temp_root = fs::canonicalize(std::env::temp_dir()).expect("temp root canonicalizes");
        let container = temp_root.join(format!(
            "radishlex-upgrade-fs-test-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&container)
            .expect("test container is created");
        let data_root = container.join("RadishLex");
        DirBuilder::new()
            .mode(0o700)
            .create(&data_root)
            .expect("data root is created");
        let owner_id = fs::metadata(&data_root).expect("data root metadata").uid();
        Self {
            container,
            data_root,
            owner_id,
        }
    }

    fn verified(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.data_root, self.owner_id).expect("data root verifies")
    }
}

impl Drop for TestDataRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

fn receipt(root: &VerifiedDataRoot) -> UpgradeReceipt {
    UpgradeReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        ProductRelease::new("0.0.9", 34).expect("source release"),
        ProductRelease::new("0.1.0", 35).expect("target release"),
        Some(8),
        9,
        vec![
            root.identity().clone(),
            UpgradeArtifactIdentity::private_file(
                UpgradeArtifactSlot::SourceDatabase,
                10,
                21,
                root.expected_owner_id,
                100,
            )
            .expect("source database identity"),
        ],
    )
    .expect("receipt")
}

#[test]
fn data_root_rejects_non_private_mode_and_symlink() {
    let fixture = TestDataRoot::new();
    fs::set_permissions(&fixture.data_root, fs::Permissions::from_mode(0o755))
        .expect("mode changes");
    assert_eq!(
        VerifiedDataRoot::verify(&fixture.data_root, fixture.owner_id)
            .expect_err("public root is rejected")
            .code(),
        UpgradeFilesystemErrorCode::UnsafeDataRoot
    );
    fs::set_permissions(&fixture.data_root, fs::Permissions::from_mode(0o700))
        .expect("mode restores");
    let alias = fixture.container.join("alias");
    symlink(&fixture.data_root, &alias).expect("alias symlink is created");
    assert_eq!(
        VerifiedDataRoot::verify(&alias, fixture.owner_id)
            .expect_err("symlink root is rejected")
            .code(),
        UpgradeFilesystemErrorCode::UnsafeDataRoot
    );
}

#[test]
fn opened_store_rejects_data_root_mode_drift() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    fs::set_permissions(&fixture.data_root, fs::Permissions::from_mode(0o755))
        .expect("data root mode drifts");

    assert_eq!(
        store
            .load()
            .expect_err("data root drift is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
    fs::set_permissions(&fixture.data_root, fs::Permissions::from_mode(0o700))
        .expect("data root mode restores");
}

#[test]
fn receipt_persists_atomically_and_only_advances_one_state() {
    let fixture = TestDataRoot::new();
    let root = fixture.verified();
    let store = UpgradeReceiptStore::open(root).expect("store opens");
    let mut receipt = receipt(&store.root);
    let guard = store.acquire_guard().expect("guard is acquired");

    store.persist(&guard, &receipt).expect("receipt persists");
    receipt
        .advance(UpgradeState::Quiesced)
        .expect("receipt advances");
    store
        .persist(&guard, &receipt)
        .expect("receipt update persists");
    let mode = fs::metadata(store.receipt_path())
        .expect("receipt metadata")
        .permissions()
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o600);
    drop(guard);

    assert_eq!(store.load().expect("receipt loads"), Some(receipt));
}

#[test]
fn receipt_rejects_new_operation_over_active_operation() {
    let fixture = TestDataRoot::new();
    let root = fixture.verified();
    let store = UpgradeReceiptStore::open(root).expect("store opens");
    let first = receipt(&store.root);
    let guard = store.acquire_guard().expect("guard is acquired");
    store
        .persist(&guard, &first)
        .expect("first receipt persists");
    let second = UpgradeReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        None,
        first.source_release().clone(),
        first.target_release().clone(),
        first.source_schema_version(),
        first.target_schema_version(),
        first.artifacts().to_vec(),
    )
    .expect("second receipt");

    assert_eq!(
        store
            .persist(&guard, &second)
            .expect_err("replacement is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InvalidReceiptReplacement
    );
}

#[test]
fn receipt_cannot_skip_a_persisted_evidence_state() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    let guard = store.acquire_guard().expect("guard is acquired");
    let mut current = receipt(&store.root);
    store
        .persist(&guard, &current)
        .expect("preflight receipt persists");
    let persisted = current.clone();
    current
        .advance(UpgradeState::Quiesced)
        .expect("quiesced state is prepared");
    current
        .record_artifact(
            UpgradeArtifactIdentity::private_file(
                UpgradeArtifactSlot::SnapshotDatabase,
                10,
                22,
                store.root.expected_owner_id,
                100,
            )
            .expect("snapshot identity"),
        )
        .expect("snapshot identity is recorded");
    current
        .advance(UpgradeState::SnapshotReady)
        .expect("snapshot state is prepared");

    assert_eq!(
        store
            .persist(&guard, &current)
            .expect_err("unpersisted quiesced evidence cannot be skipped")
            .code(),
        UpgradeFilesystemErrorCode::InvalidReceiptReplacement
    );
    drop(guard);
    assert_eq!(store.load().expect("old receipt loads"), Some(persisted));
}

#[test]
fn staged_receipt_and_symlinked_receipt_fail_closed() {
    let fixture = TestDataRoot::new();
    let root = fixture.verified();
    let store = UpgradeReceiptStore::open(root).expect("store opens");
    fs::write(store.staged_receipt_path(), b"interrupted\n").expect("staged residue is written");
    fs::set_permissions(
        store.staged_receipt_path(),
        fs::Permissions::from_mode(0o600),
    )
    .expect("staged residue mode");
    assert_eq!(
        store
            .load()
            .expect_err("interrupted write fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedReceiptWrite
    );
    fs::remove_file(store.staged_receipt_path()).expect("staged residue is removed");

    let target = fixture.container.join("receipt-target");
    fs::write(
        &target,
        receipt(&store.root).encode().expect("receipt encodes"),
    )
    .expect("target receipt is written");
    symlink(target, store.receipt_path()).expect("receipt symlink is created");
    assert_eq!(
        store
            .load()
            .expect_err("symlink receipt fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InvalidReceipt
    );
}

#[test]
fn hardlinked_receipt_fails_closed() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    let guard = store.acquire_guard().expect("guard is acquired");
    store
        .persist(&guard, &receipt(&store.root))
        .expect("receipt persists");
    drop(guard);
    let alias = fixture.container.join("receipt-hardlink");
    fs::hard_link(store.receipt_path(), &alias).expect("receipt hardlink is created");

    assert_eq!(
        store
            .load()
            .expect_err("hardlinked receipt is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InvalidReceipt
    );
    fs::remove_file(alias).expect("receipt hardlink is removed");
}

#[test]
fn state_directory_drift_and_unknown_objects_fail_closed() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    let unknown = store.state_directory.join("unexpected-object");
    fs::write(&unknown, b"synthetic\n").expect("unknown object is written");
    assert_eq!(
        store
            .load()
            .expect_err("unknown state object is rejected")
            .code(),
        UpgradeFilesystemErrorCode::UnexpectedStateObject
    );
    fs::remove_file(unknown).expect("unknown object is removed");
    fs::set_permissions(&store.state_directory, fs::Permissions::from_mode(0o755))
        .expect("state directory mode drifts");
    assert_eq!(
        store
            .load()
            .expect_err("state directory drift is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
}

#[test]
fn process_guard_rejects_overlap_and_recovers_exact_stale_socket() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    let guard = store.acquire_guard().expect("first guard is acquired");
    assert_eq!(
        store
            .acquire_guard()
            .expect_err("overlap is rejected")
            .code(),
        UpgradeFilesystemErrorCode::OperationAlreadyActive
    );
    drop(guard);

    let stale = UnixListener::bind(store.guard_path()).expect("stale socket binds");
    fs::set_permissions(store.guard_path(), fs::Permissions::from_mode(0o600))
        .expect("stale socket mode");
    drop(stale);
    let recovered = store
        .acquire_guard()
        .expect("exact stale socket is recovered");
    assert!(store.guard_path().exists());
    drop(recovered);
    assert!(!store.guard_path().exists());
}

#[test]
fn process_guard_refuses_non_socket_without_deleting_it() {
    let fixture = TestDataRoot::new();
    let store = UpgradeReceiptStore::open(fixture.verified()).expect("store opens");
    let target = fixture.container.join("guard-target");
    fs::write(&target, b"synthetic\n").expect("guard target is written");
    symlink(&target, store.guard_path()).expect("guard symlink is created");

    assert_eq!(
        store
            .acquire_guard()
            .expect_err("non-socket guard is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
    assert!(fs::symlink_metadata(store.guard_path())
        .expect("guard symlink remains")
        .file_type()
        .is_symlink());
    fs::remove_file(store.guard_path()).expect("guard symlink is removed");
}
