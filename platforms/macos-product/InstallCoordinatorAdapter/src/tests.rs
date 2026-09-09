use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_install::{
    commit_program_target, finish_source_preservation, finish_target_staging,
    preserve_program_source, record_program_source, record_staged_program, InstallFailureCode,
    InstallFilesystemErrorCode, InstallOperationKind, InstallProgramValidationPort, InstallReceipt,
    InstallReceiptStore, InstallState, ProductArtifactIdentity, ProductRelease,
    ProgramBundleIdentity, ProgramComponent, ProgramSwitchStore, VerifiedInstallRoot,
    VerifiedProgramTarget, INSTALL_PRODUCT_ID,
};
use radishlex_ime_product_upgrade::{
    ProductRelease as UpgradeProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot,
    UpgradeCandidateValidationReport, UpgradeCoordinatorCheckpoint, UpgradeCoordinatorPort,
    UpgradeFilesystemErrorCode, UpgradeInputMethodValidationEvidence,
    UpgradeManagerValidationEvidence, UpgradePostSwitchValidationReport, UpgradeReceipt,
    UpgradeReceiptStore, UpgradeRollbackValidationEvidence, UpgradeState, VerifiedDataRoot,
    UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION, UPGRADE_VALIDATION_EVIDENCE_VERSION,
};
use radishlex_ime_userdb::UserDb;

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const OPERATION_ID: &str = "00112233445566778899aabbccddeeff";

struct CoordinationFixture {
    container: PathBuf,
    data_root: PathBuf,
    install_store: InstallReceiptStore,
    install_receipt: InstallReceipt,
    manager: ProgramSwitchStore,
    input_method: ProgramSwitchStore,
    upgrade_store: UpgradeReceiptStore,
    upgrade_receipt: UpgradeReceipt,
    source_manager_inode: u64,
    source_input_method_inode: u64,
    source_database_inode: u64,
}

impl CoordinationFixture {
    fn new() -> Self {
        Self::with_source_database(|database| {
            drop(UserDb::open(database).expect("source database"));
            UserDb::migrate_and_validate(database).expect("standalone source database");
        })
    }

    fn with_source_database(initialize: impl FnOnce(&Path)) -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-install-data-coordination-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        create_directory(&container, 0o700);
        let owner_id = fs::metadata(&container).expect("owner").uid();
        let applications = container.join("Applications");
        let input_methods = container.join("Library/Input Methods");
        let data_root = container.join("Library/Application Support/RadishLex");
        create_directory(&applications, 0o755);
        create_directory(&input_methods, 0o755);
        create_directory(&data_root, 0o700);

