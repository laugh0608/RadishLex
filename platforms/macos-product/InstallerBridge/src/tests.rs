use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_install::{
    InstallFinalizationPort, InstallFinalizationValidationStage, InstallProcessGuard,
    InstallProgramValidationPort, InstallReceipt, InstallState, ProductArtifactIdentity,
    ProductRelease, ProgramBundleIdentity, ProgramComponent, ProgramSwitchStore,
    VerifiedInstallRoot, INSTALL_PRODUCT_ID,
};
use radishlex_ime_product_upgrade::{
    ProductRelease as UpgradeProductRelease, UpgradeCandidateValidationReport,
    UpgradeCoordinatorCheckpoint, UpgradePostSwitchValidationReport,
    UpgradeRollbackValidationEvidence,
};
use radishlex_macos_installer_executor::{InstallerExecutionError, InstallerPreflightEvidence};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TestFixture {
    root: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
    store: InstallReceiptStore,
    programs: PrepareOnlyPrograms,
    preflight: PassingPreflight,
    operation_ids: OneOperationId,
}

impl TestFixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .expect("temporary root")
            .join(format!(
                "radishlex-installer-bridge-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        let data_root = root.join("Library/Application Support/RadishLex");
        create_directory(&data_root, 0o700);
        let owner_id = fs::metadata(&data_root).expect("root metadata").uid();
        let store = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&data_root, owner_id).expect("verified install root"),
        )
        .expect("install store");
        Self {
            root,
            data_root,
            owner_id,
            store,
            programs: PrepareOnlyPrograms {
                target_product: product(),
            },
            preflight: PassingPreflight,
            operation_ids: OneOperationId,
        }
    }

    fn dispatch(
        &mut self,
        action: InstallerAction,
    ) -> Result<InstallerBridgeDispatch, InstallerBridgeError> {
        dispatch_installer_action(
            &self.data_root,
            self.owner_id,
            InstallerProductSituation::NotInstalled,
            action,
            InstallerUserAuthorization {
                explicit_action_confirmed: true,
                data_retention_acknowledged: true,
                neutral_input_source_selected: true,
                manager_closed: true,
            },
            &self.store,
            &mut self.programs,
            &mut self.preflight,
            &mut self.operation_ids,
            &mut UnusedUpgradePort,
        )
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct PrepareOnlyPrograms {
    target_product: ProductArtifactIdentity,
}

impl InstallerProgramPort for PrepareOnlyPrograms {
    fn target_product(&self) -> &ProductArtifactIdentity {
        &self.target_product
    }

    fn open_program_store(
        &self,
        _receipt_store: &InstallReceiptStore,
        _guard: &InstallProcessGuard,
        _component: ProgramComponent,
        _receipt: &InstallReceipt,
    ) -> Result<ProgramSwitchStore, InstallerExecutionError> {
        panic!("prepared bridge action must not open a program store")
    }

    fn verify_and_record_source(
        &self,
        _receipt_store: &InstallReceiptStore,
        _guard: &InstallProcessGuard,
        _program_store: &ProgramSwitchStore,
        _receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        panic!("prepared bridge action must not inspect a program source")
    }

    fn prepare_and_record_staged(
        &self,
        _receipt_store: &InstallReceiptStore,
        _guard: &InstallProcessGuard,
        _program_store: &ProgramSwitchStore,
        _receipt: &mut InstallReceipt,
    ) -> Result<(), InstallerExecutionError> {
        panic!("prepared bridge action must not stage a program")
    }
}

impl InstallFinalizationPort for PrepareOnlyPrograms {
    fn validate_final_state(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
        _stage: InstallFinalizationValidationStage,
    ) -> bool {
        panic!("prepared bridge action must not finalize")
    }
}

impl InstallProgramValidationPort for PrepareOnlyPrograms {
    fn validate_installed_targets(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> bool {
        panic!("prepared bridge action must not validate installed targets")
    }

    fn validate_restored_sources(
        &mut self,
        _manager: &ProgramSwitchStore,
        _input_method: &ProgramSwitchStore,
        _receipt: &InstallReceipt,
    ) -> bool {
        panic!("prepared bridge action must not validate restored sources")
    }
}

struct PassingPreflight;

impl InstallerPreflightPort for PassingPreflight {
    fn inspect_preflight(
        &mut self,
        _target_product: &ProductArtifactIdentity,
    ) -> Result<InstallerPreflightEvidence, InstallerExecutionError> {
        InstallerPreflightEvidence::new(u64::MAX)
    }
}

struct OneOperationId;

impl InstallerOperationIdSource for OneOperationId {
    fn next_operation_id(&mut self) -> Result<String, InstallerExecutionError> {
        Ok("00112233445566778899aabbccddeeff".to_owned())
    }
}

struct UnusedUpgradePort;

impl UpgradeCoordinatorPort for UnusedUpgradePort {
    fn confirm_quiescence(&mut self, _checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        panic!("first-install bridge action must not coordinate an upgrade")
    }

    fn validate_candidate(
        &mut self,
        _target_release: &UpgradeProductRelease,
        _target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        panic!("first-install bridge action must not validate an upgrade candidate")
    }

    fn validate_post_switch(
        &mut self,
        _target_release: &UpgradeProductRelease,
        _target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        panic!("first-install bridge action must not validate a switched database")
    }

    fn validate_restored_source(
        &mut self,
        _source_release: &UpgradeProductRelease,
        _source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        panic!("first-install bridge action must not validate an upgrade rollback")
    }
}

#[test]
fn bridge_reauthorizes_fresh_snapshot_and_recovers_prepared_state() {
    let mut fixture = TestFixture::new();
    let prepared = fixture
        .dispatch(InstallerAction::BeginFirstInstall)
        .expect("prepare through bridge");
    assert!(matches!(
        prepared,
        InstallerBridgeDispatch::Executed(summary) if summary.state() == InstallState::Prepared
    ));

    let restarted = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::NotInstalled,
    );
    assert_eq!(
        restarted.primary_action(),
        InstallerAction::ConfirmQuiescence
    );
    let encoded = encode_installer_snapshot(restarted);
    assert_eq!(encoded.phase, 2);
    assert_eq!(encoded.primary_action, 5);
    assert_eq!(encoded.operation_kind, 1);
    assert_eq!(encoded.receipt_state, 1);
    assert_eq!(encoded.progress_step, 1);
    assert_eq!(encoded.manual_prompt, 1);
    assert!(matches!(
        fixture
            .dispatch(InstallerAction::BeginFirstInstall)
            .expect_err("stale begin must be rejected"),
        InstallerBridgeError::Authorization(InstallerAuthorizationError::ActionNotOffered)
    ));

    let guard = fixture.store.acquire_guard().expect("active install guard");
    let active = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::NotInstalled,
    );
    assert_eq!(active.stable_error(), InstallerStableError::OperationActive);
    assert!(!active.offers(InstallerAction::ConfirmQuiescence));
    drop(guard);
}

