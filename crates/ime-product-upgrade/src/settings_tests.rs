use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::UserDb;

use crate::{ProductRelease, UpgradeState};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

const SYNTHETIC_SETTINGS: &[u8] =
    b"{\"format\":\"radishlex-manager-settings-v1\",\"privacy_mode\":false}\n";

struct SettingsFixture {
    container: PathBuf,
    settings_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
}

impl SettingsFixture {
    fn new() -> Self {
        let temp_root = fs::canonicalize(std::env::temp_dir()).expect("temp root canonicalizes");
        let container = temp_root.join(format!(
            "radishlex-upgrade-settings-test-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&container)
            .expect("test container is created");
        let data_root = container.join("RadishLex");
        DirBuilder::new()
            .mode(0o700)
            .create(&data_root)
            .expect("data root is created");
        let owner_id = fs::metadata(&data_root).expect("data root metadata").uid();
        let root = VerifiedDataRoot::verify(&data_root, owner_id).expect("data root verifies");
        let store = UpgradeReceiptStore::open(root).expect("receipt store opens");
        let userdb_path = data_root.join("userdb.sqlite3");
        drop(UserDb::open(&userdb_path).expect("source userdb opens"));
        let userdb_metadata = fs::metadata(&userdb_path).expect("source userdb metadata");
        let userdb_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            userdb_metadata.dev(),
            userdb_metadata.ino(),
            userdb_metadata.uid(),
            userdb_metadata.len(),
        )
        .expect("source userdb identity");
        let settings_path = data_root.join(SETTINGS_FILE_NAME);
        fs::write(&settings_path, SYNTHETIC_SETTINGS).expect("synthetic settings are written");
        fs::set_permissions(&settings_path, fs::Permissions::from_mode(0o600))
            .expect("settings mode is private");
        let settings_metadata = fs::metadata(&settings_path).expect("settings metadata");
        let settings_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceSettings,
            settings_metadata.dev(),
            settings_metadata.ino(),
            settings_metadata.uid(),
            settings_metadata.len(),
        )
        .expect("settings identity");
        let mut receipt = UpgradeReceipt::new(
            "22334455667788990011aabbccddeeff",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(UserDb::supported_schema_version()),
            UserDb::supported_schema_version(),
            vec![
                store.data_root_identity().clone(),
                userdb_identity,
                settings_identity,
            ],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard is acquired");
        store.persist(&guard, &receipt).expect("preflight persists");
        receipt
            .advance(UpgradeState::Quiesced)
            .expect("receipt advances to quiesced");
        store.persist(&guard, &receipt).expect("quiesced persists");
        drop(guard);
        Self {
            container,
            settings_path,
            store,
            receipt,
        }
    }
}

impl Drop for SettingsFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

struct FailAt(SettingsFaultPoint);

impl SettingsFaultInjector for FailAt {
    fn checkpoint(&self, point: SettingsFaultPoint) -> Result<(), UpgradeFilesystemError> {
        if point == self.0 {
            Err(error(UpgradeFilesystemErrorCode::SettingsBackupFailed))
        } else {
            Ok(())
        }
    }
}

#[test]
fn settings_are_preserved_exactly_and_receipt_records_backup_identity() {
    let mut fixture = SettingsFixture::new();
    let source_before = fs::read(&fixture.settings_path).expect("settings bytes are read");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    let summary = fixture
        .store
        .create_settings_backup(&guard, &mut fixture.receipt)
        .expect("settings backup succeeds");

    assert_eq!(fixture.receipt.state(), UpgradeState::Quiesced);
    assert_eq!(
        summary.backup_identity().slot(),
        UpgradeArtifactSlot::BackupSettings
    );
    assert_eq!(summary.byte_len(), source_before.len() as u64);
    assert_eq!(
        fs::read(fixture.store.settings_backup_path()).expect("backup bytes are read"),
        source_before
    );
    assert_eq!(
        fs::read(&fixture.settings_path).expect("source settings are read again"),
        source_before
    );
    drop(guard);
    assert_eq!(
        fixture.store.load().expect("stored receipt loads"),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn snapshot_cannot_advance_while_recorded_settings_lack_a_backup() {
    let mut fixture = SettingsFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_userdb_snapshot(&guard, &mut fixture.receipt, u64::MAX)
            .expect_err("snapshot without settings backup is rejected")
            .code(),
        UpgradeFilesystemErrorCode::InvalidSnapshotState
    );
    assert!(!fixture.store.staged_snapshot_path().exists());
    assert!(!fixture.store.snapshot_path().exists());
}

#[test]
fn settings_evidence_allows_snapshot_to_advance_to_snapshot_ready() {
    let mut fixture = SettingsFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_settings_backup(&guard, &mut fixture.receipt)
        .expect("settings backup succeeds");
    fixture
        .store
        .create_userdb_snapshot(&guard, &mut fixture.receipt, u64::MAX)
        .expect("snapshot succeeds after settings evidence");

    assert_eq!(fixture.receipt.state(), UpgradeState::SnapshotReady);
    assert!(fixture
        .receipt
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::BackupSettings));
    assert!(fixture
        .receipt
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::SnapshotDatabase));
}