        let install_store = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&data_root, owner_id).expect("install root"),
        )
        .expect("install store");
        let upgrade_store = UpgradeReceiptStore::open(
            VerifiedDataRoot::verify(&data_root, owner_id).expect("upgrade root"),
        )
        .expect("upgrade store");
        let manager = ProgramSwitchStore::open(
            VerifiedProgramTarget::verify(&applications, ProgramComponent::Manager, owner_id)
                .expect("Manager target"),
            OPERATION_ID,
        )
        .expect("Manager store");
        let input_method = ProgramSwitchStore::open(
            VerifiedProgramTarget::verify(&input_methods, ProgramComponent::InputMethod, owner_id)
                .expect("InputMethod target"),
            OPERATION_ID,
        )
        .expect("InputMethod store");

        create_bundle(manager.target_path(), "source-manager");
        create_bundle(input_method.target_path(), "source-input-method");
        let source_manager_inode = fs::metadata(manager.target_path())
            .expect("source Manager")
            .ino();
        let source_input_method_inode = fs::metadata(input_method.target_path())
            .expect("source InputMethod")
            .ino();
        create_bundle(manager.staged_bundle_path(), "target-manager");
        create_bundle(input_method.staged_bundle_path(), "target-input-method");

        let mut install_receipt = InstallReceipt::new(
            OPERATION_ID,
            None,
            InstallOperationKind::Upgrade,
            install_store.root_identity().clone(),
            Some(product("0.0.9", 34, 0)),
            Some(product("0.1.0", 35, 1)),
        )
        .expect("install receipt");
        let install_guard = install_store.acquire_guard().expect("install guard");
        install_store
            .persist(&install_guard, &install_receipt)
            .expect("prepared");
        install_receipt
            .advance(InstallState::Quiesced)
            .expect("quiesced");
        install_store
            .persist(&install_guard, &install_receipt)
            .expect("persist quiesced");
        for store in [&manager, &input_method] {
            record_program_source(&install_store, &install_guard, store, &mut install_receipt)
                .expect("source evidence");
            record_staged_program(&install_store, &install_guard, store, &mut install_receipt)
                .expect("staged evidence");
        }
        finish_target_staging(
            &install_store,
            &install_guard,
            &manager,
            &input_method,
            &mut install_receipt,
        )
        .expect("target staged");
        for store in [&manager, &input_method] {
            preserve_program_source(&install_store, &install_guard, store, &mut install_receipt)
                .expect("source preserved");
        }
        finish_source_preservation(
            &install_store,
            &install_guard,
            &manager,
            &input_method,
            &mut install_receipt,
        )
        .expect("preservation finished");
        for store in [&manager, &input_method] {
            commit_program_target(&install_store, &install_guard, store, &mut install_receipt)
                .expect("target committed");
        }
        drop(install_guard);

        let database = data_root.join("userdb.sqlite3");
        initialize(&database);
        let database_metadata = fs::metadata(&database).expect("database metadata");
        let source_database_inode = database_metadata.ino();
        let source_database = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            database_metadata.dev(),
            database_metadata.ino(),
            database_metadata.uid(),
            database_metadata.len(),
        )
        .expect("database identity");
        let upgrade_receipt = UpgradeReceipt::new(
            OPERATION_ID,
            None,
            UpgradeProductRelease::new("0.0.9", 34).expect("source release"),
            UpgradeProductRelease::new("0.1.0", 35).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![upgrade_store.data_root_identity().clone(), source_database],
        )
        .expect("upgrade receipt");
        let upgrade_guard = upgrade_store.acquire_guard().expect("upgrade guard");
        upgrade_store
            .persist(&upgrade_guard, &upgrade_receipt)
            .expect("persist upgrade receipt");
        drop(upgrade_guard);

        Self {
            container,
            data_root,
            install_store,
            install_receipt,
            manager,
            input_method,
            upgrade_store,
            upgrade_receipt,
            source_manager_inode,
            source_input_method_inode,
            source_database_inode,
        }
    }

    fn resume(
        &mut self,
        upgrade_port: &mut TestUpgradePort,
        program_validation: &mut TestProgramValidation,
    ) -> Result<InstallDataCoordinationSummary, InstallDataCoordinationError> {
        let install_guard = self
            .install_store
            .acquire_guard()
            .expect("install coordination guard");
        let upgrade_guard = self
            .upgrade_store
            .acquire_guard()
            .expect("upgrade coordination guard");
        resume_install_data_coordination(
            &self.install_store,
            &install_guard,
            &mut self.install_receipt,
            &self.manager,
            &self.input_method,
            &self.upgrade_store,
            &upgrade_guard,
            &mut self.upgrade_receipt,
            u64::MAX,
            upgrade_port,
            program_validation,
        )
    }

    fn finalize(
        &mut self,
        program_validation: &mut TestProgramValidation,
    ) -> Result<(), InstallProductFinalizationError> {
        let install_guard = self
            .install_store
            .acquire_guard()
            .expect("install finalization guard");
        let upgrade_guard = self
            .upgrade_store
            .acquire_guard()
            .expect("upgrade finalization guard");
        resume_upgrade_install_finalization(
            &self.install_store,
            &install_guard,
            &mut self.install_receipt,
            &self.manager,
            &self.input_method,
            &self.upgrade_store,
            &upgrade_guard,
            &self.upgrade_receipt,
            program_validation,
        )
    }

    fn assert_source_programs_restored(&self) {
        assert_eq!(
            fs::metadata(self.manager.target_path())
                .expect("restored Manager")
                .ino(),
            self.source_manager_inode
        );
        assert_eq!(
            fs::metadata(self.input_method.target_path())
                .expect("restored InputMethod")
                .ino(),
            self.source_input_method_inode
        );
    }

    fn active_database_inode(&self) -> u64 {
        fs::metadata(self.data_root.join("userdb.sqlite3"))
            .expect("active database")
            .ino()
    }
}

