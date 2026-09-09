use std::collections::VecDeque;
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_install::{
    record_program_source, record_staged_program, InstallFinalizationValidationStage,
    ProductRelease, ProgramBundleIdentity, VerifiedInstallRoot, VerifiedProgramTarget,
    INSTALL_PRODUCT_ID,
};
use radishlex_ime_product_upgrade::{
    ProductRelease as UpgradeProductRelease, UpgradeArtifactSlot, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradeInputMethodValidationEvidence,
    UpgradeManagerValidationEvidence, UpgradePostSwitchValidationReport,
    UpgradeRollbackValidationEvidence, UpgradeState, UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
    UPGRADE_VALIDATION_EVIDENCE_VERSION,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_installer_driver::{
    authorize_installer_action, inspect_installer_view, InstallerProductSituation,
    InstallerUserAuthorization,
};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[path = "recovery_tests.rs"]
mod recovery_tests;

struct Fixture {
    root: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
    store: InstallReceiptStore,
    programs: TestPrograms,
    preflight: TestPreflight,
    operation_ids: TestOperationIds,
}

impl Fixture {
    fn new(target_product: ProductArtifactIdentity) -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-installer-executor-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        let data_root = root.join("Library/Application Support/RadishLex");
        let manager_parent = root.join("Applications");
        let input_method_parent = root.join("Library/Input Methods");
        create_directory(&root, 0o700);
        create_directory(&data_root, 0o700);
        create_directory(&manager_parent, 0o755);
        create_directory(&input_method_parent, 0o755);
        let owner_id = fs::metadata(&root).expect("owner metadata").uid();
        let store = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&data_root, owner_id).expect("verified install root"),
        )
        .expect("install receipt store");

        Self {
            root,
            data_root,
            owner_id,
            store,
            programs: TestPrograms {
                target_product,
                manager_parent,
                input_method_parent,
                owner_id,
                fail_input_stage_once: false,
                reject_finalization_completion: false,
                installed_validation_calls: 0,
                recovery_checks: 0,
                reject_recovery_check: None,
            },
            preflight: TestPreflight::successful(),
            operation_ids: TestOperationIds::new(),
        }
    }

    fn authorize(
        &self,
        situation: InstallerProductSituation,
        action: InstallerAction,
    ) -> AuthorizedInstallerIntent {
        let snapshot = inspect_installer_view(&self.data_root, self.owner_id, situation);
        authorize_installer_action(
            snapshot,
            action,
            InstallerUserAuthorization {
                explicit_action_confirmed: true,
                data_retention_acknowledged: true,
                neutral_input_source_selected: true,
                manager_closed: true,
            },
        )
        .expect("authorized Installer action")
    }

    fn execute<U: InstallerUpgradeExecution<TestPrograms>>(
        &mut self,
        intent: AuthorizedInstallerIntent,
        upgrade: &mut U,
    ) -> Result<InstallerExecutionSummary, InstallerExecutionError> {
        execute_authorized_intent(
            intent,
            &self.store,
            &mut self.programs,
            &mut self.preflight,
            &mut self.operation_ids,
            upgrade,
        )
    }

    fn execute_with_upgrade_bootstrap<U: UpgradeCoordinatorPort>(
        &mut self,
        intent: AuthorizedInstallerIntent,
        upgrade: &mut U,
    ) -> Result<InstallerExecutionSummary, InstallerExecutionError> {
        execute_authorized_intent_with_upgrade_bootstrap(
            intent,
            &self.store,
            &self.data_root,
            self.owner_id,
            &mut self.programs,
            &mut self.preflight,
            &mut self.operation_ids,
            upgrade,
        )
    }

    fn begin_and_confirm(
        &mut self,
        situation: InstallerProductSituation,
        begin_action: InstallerAction,
    ) -> InstallerExecutionSummary {
        let begin = self.authorize(situation, begin_action);
        let prepared = self
            .execute(begin, &mut NoUpgradeExecution)
            .expect("prepare operation");
        assert_eq!(
            prepared.disposition(),
            InstallerExecutionDisposition::AwaitingUserAction
        );
        assert_eq!(prepared.state(), InstallState::Prepared);

        let confirm = self.authorize(situation, InstallerAction::ConfirmQuiescence);
        self.execute(confirm, &mut NoUpgradeExecution)
            .expect("confirm and execute operation")
    }

    fn receipt(&self) -> InstallReceipt {
        self.store
            .load()
            .expect("load install receipt")
            .expect("stored install receipt")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct TestPrograms {
    target_product: ProductArtifactIdentity,
    manager_parent: PathBuf,
    input_method_parent: PathBuf,
    owner_id: u32,
    fail_input_stage_once: bool,
    reject_finalization_completion: bool,
    installed_validation_calls: usize,
    recovery_checks: usize,
    reject_recovery_check: Option<usize>,
}

impl TestPrograms {
    fn target_path(&self, component: ProgramComponent) -> PathBuf {
        match component {
            ProgramComponent::Manager => self.manager_parent.join("RadishLex Manager.app"),
            ProgramComponent::InputMethod => {
                self.input_method_parent.join("RadishLexInputMethod.app")
            }
        }
    }
}

impl InstallerProgramPort for TestPrograms {
    fn verify_recovery_material(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        self.recovery_checks += 1;
        if self.reject_recovery_check == Some(self.recovery_checks) {
            return Err(InstallerExecutionError::PreflightNotProven);
        }
        for store in [manager, input_method] {
            if !store.source_backup_bundle_path().is_dir() && !store.target_path().is_dir() {
                return Err(InstallerExecutionError::InvalidState);
            }
        }
        Ok(())
    }

    fn target_product(&self) -> &ProductArtifactIdentity {
        &self.target_product
    }

    fn open_program_store(
        &self,
        _receipt_store: &InstallReceiptStore,
        _guard: &InstallProcessGuard,
        component: ProgramComponent,
        receipt: &InstallReceipt,
    ) -> Result<ProgramSwitchStore, InstallerExecutionError> {
        let parent = match component {
            ProgramComponent::Manager => &self.manager_parent,
            ProgramComponent::InputMethod => &self.input_method_parent,
        };
        let target = VerifiedProgramTarget::verify(parent, component, self.owner_id)
            .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))?;
        ProgramSwitchStore::open(target, receipt.operation_id())
            .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))
    }

    fn verify_and_record_source(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        record_program_source(receipt_store, guard, program_store, receipt)
            .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))
    }

    fn prepare_and_record_staged(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        program_store: &ProgramSwitchStore,
        receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        if self.fail_input_stage_once
            && program_store.component() == ProgramComponent::InputMethod
            && fs::symlink_metadata(program_store.staged_bundle_path()).is_err()
        {
            return Err(InstallerExecutionError::InvalidState);
        }
        if fs::symlink_metadata(program_store.staged_bundle_path()).is_err() {
            create_bundle(
                program_store.staged_bundle_path(),
                match program_store.component() {
                    ProgramComponent::Manager => "target-manager",
                    ProgramComponent::InputMethod => "target-input-method",
                },
            );
        }
        record_staged_program(receipt_store, guard, program_store, receipt)
            .map_err(|error| InstallerExecutionError::ProgramSwitch(error.code()))
    }
}

