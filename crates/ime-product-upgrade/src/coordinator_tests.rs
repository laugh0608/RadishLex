use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::UserDb;

use crate::{ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct CoordinatorFixture {
    container: PathBuf,
    active_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
    source_inode: u64,
}

impl CoordinatorFixture {
    fn new() -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-upgrade-coordinator-test-{}-{}",
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
        drop(UserDb::open(&active_path).expect("source database"));
        UserDb::migrate_and_validate(&active_path).expect("standalone source");
        let source_identity =
            private_file_identity(UpgradeArtifactSlot::SourceDatabase, &active_path);
        let source_inode = source_identity.inode();

        let settings_path = data_root.join("manager-settings.json");
        fs::write(
            &settings_path,
            b"{\"format_version\":1,\"privacy_mode\":false}\n",
        )
        .expect("settings");
        fs::set_permissions(&settings_path, fs::Permissions::from_mode(0o600))
            .expect("settings mode");
        let settings_identity =
            private_file_identity(UpgradeArtifactSlot::SourceSettings, &settings_path);

        let receipt = UpgradeReceipt::new(
            "ddeeff00112233445566778899aabbcc",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![
                store.data_root_identity().clone(),
                source_identity,
                settings_identity,
            ],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard");
        store.persist(&guard, &receipt).expect("persist");
        drop(guard);
        Self {
            container,
            active_path,
            store,
            receipt,
            source_inode,
        }
    }
}

impl Drop for CoordinatorFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateResult {
    Passed,
    ManagerFailed,
}

struct TestPort {
    checkpoints: Vec<UpgradeCoordinatorCheckpoint>,
    reject_checkpoint: Option<UpgradeCoordinatorCheckpoint>,
    candidate_result: CandidateResult,
    post_switch_passed: bool,
    source_validation_available: bool,
}

impl TestPort {
    fn successful() -> Self {
        Self {
            checkpoints: Vec::new(),
            reject_checkpoint: None,
            candidate_result: CandidateResult::Passed,
            post_switch_passed: true,
            source_validation_available: true,
        }
    }

    fn manager(&self, schema_version: i64) -> UpgradeManagerValidationEvidence {
        UpgradeManagerValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            schema_version,
            1,
            1,
        )
    }

    fn input_method(&self, schema_version: i64) -> UpgradeInputMethodValidationEvidence {
        UpgradeInputMethodValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            schema_version,
            1,
            1,
        )
    }
}

impl UpgradeCoordinatorPort for TestPort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        self.checkpoints.push(checkpoint);
        self.reject_checkpoint != Some(checkpoint)
    }

    fn validate_candidate(
        &mut self,
        _target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        match self.candidate_result {
            CandidateResult::Passed => UpgradeCandidateValidationReport::passed(
                self.manager(target_schema_version),
                self.input_method(target_schema_version),
            ),
            CandidateResult::ManagerFailed => UpgradeCandidateValidationReport::manager_failed(),
        }
    }

    fn validate_post_switch(
        &mut self,
        _target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        if self.post_switch_passed {
            UpgradePostSwitchValidationReport::passed(
                self.manager(target_schema_version),
                self.input_method(target_schema_version),
            )
        } else {
            UpgradePostSwitchValidationReport::manager_failed()
        }
    }

    fn validate_restored_source(
        &mut self,
        _source_release: &ProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        self.source_validation_available.then(|| {
            UpgradeRollbackValidationEvidence::new(
                UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
                source_schema_version,
                1,
                1,
            )
        })
    }
}

