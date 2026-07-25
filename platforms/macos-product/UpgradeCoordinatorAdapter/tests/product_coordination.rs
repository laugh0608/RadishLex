#![cfg(all(target_os = "macos", feature = "qualification-harness"))]

use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use radishlex_ime_product_upgrade::{
    ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradeCoordinatorDisposition, UpgradeCoordinatorError,
    UpgradeCoordinatorPort, UpgradeManagerValidationEvidence, UpgradePostSwitchValidationReport,
    UpgradeReceipt, UpgradeReceiptStore, UpgradeRollbackValidationEvidence, UpgradeState,
    VerifiedDataRoot, UPGRADE_VALIDATION_EVIDENCE_VERSION,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_upgrade_coordinator::MacOsUpgradeCoordinatorAdapter;

const QUALIFICATION_ROOT_ENV: &str = "RADISHLEX_UPGRADE_QUALIFICATION_ROOT";
const SOURCE_PRODUCT_ENV: &str = "RADISHLEX_UPGRADE_QUALIFICATION_SOURCE_PRODUCT";
const TARGET_PRODUCT_ENV: &str = "RADISHLEX_UPGRADE_QUALIFICATION_TARGET_PRODUCT";
const SOURCE_VERSION: &str = "0.0.9";
const SOURCE_BUILD: u64 = 34;
const TARGET_VERSION: &str = "0.1.0";
const TARGET_BUILD: u64 = 35;

#[test]
fn real_product_hosts_drive_isolated_coordination_scenarios() {
    let roots = QualificationRoots::from_environment();

    successful_upgrade_preserves_source_backup(&roots);
    manager_and_input_method_failures_preserve_source(&roots);
    quiescence_loss_resumes_from_last_persisted_state(&roots);
    post_switch_failure_restores_source_and_resumes_validation(&roots);
}

struct QualificationRoots {
    qualification: PathBuf,
    source_product: PathBuf,
    target_product: PathBuf,
}

impl QualificationRoots {
    fn from_environment() -> Self {
        let qualification = required_canonical_directory(QUALIFICATION_ROOT_ENV);
        let source_product = required_canonical_directory(SOURCE_PRODUCT_ENV);
        let target_product = required_canonical_directory(TARGET_PRODUCT_ENV);
        assert!(source_product.starts_with(&qualification));
        assert!(target_product.starts_with(&qualification));
        Self {
            qualification,
            source_product,
            target_product,
        }
    }
}

struct ScenarioFixture<'a> {
    roots: &'a QualificationRoots,
    scenario_root: PathBuf,
    home: PathBuf,
    active_database: PathBuf,
    source_bytes: Vec<u8>,
    source_inode: u64,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl<'a> ScenarioFixture<'a> {
    fn new(roots: &'a QualificationRoots, name: &str, operation_id: &str) -> Self {
        assert!(name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-'));
        let scenario_root = roots.qualification.join("scenarios").join(name);
        let home = scenario_root.join("home");
        let data_root = home.join("Library/Application Support/RadishLex");
        create_private_directory_chain(&roots.qualification, &data_root);

        let owner_id = fs::metadata(&data_root).expect("data root metadata").uid();
        let verified_root =
            VerifiedDataRoot::verify(&data_root, owner_id).expect("verify synthetic data root");
        let store = UpgradeReceiptStore::open(verified_root).expect("open receipt store");
        let active_database = data_root.join("userdb.sqlite3");
        drop(UserDb::open(&active_database).expect("create source database"));
        UserDb::migrate_and_validate(&active_database).expect("stabilize source database");
        let source_bytes = fs::read(&active_database).expect("source database bytes");
        let source_identity =
            private_file_identity(UpgradeArtifactSlot::SourceDatabase, &active_database);
        let source_inode = source_identity.inode();

        let settings = data_root.join("manager-settings.json");
        fs::write(
            &settings,
            b"{\"format_version\":1,\"privacy_mode\":false}\n",
        )
        .expect("write synthetic settings");
        fs::set_permissions(&settings, fs::Permissions::from_mode(0o600))
            .expect("set settings mode");
        let settings_identity =
            private_file_identity(UpgradeArtifactSlot::SourceSettings, &settings);
        let receipt = UpgradeReceipt::new(
            operation_id,
            None,
            ProductRelease::new(SOURCE_VERSION, SOURCE_BUILD).expect("source release"),
            ProductRelease::new(TARGET_VERSION, TARGET_BUILD).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![
                store.data_root_identity().clone(),
                source_identity,
                settings_identity,
            ],
        )
        .expect("create preflighted receipt");
        let guard = store.acquire_guard().expect("initial guard");
        store
            .persist(&guard, &receipt)
            .expect("persist initial receipt");
        drop(guard);
        Self {
            roots,
            scenario_root,
            home,
            active_database,
            source_bytes,
            source_inode,
            store,
            receipt,
        }
    }

    fn adapter(&self) -> MacOsUpgradeCoordinatorAdapter {
        MacOsUpgradeCoordinatorAdapter::load_for_qualification(
            &self.roots.source_product,
            &self.roots.target_product,
            &self.roots.qualification,
            &self.home,
        )
        .expect("load qualification adapter")
    }

    fn assert_source_is_active(&self) {
        let metadata = fs::metadata(&self.active_database).expect("active database metadata");
        assert_eq!(metadata.ino(), self.source_inode);
        assert_eq!(
            fs::read(&self.active_database).expect("active database bytes"),
            self.source_bytes
        );
    }
}

impl Drop for ScenarioFixture<'_> {
    fn drop(&mut self) {
        if self.scenario_root.starts_with(&self.roots.qualification) {
            let _ = fs::remove_dir_all(&self.scenario_root);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateFailure {
    Manager,
    InputMethod,
}

struct FaultPort {
    adapter: MacOsUpgradeCoordinatorAdapter,
    candidate_failure: Option<CandidateFailure>,
    reject_checkpoint_once: Option<UpgradeCoordinatorCheckpoint>,
    post_switch_failure: bool,
    source_unavailable_once: bool,
}

impl FaultPort {
    fn normal(adapter: MacOsUpgradeCoordinatorAdapter) -> Self {
        Self {
            adapter,
            candidate_failure: None,
            reject_checkpoint_once: None,
            post_switch_failure: false,
            source_unavailable_once: false,
        }
    }
}

impl UpgradeCoordinatorPort for FaultPort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        if !self.adapter.confirm_quiescence(checkpoint) {
            return false;
        }
        if self.reject_checkpoint_once == Some(checkpoint) {
            self.reject_checkpoint_once = None;
            return false;
        }
        true
    }

    fn validate_candidate(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        let actual = self
            .adapter
            .validate_candidate(target_release, target_schema_version);
        if !matches!(actual, UpgradeCandidateValidationReport::Passed { .. }) {
            panic!("real candidate validation failed before injection: {actual:?}");
        }
        match self.candidate_failure.take() {
            Some(CandidateFailure::Manager) => UpgradeCandidateValidationReport::manager_failed(),
            Some(CandidateFailure::InputMethod) => {
                UpgradeCandidateValidationReport::input_method_failed(manager_evidence(
                    target_schema_version,
                ))
            }
            None => actual,
        }
    }

    fn validate_post_switch(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        let actual = self
            .adapter
            .validate_post_switch(target_release, target_schema_version);
        if !matches!(actual, UpgradePostSwitchValidationReport::Passed { .. }) {
            panic!("real post-switch validation failed before injection: {actual:?}");
        }
        if self.post_switch_failure {
            self.post_switch_failure = false;
            UpgradePostSwitchValidationReport::manager_failed()
        } else {
            actual
        }
    }

    fn validate_restored_source(
        &mut self,
        source_release: &ProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        let actual = self
            .adapter
            .validate_restored_source(source_release, source_schema_version);
        assert!(actual.is_some());
        if self.source_unavailable_once {
            self.source_unavailable_once = false;
            None
        } else {
            actual
        }
    }
}

fn successful_upgrade_preserves_source_backup(roots: &QualificationRoots) {
    let mut fixture = ScenarioFixture::new(
        roots,
        "successful-upgrade",
        "10000000000000000000000000000001",
    );
    let mut adapter = fixture.adapter();
    let available = adapter
        .inspect_preflight()
        .expect("initial preflight")
        .available_bytes();
    let mut port = FaultPort::normal(adapter);
    let guard = fixture.store.acquire_guard().expect("upgrade guard");

    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut port)
        .expect("complete product upgrade");

    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::Completed
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::Completed);
    assert_ne!(
        fs::metadata(&fixture.active_database)
            .expect("target database metadata")
            .ino(),
        fixture.source_inode
    );
    let backup = fixture
        .active_database
        .parent()
        .expect("data root")
        .join(".radishlex-upgrade-v1/source-backup.sqlite3");
    assert_eq!(
        fs::metadata(&backup).expect("source backup metadata").ino(),
        fixture.source_inode
    );
    assert_eq!(
        fs::read(backup).expect("source backup bytes"),
        fixture.source_bytes
    );
}

