//! Synthetic SQLite lifecycle diagnosis; no installed bundles or user paths.

use std::process::Command;

use radishlex_ime_product_upgrade::{
    inspect_startup_gate, UpgradeCoordinatorError, UpgradeFailureCode,
};
use radishlex_ime_userdb::SelectionEventDraft;

use super::*;

const CHILD_HOME: &str = "RADISHLEX_TEST_WAL_HOME";
const CHILD_MODE: &str = "RADISHLEX_TEST_WAL_MODE";
const CHILD_TEST: &str = "tests::wal_residue_tests::wal_writer_child";
const CHILD_MARKER: &str = "synthetic-wal-writer-only";

#[test]
#[ignore = "invoked only in a child process with a private synthetic home"]
fn wal_writer_child() {
    let home = PathBuf::from(std::env::var_os(CHILD_HOME).expect("parent-created test home"));
    let temp = fs::canonicalize(std::env::temp_dir()).expect("temporary root");
    assert_eq!(home.parent(), Some(temp.as_path()));
    assert_eq!(fs::canonicalize(&home).expect("canonical test home"), home);
    assert!(home
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-install-data-coordination-"));
    assert_eq!(
        fs::read(home.join(CHILD_MARKER)).unwrap(),
        CHILD_MARKER.as_bytes()
    );
    let database = home.join("Library/Application Support/RadishLex/userdb.sqlite3");
    assert_eq!(fs::canonicalize(&database).unwrap(), database);
    let mut db = UserDb::open(&database).expect("existing synthetic database");
    assert_eq!(db.selection_event_count().unwrap(), 0);
    db.record_selection(SelectionEventDraft::new("synthetic-wal", "shi", "时", 0, 1))
        .expect("first committed event");
    db.record_selection(SelectionEventDraft::new(
        "synthetic-wal",
        "shanchu",
        "删除测试",
        0,
        1,
    ))
    .expect("second committed event");
    db.delete_term("shanchu", "删除测试", None)
        .expect("tombstone");
    assert_eq!(db.selection_event_count().unwrap(), 2);
    match std::env::var(CHILD_MODE).unwrap().as_str() {
        "clean" => drop(db),
        // Exit only this test subprocess, bypassing SQLite connection Drop.
        // This leaves a real committed WAL without killing a user's process.
        "residue" => std::process::exit(0),
        _ => panic!("unknown synthetic writer mode"),
    }
}

fn seed_source(database: &Path, mode: &str) {
    drop(UserDb::open(database).expect("initialize schema, then close normally"));
    let before = fs::read(database).expect("initial main database");
    let home = database.ancestors().nth(4).expect("fixture home");
    fs::write(home.join(CHILD_MARKER), CHILD_MARKER).expect("child marker");
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
        .env(CHILD_HOME, home)
        .env(CHILD_MODE, mode)
        .output()
        .expect("run synthetic writer child");
    assert!(
        output.status.success(),
        "writer failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let wal = database.with_file_name("userdb.sqlite3-wal");
    if mode == "residue" {
        assert!(fs::metadata(&wal).expect("real WAL remains").len() > 32);
        assert_eq!(
            fs::read(database).unwrap(),
            before,
            "commits remain only in WAL"
        );
    } else {
        assert!(!wal.exists(), "normal close checkpoints and removes WAL");
        assert_ne!(fs::read(database).unwrap(), before);
    }
}

fn assert_learning_and_deletion(path: &Path) {
    let db = UserDb::open_read_only_current(path).expect("read synthetic database");
    assert_eq!(db.selection_event_count().unwrap(), 2);
    let active = db.list_active_terms().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].text, "时");
    let deleted = db.list_deleted_term_tombstones().unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].text, "删除测试");
}

fn pause_with_valid_wal() -> CoordinationFixture {
    let mut fixture =
        CoordinationFixture::with_source_database(|path| seed_source(path, "residue"));
    let source = fixture.data_root.join("userdb.sqlite3");
    let wal = fixture.data_root.join("userdb.sqlite3-wal");
    let main_before = fs::read(&source).unwrap();
    let wal_before = fs::read(&wal).unwrap();
    let error = fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect_err("valid leftover WAL prevents the current standalone switch");
    assert_eq!(
        error,
        InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::Filesystem(
            UpgradeFilesystemErrorCode::InterruptedSwitch
        ))
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::CandidateVerified
    );
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::read(&wal).unwrap(), wal_before);
    assert!(!fixture
        .data_root
        .join(".radishlex-upgrade-v1/source-backup.sqlite3")
        .exists());
    assert_learning_and_deletion(&source);
    for name in ["source-snapshot.sqlite3", "migration-candidate.sqlite3"] {
        assert_learning_and_deletion(&fixture.data_root.join(".radishlex-upgrade-v1").join(name));
    }
    fixture
}

#[test]
fn valid_committed_wal_reproduces_nonterminal_upgrade_without_losing_data() {
    let fixture = pause_with_valid_wal();
    assert_eq!(
        fixture.upgrade_store.load().unwrap().as_ref(),
        Some(&fixture.upgrade_receipt)
    );
}

#[test]
fn normally_closed_wal_source_recreates_sidecars_before_the_standalone_switch() {
    let mut fixture = CoordinationFixture::with_source_database(|path| seed_source(path, "clean"));
    let source = fixture.data_root.join("userdb.sqlite3");
    let wal = fixture.data_root.join("userdb.sqlite3-wal");
    let main_before = fs::read(&source).unwrap();
    assert!(!wal.exists());
    let error = fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect_err("read-only snapshot of persistent WAL mode can recreate sidecars");
    assert_eq!(
        error,
        InstallDataCoordinationError::Upgrade(UpgradeCoordinatorError::Filesystem(
            UpgradeFilesystemErrorCode::InterruptedSwitch
        ))
    );
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::CandidateVerified
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::metadata(wal).expect("empty WAL was recreated").len(), 0);
    assert!(fixture.data_root.join("userdb.sqlite3-shm").exists());
    assert_learning_and_deletion(&source);
}