impl InstallFinalizationPort for TestPrograms {
    fn validate_final_state(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        stage: InstallFinalizationValidationStage,
    ) -> bool {
        self.installed_validation_calls += 1;
        if self.reject_finalization_completion
            && stage == InstallFinalizationValidationStage::BeforeCompleted
        {
            return false;
        }
        let manager_exists = fs::symlink_metadata(manager.target_path()).is_ok();
        let input_method_exists = fs::symlink_metadata(input_method.target_path()).is_ok();
        if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
            !manager_exists && !input_method_exists
        } else {
            manager_exists && input_method_exists
        }
    }
}

impl InstallProgramValidationPort for TestPrograms {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.installed_validation_calls += 1;
        if self.reject_finalization_completion && receipt.state() == InstallState::FinalVerified {
            return false;
        }
        fs::symlink_metadata(manager.target_path()).is_ok()
            && fs::symlink_metadata(input_method.target_path()).is_ok()
    }

    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> bool {
        fs::symlink_metadata(manager.target_path()).is_ok()
            && fs::symlink_metadata(input_method.target_path()).is_ok()
    }
}

struct TestPreflight {
    results: VecDeque<Result<u64, InstallerExecutionError>>,
    calls: usize,
}

impl TestPreflight {
    fn successful() -> Self {
        Self {
            results: VecDeque::new(),
            calls: 0,
        }
    }
}

impl InstallerPreflightPort for TestPreflight {
    fn inspect_preflight(
        &mut self,
        _target_product: &ProductArtifactIdentity,
    ) -> Result<InstallerPreflightEvidence, InstallerExecutionError> {
        self.calls += 1;
        InstallerPreflightEvidence::new(self.results.pop_front().unwrap_or(Ok(u64::MAX))?)
    }
}

struct TestOperationIds {
    ids: VecDeque<String>,
}