#[test]
fn ffi_contract_is_versioned_and_fails_closed_without_release_identity() {
    assert_eq!(
        radishlex_installer_bridge_contract_version(),
        INSTALLER_BRIDGE_CONTRACT_VERSION
    );
    let snapshot = radishlex_installer_bridge_snapshot_v1();
    assert_eq!(snapshot.contract_version, 1);
    assert_eq!(snapshot.phase, 6);
    assert_eq!(snapshot.primary_action, 1);
    assert_eq!(snapshot.stable_error, 10);

    let unknown = radishlex_installer_bridge_perform_v1(99, 0);
    assert_eq!(unknown.phase, 6);
    assert_eq!(unknown.stable_error, 13);
    let unknown_flags = radishlex_installer_bridge_perform_v1(1, 1 << 31);
    assert_eq!(unknown_flags.phase, 6);
    assert_eq!(unknown_flags.stable_error, 13);
    assert_eq!(
        decode_authorization(0b1111),
        Some(InstallerUserAuthorization {
            explicit_action_confirmed: true,
            data_retention_acknowledged: true,
            neutral_input_source_selected: true,
            manager_closed: true,
        })
    );
}

#[test]
fn upgrade_source_selection_uses_outer_receipt_identity() {
    let source = product_at("0.0.9", 34, 'a');
    let target = product_at("0.1.0", 35, 'b');
    let root = TestFixture::new();
    let receipt = InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        radishlex_ime_product_install::InstallOperationKind::Upgrade,
        root.store.root_identity().clone(),
        Some(source.clone()),
        Some(target),
    )
    .expect("upgrade receipt");
    assert_eq!(upgrade_source_release(&receipt), Some(source.release()));
    let selected = Path::new("/manifest-bound/source");
    assert_eq!(
        select_upgrade_source_root(&receipt, |release| {
            assert_eq!(release, source.release());
            Some(selected)
        }),
        Ok(selected)
    );
    assert_eq!(
        select_upgrade_source_root(&receipt, |_| None),
        Err(InstallerStableError::DriverUnavailable)
    );
}

