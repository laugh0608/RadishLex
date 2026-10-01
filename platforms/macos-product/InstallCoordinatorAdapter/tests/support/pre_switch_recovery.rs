use super::*;
use radishlex_ime_product_install::InstallProcessGuard;
use radishlex_macos_product_install::PreSwitchRecoveryEvidence;
use radishlex_macos_product_install_coordinator::{
    abort_pre_switch_install_upgrade, InstallDataCoordinationError, PreSwitchRecoveryCheckpoint,
    PreSwitchRecoveryValidationPort,
};

pub(super) fn preserves_wal_and_restarts_with_exact_source_programs(roots: &QualificationRoots) {
    let mut fixture = ProductFixture::prepared_with_program_commit(
        roots,
        "pre-switch-wal-recovery",
        "9876543210abcdef9876543210abcdef",
        true,
        true,
    );
    let mut upgrade = fixture.upgrade_adapter();
    let outer_guard = fixture.install_store.acquire_guard().unwrap();
    let inner_guard = fixture.upgrade_store.acquire_guard().unwrap();
    assert!(resume_install_data_coordination(
        &fixture.install_store,
        &outer_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &inner_guard,
        &mut fixture.upgrade_receipt,
        u64::MAX,
        &mut upgrade,
        &mut fixture.target_adapter
    )
    .is_err());
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::CandidateVerified
    );
    let db_path = fixture.data_root.join("userdb.sqlite3");
    let wal_path = fixture.data_root.join("userdb.sqlite3-wal");
    let before_main = fs::read(&db_path).unwrap();
    let before_wal = fs::read(&wal_path).unwrap();
    let initial_data = fixture.upgrade_receipt.encode().unwrap();
    let initial_outer = fixture.install_receipt.encode().unwrap();
    let mut validation = ActualPreservation {
        root: &fixture.data_root,
        store: &fixture.install_store,
        guard: &outer_guard,
        adapter: &mut fixture.target_adapter,
        evidence: None,
        initial_data: &initial_data,
        reject: None,
    };
    // A changed source backup must be rejected before abort or baseline creation.
    let foreign = fixture
        .manager
        .source_backup_bundle_path()
        .join("unexpected-recovery-object");
    fs::write(&foreign, b"synthetic qualification drift").unwrap();
    assert_eq!(
        abort_pre_switch_install_upgrade(
            &fixture.install_store,
            &outer_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &inner_guard,
            &mut fixture.upgrade_receipt,
            &mut upgrade,
            &mut validation
        ),
        Err(InstallDataCoordinationError::RecoveryValidationNotProven(
            PreSwitchRecoveryCheckpoint::BeforeAbort
        ))
    );
    assert!(validation.evidence.is_none());
    assert_eq!(fixture.install_receipt.encode().unwrap(), initial_outer);
    assert_eq!(fixture.upgrade_receipt.encode().unwrap(), initial_data);
    fs::remove_file(&foreign).unwrap();

    validation.reject = Some(PreSwitchRecoveryCheckpoint::BeforeInputMethodRestore);
    assert_eq!(
        abort_pre_switch_install_upgrade(
            &fixture.install_store,
            &outer_guard,
            &mut fixture.install_receipt,
            &fixture.manager,
            &fixture.input_method,
            &fixture.upgrade_store,
            &inner_guard,
            &mut fixture.upgrade_receipt,
            &mut upgrade,
            &mut validation
        ),
        Err(InstallDataCoordinationError::RecoveryValidationNotProven(
            PreSwitchRecoveryCheckpoint::BeforeInputMethodRestore
        ))
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::RollbackRequired
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    assert!(!fixture.manager.source_backup_bundle_path().exists());
    assert!(fixture.input_method.source_backup_bundle_path().exists());
    drop(validation);
    drop(inner_guard);
    drop(outer_guard);

    // Reload the actual receipt files and immutable preservation evidence.
    fixture.install_receipt = fixture.install_store.load().unwrap().unwrap();
    fixture.upgrade_receipt = fixture.upgrade_store.load().unwrap().unwrap();
    let outer_guard = fixture.install_store.acquire_guard().unwrap();
    let inner_guard = fixture.upgrade_store.acquire_guard().unwrap();
    let evidence = PreSwitchRecoveryEvidence::load(&fixture.data_root, &fixture.install_receipt)
        .unwrap()
        .unwrap();
    assert_eq!(evidence.initial_data_receipt(), initial_data);
    let mut validation = ActualPreservation {
        root: &fixture.data_root,
        store: &fixture.install_store,
        guard: &outer_guard,
        adapter: &mut fixture.target_adapter,
        evidence: Some(evidence),
        initial_data: &initial_data,
        reject: None,
    };
    let result = abort_pre_switch_install_upgrade(
        &fixture.install_store,
        &outer_guard,
        &mut fixture.install_receipt,
        &fixture.manager,
        &fixture.input_method,
        &fixture.upgrade_store,
        &inner_guard,
        &mut fixture.upgrade_receipt,
        &mut upgrade,
        &mut validation,
    )
    .expect("exact source programs restored after restart");
    assert_eq!(
        result.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    validation
        .evidence
        .as_ref()
        .unwrap()
        .verify_preserved(
            &fixture.install_store,
            &outer_guard,
            &fixture.install_receipt,
        )
        .unwrap();
    assert_eq!(fs::read(&db_path).unwrap(), before_main);
    assert_eq!(fs::read(&wal_path).unwrap(), before_wal);
    drop(validation);
    drop(inner_guard);
    drop(outer_guard);
    assert!(fixture.target_adapter.validate_restored_sources(
        &fixture.manager,
        &fixture.input_method,
        &fixture.install_receipt
    ));
    for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
        assert!(inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(false, component)
        )
        .is_allowed());
        assert!(!inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &fixture.running(true, component)
        )
        .is_allowed());
    }
    assert!(inspect_startup_gate(&fixture.data_root, fixture.owner_id).is_allowed());
    let db = UserDb::open_read_only_current(db_path).unwrap();
    assert_eq!(db.selection_event_count().unwrap(), 2);
    assert_eq!(db.list_active_terms().unwrap()[0].text, "时");
    assert_eq!(
        db.list_deleted_term_tombstones().unwrap()[0].text,
        "删除测试"
    );
}

