#![cfg(all(target_os = "macos", feature = "qualification-harness"))]

use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use radishlex_ime_product_install::{
    commit_program_target, finish_source_preservation, finish_target_staging,
    inspect_install_startup_gate, preserve_program_source, InstallFinalizationError,
    InstallFinalizationValidationStage, InstallOperationKind, InstallProgramValidationPort,
    InstallReceipt, InstallReceiptStore, InstallStartupGateErrorCode, InstallState,
    ProgramComponent, ProgramSwitchStore, RunningProgramIdentity, VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::{
    inspect_startup_gate, ProductRelease as UpgradeProductRelease, StartupGateErrorCode,
    UpgradeArtifactIdentity, UpgradeArtifactSlot, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradeCoordinatorPort, UpgradePostSwitchValidationReport,
    UpgradeReceipt, UpgradeReceiptStore, UpgradeRollbackValidationEvidence, UpgradeState,
    VerifiedDataRoot,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_product_install_coordinator::{
    resume_install_data_coordination, resume_upgrade_install_finalization,
    InstallDataCoordinationDisposition, InstallProductFinalizationError,
};
use radishlex_macos_upgrade_coordinator::MacOsUpgradeCoordinatorAdapter;

const QUALIFICATION_ROOT_ENV: &str = "RADISHLEX_INSTALL_QUALIFICATION_ROOT";
const SOURCE_PRODUCT_ENV: &str = "RADISHLEX_INSTALL_QUALIFICATION_SOURCE_PRODUCT";
const TARGET_PRODUCT_ENV: &str = "RADISHLEX_INSTALL_QUALIFICATION_TARGET_PRODUCT";
const SOURCE_PAYLOAD_ENV: &str = "RADISHLEX_INSTALL_QUALIFICATION_SOURCE_PAYLOAD";
const TARGET_PAYLOAD_ENV: &str = "RADISHLEX_INSTALL_QUALIFICATION_TARGET_PAYLOAD";
#[path = "support/pre_switch_recovery.rs"]
mod pre_switch_recovery;

#[test]
fn isolated_real_product_install_and_data_recovery_gate() {
    let roots = QualificationRoots::from_environment();
    interrupted_finalization_resumes_and_opens_both_gates(&roots);
    candidate_failure_restores_exact_source_product(&roots);
    partial_program_commit_restarts_into_completed_product(&roots);
    pre_switch_recovery::preserves_wal_and_restarts_with_exact_source_programs(&roots);
}

struct QualificationRoots {
    qualification: PathBuf,
    source_product: PathBuf,
    target_product: PathBuf,
    source_payload: PathBuf,
    target_payload: PathBuf,
}

impl QualificationRoots {
    fn from_environment() -> Self {
        let qualification = required_canonical_directory(QUALIFICATION_ROOT_ENV);
        let source_product = required_canonical_descendant(SOURCE_PRODUCT_ENV, &qualification);
        let target_product = required_canonical_descendant(TARGET_PRODUCT_ENV, &qualification);
        let source_payload = required_canonical_descendant(SOURCE_PAYLOAD_ENV, &qualification);
        let target_payload = required_canonical_descendant(TARGET_PAYLOAD_ENV, &qualification);
        Self {
            qualification,
            source_product,
            target_product,
            source_payload,
            target_payload,
        }
    }
}

struct ProductFixture<'a> {
    roots: &'a QualificationRoots,
    scenario_root: PathBuf,
    home: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
    install_store: InstallReceiptStore,
    install_receipt: InstallReceipt,
    manager: ProgramSwitchStore,
    input_method: ProgramSwitchStore,
    upgrade_store: UpgradeReceiptStore,
    upgrade_receipt: UpgradeReceipt,
    target_adapter: MacOsProductInstallAdapter,
}

impl<'a> ProductFixture<'a> {
    fn prepared(roots: &'a QualificationRoots, name: &str, operation_id: &'static str) -> Self {
        Self::prepared_with_program_commit(roots, name, operation_id, true, false)
    }

    fn prepared_with_program_commit(
        roots: &'a QualificationRoots,
        name: &str,
        operation_id: &'static str,
        commit_input_method: bool,
        wal_source: bool,
    ) -> Self {
        let scenario_root = roots.qualification.join("install-scenarios").join(name);
        let home = scenario_root.join("home");
        let applications = home.join("Applications");
        let input_methods = home.join("Library/Input Methods");
        let data_root = home.join("Library/Application Support/RadishLex");
        for directory in [
            &scenario_root,
            &home,
            &applications,
            &input_methods,
            &data_root,
        ] {
            create_private_directory(directory);
        }
        let owner_id = fs::metadata(&home).expect("synthetic home metadata").uid();
        ditto(
            &roots
                .source_product
                .join("Components/radishlex_manager.app"),
            &applications.join("RadishLex Manager.app"),
        );
        ditto(
            &roots
                .source_product
                .join("Components/RadishLexInputMethod.app"),
            &input_methods.join("RadishLexInputMethod.app"),
        );

        let source_adapter = MacOsProductInstallAdapter::load_for_qualification(
            &roots.source_payload,
            &home,
            owner_id,
            &roots.qualification,
        )
        .expect("load source install payload");
        let target_adapter = MacOsProductInstallAdapter::load_for_qualification(
            &roots.target_payload,
            &home,
            owner_id,
            &roots.qualification,
        )
        .expect("load target install payload");
        let install_store = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&data_root, owner_id).expect("verified install root"),
        )
        .expect("open install receipt store");
        let mut install_receipt = InstallReceipt::new(
            operation_id,
            None,
            InstallOperationKind::Upgrade,
            install_store.root_identity().clone(),
            Some(source_adapter.target_product().clone()),
            Some(target_adapter.target_product().clone()),
        )
        .expect("create install receipt");
        let install_guard = install_store
            .acquire_guard()
            .expect("initial install guard");
        install_store
            .persist(&install_guard, &install_receipt)
            .expect("persist prepared install");
        install_receipt
            .advance(InstallState::Quiesced)
            .expect("advance install quiesced");
        install_store
            .persist(&install_guard, &install_receipt)
            .expect("persist install quiesced");

        let manager = target_adapter
            .open_program_store(
                &install_store,
                &install_guard,
                ProgramComponent::Manager,
                &install_receipt,
            )
            .expect("open Manager store");
        let input_method = target_adapter
            .open_program_store(
                &install_store,
                &install_guard,
                ProgramComponent::InputMethod,
                &install_receipt,
            )
            .expect("open InputMethod store");
        for store in [&manager, &input_method] {
            target_adapter
                .verify_and_record_source(
                    &install_store,
                    &install_guard,
                    store,
                    &mut install_receipt,
                )
                .expect("record source program");
            target_adapter
                .prepare_and_record_staged(
                    &install_store,
                    &install_guard,
                    store,
                    &mut install_receipt,
                )
                .expect("stage target program");
        }
        finish_target_staging(
            &install_store,
            &install_guard,
            &manager,
            &input_method,
            &mut install_receipt,
        )
        .expect("finish target staging");
        for store in [&manager, &input_method] {
            preserve_program_source(&install_store, &install_guard, store, &mut install_receipt)
                .expect("preserve source program");
        }
        finish_source_preservation(
            &install_store,
            &install_guard,
            &manager,
            &input_method,
            &mut install_receipt,
        )
        .expect("finish source preservation");
        let stores: &[&ProgramSwitchStore] = if commit_input_method {
            &[&manager, &input_method]
        } else {
            &[&manager]
        };
        for store in stores {
            commit_program_target(&install_store, &install_guard, store, &mut install_receipt)
                .expect("commit target program");
        }
        drop(install_guard);

        let database = data_root.join("userdb.sqlite3");
        let mut db = UserDb::open(&database).expect("create source database");
        if wal_source {
            db.record_selection(radishlex_ime_userdb::SelectionEventDraft::new(
                "recovery-qualified",
                "shi",
                "时",
                0,
                1,
            ))
            .unwrap();
            db.record_selection(radishlex_ime_userdb::SelectionEventDraft::new(
                "recovery-qualified",
                "shanchu",
                "删除测试",
                0,
                1,
            ))
            .unwrap();
            db.delete_term("shanchu", "删除测试", None).unwrap();
        }
        drop(db);
        if !wal_source {
            UserDb::migrate_and_validate(&database).expect("stabilize source database");
        }
        let settings = data_root.join("manager-settings.json");
        fs::write(
            &settings,
            b"{\"format_version\":1,\"privacy_mode\":false}\n",
        )
        .expect("write synthetic settings");
        fs::set_permissions(&settings, fs::Permissions::from_mode(0o600))
            .expect("set settings permissions");
        let upgrade_store = UpgradeReceiptStore::open(
            VerifiedDataRoot::verify(&data_root, owner_id).expect("verified data root"),
        )
        .expect("open upgrade receipt store");
        let upgrade_receipt = UpgradeReceipt::new(
            operation_id,
            None,
            UpgradeProductRelease::new(
                source_adapter.target_product().release().product_version(),
                source_adapter.target_product().release().build_number(),
            )
            .expect("source release"),
            UpgradeProductRelease::new(
                target_adapter.target_product().release().product_version(),
                target_adapter.target_product().release().build_number(),
            )
            .expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![
                upgrade_store.data_root_identity().clone(),
                private_file_identity(UpgradeArtifactSlot::SourceDatabase, &database),
                private_file_identity(UpgradeArtifactSlot::SourceSettings, &settings),
            ],
        )
        .expect("create upgrade receipt");
        let upgrade_guard = upgrade_store
            .acquire_guard()
            .expect("initial upgrade guard");
        upgrade_store
            .persist(&upgrade_guard, &upgrade_receipt)
            .expect("persist upgrade receipt");
        drop(upgrade_guard);

        Self {
            roots,
            scenario_root,
            home,
            data_root,
            owner_id,
            install_store,
            install_receipt,
            manager,
            input_method,
            upgrade_store,
            upgrade_receipt,
            target_adapter,
        }
    }

    fn upgrade_adapter(&self) -> MacOsUpgradeCoordinatorAdapter {
        MacOsUpgradeCoordinatorAdapter::load_for_qualification(
            &self.roots.source_product,
            &self.roots.target_product,
            &self.roots.qualification,
            &self.home,
        )
        .expect("load real product upgrade adapter")
    }

    fn running(&self, target: bool, component: ProgramComponent) -> RunningProgramIdentity {
        let product = if target {
            self.install_receipt
                .target_product()
                .expect("target product")
        } else {
            self.install_receipt
                .source_product()
                .expect("source product")
        };
        RunningProgramIdentity::new(
            product.release().clone(),
            product.program(component).clone(),
        )
        .expect("running product identity")
    }

    fn assert_active_gates_closed(&self) {
        let install_guard = self.install_store.acquire_guard().expect("install guard");
        let upgrade_guard = self.upgrade_store.acquire_guard().expect("upgrade guard");
        assert_eq!(
            inspect_install_startup_gate(
                &self.data_root,
                self.owner_id,
                &self.running(true, ProgramComponent::Manager),
            )
            .error_code(),
            InstallStartupGateErrorCode::ActiveGuard
        );
        assert_eq!(
            inspect_startup_gate(&self.data_root, self.owner_id).error_code(),
            StartupGateErrorCode::ActiveGuard
        );
        drop(upgrade_guard);
        drop(install_guard);
    }
}

