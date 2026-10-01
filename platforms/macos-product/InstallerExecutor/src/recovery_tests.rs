use super::*;
use radishlex_ime_product_upgrade::{
    UpgradeCoordinatorError, UpgradeFilesystemErrorCode, VerifiedDataRoot,
};
use radishlex_ime_userdb::SelectionEventDraft;
use radishlex_macos_product_install::RecoveryEvidenceError;
use std::process::Command;

const CHILD_HOME: &str = "RADISHLEX_RECOVERY_TEST_HOME";

#[test]
#[ignore = "invoked by a parent test with its own private synthetic home"]
fn committed_wal_writer() {
    let root = PathBuf::from(std::env::var_os(CHILD_HOME).expect("synthetic home"));
    assert_eq!(
        root.parent(),
        Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
    );
    assert_eq!(fs::canonicalize(&root).unwrap(), root);
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-installer-executor-"));
    assert_eq!(
        fs::read(root.join("recovery-writer.marker")).unwrap(),
        b"synthetic-only"
    );
    let path = root.join("Library/Application Support/RadishLex/userdb.sqlite3");
    assert_eq!(fs::canonicalize(&path).unwrap(), path);
    let mut db = UserDb::open(path).unwrap();
    assert_eq!(db.selection_event_count().unwrap(), 0);
    db.record_selection(SelectionEventDraft::new("recovery-test", "shi", "时", 0, 1))
        .unwrap();
    db.record_selection(SelectionEventDraft::new(
        "recovery-test",
        "shanchu",
        "删除测试",
        0,
        1,
    ))
    .unwrap();
    db.delete_term("shanchu", "删除测试", None).unwrap();
    std::process::exit(0);
}

fn paused() -> Fixture {
    let mut fixture = Fixture::new(product("1.0.0", 1, 0));
    fixture.begin_and_confirm(
        InstallerProductSituation::NotInstalled,
        InstallerAction::BeginFirstInstall,
    );
    fixture.programs.target_product = product("2.0.0", 2, 1);
    prepare_upgrade_data(&fixture.data_root);
    fs::write(
        fixture.root.join("recovery-writer.marker"),
        b"synthetic-only",
    )
    .unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::recovery_tests::committed_wal_writer",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_HOME, &fixture.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "writer: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let begin = fixture.authorize(
        InstallerProductSituation::OlderReleaseInstalled,
        InstallerAction::BeginUpgrade,
    );
    fixture
        .execute_with_upgrade_bootstrap(begin, &mut TestUpgradePort)
        .unwrap();
    let confirm = fixture.authorize(
        InstallerProductSituation::OlderReleaseInstalled,
        InstallerAction::ConfirmQuiescence,
    );
    assert_eq!(
        fixture.execute_with_upgrade_bootstrap(confirm, &mut TestUpgradePort),
        Err(InstallerExecutionError::DataCoordination(
            InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::Filesystem(
                UpgradeFilesystemErrorCode::InterruptedSwitch
            ))
        ))
    );
    assert_eq!(fixture.receipt().state(), InstallState::DataCoordinating);
    assert_eq!(inner(&fixture).1.state(), UpgradeState::CandidateVerified);
    fixture
}

fn inner(fixture: &Fixture) -> (UpgradeReceiptStore, UpgradeReceipt) {
    let store = UpgradeReceiptStore::open_existing(
        VerifiedDataRoot::verify(&fixture.data_root, fixture.owner_id).unwrap(),
    )
    .unwrap();
    let receipt = store.load_for_recovery_inspection().unwrap().unwrap();
    (store, receipt)
}

fn view(fixture: &Fixture) -> radishlex_macos_installer_driver::InstallerViewSnapshot {
    inspect_installer_view_with_recovery(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::IdentityUnavailable,
    )
}