struct ActualPreservation<'a> {
    root: &'a Path,
    store: &'a InstallReceiptStore,
    guard: &'a InstallProcessGuard,
    adapter: &'a mut MacOsProductInstallAdapter,
    evidence: Option<PreSwitchRecoveryEvidence>,
    initial_data: &'a [u8],
    reject: Option<PreSwitchRecoveryCheckpoint>,
}

impl InstallProgramValidationPort for ActualPreservation<'_> {
    fn validate_installed_targets(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.adapter
            .validate_installed_targets(manager, input_method, receipt)
    }
    fn validate_restored_sources(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> bool {
        self.adapter
            .validate_restored_sources(manager, input_method, receipt)
    }
}

impl PreSwitchRecoveryValidationPort for ActualPreservation<'_> {
    fn validate_recovery_checkpoint(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        checkpoint: PreSwitchRecoveryCheckpoint,
    ) -> bool {
        if self.reject == Some(checkpoint) {
            return false;
        }
        if [manager, input_method].iter().any(|program| {
            self.adapter
                .verify_program_recovery_material(program, receipt)
                .is_err()
        }) {
            return false;
        }
        if self.evidence.is_none() {
            self.evidence = Some(
                PreSwitchRecoveryEvidence::create(
                    self.root,
                    self.store,
                    self.guard,
                    receipt,
                    self.initial_data,
                )
                .expect("private preservation baseline"),
            );
        }
        self.evidence
            .as_ref()
            .unwrap()
            .verify_preserved(self.store, self.guard, receipt)
            .is_ok()
    }
}
