use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::UserDb;

use crate::{ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct ValidationFixture {
    container: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl ValidationFixture {
    fn new() -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-upgrade-validation-test-{}-{}",
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
        let owner_id = fs::metadata(&data_root).expect("root metadata").uid();
        let root = VerifiedDataRoot::verify(&data_root, owner_id).expect("verified root");
        let store = UpgradeReceiptStore::open(root).expect("store");
        let source_path = data_root.join("userdb.sqlite3");
        drop(UserDb::open(&source_path).expect("source database"));
        UserDb::migrate_and_validate(&source_path).expect("standalone source");
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
            "aabbccddeeff00112233445566778899",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![store.data_root_identity().clone(), source_identity],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard");
        store.persist(&guard, &receipt).expect("preflight receipt");
        receipt.advance(UpgradeState::Quiesced).expect("quiesced");
        store.persist(&guard, &receipt).expect("quiesced receipt");
        store
            .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
            .expect("snapshot");
        store
            .create_userdb_candidate(&guard, &mut receipt)
            .expect("candidate");
        drop(guard);
        Self {
            container,
            store,
            receipt,
        }
    }

    fn manager(&self) -> UpgradeManagerValidationEvidence {
        UpgradeManagerValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            self.receipt.target_schema_version(),
            1,
            1,
        )
    }

    fn input_method(&self) -> UpgradeInputMethodValidationEvidence {
        UpgradeInputMethodValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            self.receipt.target_schema_version(),
            1,
            1,
        )
    }

    fn advance_to_switched(&mut self) {
        let guard = self.store.acquire_guard().expect("guard");
        let manager = self.manager();
        let input_method = self.input_method();
        self.store
            .record_candidate_validation(
                &guard,
                &mut self.receipt,
                UpgradeCandidateValidationReport::passed(manager, input_method),
            )
            .expect("candidate validation");
        self.store
            .switch_userdb_candidate(&guard, &mut self.receipt)
            .expect("switch");
    }
}

impl Drop for ValidationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