fn intent(fixture: &Fixture) -> AuthorizedInstallerIntent {
    authorize_installer_action(
        view(fixture),
        InstallerAction::AbortPreSwitchUpgrade,
        InstallerUserAuthorization {
            explicit_action_confirmed: true,
            data_retention_acknowledged: true,
            neutral_input_source_selected: true,
            manager_closed: true,
        },
    )
    .unwrap()
}

fn evidence_path(fixture: &Fixture) -> PathBuf {
    fixture
        .data_root
        .join(".radishlex-pre-switch-recovery-v1")
        .join(fixture.receipt().operation_id())
        .join("evidence.json")
}

#[test]
fn recovery_action_preserves_committed_wal_and_both_receipt_contracts() {
    let mut fixture = paused();
    assert!(view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
    assert!(!evidence_path(&fixture).exists());
    let db_path = fixture.data_root.join("userdb.sqlite3");
    let wal_path = fixture.data_root.join("userdb.sqlite3-wal");
    let db_before = fs::read(&db_path).unwrap();
    let wal_before = fs::read(&wal_path).unwrap();
    let db_inode = fs::metadata(&db_path).unwrap().ino();
    let source_inodes: Vec<_> = [
        radishlex_ime_product_install::InstallArtifactSlot::SourceManager,
        radishlex_ime_product_install::InstallArtifactSlot::SourceInputMethod,
    ]
    .iter()
    .map(|slot| {
        fixture
            .receipt()
            .artifact(*slot)
            .unwrap()
            .filesystem_identity()
            .inode()
    })
    .collect();
    let id = fixture.receipt().operation_id().to_owned();
    let action = intent(&fixture);
    let result = fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .unwrap();
    assert_eq!(result.state(), InstallState::RolledBack);
    assert_eq!(fixture.receipt().operation_id(), id);
    assert_eq!(inner(&fixture).1.state(), UpgradeState::AbortedPreserved);
    assert_eq!(fixture.programs.recovery_checks, 5);
    for (index, component) in [ProgramComponent::Manager, ProgramComponent::InputMethod]
        .iter()
        .enumerate()
    {
        assert_eq!(
            fs::metadata(fixture.programs.target_path(*component))
                .unwrap()
                .ino(),
            source_inodes[index]
        );
    }
    assert_eq!(fs::metadata(&db_path).unwrap().ino(), db_inode);
    assert_eq!(fs::read(&db_path).unwrap(), db_before);
    assert_eq!(fs::read(&wal_path).unwrap(), wal_before);
    assert!(evidence_path(&fixture).is_file());
    assert!(radishlex_ime_product_upgrade::inspect_startup_gate(
        &fixture.data_root,
        fixture.owner_id
    )
    .is_allowed());
    let db = UserDb::open_read_only_current(db_path).unwrap();
    assert_eq!(db.selection_event_count().unwrap(), 2);
    assert_eq!(db.list_active_terms().unwrap()[0].text, "时");
    assert_eq!(
        db.list_deleted_term_tombstones().unwrap()[0].text,
        "删除测试"
    );
}

#[test]
fn every_restore_boundary_reloads_the_same_evidence_and_rejects_ordinary_resume() {
    for reject in 2..=5 {
        let mut fixture = paused();
        let ordinary = fixture.authorize(
            InstallerProductSituation::IdentityUnavailable,
            InstallerAction::ResumeOperation,
        );
        let action = intent(&fixture);
        fixture.programs.reject_recovery_check = Some(reject);
        assert_eq!(
            fixture.execute_with_upgrade_bootstrap(action, &mut TestUpgradePort),
            Err(InstallerExecutionError::PreflightNotProven)
        );
        assert_eq!(inner(&fixture).1.state(), UpgradeState::AbortedPreserved);
        assert_eq!(
            fixture.receipt().state(),
            if reject == 5 {
                InstallState::ProgramsRestored
            } else {
                InstallState::RollbackRequired
            }
        );
        let evidence = fs::read(evidence_path(&fixture)).unwrap();
        assert_eq!(
            view(&fixture).primary_action(),
            InstallerAction::AbortPreSwitchUpgrade
        );
        assert!(!view(&fixture).offers(InstallerAction::ResumeOperation));
        assert_eq!(
            fixture.execute_with_upgrade_bootstrap(ordinary, &mut TestUpgradePort),
            Err(InstallerExecutionError::InvalidIntent)
        );
        fixture.programs.reject_recovery_check = None;
        let fresh = intent(&fixture);
        fixture
            .execute_with_upgrade_bootstrap(fresh, &mut TestUpgradePort)
            .unwrap();
        assert_eq!(fixture.receipt().state(), InstallState::RolledBack);
        assert_eq!(fs::read(evidence_path(&fixture)).unwrap(), evidence);
    }
}

#[test]
fn missing_recovery_evidence_cannot_fall_back_to_ordinary_resume() {
    for reject in 2..=5 {
        let mut fixture = paused();
        let ordinary = fixture.authorize(
            InstallerProductSituation::IdentityUnavailable,
            InstallerAction::ResumeOperation,
        );
        let action = intent(&fixture);
        fixture.programs.reject_recovery_check = Some(reject);
        assert!(fixture
            .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
            .is_err());
        let receipt = fixture.receipt();
        let path = evidence_path(&fixture);
        // Move only this test's private evidence, simulating complete evidence loss.
        fs::rename(path.parent().unwrap(), fixture.root.join("saved-evidence")).unwrap();
        assert!(!view(&fixture).offers(InstallerAction::ResumeOperation));
        assert!(!view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
        fixture.programs.reject_recovery_check = None;
        assert!(fixture
            .execute_with_upgrade_bootstrap(ordinary, &mut TestUpgradePort)
            .is_err());
        assert!(fixture
            .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
            .is_err());
        assert_eq!(fixture.receipt(), receipt);
        assert!(!path.exists());
    }
}

#[test]
fn recovery_authorization_cannot_cross_roots_or_target_releases() {
    let first = paused();
    let action = intent(&first);
    let mut other = paused();
    let before = other.receipt();
    assert_eq!(
        other.execute_with_upgrade_bootstrap(action, &mut TestUpgradePort),
        Err(InstallerExecutionError::InvalidIntent)
    );
    let fresh = intent(&other);
    other.programs.target_product = product("3.0.0", 3, 1);
    assert_eq!(
        other.execute_with_upgrade_bootstrap(fresh, &mut TestUpgradePort),
        Err(InstallerExecutionError::InvalidIntent)
    );
    assert_eq!(other.receipt(), before);
    assert!(!evidence_path(&other).exists());
}

#[test]
fn guards_preflight_and_stale_action_fail_before_a_recovery_baseline() {
    let mut fixture = paused();
    let action = intent(&fixture);
    let outer_guard = fixture.store.acquire_guard().unwrap();
    assert!(fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .is_err());
    drop(outer_guard);
    let (store, mut data) = inner(&fixture);
    let guard = store.acquire_guard().unwrap();
    assert!(!view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
    assert!(fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .is_err());
    drop(guard);
    fixture
        .preflight
        .results
        .push_back(Err(InstallerExecutionError::PreflightNotProven));
    assert_eq!(
        fixture.execute_with_upgrade_bootstrap(action, &mut TestUpgradePort),
        Err(InstallerExecutionError::PreflightNotProven)
    );
    assert!(!evidence_path(&fixture).exists());
    let guard = store.acquire_guard().unwrap();
    data.abort_preserved(
        radishlex_ime_product_upgrade::UpgradeFailureCode::SnapshotFailed,
        false,
    )
    .unwrap();
    store.persist(&guard, &data).unwrap();
    drop(guard);
    assert!(!view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
    assert!(fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .is_err());
    assert!(!evidence_path(&fixture).exists());
}

#[test]
fn preservation_baseline_rejects_content_identity_mode_and_sidecar_drift() {
    for path in [
        "userdb.sqlite3",
        "userdb.sqlite3-wal",
        "userdb.sqlite3-shm",
        "manager-settings.json",
        ".radishlex-upgrade-v1/source-snapshot.sqlite3",
        ".radishlex-upgrade-v1/migration-candidate.sqlite3",
        ".radishlex-upgrade-v1/source-settings.json",
    ] {
        let mut fixture = paused();
        fixture.programs.reject_recovery_check = Some(2);
        let action = intent(&fixture);
        fixture
            .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
            .unwrap_err();
        fixture.programs.reject_recovery_check = None;
        let full = fixture.data_root.join(path);
        let mut bytes = fs::read(&full).unwrap();
        bytes[0] ^= 1;
        fs::write(&full, bytes).unwrap();
        let outer_before = fixture.receipt().encode().unwrap();
        assert_eq!(
            fixture.execute_with_upgrade_bootstrap(action, &mut TestUpgradePort),
            Err(InstallerExecutionError::RecoveryEvidence(
                RecoveryEvidenceError::DataChanged
            )),
            "{path}"
        );
        assert_eq!(fixture.receipt().encode().unwrap(), outer_before);
    }
    for mutation in [
        "replace",
        "symlink",
        "mode",
        "new_sidecar",
        "remove_wal",
        "hardlink",
    ] {
        let mut fixture = paused();
        fixture.programs.reject_recovery_check = Some(2);
        let action = intent(&fixture);
        fixture
            .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
            .unwrap_err();
        fixture.programs.reject_recovery_check = None;
        let wal = fixture.data_root.join("userdb.sqlite3-wal");
        match mutation {
            "replace" | "symlink" => {
                let old = fixture.root.join("original-wal");
                fs::rename(&wal, &old).unwrap();
                if mutation == "replace" {
                    fs::copy(&old, &wal).unwrap();
                } else {
                    std::os::unix::fs::symlink(&old, &wal).unwrap();
                }
            }
            "mode" => fs::set_permissions(&wal, fs::Permissions::from_mode(0o644)).unwrap(),
            "new_sidecar" => {
                fs::write(
                    fixture.data_root.join("userdb.sqlite3-journal"),
                    b"synthetic",
                )
                .unwrap();
            }
            "remove_wal" => {
                fs::rename(&wal, fixture.root.join("original-wal")).unwrap();
            }
            "hardlink" => fs::hard_link(&wal, fixture.root.join("alias-wal")).unwrap(),
            _ => unreachable!(),
        }
        let before = fixture.receipt().encode().unwrap();
        assert!(
            fixture
                .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
                .is_err(),
            "{mutation}"
        );
        assert_eq!(fixture.receipt().encode().unwrap(), before);
    }
}

#[test]
fn incomplete_evidence_is_retained_and_never_recreated() {
    let mut fixture = paused();
    let action = intent(&fixture);
    fixture.programs.reject_recovery_check = Some(2);
    fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .unwrap_err();
    let path = evidence_path(&fixture);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    fixture.programs.reject_recovery_check = None;
    assert!(!view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
    assert!(fixture
        .execute_with_upgrade_bootstrap(action, &mut TestUpgradePort)
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes[..bytes.len() / 2]);
}

#[test]
fn recovery_inspection_does_not_create_a_data_state_or_proof_directory() {
    let fixture = Fixture::new(product("1.0.0", 1, 0));
    let before = fs::read_dir(&fixture.data_root).unwrap().count();
    assert!(!view(&fixture).offers(InstallerAction::AbortPreSwitchUpgrade));
    assert_eq!(fs::read_dir(&fixture.data_root).unwrap().count(), before);
    assert!(!fixture.data_root.join(".radishlex-upgrade-v1").exists());
    assert!(!fixture
        .data_root
        .join(".radishlex-pre-switch-recovery-v1")
        .exists());
}