impl TestOperationIds {
    fn new() -> Self {
        Self {
            ids: [
                "00112233445566778899aabbccddeeff",
                "11112222333344445555666677778888",
                "9999aaaabbbbccccddddeeeeffff0000",
                "0123456789abcdef0123456789abcdef",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

impl InstallerOperationIdSource for TestOperationIds {
    fn next_operation_id(&mut self) -> Result<String, InstallerExecutionError> {
        self.ids
            .pop_front()
            .ok_or(InstallerExecutionError::OperationIdUnavailable)
    }
}

struct TestUpgradePort;

impl UpgradeCoordinatorPort for TestUpgradePort {
    fn confirm_quiescence(&mut self, _checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        true
    }

    fn validate_candidate(
        &mut self,
        _target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        UpgradeCandidateValidationReport::passed(
            manager_evidence(target_schema_version),
            input_method_evidence(target_schema_version),
        )
    }

    fn validate_post_switch(
        &mut self,
        _target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        UpgradePostSwitchValidationReport::passed(
            manager_evidence(target_schema_version),
            input_method_evidence(target_schema_version),
        )
    }

    fn validate_restored_source(
        &mut self,
        _source_release: &UpgradeProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        Some(UpgradeRollbackValidationEvidence::new(
            UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
            source_schema_version,
            1,
            1,
        ))
    }
}

#[test]
fn first_install_repair_and_remove_share_the_restartable_executor() {
    let mut fixture = Fixture::new(product("1.0.0", 1, 0));
    let installed = fixture.begin_and_confirm(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    assert_eq!(
        installed.disposition(),
        InstallerExecutionDisposition::Completed
    );
    assert!(fixture
        .programs
        .target_path(ProgramComponent::Manager)
        .is_dir());
    assert!(fixture
        .programs
        .target_path(ProgramComponent::InputMethod)
        .is_dir());

    fixture.programs.target_product = product("1.0.0", 1, 1);
    let repaired = fixture.begin_and_confirm(
        InstallerProductSituation::MatchingReleaseInstalled,
        InstallerAction::BeginRepair,
    );
    assert_eq!(repaired.operation_kind(), InstallOperationKind::Repair);
    assert_eq!(repaired.state(), InstallState::Completed);

    let removed = fixture.begin_and_confirm(
        InstallerProductSituation::MatchingReleaseInstalled,
        InstallerAction::RemovePrograms,
    );
    assert_eq!(
        removed.operation_kind(),
        InstallOperationKind::RemovePrograms
    );
    assert_eq!(removed.state(), InstallState::Completed);
    assert!(!fixture
        .programs
        .target_path(ProgramComponent::Manager)
        .exists());
    assert!(!fixture
        .programs
        .target_path(ProgramComponent::InputMethod)
        .exists());
    assert!(fixture.data_root.is_dir());
}

#[test]
fn upgrade_runs_existing_data_transaction_and_both_final_checkpoints() {
    let mut fixture = Fixture::new(product("1.0.0", 1, 0));
    fixture.begin_and_confirm(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    fixture.programs.target_product = product("2.0.0", 2, 1);
    prepare_upgrade_data(&fixture.data_root);
    let mut upgrade_port = TestUpgradePort;

    let begin = fixture.authorize(
        InstallerProductSituation::OlderReleaseInstalled,
        InstallerAction::BeginUpgrade,
    );
    let prepared = fixture
        .execute_with_upgrade_bootstrap(begin, &mut upgrade_port)
        .expect("prepare upgrade");
    assert_eq!(prepared.state(), InstallState::Prepared);
    let outer = fixture.receipt();
    let bootstrapped = bootstrap_upgrade_receipt(&outer, &fixture.data_root, fixture.owner_id)
        .expect("reload bootstrapped upgrade receipt");
    assert!(bootstrapped
        .receipt()
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceSettings));
    assert!(bootstrapped
        .receipt()
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::RimeRoot));
    let drifted_outer = InstallReceipt::new(
        outer.operation_id(),
        None,
        InstallOperationKind::Upgrade,
        outer.root_identity().clone(),
        outer.source_product().cloned(),
        Some(product("3.0.0", 3, 2)),
    )
    .expect("drifted outer receipt");
    let drift_error =
        match bootstrap_upgrade_receipt(&drifted_outer, &fixture.data_root, fixture.owner_id) {
            Ok(_) => panic!("target release drift must be rejected"),
            Err(error) => error,
        };
    assert_eq!(
        drift_error.code(),
        UpgradeBootstrapErrorCode::ReceiptBindingChanged
    );
    fixture.programs.reject_finalization_completion = true;
    let confirm = fixture.authorize(
        InstallerProductSituation::OlderReleaseInstalled,
        InstallerAction::ConfirmQuiescence,
    );
    assert!(matches!(
        fixture
            .execute_with_upgrade_bootstrap(confirm, &mut upgrade_port)
            .expect_err("interrupt second finalization checkpoint"),
        InstallerExecutionError::UpgradeFinalization(_)
    ));
    assert_eq!(fixture.receipt().state(), InstallState::FinalVerified);

    let restarted =
        bootstrap_upgrade_receipt(&fixture.receipt(), &fixture.data_root, fixture.owner_id)
            .expect("reload progressed upgrade receipt after restart");
    assert_eq!(restarted.receipt().state(), UpgradeState::Completed);
    fixture.programs.reject_finalization_completion = false;
    let resume = fixture.authorize(
        InstallerProductSituation::OlderReleaseInstalled,
        InstallerAction::ResumeOperation,
    );
    let completed = fixture
        .execute_with_upgrade_bootstrap(resume, &mut upgrade_port)
        .expect("resume final_verified upgrade");

    assert_eq!(completed.operation_kind(), InstallOperationKind::Upgrade);
    assert_eq!(completed.state(), InstallState::Completed);
    assert!(fixture.programs.installed_validation_calls >= 12);
}

#[test]
fn interruption_restarts_from_persisted_evidence_without_a_new_operation() {
    let mut fixture = Fixture::new(product("1.0.0", 1, 0));
    let begin = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    fixture
        .execute(begin, &mut NoUpgradeExecution)
        .expect("prepare first install");
    let operation_id = fixture.receipt().operation_id().to_owned();

    fixture.programs.fail_input_stage_once = true;
    let confirm = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::ConfirmQuiescence,
    );
    assert_eq!(
        fixture
            .execute(confirm, &mut NoUpgradeExecution)
            .expect_err("injected staging interruption"),
        InstallerExecutionError::InvalidState
    );
    assert_eq!(fixture.receipt().state(), InstallState::Quiesced);

    fixture.programs.fail_input_stage_once = false;
    let resume = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::ResumeOperation,
    );
    let completed = fixture
        .execute(resume, &mut NoUpgradeExecution)
        .expect("resume interrupted operation");
    assert_eq!(completed.state(), InstallState::Completed);
    assert_eq!(fixture.receipt().operation_id(), operation_id);
}

#[test]
fn preflight_active_guard_and_stale_intents_fail_closed_before_program_mutation() {
    let mut fixture = Fixture::new(product("1.0.0", 1, 0));
    fixture
        .preflight
        .results
        .push_back(Err(InstallerExecutionError::PreflightNotProven));
    let begin = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    assert_eq!(
        fixture
            .execute(begin, &mut NoUpgradeExecution)
            .expect_err("preflight must fail closed"),
        InstallerExecutionError::PreflightNotProven
    );
    assert!(fixture.store.load().expect("empty store").is_none());

    let begin = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    let stale = begin;
    fixture
        .execute(begin, &mut NoUpgradeExecution)
        .expect("prepared operation");
    assert_eq!(
        fixture
            .execute(stale, &mut NoUpgradeExecution)
            .expect_err("stale begin intent"),
        InstallerExecutionError::InvalidIntent
    );
    assert_eq!(fixture.receipt().state(), InstallState::Prepared);

    let confirm = fixture.authorize(
        InstallerProductSituation::NotInstalled,
        InstallerAction::ConfirmQuiescence,
    );
    let guard = fixture
        .store
        .acquire_guard()
        .expect("external active guard");
    let error = fixture
        .execute(confirm, &mut NoUpgradeExecution)
        .expect_err("active guard");
    assert_eq!(error.code(), "install_filesystem_failure");
    drop(guard);
    assert_eq!(fixture.receipt().state(), InstallState::Prepared);
}

#[test]
fn system_operation_ids_are_fixed_lowercase_hex() {
    let operation_id = SystemInstallerOperationIdSource
        .next_operation_id()
        .expect("system operation identity");

    assert_eq!(operation_id.len(), 32);
    assert!(operation_id
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
}

fn prepare_upgrade_data(data_root: &Path) {
    let database = data_root.join("userdb.sqlite3");
    drop(UserDb::open(&database).expect("source userdb"));
    UserDb::migrate_and_validate(&database).expect("validated source userdb");
    let settings = data_root.join("manager-settings.json");
    fs::write(
        &settings,
        b"{\"format_version\":1,\"privacy_mode\":false}\n",
    )
    .expect("synthetic settings");
    fs::set_permissions(&settings, fs::Permissions::from_mode(0o600)).expect("private settings");
    create_directory(&data_root.join("Rime"), 0o700);
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
