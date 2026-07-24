use std::fs::{self, DirBuilder, File};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::UserDb;

use crate::{ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct RollbackFixture {
    container: PathBuf,
    active_path: PathBuf,
    candidate_path: PathBuf,
    backup_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
    source_bytes: Vec<u8>,
    candidate_bytes: Vec<u8>,
}

impl RollbackFixture {
    fn new() -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-upgrade-rollback-test-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new()
            .mode(0o700)
            .create(&container)
            .expect("container");
        let data_root = container.join("RadishLex");
        DirBuilder::new()
            .mode(0o700)
            .create(&data_root)
            .expect("data root");
        let owner_id = fs::metadata(&data_root).expect("owner").uid();
        let root = VerifiedDataRoot::verify(&data_root, owner_id).expect("root");
        let store = UpgradeReceiptStore::open(root).expect("store");
        let active_path = data_root.join(snapshot::USERDB_FILE_NAME);
        File::create(&active_path).expect("schema-zero source");
        fs::set_permissions(&active_path, fs::Permissions::from_mode(0o600)).expect("source mode");
        let source_inspection = UserDb::inspect_file(&active_path).expect("source inspection");
        let source_metadata = fs::metadata(&active_path).expect("source metadata");
        let source_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            source_metadata.dev(),
            source_metadata.ino(),
            source_metadata.uid(),
            source_metadata.len(),
        )
        .expect("source identity");
        let mut receipt = UpgradeReceipt::new(
            "ccddaabb00112233445566778899eeff",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(source_inspection.schema_version),
            UserDb::supported_schema_version(),
            vec![store.data_root_identity().clone(), source_identity],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard");
        store.persist(&guard, &receipt).expect("preflight");
        receipt.advance(UpgradeState::Quiesced).expect("quiesced");
        store.persist(&guard, &receipt).expect("quiesced receipt");
        store
            .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
            .expect("snapshot");
        store
            .create_userdb_candidate(&guard, &mut receipt)
            .expect("candidate");
        let manager = UpgradeManagerValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            receipt.target_schema_version(),
            1,
            1,
        );
        let input_method = UpgradeInputMethodValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            receipt.target_schema_version(),
            1,
            1,
        );
        store
            .record_candidate_validation(
                &guard,
                &mut receipt,
                UpgradeCandidateValidationReport::passed(manager, input_method),
            )
            .expect("candidate validation");
        store
            .switch_userdb_candidate(&guard, &mut receipt)
            .expect("switch");
        store
            .record_post_switch_validation(
                &guard,
                &mut receipt,
                UpgradePostSwitchValidationReport::manager_failed(),
            )
            .expect("rollback required");
        drop(guard);

        let candidate_path = store.candidate_path();
        let backup_path = store.source_backup_path();
        let source_bytes = fs::read(&backup_path).expect("source bytes");
        let candidate_bytes = fs::read(&active_path).expect("candidate bytes");
        assert_ne!(source_bytes, candidate_bytes);
        Self {
            container,
            active_path,
            candidate_path,
            backup_path,
            store,
            receipt,
            source_bytes,
            candidate_bytes,
        }
    }

    fn evidence(&self) -> UpgradeRollbackValidationEvidence {
        UpgradeRollbackValidationEvidence::new(
            UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
            self.receipt.source_schema_version().expect("source schema"),
            1,
            1,
        )
    }

    fn assert_restored_files(&self) {
        assert_eq!(
            fs::read(&self.active_path).expect("active bytes"),
            self.source_bytes
        );
        assert_eq!(
            fs::read(&self.candidate_path).expect("candidate bytes"),
            self.candidate_bytes
        );
        assert!(!self.backup_path.exists());
        switch::ensure_no_database_sidecars(&self.active_path).expect("source standalone");
        switch::ensure_no_database_sidecars(&self.candidate_path).expect("candidate standalone");
    }
}

impl Drop for RollbackFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

struct FailAt(RollbackFaultPoint);

impl RollbackFaultInjector for FailAt {
    fn checkpoint(&self, point: RollbackFaultPoint) -> Result<(), UpgradeFilesystemError> {
        if point == self.0 {
            Err(error(UpgradeFilesystemErrorCode::SwitchFailed))
        } else {
            Ok(())
        }
    }
}

#[test]
fn rollback_restores_exact_inodes_but_waits_for_source_release_evidence() {
    let mut fixture = RollbackFixture::new();
    let source_inode = fs::metadata(&fixture.backup_path)
        .expect("backup metadata")
        .ino();
    let candidate_inode = fs::metadata(&fixture.active_path)
        .expect("active metadata")
        .ino();
    let guard = fixture.store.acquire_guard().expect("guard");
    let summary = fixture
        .store
        .restore_userdb_backup(&guard, &fixture.receipt)
        .expect("restore");
    assert_eq!(
        summary.disposition(),
        UpgradeRollbackRestoreDisposition::RestoredAwaitingValidation
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RollbackRequired);
    assert_eq!(
        fs::metadata(&fixture.active_path)
            .expect("restored metadata")
            .ino(),
        source_inode
    );
    assert_eq!(
        fs::metadata(&fixture.candidate_path)
            .expect("candidate metadata")
            .ino(),
        candidate_inode
    );
    fixture.assert_restored_files();

    let replay = fixture
        .store
        .restore_userdb_backup(&guard, &fixture.receipt)
        .expect("idempotent restore");
    assert_eq!(
        replay.disposition(),
        UpgradeRollbackRestoreDisposition::AlreadyRestoredAwaitingValidation
    );
    let evidence = fixture.evidence();
    let summary = fixture
        .store
        .record_rollback_validation(&guard, &mut fixture.receipt, evidence)
        .expect("rollback validation");
    assert_eq!(
        summary.disposition(),
        UpgradeRollbackValidationDisposition::RolledBack
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RolledBack);
    fixture.assert_restored_files();
}