fn manager_and_input_method_failures_preserve_source(roots: &QualificationRoots) {
    for (name, operation_id, failure) in [
        (
            "manager-candidate-failure",
            "20000000000000000000000000000001",
            CandidateFailure::Manager,
        ),
        (
            "input-candidate-failure",
            "20000000000000000000000000000002",
            CandidateFailure::InputMethod,
        ),
    ] {
        let mut fixture = ScenarioFixture::new(roots, name, operation_id);
        let mut adapter = fixture.adapter();
        let available = adapter
            .inspect_preflight()
            .expect("initial preflight")
            .available_bytes();
        let mut port = FaultPort::normal(adapter);
        port.candidate_failure = Some(failure);
        let guard = fixture.store.acquire_guard().expect("upgrade guard");

        let summary = fixture
            .store
            .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut port)
            .expect("candidate endpoint failure is terminal");

        assert_eq!(
            summary.disposition(),
            UpgradeCoordinatorDisposition::AbortedPreserved
        );
        assert_eq!(fixture.receipt.state(), UpgradeState::AbortedPreserved);
        fixture.assert_source_is_active();
    }
}

fn quiescence_loss_resumes_from_last_persisted_state(roots: &QualificationRoots) {
    let mut fixture = ScenarioFixture::new(
        roots,
        "quiescence-loss-restart",
        "30000000000000000000000000000001",
    );
    let mut adapter = fixture.adapter();
    let available = adapter
        .inspect_preflight()
        .expect("initial preflight")
        .available_bytes();
    let mut interrupted = FaultPort::normal(adapter);
    interrupted.reject_checkpoint_once =
        Some(UpgradeCoordinatorCheckpoint::AfterCandidateValidation);
    let guard = fixture.store.acquire_guard().expect("upgrade guard");
    assert_eq!(
        fixture
            .store
            .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut interrupted,)
            .expect_err("quiescence loss must stop"),
        UpgradeCoordinatorError::QuiescenceNotProven(
            UpgradeCoordinatorCheckpoint::AfterCandidateValidation
        )
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::CandidateMigrated);
    drop(guard);

    fixture.receipt = fixture
        .store
        .load()
        .expect("load interrupted receipt")
        .expect("persisted receipt");
    let mut adapter = fixture.adapter();
    let available = adapter
        .inspect_preflight()
        .expect("recovery preflight")
        .available_bytes();
    let mut recovered = FaultPort::normal(adapter);
    let guard = fixture.store.acquire_guard().expect("recovery guard");
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut recovered)
        .expect("resume interrupted product upgrade");
    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::Completed
    );
}

