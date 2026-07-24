use std::fs::{self, DirBuilder, File};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::{SelectionEventDraft, UserDb, UserDbSchemaCompatibility};

use crate::{ProductRelease, UpgradeState};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum SourceFixture {
    Current,
    EmptySchemaZero,
}

struct CandidateFixture {
    container: PathBuf,
    source_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl CandidateFixture {
    fn new(source_fixture: SourceFixture) -> Self {
        let temp_root = fs::canonicalize(std::env::temp_dir()).expect("temp root canonicalizes");
        let container = temp_root.join(format!(
            "radishlex-upgrade-candidate-test-{}-{}",
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
        let source_path = data_root.join("userdb.sqlite3");
        match source_fixture {
            SourceFixture::Current => {
                let mut source = UserDb::open(&source_path).expect("source userdb opens");
                source
                    .record_selection(
                        SelectionEventDraft::new(
                            "synthetic-candidate-session",
                            "qianyi",
                            "迁移词",
                            0,
                            1,
                        )
                        .with_reading("qian yi")
                        .with_context_kind("editor"),
                    )
                    .expect("synthetic selection is recorded");
            }
            SourceFixture::EmptySchemaZero => {
                File::create(&source_path).expect("empty schema-zero source is created");
                fs::set_permissions(&source_path, fs::Permissions::from_mode(0o600))
                    .expect("schema-zero source mode is private");
            }
        }
        let source_inspection = UserDb::inspect_file(&source_path).expect("source inspects");
        let source_metadata = fs::metadata(&source_path).expect("source metadata");
        let source_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            source_metadata.dev(),
            source_metadata.ino(),
            source_metadata.uid(),
            source_metadata.len(),
        )
        .expect("source identity");
        let mut receipt = UpgradeReceipt::new(
            "11223344556677889900aabbccddeeff",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(source_inspection.schema_version),
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
        store
            .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
            .expect("snapshot succeeds");
        drop(guard);
        Self {
            container,
            source_path,
            store,
            receipt,
        }
    }
}

impl Drop for CandidateFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

struct FailAt(CandidateFaultPoint);

impl CandidateFaultInjector for FailAt {
    fn checkpoint(&self, point: CandidateFaultPoint) -> Result<(), UpgradeFilesystemError> {
        if point == self.0 {
            Err(error(UpgradeFilesystemErrorCode::CandidateMigrationFailed))
        } else {
            Ok(())
        }
    }
}

#[test]
fn candidate_is_created_from_snapshot_without_mutating_preserved_artifacts() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let source_before = fs::read(&fixture.source_path).expect("source bytes are read");
    let snapshot_before = fs::read(fixture.store.snapshot_path()).expect("snapshot bytes are read");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    let summary = fixture
        .store
        .create_userdb_candidate(&guard, &mut fixture.receipt)
        .expect("candidate migration succeeds");

    assert_eq!(fixture.receipt.state(), UpgradeState::CandidateMigrated);
    assert_eq!(
        summary.candidate_identity().slot(),
        UpgradeArtifactSlot::CandidateDatabase
    );
    assert_eq!(summary.source_schema_version(), 9);
    assert_eq!(summary.target_schema_version(), 9);
    assert!(!summary.migrated());
    assert_eq!(
        fs::read(&fixture.source_path).expect("source bytes are read again"),
        source_before
    );
    assert_eq!(
        fs::read(fixture.store.snapshot_path()).expect("snapshot bytes are read again"),
        snapshot_before
    );
    assert_eq!(
        UserDb::inspect_file(fixture.store.candidate_path())
            .expect("candidate inspects")
            .compatibility,
        UserDbSchemaCompatibility::Current
    );
    ensure_no_sidecars(&fixture.store.candidate_path()).expect("candidate is standalone");
    drop(guard);
    assert_eq!(
        fixture.store.load().expect("stored receipt loads"),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn schema_zero_snapshot_migrates_only_the_candidate() {
    let mut fixture = CandidateFixture::new(SourceFixture::EmptySchemaZero);
    let snapshot_before = fs::read(fixture.store.snapshot_path()).expect("snapshot bytes are read");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    let summary = fixture
        .store
        .create_userdb_candidate(&guard, &mut fixture.receipt)
        .expect("schema-zero candidate migration succeeds");

    assert_eq!(summary.source_schema_version(), 0);
    assert_eq!(summary.target_schema_version(), 9);
    assert!(summary.migrated());
    assert_eq!(
        UserDb::inspect_file(fixture.store.snapshot_path())
            .expect("snapshot remains schema zero")
            .compatibility,
        UserDbSchemaCompatibility::MigrationRequired
    );
    assert_eq!(
        fs::read(fixture.store.snapshot_path()).expect("snapshot bytes are read again"),
        snapshot_before
    );
}

#[test]
fn copied_candidate_fault_preserves_staged_file_and_load_fails_closed() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_candidate_with_faults(
                &guard,
                &mut fixture.receipt,
                &FailAt(CandidateFaultPoint::SnapshotCopied),
            )
            .expect_err("post-copy fault is injected")
            .code(),
        UpgradeFilesystemErrorCode::CandidateMigrationFailed
    );
    assert!(fixture.store.staged_candidate_path().exists());
    assert!(!fixture.store.candidate_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("staged candidate fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedCandidate
    );
}

#[test]
fn rename_before_candidate_evidence_fails_closed_without_guessing() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_userdb_candidate_with_faults(
            &guard,
            &mut fixture.receipt,
            &FailAt(CandidateFaultPoint::CandidateRenamed),
        )
        .expect_err("post-rename fault is injected");
    assert!(!fixture.store.staged_candidate_path().exists());
    assert!(fixture.store.candidate_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("unrecorded candidate fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedCandidate
    );
}

#[test]
fn persisted_candidate_evidence_advances_idempotently() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_userdb_candidate_with_faults(
            &guard,
            &mut fixture.receipt,
            &FailAt(CandidateFaultPoint::ReceiptEvidencePersisted),
        )
        .expect_err("post-evidence fault is injected");
    drop(guard);
    let mut stored = fixture
        .store
        .load()
        .expect("evidence-bearing receipt loads")
        .expect("receipt exists");
    assert_eq!(stored.state(), UpgradeState::SnapshotReady);
    assert!(stored
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::CandidateDatabase));
    let guard = fixture.store.acquire_guard().expect("recovery guard");
    let summary = fixture
        .store
        .create_userdb_candidate(&guard, &mut stored)
        .expect("recorded candidate evidence advances");
    assert_eq!(stored.state(), UpgradeState::CandidateMigrated);
    assert_eq!(
        summary.candidate_identity(),
        stored
            .artifacts()
            .iter()
            .find(|artifact| artifact.slot() == UpgradeArtifactSlot::CandidateDatabase)
            .expect("candidate identity")
    );
}

#[test]
fn corrupt_snapshot_is_rejected_before_candidate_file_creation() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let snapshot_path = fixture.store.snapshot_path();
    let snapshot_len = fs::metadata(&snapshot_path)
        .expect("snapshot metadata")
        .len();
    fs::write(&snapshot_path, vec![b'x'; snapshot_len as usize])
        .expect("snapshot is corrupted in place");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_candidate(&guard, &mut fixture.receipt)
            .expect_err("corrupt snapshot is rejected")
            .code(),
        UpgradeFilesystemErrorCode::CandidateMigrationFailed
    );
    assert!(!fixture.store.staged_candidate_path().exists());
    assert!(!fixture.store.candidate_path().exists());
}

#[test]
fn replaced_snapshot_identity_is_rejected_before_candidate_file_creation() {
    let mut fixture = CandidateFixture::new(SourceFixture::Current);
    let snapshot_path = fixture.store.snapshot_path();
    let preserved_path = fixture.container.join("preserved-snapshot.sqlite3");
    fs::rename(&snapshot_path, &preserved_path).expect("recorded snapshot is preserved");
    fs::copy(&preserved_path, &snapshot_path).expect("replacement snapshot is copied");
    fs::set_permissions(&snapshot_path, fs::Permissions::from_mode(0o600))
        .expect("replacement snapshot mode is private");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_candidate(&guard, &mut fixture.receipt)
            .expect_err("replacement snapshot identity is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
    assert!(!fixture.store.staged_candidate_path().exists());
    assert!(!fixture.store.candidate_path().exists());
}
