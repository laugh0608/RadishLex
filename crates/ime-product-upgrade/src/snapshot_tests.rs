use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::{SelectionEventDraft, UserDb, UserDbSchemaCompatibility};

use crate::{ProductRelease, UpgradeState};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct SnapshotFixture {
    container: PathBuf,
    data_root: PathBuf,
    source_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl SnapshotFixture {
    fn new() -> Self {
        let temp_root = fs::canonicalize(std::env::temp_dir()).expect("temp root canonicalizes");
        let container = temp_root.join(format!(
            "radishlex-upgrade-snapshot-test-{}-{}",
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
        let root = VerifiedDataRoot::verify(&data_root, owner_id).expect("data root verifies");
        let store = UpgradeReceiptStore::open(root).expect("receipt store opens");
        let source_path = data_root.join(USERDB_FILE_NAME);
        {
            let mut source = UserDb::open(&source_path).expect("source userdb opens");
            source
                .record_selection(
                    SelectionEventDraft::new(
                        "synthetic-snapshot-session",
                        "kuaizhao",
                        "快照词",
                        0,
                        1,
                    )
                    .with_reading("kuai zhao")
                    .with_context_kind("editor"),
                )
                .expect("synthetic selection is recorded");
        }
        let source_metadata = fs::metadata(&source_path).expect("source metadata");
        assert_eq!(source_metadata.permissions().mode() & 0o7777, 0o600);
        let source_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            source_metadata.dev(),
            source_metadata.ino(),
            source_metadata.uid(),
            source_metadata.len(),
        )
        .expect("source identity");
        let mut receipt = UpgradeReceipt::new(
            "00112233445566778899aabbccddeeff",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![store.data_root_identity().clone(), source_identity],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard is acquired");
        store.persist(&guard, &receipt).expect("preflight persists");
        receipt
            .advance(UpgradeState::Quiesced)
            .expect("receipt advances to quiesced");
        store.persist(&guard, &receipt).expect("quiesced persists");
        drop(guard);
        Self {
            container,
            data_root,
            source_path,
            store,
            receipt,
        }
    }
}

impl Drop for SnapshotFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

struct FailAt(SnapshotFaultPoint);

impl SnapshotFaultInjector for FailAt {
    fn checkpoint(&self, point: SnapshotFaultPoint) -> Result<(), UpgradeFilesystemError> {
        if point == self.0 {
            Err(error(UpgradeFilesystemErrorCode::SnapshotFailed))
        } else {
            Ok(())
        }
    }
}

#[test]
fn space_budget_is_conservative_and_rejects_shortfall_or_overflow() {
    let logical_bytes = 4 * 1024 * 1024;
    let required =
        logical_bytes * SNAPSHOT_WORKING_COPY_MULTIPLIER + MINIMUM_FREE_SPACE_RESERVE_BYTES;
    let budget = UpgradeSnapshotSpaceBudget::evaluate(logical_bytes, required)
        .expect("exact budget is accepted");
    assert_eq!(budget.logical_snapshot_bytes(), logical_bytes);
    assert_eq!(budget.required_available_bytes(), required);
    assert_eq!(budget.reported_available_bytes(), required);
    assert_eq!(
        UpgradeSnapshotSpaceBudget::evaluate(logical_bytes, required - 1)
            .expect_err("one-byte shortfall is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InsufficientSpace
    );
    assert_eq!(
        UpgradeSnapshotSpaceBudget::evaluate(u64::MAX, u64::MAX)
            .expect_err("overflow is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InsufficientSpace
    );
}

#[test]
fn snapshot_uses_fixed_source_and_persists_evidence_before_state() {
    let mut fixture = SnapshotFixture::new();
    let source_before = fs::read(&fixture.source_path).expect("source bytes are read");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    let summary = fixture
        .store
        .create_userdb_snapshot(&guard, &mut fixture.receipt, u64::MAX)
        .expect("snapshot succeeds");

    assert_eq!(fixture.receipt.state(), UpgradeState::SnapshotReady);
    assert_eq!(
        summary.snapshot_identity().slot(),
        UpgradeArtifactSlot::SnapshotDatabase
    );
    assert_eq!(summary.source_schema_version(), 9);
    assert_eq!(summary.snapshot_schema_version(), 9);
    assert!(summary.page_size_bytes() > 0);
    assert!(summary.page_count() > 0);
    assert_eq!(
        UserDb::inspect_file(fixture.store.snapshot_path())
            .expect("snapshot inspects")
            .compatibility,
        UserDbSchemaCompatibility::Current
    );
    assert_eq!(
        fs::read(&fixture.source_path).expect("source bytes are read again"),
        source_before
    );
    drop(guard);
    assert_eq!(
        fixture.store.load().expect("stored receipt loads"),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn insufficient_space_fails_before_creating_snapshot_files() {
    let mut fixture = SnapshotFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_snapshot(&guard, &mut fixture.receipt, 0)
            .expect_err("space shortfall is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InsufficientSpace
    );
    assert!(!fixture.store.staged_snapshot_path().exists());
    assert!(!fixture.store.snapshot_path().exists());
    assert_eq!(fixture.receipt.state(), UpgradeState::Quiesced);
    drop(guard);
    assert_eq!(
        fixture.store.load().expect("quiesced receipt loads"),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn source_identity_drift_fails_before_snapshot_creation() {
    let mut fixture = SnapshotFixture::new();
    let preserved = fixture.data_root.join("preserved-source.sqlite3");
    fs::rename(&fixture.source_path, &preserved).expect("source is moved aside");
    fs::File::create(&fixture.source_path).expect("replacement source is created");
    fs::set_permissions(&fixture.source_path, fs::Permissions::from_mode(0o600))
        .expect("replacement mode is private");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_snapshot(&guard, &mut fixture.receipt, u64::MAX)
            .expect_err("source identity drift is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
    assert!(!fixture.store.staged_snapshot_path().exists());
    assert!(!fixture.store.snapshot_path().exists());
}

#[test]
fn backup_failure_preserves_staged_file_and_load_fails_closed() {
    let mut fixture = SnapshotFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_snapshot_with_faults(
                &guard,
                &mut fixture.receipt,
                u64::MAX,
                &FailAt(SnapshotFaultPoint::SqliteBackupCompleted),
            )
            .expect_err("post-backup fault is injected")
            .code(),
        UpgradeFilesystemErrorCode::SnapshotFailed
    );
    assert!(fixture.store.staged_snapshot_path().exists());
    assert!(!fixture.store.snapshot_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("staged snapshot fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedSnapshot
    );
}

#[test]
fn rename_before_receipt_evidence_fails_closed_without_guessing() {
    let mut fixture = SnapshotFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_userdb_snapshot_with_faults(
            &guard,
            &mut fixture.receipt,
            u64::MAX,
            &FailAt(SnapshotFaultPoint::SnapshotRenamed),
        )
        .expect_err("post-rename fault is injected");
    assert!(!fixture.store.staged_snapshot_path().exists());
    assert!(fixture.store.snapshot_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("unrecorded snapshot fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedSnapshot
    );
}

#[test]
fn persisted_snapshot_evidence_advances_idempotently() {
    let mut fixture = SnapshotFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_userdb_snapshot_with_faults(
            &guard,
            &mut fixture.receipt,
            u64::MAX,
            &FailAt(SnapshotFaultPoint::ReceiptEvidencePersisted),
        )
        .expect_err("post-evidence fault is injected");
    drop(guard);
    let mut stored = fixture
        .store
        .load()
        .expect("evidence-bearing receipt loads")
        .expect("receipt exists");
    assert_eq!(stored.state(), UpgradeState::Quiesced);
    assert!(stored
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase));
    let guard = fixture.store.acquire_guard().expect("recovery guard");
    let summary = fixture
        .store
        .create_userdb_snapshot(&guard, &mut stored, u64::MAX)
        .expect("recorded snapshot evidence advances");
    assert_eq!(stored.state(), UpgradeState::SnapshotReady);
    assert_eq!(
        summary.snapshot_identity(),
        stored
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase)
            .expect("snapshot identity")
    );
}