#[test]
fn exact_dual_product_evidence_persists_candidate_verified() {
    let mut fixture = ValidationFixture::new();
    let before =
        fs::read(fixture.store.state_directory.join(CANDIDATE_FILE_NAME)).expect("candidate bytes");
    let guard = fixture.store.acquire_guard().expect("guard");
    let manager = fixture.manager();
    let input_method = fixture.input_method();
    let summary = fixture
        .store
        .record_candidate_validation(
            &guard,
            &mut fixture.receipt,
            UpgradeCandidateValidationReport::passed(manager, input_method),
        )
        .expect("validation persists");
    assert_eq!(
        summary.disposition(),
        UpgradeCandidateValidationDisposition::CandidateVerified
    );
    assert_eq!(summary.failure_code(), None);
    assert_eq!(fixture.receipt.state(), UpgradeState::CandidateVerified);
    assert_eq!(
        fs::read(fixture.store.state_directory.join(CANDIDATE_FILE_NAME))
            .expect("candidate bytes after"),
        before
    );
    drop(guard);
    assert_eq!(
        fixture.store.load().expect("stored receipt"),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn endpoint_failure_and_invalid_evidence_abort_with_stable_precedence() {
    let mut manager_failed = ValidationFixture::new();
    let guard = manager_failed.store.acquire_guard().expect("guard");
    let summary = manager_failed
        .store
        .record_candidate_validation(
            &guard,
            &mut manager_failed.receipt,
            UpgradeCandidateValidationReport::manager_failed(),
        )
        .expect("manager failure persists");
    assert_eq!(
        summary.failure_code(),
        Some(UpgradeFailureCode::ManagerValidationFailed)
    );
    assert_eq!(
        manager_failed.receipt.state(),
        UpgradeState::AbortedPreserved
    );

    let mut input_failed = ValidationFixture::new();
    let guard = input_failed.store.acquire_guard().expect("guard");
    let manager = input_failed.manager();
    let summary = input_failed
        .store
        .record_candidate_validation(
            &guard,
            &mut input_failed.receipt,
            UpgradeCandidateValidationReport::input_method_failed(manager),
        )
        .expect("input failure persists");
    assert_eq!(
        summary.failure_code(),
        Some(UpgradeFailureCode::InputMethodValidationFailed)
    );

    let mut invalid = ValidationFixture::new();
    let guard = invalid.store.acquire_guard().expect("guard");
    let wrong_manager = UpgradeManagerValidationEvidence::new(
        UPGRADE_VALIDATION_EVIDENCE_VERSION + 1,
        invalid.receipt.target_schema_version(),
        1,
        1,
    );
    let input_method = invalid.input_method();
    let summary = invalid
        .store
        .record_candidate_validation(
            &guard,
            &mut invalid.receipt,
            UpgradeCandidateValidationReport::passed(wrong_manager, input_method),
        )
        .expect("invalid manager evidence aborts");
    assert_eq!(
        summary.failure_code(),
        Some(UpgradeFailureCode::ManagerValidationFailed)
    );

    let mut invalid_input = ValidationFixture::new();
    let guard = invalid_input.store.acquire_guard().expect("guard");
    let manager = invalid_input.manager();
    let incomplete_input = UpgradeInputMethodValidationEvidence::new(
        UPGRADE_VALIDATION_EVIDENCE_VERSION,
        invalid_input.receipt.target_schema_version(),
        1,
        0,
    );
    let summary = invalid_input
        .store
        .record_candidate_validation(
            &guard,
            &mut invalid_input.receipt,
            UpgradeCandidateValidationReport::passed(manager, incomplete_input),
        )
        .expect("incomplete input evidence aborts");
    assert_eq!(
        summary.failure_code(),
        Some(UpgradeFailureCode::InputMethodValidationFailed)
    );
}

#[test]
fn corrupt_candidate_is_rechecked_and_cannot_be_verified() {
    let mut fixture = ValidationFixture::new();
    let candidate_path = fixture.store.state_directory.join(CANDIDATE_FILE_NAME);
    let candidate_len = fs::metadata(&candidate_path)
        .expect("candidate metadata")
        .len();
    fs::write(&candidate_path, vec![b'x'; candidate_len as usize]).expect("corrupt candidate");
    fs::set_permissions(&candidate_path, fs::Permissions::from_mode(0o600))
        .expect("candidate mode");
    let guard = fixture.store.acquire_guard().expect("guard");
    let manager = fixture.manager();
    let input_method = fixture.input_method();
    let summary = fixture
        .store
        .record_candidate_validation(
            &guard,
            &mut fixture.receipt,
            UpgradeCandidateValidationReport::passed(manager, input_method),
        )
        .expect("corrupt candidate aborts");
    assert_eq!(
        summary.failure_code(),
        Some(UpgradeFailureCode::ManagerValidationFailed)
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::AbortedPreserved);
}

#[test]
fn sidecar_or_stale_receipt_keeps_candidate_migrated_fail_closed() {
    let mut fixture = ValidationFixture::new();
    let candidate_path = fixture.store.state_directory.join(CANDIDATE_FILE_NAME);
    let guard = fixture.store.acquire_guard().expect("guard");
    fs::write(
        candidate_path.as_os_str().to_string_lossy().into_owned() + "-wal",
        b"unexpected",
    )
    .expect("sidecar");
    let manager = fixture.manager();
    let input_method = fixture.input_method();
    assert!(fixture
        .store
        .record_candidate_validation(
            &guard,
            &mut fixture.receipt,
            UpgradeCandidateValidationReport::passed(manager, input_method),
        )
        .is_err());
    assert_eq!(fixture.receipt.state(), UpgradeState::CandidateMigrated);

    let stale = ValidationFixture::new();
    let mut stale_receipt = stale.receipt.clone();
    stale_receipt
        .abort_preserved(UpgradeFailureCode::ManagerValidationFailed, false)
        .expect("local stale receipt");
    let guard = stale.store.acquire_guard().expect("guard");
    assert_eq!(
        stale
            .store
            .record_candidate_validation(
                &guard,
                &mut stale_receipt,
                UpgradeCandidateValidationReport::passed(stale.manager(), stale.input_method()),
            )
            .expect_err("stale receipt fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InvalidCandidateValidation
    );
    assert_eq!(stale.receipt.state(), UpgradeState::CandidateMigrated);
}

#[test]
fn post_switch_dual_evidence_persists_then_completes_in_a_separate_step() {
    let mut fixture = ValidationFixture::new();
    fixture.advance_to_switched();
    let guard = fixture.store.acquire_guard().expect("guard");
    let manager = fixture.manager();
    let input_method = fixture.input_method();
    let summary = fixture
        .store
        .record_post_switch_validation(
            &guard,
            &mut fixture.receipt,
            UpgradePostSwitchValidationReport::passed(manager, input_method),
        )
        .expect("post-switch validation");
    assert_eq!(
        summary.disposition(),
        UpgradePostSwitchValidationDisposition::PostSwitchVerified
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::PostSwitchVerified);
    drop(guard);

    let mut loaded = fixture.store.load().expect("load").expect("receipt");
    let guard = fixture.store.acquire_guard().expect("completion guard");
    assert_eq!(
        fixture
            .store
            .complete_post_switch_validation(&guard, &mut loaded)
            .expect("completion"),
        UpgradeCompletionDisposition::Completed
    );
    assert_eq!(loaded.state(), UpgradeState::Completed);
    assert_eq!(
        fixture
            .store
            .complete_post_switch_validation(&guard, &mut loaded)
            .expect("idempotent completion"),
        UpgradeCompletionDisposition::AlreadyCompleted
    );
}

#[test]
fn post_switch_endpoint_failure_or_corruption_requires_rollback() {
    let mut endpoint_failure = ValidationFixture::new();
    endpoint_failure.advance_to_switched();
    let guard = endpoint_failure.store.acquire_guard().expect("guard");
    let summary = endpoint_failure
        .store
        .record_post_switch_validation(
            &guard,
            &mut endpoint_failure.receipt,
            UpgradePostSwitchValidationReport::manager_failed(),
        )
        .expect("failure persists");
    assert_eq!(
        summary.disposition(),
        UpgradePostSwitchValidationDisposition::RollbackRequired
    );
    assert_eq!(
        endpoint_failure.receipt.failure_code(),
        Some(UpgradeFailureCode::PostSwitchValidationFailed)
    );
    assert!(endpoint_failure.receipt.manual_recovery_required());

    let mut corrupt = ValidationFixture::new();
    corrupt.advance_to_switched();
    let active_path = corrupt.store.active_userdb_path();
    let active_len = fs::metadata(&active_path).expect("active metadata").len();
    fs::write(&active_path, vec![b'x'; active_len as usize]).expect("corrupt active");
    fs::set_permissions(&active_path, fs::Permissions::from_mode(0o600)).expect("active mode");
    let guard = corrupt.store.acquire_guard().expect("corrupt guard");
    let manager = corrupt.manager();
    let input_method = corrupt.input_method();
    let summary = corrupt
        .store
        .record_post_switch_validation(
            &guard,
            &mut corrupt.receipt,
            UpgradePostSwitchValidationReport::passed(manager, input_method),
        )
        .expect("core revalidation failure persists rollback");
    assert_eq!(
        summary.disposition(),
        UpgradePostSwitchValidationDisposition::RollbackRequired
    );
}

#[test]
fn completion_recheck_failure_requires_rollback() {
    let mut fixture = ValidationFixture::new();
    fixture.advance_to_switched();
    let guard = fixture.store.acquire_guard().expect("guard");
    let manager = fixture.manager();
    let input_method = fixture.input_method();
    fixture
        .store
        .record_post_switch_validation(
            &guard,
            &mut fixture.receipt,
            UpgradePostSwitchValidationReport::passed(manager, input_method),
        )
        .expect("post-switch validation");
    let active_path = fixture.store.active_userdb_path();
    let active_len = fs::metadata(&active_path).expect("active metadata").len();
    fs::write(&active_path, vec![b'x'; active_len as usize])
        .expect("corrupt final database on the same inode");
    fs::set_permissions(&active_path, fs::Permissions::from_mode(0o600)).expect("active mode");

    assert_eq!(
        fixture
            .store
            .complete_post_switch_validation(&guard, &mut fixture.receipt)
            .expect("completion failure persists rollback"),
        UpgradeCompletionDisposition::RollbackRequired
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RollbackRequired);
    assert_eq!(
        fixture.receipt.failure_after_state(),
        Some(UpgradeState::PostSwitchVerified)
    );
}