#[path = "wal_residue_tests.rs"]
mod wal_residue_tests;

impl Drop for CoordinationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateDisposition {
    Passed,
    ManagerFailed,
}

struct TestUpgradePort {
    reject_checkpoint: Option<UpgradeCoordinatorCheckpoint>,
    candidate: CandidateDisposition,
    post_switch_passed: bool,
    restored_source_available: bool,
}

impl TestUpgradePort {
    fn successful() -> Self {
        Self {
            reject_checkpoint: None,
            candidate: CandidateDisposition::Passed,
            post_switch_passed: true,
            restored_source_available: true,
        }
    }
}

impl UpgradeCoordinatorPort for TestUpgradePort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        self.reject_checkpoint != Some(checkpoint)
    }

    fn validate_candidate(
        &mut self,
        _target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        match self.candidate {
            CandidateDisposition::Passed => UpgradeCandidateValidationReport::passed(
                manager_evidence(target_schema_version),
                input_method_evidence(target_schema_version),
            ),
            CandidateDisposition::ManagerFailed => {
                UpgradeCandidateValidationReport::manager_failed()
            }
        }
    }

    fn validate_post_switch(
        &mut self,
        _target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        if self.post_switch_passed {
            UpgradePostSwitchValidationReport::passed(
                manager_evidence(target_schema_version),
                input_method_evidence(target_schema_version),
            )
        } else {
            UpgradePostSwitchValidationReport::manager_failed()
        }
    }

    fn validate_restored_source(
        &mut self,
        _source_release: &UpgradeProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        self.restored_source_available.then(|| {
            UpgradeRollbackValidationEvidence::new(
                UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
                source_schema_version,
                1,
                1,
            )
        })
    }
}

struct TestProgramValidation {
    restored_valid: bool,
    installed_valid_through_call: Option<usize>,
    installed_calls: usize,
    restored_calls: usize,
}

impl TestProgramValidation {
    fn successful() -> Self {
        Self {
            restored_valid: true,
            installed_valid_through_call: None,
            installed_calls: 0,
            restored_calls: 0,
        }
    }
}

impl InstallProgramValidationPort for TestProgramValidation {
    fn validate_installed_targets(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> bool {
        self.installed_calls += 1;
        self.installed_valid_through_call
            .is_none_or(|limit| self.installed_calls <= limit)
    }

    fn validate_restored_sources(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> bool {
        self.restored_calls += 1;
        self.restored_valid
    }
}

#[test]
fn completed_data_advances_outer_receipt_to_data_settled() {
    let mut fixture = CoordinationFixture::new();
    let mut upgrade_port = TestUpgradePort::successful();
    let mut program_validation = TestProgramValidation::successful();
    let summary = fixture
        .resume(&mut upgrade_port, &mut program_validation)
        .expect("data coordination completes");

    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::DataSettled
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::DataSettled);
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Completed);
    assert!(program_validation.installed_calls >= 10);
    assert_eq!(program_validation.restored_calls, 0);
    assert_ne!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
}

#[test]
fn completed_data_finalizes_outer_receipt_through_two_persisted_checkpoints() {
    let mut fixture = CoordinationFixture::new();
    fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect("data coordination");
    let mut program_validation = TestProgramValidation::successful();
    fixture
        .finalize(&mut program_validation)
        .expect("product finalization");
    assert_eq!(fixture.install_receipt.state(), InstallState::Completed);
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Completed);
    assert_eq!(program_validation.installed_calls, 2);

    let mut replay_validation = TestProgramValidation::successful();
    fixture
        .finalize(&mut replay_validation)
        .expect("completed replay revalidates final product");
    assert_eq!(replay_validation.installed_calls, 1);
    assert_eq!(fixture.install_receipt.state(), InstallState::Completed);
}