fn post_switch_failure_restores_source_and_resumes_validation(roots: &QualificationRoots) {
    let mut fixture = ScenarioFixture::new(
        roots,
        "rollback-validation-restart",
        "40000000000000000000000000000001",
    );
    let mut adapter = fixture.adapter();
    let available = adapter
        .inspect_preflight()
        .expect("initial preflight")
        .available_bytes();
    let mut interrupted = FaultPort::normal(adapter);
    interrupted.post_switch_failure = true;
    interrupted.source_unavailable_once = true;
    let guard = fixture.store.acquire_guard().expect("upgrade guard");
    assert_eq!(
        fixture
            .store
            .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut interrupted,)
            .expect_err("source evidence loss must stop"),
        UpgradeCoordinatorError::SourceValidationNotProven
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RollbackRequired);
    fixture.assert_source_is_active();
    drop(guard);

    fixture.receipt = fixture
        .store
        .load()
        .expect("load rollback receipt")
        .expect("persisted rollback receipt");
    let mut adapter = fixture.adapter();
    let available = adapter
        .inspect_preflight()
        .expect("rollback recovery preflight")
        .available_bytes();
    let mut recovered = FaultPort::normal(adapter);
    let guard = fixture
        .store
        .acquire_guard()
        .expect("rollback recovery guard");
    let summary = fixture
        .store
        .resume_userdb_upgrade(&guard, &mut fixture.receipt, available, &mut recovered)
        .expect("complete rollback validation");
    assert_eq!(
        summary.disposition(),
        UpgradeCoordinatorDisposition::RolledBack
    );
    assert_eq!(fixture.receipt.state(), UpgradeState::RolledBack);
    fixture.assert_source_is_active();
}

fn manager_evidence(schema_version: i64) -> UpgradeManagerValidationEvidence {
    UpgradeManagerValidationEvidence::new(UPGRADE_VALIDATION_EVIDENCE_VERSION, schema_version, 1, 1)
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

fn create_private_directory_chain(qualification_root: &Path, target: &Path) {
    let relative = target
        .strip_prefix(qualification_root)
        .expect("scenario remains inside qualification root");
    let mut current = qualification_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match DirBuilder::new().mode(0o700).create(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("create private directory: {error}"),
        }
        fs::set_permissions(&current, fs::Permissions::from_mode(0o700))
            .expect("set private directory mode");
    }
}

fn required_canonical_directory(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required"));
    let path = PathBuf::from(value);
    let metadata = fs::symlink_metadata(&path).expect("qualification path metadata");
    assert!(!metadata.file_type().is_symlink());
    assert!(metadata.is_dir());
    fs::canonicalize(path).expect("canonical qualification path")
}