#[test]
fn bootstrap_derives_only_fixed_bundle_and_user_domain_paths() {
    let root = fs::canonicalize(std::env::temp_dir())
        .expect("temporary root")
        .join(format!(
            "radishlex-installer-bootstrap-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
    let home = root.join("home");
    let executable = root.join("RadishLex Installer.app/Contents/MacOS/RadishLex Installer");
    let resources = root.join("RadishLex Installer.app/Contents/Resources");
    create_directory(&home, 0o700);
    create_directory(executable.parent().expect("executable parent"), 0o700);
    create_directory(&resources, 0o700);
    fs::write(&executable, b"synthetic executable").expect("write executable");
    let owner_id = fs::metadata(&home).expect("home metadata").uid();

    let context = InstallerBootstrapContext::discover_from(&executable, owner_id, &home)
        .expect("fixed bootstrap context");
    assert_eq!(context.owner_id(), owner_id);
    assert_eq!(context.user_home(), home);
    assert_eq!(
        context.data_root(),
        home.join("Library/Application Support/RadishLex")
    );
    assert_eq!(context.payload_root(), resources.join("InstallPayload"));
    assert_eq!(
        context.product_root(),
        resources.join("InstallPayload/Product")
    );
    assert_eq!(
        context
            .release_requirements()
            .expect_err("missing release identity"),
        InstallerBootstrapError::ReleaseIdentityUnavailable
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bootstrap_accepts_only_strict_release_identity_resource() {
    let root = fs::canonicalize(std::env::temp_dir())
        .expect("temporary root")
        .join(format!(
            "radishlex-installer-release-identity-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
    let home = root.join("home");
    let executable = root.join("RadishLex Installer.app/Contents/MacOS/RadishLex Installer");
    let resources = root.join("RadishLex Installer.app/Contents/Resources");
    create_directory(&home, 0o700);
    create_directory(executable.parent().expect("executable parent"), 0o700);
    create_directory(&resources, 0o700);
    fs::write(&executable, b"synthetic executable").expect("write executable");
    let owner_id = fs::metadata(&home).expect("home metadata").uid();
    let context = InstallerBootstrapContext::discover_from(&executable, owner_id, &home)
        .expect("fixed bootstrap context");
    let manager_requirement = "cdhash H\"0000000000000000000000000000000000000000\"";
    let input_method_requirement = "cdhash H\"1111111111111111111111111111111111111111\"";
    fs::write(
        resources.join("ReleaseIdentity.json"),
        format!(
            concat!(
                "{{\n",
                "  \"format_version\": 2,\n",
                "  \"distribution_identity\": \"community-adhoc-v1\",\n",
                "  \"manager_designated_requirements\": [{manager:?}],\n",
                "  \"input_method_designated_requirements\": [{input_method:?}]\n",
                "}}\n"
            ),
            manager = manager_requirement,
            input_method = input_method_requirement,
        ),
    )
    .expect("write release identity");
    context
        .unsealed_release_requirements_for_test()
        .expect("strict release identity");
    assert_eq!(
        context
            .release_requirements()
            .expect_err("unsigned synthetic Installer cannot seal the identity"),
        InstallerBootstrapError::ReleaseIdentityUnavailable
    );

    let identity_path = resources.join("ReleaseIdentity.json");
    let identity = fs::read_to_string(&identity_path).expect("read identity");
    fs::write(
        &identity_path,
        identity.replace("community-adhoc-v1", "developer-id-v1"),
    )
    .expect("replace distribution identity");
    assert_eq!(
        context
            .unsealed_release_requirements_for_test()
            .expect_err("foreign distribution identity must fail"),
        InstallerBootstrapError::ReleaseIdentityUnavailable
    );

    fs::write(
        identity_path,
        b"{\"format_version\":2,\"distribution_identity\":\"community-adhoc-v1\",\"manager_designated_requirements\":[\"adhoc\"],\"input_method_designated_requirements\":[\"adhoc\"]}\n",
    )
    .expect("replace release identity");
    assert_eq!(
        context
            .release_requirements()
            .expect_err("malformed ad-hoc requirements must fail"),
        InstallerBootstrapError::ReleaseIdentityUnavailable
    );

    let _ = fs::remove_dir_all(root);
}

fn product() -> ProductArtifactIdentity {
    product_at("1.0.0", 1, '0')
}

fn product_at(version: &str, build: u64, marker: char) -> ProductArtifactIdentity {
    ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new(version, build).expect("release"),
        hash(marker),
        ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            "org.radishlex.manager",
            hash('1'),
            hash('2'),
        )
        .expect("Manager identity"),
        ProgramBundleIdentity::new(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod",
            hash('3'),
            hash('4'),
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