#[test]
fn final_verified_restarts_only_after_reproving_bound_data_and_programs() {
    let mut fixture = CoordinationFixture::new();
    fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect("data coordination");
    let mut interrupted = TestProgramValidation::successful();
    interrupted.installed_valid_through_call = Some(1);
    assert_eq!(
        fixture
            .finalize(&mut interrupted)
            .expect_err("second checkpoint unavailable"),
        InstallProductFinalizationError::InstallFinalization(
            InstallFinalizationError::FinalStateNotProven(
                InstallFinalizationValidationStage::BeforeCompleted
            )
        )
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::FinalVerified);
    fixture.install_receipt = fixture
        .install_store
        .load()
        .expect("load install receipt")
        .expect("install receipt");

    let mut resumed = TestProgramValidation::successful();
    fixture
        .finalize(&mut resumed)
        .expect("restart finalization");
    assert_eq!(resumed.installed_calls, 1);
    assert_eq!(fixture.install_receipt.state(), InstallState::Completed);
}

#[test]
fn finalization_rejects_noncompleted_or_drifted_data_receipt() {
    let mut fixture = CoordinationFixture::new();
    fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect("data coordination");
    let nonterminal_fixture = CoordinationFixture::new();
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    assert_eq!(
        resume_upgrade_install_finalization(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &nonterminal_fixture.upgrade_receipt,
            &mut TestProgramValidation::successful(),
        ),
        Err(InstallProductFinalizationError::InvalidBinding)
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::DataSettled);
}

#[test]
fn candidate_abort_restores_source_programs_and_outer_terminal() {
    let mut fixture = CoordinationFixture::new();
    let mut upgrade_port = TestUpgradePort::successful();
    upgrade_port.candidate = CandidateDisposition::ManagerFailed;
    let mut program_validation = TestProgramValidation::successful();
    let summary = fixture
        .resume(&mut upgrade_port, &mut program_validation)
        .expect("candidate failure rolls back programs");

    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
    assert_eq!(
        fixture.install_receipt.failure_code(),
        Some(InstallFailureCode::DataCoordinationFailed)
    );
    fixture.assert_source_programs_restored();
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
    assert!(program_validation.restored_calls >= 2);
}

#[test]
fn post_switch_failure_rolls_back_data_before_source_programs() {
    let mut fixture = CoordinationFixture::new();
    let mut upgrade_port = TestUpgradePort::successful();
    upgrade_port.post_switch_passed = false;
    let mut program_validation = TestProgramValidation::successful();
    let summary = fixture
        .resume(&mut upgrade_port, &mut program_validation)
        .expect("post-switch failure fully rolls back");

    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::RolledBack);
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
    fixture.assert_source_programs_restored();
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
}

#[test]
fn quiescence_loss_keeps_both_receipts_recoverable() {
    let mut fixture = CoordinationFixture::new();
    let mut interrupted = TestUpgradePort::successful();
    interrupted.reject_checkpoint = Some(UpgradeCoordinatorCheckpoint::AfterCandidateValidation);
    let mut program_validation = TestProgramValidation::successful();
    assert_eq!(
        fixture
            .resume(&mut interrupted, &mut program_validation)
            .expect_err("quiescence loss"),
        InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::QuiescenceNotProven(
            UpgradeCoordinatorCheckpoint::AfterCandidateValidation
        ))
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::CandidateMigrated
    );

    let mut recovered = TestUpgradePort::successful();
    let summary = fixture
        .resume(&mut recovered, &mut program_validation)
        .expect("coordination resumes");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::DataSettled
    );
}