#[test]
fn every_rollback_rename_and_sync_boundary_recovers_exactly() {
    let fault_points = [
        RollbackFaultPoint::CandidateReturned,
        RollbackFaultPoint::BeforeCandidateReturnDestinationSync,
        RollbackFaultPoint::CandidateReturnDestinationSynced,
        RollbackFaultPoint::BeforeCandidateReturnSourceSync,
        RollbackFaultPoint::CandidateReturnSourceSynced,
        RollbackFaultPoint::SourceRestored,
        RollbackFaultPoint::BeforeSourceRestoreDestinationSync,
        RollbackFaultPoint::SourceRestoreDestinationSynced,
        RollbackFaultPoint::BeforeSourceRestoreSourceSync,
        RollbackFaultPoint::SourceRestoreSourceSynced,
    ];

    for point in fault_points {
        let fixture = RollbackFixture::new();
        let guard = fixture.store.acquire_guard().expect("guard");
        assert_eq!(
            fixture
                .store
                .restore_userdb_backup_with_faults(&guard, &fixture.receipt, &FailAt(point),)
                .expect_err("fault")
                .code(),
            UpgradeFilesystemErrorCode::SwitchFailed,
            "fault point: {point:?}"
        );
        drop(guard);

        let recovered = fixture.store.load().expect("load").expect("receipt");
        assert_eq!(recovered.state(), UpgradeState::RollbackRequired);
        let guard = fixture.store.acquire_guard().expect("recovery guard");
        fixture
            .store
            .restore_userdb_backup(&guard, &recovered)
            .expect("restore recovery");
        fixture.assert_restored_files();
    }
}

#[test]
fn rollback_validation_rejects_unproven_or_corrupt_restored_source() {
    let mut invalid_evidence = RollbackFixture::new();
    let guard = invalid_evidence.store.acquire_guard().expect("guard");
    invalid_evidence
        .store
        .restore_userdb_backup(&guard, &invalid_evidence.receipt)
        .expect("restore");
    let invalid = UpgradeRollbackValidationEvidence::new(
        UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
        invalid_evidence
            .receipt
            .source_schema_version()
            .expect("schema"),
        0,
        1,
    );
    assert!(invalid_evidence
        .store
        .record_rollback_validation(&guard, &mut invalid_evidence.receipt, invalid)
        .is_err());
    assert_eq!(
        invalid_evidence.receipt.state(),
        UpgradeState::RollbackRequired
    );

    let mut corrupt = RollbackFixture::new();
    let guard = corrupt.store.acquire_guard().expect("corrupt guard");
    corrupt
        .store
        .restore_userdb_backup(&guard, &corrupt.receipt)
        .expect("restore");
    fs::write(&corrupt.active_path, b"corrupt").expect("corrupt source");
    fs::set_permissions(&corrupt.active_path, fs::Permissions::from_mode(0o600))
        .expect("source mode");
    let evidence = corrupt.evidence();
    assert!(corrupt
        .store
        .record_rollback_validation(&guard, &mut corrupt.receipt, evidence)
        .is_err());
    assert_eq!(corrupt.receipt.state(), UpgradeState::RollbackRequired);
}

#[test]
fn rolled_back_receipt_persistence_is_idempotently_recovered() {
    let mut fixture = RollbackFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    fixture
        .store
        .restore_userdb_backup(&guard, &fixture.receipt)
        .expect("restore");
    let evidence = fixture.evidence();
    assert_eq!(
        fixture
            .store
            .record_rollback_validation_with_faults(
                &guard,
                &mut fixture.receipt,
                evidence,
                &FailAt(RollbackFaultPoint::RolledBackReceiptPersisted),
            )
            .expect_err("post-persist fault")
            .code(),
        UpgradeFilesystemErrorCode::SwitchFailed
    );
    drop(guard);

    let mut loaded = fixture.store.load().expect("load").expect("receipt");
    assert_eq!(loaded.state(), UpgradeState::RolledBack);
    let guard = fixture.store.acquire_guard().expect("guard");
    let evidence = fixture.evidence();
    assert_eq!(
        fixture
            .store
            .record_rollback_validation(&guard, &mut loaded, evidence)
            .expect("idempotent validation")
            .disposition(),
        UpgradeRollbackValidationDisposition::AlreadyRolledBack
    );
}