#[test]
fn guard_bound_driver_completes_all_success_states() {
    let mut fixture = CoordinatorFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    let mut port = TestPort::successful();
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, u64::MAX, &mut port)
        .expect("upgrade completes");

    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::Completed
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::Completed);
    assert_eq!(
        port.checkpoints,
        vec![
            UpgradeCoordinatorCheckpoint::BeforeQuiesced,
            UpgradeCoordinatorCheckpoint::BeforeSettingsBackup,
            UpgradeCoordinatorCheckpoint::BeforeSnapshot,
            UpgradeCoordinatorCheckpoint::BeforeCandidateMigration,
            UpgradeCoordinatorCheckpoint::BeforeCandidateValidation,
            UpgradeCoordinatorCheckpoint::AfterCandidateValidation,
            UpgradeCoordinatorCheckpoint::BeforeSwitch,
            UpgradeCoordinatorCheckpoint::BeforePostSwitchValidation,
            UpgradeCoordinatorCheckpoint::AfterPostSwitchValidation,
            UpgradeCoordinatorCheckpoint::BeforeCompletion,
        ]
    );
}

#[test]
fn candidate_failure_stops_with_original_database_preserved() {
    let mut fixture = CoordinatorFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    let mut port = TestPort::successful();
    port.candidate_result = CandidateResult::ManagerFailed;
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, u64::MAX, &mut port)
        .expect("candidate failure is terminal");

    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::AbortedPreserved
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::AbortedPreserved);
    assert_eq!(
        fs::metadata(&fixture.active_path)
            .expect("active metadata")
            .ino(),
        fixture.source_inode
    );
}

#[test]
fn post_switch_failure_restores_and_validates_the_source() {
    let mut fixture = CoordinatorFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    let mut port = TestPort::successful();
    port.post_switch_passed = false;
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, u64::MAX, &mut port)
        .expect("rollback completes");

    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::RolledBack
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RolledBack);
    assert_eq!(
        fs::metadata(&fixture.active_path)
            .expect("active metadata")
            .ino(),
        fixture.source_inode
    );
}

#[test]
fn quiescence_loss_preserves_the_last_proven_state_and_can_resume() {
    let mut fixture = CoordinatorFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    let mut interrupted = TestPort::successful();
    interrupted.reject_checkpoint = Some(UpgradeCoordinatorCheckpoint::AfterCandidateValidation);
    assert_eq!(
        fixture
            .store
            .resume_userdb_upgrade(&guard, &mut fixture.receipt, u64::MAX, &mut interrupted,)
            .expect_err("quiescence loss stops"),
        UpgradeCoordinatorError::QuiescenceNotProven(
            UpgradeCoordinatorCheckpoint::AfterCandidateValidation
        )
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::CandidateMigrated);
    drop(guard);

    let mut loaded = fixture.store.load().expect("load").expect("receipt");
    let guard = fixture.store.acquire_guard().expect("recovery guard");
    let mut recovered = TestPort::successful();
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut loaded, u64::MAX, &mut recovered)
        .expect("recovery completes");
    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::Completed
    );
}

#[test]
fn unavailable_source_validation_keeps_restored_receipt_recoverable() {
    let mut fixture = CoordinatorFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard");
    let mut unavailable = TestPort::successful();
    unavailable.post_switch_passed = false;
    unavailable.source_validation_available = false;
    assert_eq!(
        fixture
            .store
            .resume_userdb_upgrade(&guard, &mut fixture.receipt, u64::MAX, &mut unavailable,)
            .expect_err("source validation remains required"),
        UpgradeCoordinatorError::SourceValidationNotProven
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RollbackRequired);
    assert_eq!(
        fs::metadata(&fixture.active_path)
            .expect("active metadata")
            .ino(),
        fixture.source_inode
    );
    drop(guard);

    let mut loaded = fixture.store.load().expect("load").expect("receipt");
    let guard = fixture.store.acquire_guard().expect("recovery guard");
    let mut recovered = TestPort::successful();
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut loaded, u64::MAX, &mut recovered)
        .expect("source validation recovery completes");
    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::RolledBack
    );
}

fn private_file_identity(
    slot: UpgradeArtifactSlot,
    path: &std::path::Path,
) -> UpgradeArtifactIdentity {
    let metadata = fs::metadata(path).expect("private file metadata");
    UpgradeArtifactIdentity::private_file(
        slot,
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.len(),
    )
    .expect("private file identity")
}