#[test]
fn installed_program_drift_stops_data_and_can_resume_after_revalidation() {
    let mut fixture = CoordinationFixture::new();
    let mut upgrade_port = TestUpgradePort::successful();
    let mut drifted = TestProgramValidation::successful();
    drifted.installed_valid_through_call = Some(2);
    assert_eq!(
        fixture
            .resume(&mut upgrade_port, &mut drifted)
            .expect_err("installed target drift"),
        InstallDataCoordinationError::ProgramValidationNotProven(
            InstallProgramValidationStage::InstalledTarget
        )
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Quiesced);
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );

    let mut recovered_port = TestUpgradePort::successful();
    let mut recovered_validation = TestProgramValidation::successful();
    let summary = fixture
        .resume(&mut recovered_port, &mut recovered_validation)
        .expect("installed target revalidation recovers");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::DataSettled
    );
}

#[test]
fn restored_program_validation_failure_stays_recoverable() {
    let mut fixture = CoordinationFixture::new();
    let mut upgrade_port = TestUpgradePort::successful();
    upgrade_port.candidate = CandidateDisposition::ManagerFailed;
    let mut unavailable = TestProgramValidation::successful();
    unavailable.restored_valid = false;
    assert_eq!(
        fixture
            .resume(&mut upgrade_port, &mut unavailable)
            .expect_err("source program validation unavailable"),
        InstallDataCoordinationError::ProgramValidationNotProven(
            InstallProgramValidationStage::RestoredSource
        )
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::RollbackRequired
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    fixture.assert_source_programs_restored();

    let mut recovered_upgrade_port = TestUpgradePort::successful();
    let mut recovered_validation = TestProgramValidation::successful();
    let summary = fixture
        .resume(&mut recovered_upgrade_port, &mut recovered_validation)
        .expect("source validation recovers");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
}

#[test]
fn operation_and_release_mismatch_fail_before_data_mutation() {
    let mut fixture = CoordinationFixture::new();
    let data_root = fixture
        .upgrade_receipt
        .artifacts()
        .iter()
        .find(|artifact| artifact.slot() == UpgradeArtifactSlot::DataRoot)
        .expect("data root")
        .clone();
    let source_database = fixture
        .upgrade_receipt
        .artifacts()
        .iter()
        .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceDatabase)
        .expect("source database")
        .clone();
    let mut mismatched = UpgradeReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        None,
        UpgradeProductRelease::new("0.0.8", 33).expect("wrong source"),
        UpgradeProductRelease::new("0.1.0", 35).expect("target"),
        Some(UserDb::supported_schema_version()),
        UserDb::supported_schema_version(),
        vec![data_root.clone(), source_database.clone()],
    )
    .expect("mismatched receipt");
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    let mut upgrade_port = TestUpgradePort::successful();
    let mut program_validation = TestProgramValidation::successful();
    assert_eq!(
        resume_install_data_coordination(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &mut mismatched,
            u64::MAX,
            &mut upgrade_port,
            &mut program_validation,
        )
        .expect_err("mismatched binding"),
        InstallDataCoordinationError::InvalidBinding
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::ProgramsCommitted
    );
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Preflighted);
    assert_eq!(program_validation.installed_calls, 0);

    let mut wrong_release = UpgradeReceipt::new(
        OPERATION_ID,
        None,
        UpgradeProductRelease::new("0.0.8", 33).expect("wrong source"),
        UpgradeProductRelease::new("0.1.0", 35).expect("target"),
        Some(UserDb::supported_schema_version()),
        UserDb::supported_schema_version(),
        vec![data_root.clone(), source_database.clone()],
    )
    .expect("wrong release receipt");
    assert_eq!(
        resume_install_data_coordination(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &mut wrong_release,
            u64::MAX,
            &mut upgrade_port,
            &mut program_validation,
        )
        .expect_err("release mismatch"),
        InstallDataCoordinationError::InvalidBinding
    );

    let wrong_root = UpgradeArtifactIdentity::private_directory(
        UpgradeArtifactSlot::DataRoot,
        data_root.device_id(),
        data_root.inode() + 1,
        data_root.owner_id(),
        data_root.link_count(),
    )
    .expect("wrong root identity");
    let mut mismatched_root = UpgradeReceipt::new(
        OPERATION_ID,
        None,
        UpgradeProductRelease::new("0.0.9", 34).expect("source"),
        UpgradeProductRelease::new("0.1.0", 35).expect("target"),
        Some(UserDb::supported_schema_version()),
        UserDb::supported_schema_version(),
        vec![wrong_root, source_database],
    )
    .expect("wrong root receipt");
    assert_eq!(
        resume_install_data_coordination(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &mut mismatched_root,
            u64::MAX,
            &mut upgrade_port,
            &mut program_validation,
        )
        .expect_err("root mismatch"),
        InstallDataCoordinationError::InvalidBinding
    );
}