impl Drop for ProductFixture<'_> {
    fn drop(&mut self) {
        if self.scenario_root.starts_with(&self.roots.qualification) {
            let _ = fs::remove_dir_all(&self.scenario_root);
        }
    }
}

fn interrupted_finalization_resumes_and_opens_both_gates(roots: &QualificationRoots) {
    let mut fixture = ProductFixture::prepared(
        roots,
        "interrupted-finalization",
        "00112233445566778899aabbccddeeff",
    );
    fixture.assert_active_gates_closed();
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    let mut upgrade_adapter = fixture.upgrade_adapter();
    let summary = resume_install_data_coordination(
        &fixture.install_store,
        &install_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &upgrade_guard,
        &mut fixture.upgrade_receipt,
        u64::MAX,
        &mut upgrade_adapter,
        &mut fixture.target_adapter,
    )
    .expect("real product data coordination");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::DataSettled
    );

    let mut interrupted = InterruptSecondValidation {
        inner: &mut fixture.target_adapter,
        calls: 0,
    };
    assert_eq!(
        resume_upgrade_install_finalization(
            &fixture.install_store,
            &install_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &upgrade_guard,
            &fixture.upgrade_receipt,
            &mut interrupted,
        ),
        Err(InstallProductFinalizationError::InstallFinalization(
            InstallFinalizationError::FinalStateNotProven(
                InstallFinalizationValidationStage::BeforeCompleted
            )
        ))
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::FinalVerified);
    drop(upgrade_guard);
    drop(install_guard);

    fixture.install_receipt = fixture
        .install_store
        .load()
        .expect("reload install receipt")
        .expect("persisted install receipt");
    fixture.upgrade_receipt = fixture
        .upgrade_store
        .load()
        .expect("reload upgrade receipt")
        .expect("persisted upgrade receipt");
    assert_eq!(
        inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(true, ProgramComponent::Manager),
        )
        .error_code(),
        InstallStartupGateErrorCode::InstallInProgress
    );
    assert!(inspect_startup_gate(&fixture.data_root, fixture.owner_id).is_allowed());

    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("resume install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("resume upgrade guard");
    resume_upgrade_install_finalization(
        &fixture.install_store,
        &install_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &upgrade_guard,
        &fixture.upgrade_receipt,
        &mut fixture.target_adapter,
    )
    .expect("resume finalization");
    drop(upgrade_guard);
    drop(install_guard);
    assert_eq!(fixture.install_receipt.state(), InstallState::Completed);
    assert_eq!(fixture.upgrade_receipt.state(), UpgradeState::Completed);
    for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
        assert!(inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(true, component),
        )
        .is_allowed());
    }
    assert!(inspect_startup_gate(&fixture.data_root, fixture.owner_id).is_allowed());
}