#[test]
fn explicit_pre_switch_abort_restores_source_programs_and_preserves_valid_wal() {
    let mut fixture = pause_with_valid_wal();
    let source = fixture.data_root.join("userdb.sqlite3");
    let wal = fixture.data_root.join("userdb.sqlite3-wal");
    let main_before = fs::read(&source).unwrap();
    let wal_before = fs::read(&wal).unwrap();
    let install_guard = fixture
        .install_store
        .acquire_guard()
        .expect("outer abort guard");
    let guard = fixture
        .upgrade_store
        .acquire_guard()
        .expect("synthetic abort guard");
    fixture
        .install_store
        .verify_current(&install_guard, &fixture.install_receipt)
        .expect("same current outer receipt");
    fixture
        .upgrade_store
        .verify_current(&guard, &fixture.upgrade_receipt)
        .expect("same current data receipt and unswitched files");
    fixture
        .upgrade_receipt
        .abort_preserved(UpgradeFailureCode::SwitchFailed, false)
        .expect("explicit abort is legal before switch");
    fixture
        .upgrade_store
        .persist(&guard, &fixture.upgrade_receipt)
        .expect("persist abort");
    drop(guard);
    drop(install_guard);
    let summary = fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect("existing coordinator restores only source programs");
    assert_eq!(
        summary.disposition(),
        InstallDataCoordinationDisposition::RolledBack
    );
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    fixture.assert_source_programs_restored();
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::read(&wal).unwrap(), wal_before);
    assert_learning_and_deletion(&source);
    let owner = fs::metadata(&fixture.data_root).unwrap().uid();
    assert!(inspect_startup_gate(&fixture.data_root, owner).is_allowed());
}

#[test]
fn abort_recovery_rechecks_quiescence_and_restored_programs_after_interruption() {
    let mut fixture = pause_with_valid_wal();
    let source = fixture.data_root.join("userdb.sqlite3");
    let wal = fixture.data_root.join("userdb.sqlite3-wal");
    let main_before = fs::read(&source).unwrap();
    let wal_before = fs::read(&wal).unwrap();
    let install_guard = fixture.install_store.acquire_guard().unwrap();
    let upgrade_guard = fixture.upgrade_store.acquire_guard().unwrap();
    fixture
        .install_store
        .verify_current(&install_guard, &fixture.install_receipt)
        .unwrap();
    fixture
        .upgrade_store
        .verify_current(&upgrade_guard, &fixture.upgrade_receipt)
        .unwrap();
    fixture
        .upgrade_receipt
        .abort_preserved(UpgradeFailureCode::SwitchFailed, false)
        .unwrap();
    fixture
        .upgrade_store
        .persist(&upgrade_guard, &fixture.upgrade_receipt)
        .unwrap();
    drop(upgrade_guard);
    drop(install_guard);

    let mut busy = TestUpgradePort::successful();
    busy.reject_checkpoint = Some(UpgradeCoordinatorCheckpoint::BeforeRollbackRestore);
    assert!(fixture
        .resume(&mut busy, &mut TestProgramValidation::successful())
        .is_err());
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::DataCoordinating
    );
    assert_ne!(
        fs::metadata(fixture.manager.target_path()).unwrap().ino(),
        fixture.source_manager_inode
    );
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::read(&wal).unwrap(), wal_before);

    let mut unavailable = TestProgramValidation::successful();
    unavailable.restored_valid = false;
    assert_eq!(
        fixture.resume(&mut TestUpgradePort::successful(), &mut unavailable),
        Err(InstallDataCoordinationError::ProgramValidationNotProven(
            InstallProgramValidationStage::RestoredSource
        ))
    );
    assert_eq!(
        fixture.install_receipt.state(),
        InstallState::RollbackRequired
    );
    fixture.assert_source_programs_restored();

    fixture.install_receipt = fixture.install_store.load().unwrap().unwrap();
    fixture.upgrade_receipt = fixture.upgrade_store.load().unwrap().unwrap();
    fixture
        .resume(
            &mut TestUpgradePort::successful(),
            &mut TestProgramValidation::successful(),
        )
        .expect("same operation resumes only after source program revalidation");
    assert_eq!(fixture.install_receipt.state(), InstallState::RolledBack);
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::AbortedPreserved
    );
    assert_eq!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::read(&wal).unwrap(), wal_before);
    assert_learning_and_deletion(&source);
}

#[test]
fn source_inode_drift_blocks_receipt_rebinding_with_valid_wal() {
    let fixture = pause_with_valid_wal();
    let source = fixture.data_root.join("userdb.sqlite3");
    let original = fixture.container.join("preserved-original.sqlite3");
    let receipt_path = fixture.data_root.join(".radishlex-upgrade-v1/receipt.json");
    let receipt_before = fs::read(&receipt_path).unwrap();
    fs::rename(&source, &original).unwrap();
    fs::copy(&original, &source).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    assert_ne!(
        fixture.active_database_inode(),
        fixture.source_database_inode
    );
    let guard = fixture.upgrade_store.acquire_guard().unwrap();
    assert!(fixture
        .upgrade_store
        .verify_current(&guard, &fixture.upgrade_receipt)
        .is_err());
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_before);
    assert_eq!(
        fixture.upgrade_receipt.state(),
        UpgradeState::CandidateVerified
    );
}