#[test]
fn unstored_outer_state_is_rejected_before_data_mutation() {
    let mut fixture = CoordinationFixture::new();
    let mut unstored = fixture.install_receipt.clone();
    unstored
        .advance(InstallState::DataCoordinating)
        .expect("syntactic in-memory successor");
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    let mut upgrade_port = TestUpgradePort::successful();
    let mut program_validation = TestProgramValidation::successful();
    assert_eq!(
        resume_install_data_coordination(
            &fixture.install_store,
            &install_guard,
            &mut unstored,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &mut fixture.upgrade_receipt,
            u64::MAX,
            &mut upgrade_port,
            &mut program_validation,
        )
        .expect_err("unstored outer state"),
        InstallDataCoordinationError::InstallFilesystem(
            InstallFilesystemErrorCode::InvalidReceiptReplacement
        )
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::ProgramsCommitted
    );
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Preflighted);
    assert_eq!(program_validation.installed_calls, 0);
}

#[test]
fn unstored_data_state_is_rejected_before_outer_mutation() {
    let mut fixture = CoordinationFixture::new();
    let mut unstored = fixture.upgrade_receipt.clone();
    unstored
        .advance(UpgradeState::Quiesced)
        .expect("syntactic in-memory successor");
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    let mut upgrade_port = TestUpgradePort::successful();
    let mut program_validation = TestProgramValidation::successful();
    assert_eq!(
        resume_install_data_coordination(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &mut unstored,
            u64::MAX,
            &mut upgrade_port,
            &mut program_validation,
        )
        .expect_err("unstored data state"),
        InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::Filesystem(
            UpgradeFilesystemErrorCode::InvalidReceiptReplacement
        ))
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::ProgramsCommitted
    );
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Preflighted);
    assert_eq!(program_validation.installed_calls, 0);
}

fn manager_evidence(schema_version: i64) -> UpgradeManagerValidationEvidence {
    UpgradeManagerValidationEvidence::new(UPGRADE_VALIDATION_EVIDENCE_VERSION, schema_version, 1, 1)
}

fn input_method_evidence(schema_version: i64) -> UpgradeInputMethodValidationEvidence {
    UpgradeInputMethodValidationEvidence::new(
        UPGRADE_VALIDATION_EVIDENCE_VERSION,
        schema_version,
        1,
        1,
    )
}

fn product(version: &str, build: u64, marker: u8) -> ProductArtifactIdentity {
    let hex =
        |offset: u8| char::from_digit(u32::from(marker * 5 + offset), 16).expect("hex marker");
    ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new(version, build).expect("release"),
        hash(hex(0)),
        ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            "org.radishlex.manager",
            hash(hex(1)),
            hash(hex(2)),
        )
        .expect("Manager identity"),
        ProgramBundleIdentity::new(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod",
            hash(hex(3)),
            hash(hex(4)),
        )
        .expect("InputMethod identity"),
    )
    .expect("product identity")
}

fn hash(marker: char) -> String {
    marker.to_string().repeat(64)
}

fn create_directory(path: &Path, mode: u32) {
    DirBuilder::new()
        .recursive(true)
        .mode(mode)
        .create(path)
        .expect("create directory");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("directory mode");
}

fn create_bundle(path: &Path, marker: &str) {
    create_directory(path, 0o755);
    fs::write(path.join("marker.txt"), marker.as_bytes()).expect("bundle marker");
}