fn candidate_failure_restores_exact_source_product(roots: &QualificationRoots) {
    let mut fixture = ProductFixture::prepared(
        roots,
        "candidate-failure",
        "ffeeddccbbaa99887766554433221100",
    );
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("install guard");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("upgrade guard");
    let mut failing_upgrade = CandidateFailurePort {
        inner: fixture.upgrade_adapter(),
        injected: false,
    };
    let summary = resume_install_data_coordination(
        &fixture.install_store,
        &install_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &upgrade_guard,
        &mut fixture.upgrade_receipt,
        u64::MAX,
        &mut failing_upgrade,
        &mut fixture.target_adapter,
    )
    .expect("candidate failure restores programs");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    drop(upgrade_guard);
    drop(install_guard);
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
        assert!(inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(false, component),
        )
        .is_allowed());
        assert_eq!(
            inspect_install_startup_gate(
                &fixture.data_root,
                fixture.owner_id,
                &fixture.running(true, component),
            )
            .error_code(),
            InstallStartupGateErrorCode::ProgramIdentityChanged
        );
    }
    assert!(inspect_startup_gate(&fixture.data_root, fixture.owner_id).is_allowed());
}

fn partial_program_commit_restarts_into_completed_product(roots: &QualificationRoots) {
    let mut fixture = ProductFixture::prepared_with_program_commit(
        roots,
        "partial-program-commit",
        "1234567890abcdef1234567890abcdef",
        false,
        false,
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::ManagerCommitted
    );
    assert_eq!(
        inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(true, ProgramComponent::Manager),
        )
        .error_code(),
        InstallStartupGateErrorCode::InstallInProgress
    );
    fixture.install_receipt = fixture
        .install_store
        .load()
        .expect("reload partial install receipt")
        .expect("persisted partial install receipt");

    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("restart install guard");
    commit_program_target(
        &fixture.install_store,
        &install_guard,
        &fixture.input_method,
        &mut fixture.install_receipt,
    )
    .expect("resume InputMethod commit");
    let upgrade_guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("restart upgrade guard");
    let mut upgrade_adapter = fixture.upgrade_adapter();
    let summary = resume_install_data_coordination(
        &fixture.install_store,
        &install_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &upgrade_guard,
        &mut fixture.upgrade_receipt,
        u64::MAX,
        &mut upgrade_adapter,
        &mut fixture.target_adapter,
    )
    .expect("resume data after partial program commit");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::DataSettled
    );
    resume_upgrade_install_finalization(
        &fixture.install_store,
        &install_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &upgrade_guard,
        &fixture.upgrade_receipt,
        &mut fixture.target_adapter,
    )
    .expect("finalize restarted product");
    drop(upgrade_guard);
    drop(install_guard);
    assert_eq!(fixture.install_receipt.state(), InstallState::Completed);
    for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
        assert!(inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(true, component),
        )
        .is_allowed());
    }
}