#[test]
fn copied_settings_fault_preserves_staged_file_and_load_fails_closed() {
    let mut fixture = SettingsFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_settings_backup_with_faults(
            &guard,
            &mut fixture.receipt,
            &FailAt(SettingsFaultPoint::SettingsCopied),
        )
        .expect_err("post-copy fault is injected");
    assert!(fixture.store.staged_settings_backup_path().exists());
    assert!(!fixture.store.settings_backup_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("staged settings backup fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedSettingsBackup
    );
}

#[test]
fn renamed_settings_without_receipt_evidence_fail_closed() {
    let mut fixture = SettingsFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_settings_backup_with_faults(
            &guard,
            &mut fixture.receipt,
            &FailAt(SettingsFaultPoint::BackupRenamed),
        )
        .expect_err("post-rename fault is injected");
    assert!(!fixture.store.staged_settings_backup_path().exists());
    assert!(fixture.store.settings_backup_path().exists());
    drop(guard);
    assert_eq!(
        fixture
            .store
            .load()
            .expect_err("unrecorded settings backup fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedSettingsBackup
    );
}

#[test]
fn persisted_settings_evidence_loads_without_advancing_state() {
    let mut fixture = SettingsFixture::new();
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    fixture
        .store
        .create_settings_backup_with_faults(
            &guard,
            &mut fixture.receipt,
            &FailAt(SettingsFaultPoint::ReceiptEvidencePersisted),
        )
        .expect_err("post-evidence fault is injected");
    drop(guard);
    let stored = fixture
        .store
        .load()
        .expect("evidence-bearing receipt loads")
        .expect("receipt exists");
    assert_eq!(stored.state(), UpgradeState::Quiesced);
    assert!(stored
        .artifacts()
        .iter()
        .any(|artifact| artifact.slot() == UpgradeArtifactSlot::BackupSettings));
}

#[test]
fn replaced_settings_identity_is_rejected_before_backup_creation() {
    let mut fixture = SettingsFixture::new();
    let preserved = fixture.container.join("preserved-settings.json");
    fs::rename(&fixture.settings_path, &preserved).expect("recorded settings are preserved");
    fs::copy(&preserved, &fixture.settings_path).expect("replacement settings are copied");
    fs::set_permissions(&fixture.settings_path, fs::Permissions::from_mode(0o600))
        .expect("replacement mode is private");
    let guard = fixture.store.acquire_guard().expect("guard is acquired");

    assert_eq!(
        fixture
            .store
            .create_settings_backup(&guard, &mut fixture.receipt)
            .expect_err("replacement settings identity is rejected")
            .code(),
        UpgradeFilesystemErrorCode::IdentityChanged
    );
    assert!(!fixture.store.staged_settings_backup_path().exists());
    assert!(!fixture.store.settings_backup_path().exists());
}