struct InterruptSecondValidation<'a> {
    inner: &'a mut MacOsProductInstallAdapter,
    calls: usize,
}

impl InstallProgramValidationPort for InterruptSecondValidation<'_> {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.calls += 1;
        self.calls == 1
            && self
                .inner
                .validate_installed_targets(manager, input_method, receipt)
    }

    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.inner
            .validate_restored_sources(manager, input_method, receipt)
    }
}

struct CandidateFailurePort {
    inner: MacOsUpgradeCoordinatorAdapter,
    injected: bool,
}

impl UpgradeCoordinatorPort for CandidateFailurePort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        self.inner.confirm_quiescence(checkpoint)
    }

    fn validate_candidate(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        let actual = self
            .inner
            .validate_candidate(target_release, target_schema_version);
        assert!(matches!(
            actual,
            UpgradeCandidateValidationReport::Passed { .. }
        ));
        assert!(!self.injected);
        self.injected = true;
        UpgradeCandidateValidationReport::manager_failed()
    }

    fn validate_post_switch(
        &mut self,
        target_release: &UpgradeProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        self.inner
            .validate_post_switch(target_release, target_schema_version)
    }

    fn validate_restored_source(
        &mut self,
        source_release: &UpgradeProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        self.inner
            .validate_restored_source(source_release, source_schema_version)
    }
}

fn private_file_identity(slot: UpgradeArtifactSlot, path: &Path) -> UpgradeArtifactIdentity {
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

fn ditto(source: &Path, target: &Path) {
    let status = Command::new("/usr/bin/ditto")
        .env_clear()
        .arg(source)
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run ditto");
    assert!(status.success());
}

fn create_private_directory(path: &Path) {
    if path.is_dir() {
        return;
    }
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .expect("create private directory");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("set private directory mode");
}

fn required_canonical_directory(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required"));
    let path = PathBuf::from(value);
    let canonical = fs::canonicalize(&path).unwrap_or_else(|_| panic!("{name} is unavailable"));
    assert!(canonical.is_dir(), "{name} must be a directory");
    canonical
}

fn required_canonical_descendant(name: &str, root: &Path) -> PathBuf {
    let path = required_canonical_directory(name);
    assert!(
        path.starts_with(root),
        "{name} must remain inside qualification root"
    );
    path
}
