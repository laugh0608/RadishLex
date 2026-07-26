use std::fs::{self, DirBuilder, File};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_userdb::UserDb;

use crate::{ProductRelease, UpgradeArtifactIdentity, UpgradeArtifactSlot};

use super::*;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct SwitchFixture {
    container: PathBuf,
    active_path: PathBuf,
    candidate_path: PathBuf,
    backup_path: PathBuf,
    store: UpgradeReceiptStore,
    receipt: UpgradeReceipt,
    source_bytes: Vec<u8>,
    candidate_bytes: Vec<u8>,
}

impl SwitchFixture {
    fn new() -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-upgrade-switch-test-{}-{}",
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
        let active_path = data_root.join(snapshot::USERDB_FILE_NAME);
        File::create(&active_path).expect("schema-zero source");
        fs::set_permissions(&active_path, fs::Permissions::from_mode(0o600))
            .expect("source permissions");

        let source_inspection = UserDb::inspect_file(&active_path).expect("source inspection");
        let source_metadata = fs::metadata(&active_path).expect("source metadata");
        let source_identity = UpgradeArtifactIdentity::private_file(
            UpgradeArtifactSlot::SourceDatabase,
            source_metadata.dev(),
            source_metadata.ino(),
            source_metadata.uid(),
            source_metadata.len(),
        )
        .expect("source identity");
        let mut receipt = UpgradeReceipt::new(
            "abcdef00112233445566778899aabbcc",
            None,
            ProductRelease::new("0.0.9", 34).expect("source release"),
            ProductRelease::new("0.1.0", 35).expect("target release"),
            Some(source_inspection.schema_version),
            UserDb::supported_schema_version(),
            vec![store.data_root_identity().clone(), source_identity],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard");
        store.persist(&guard, &receipt).expect("preflight");
        receipt.advance(UpgradeState::Quiesced).expect("quiesced");
        store.persist(&guard, &receipt).expect("quiesced receipt");
        store
            .create_userdb_snapshot(&guard, &mut receipt, u64::MAX)
            .expect("snapshot");
        store
            .create_userdb_candidate(&guard, &mut receipt)
            .expect("candidate");
        let manager = UpgradeManagerValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            receipt.target_schema_version(),
            1,
            1,
        );
        let input_method = UpgradeInputMethodValidationEvidence::new(
            UPGRADE_VALIDATION_EVIDENCE_VERSION,
            receipt.target_schema_version(),
            1,
            1,
        );
        store
            .record_candidate_validation(
                &guard,
                &mut receipt,
                UpgradeCandidateValidationReport::passed(manager, input_method),
            )
            .expect("candidate validation");
        drop(guard);

        let candidate_path = store.candidate_path();
        let backup_path = store.source_backup_path();
        let source_bytes = fs::read(&active_path).expect("source bytes");
        let candidate_bytes = fs::read(&candidate_path).expect("candidate bytes");
        assert_ne!(source_bytes, candidate_bytes);
        Self {
            container,
            active_path,
            candidate_path,
            backup_path,
            store,
            receipt,
            source_bytes,
            candidate_bytes,
        }
    }

    fn assert_switched_files(&self) {
        assert_eq!(
            fs::read(&self.active_path).expect("active bytes"),
            self.candidate_bytes
        );
        assert_eq!(
            fs::read(&self.backup_path).expect("backup bytes"),
            self.source_bytes
        );
        assert!(!self.candidate_path.exists());
        ensure_no_database_sidecars(&self.active_path).expect("active standalone");
        ensure_no_database_sidecars(&self.backup_path).expect("backup standalone");
    }
}

impl Drop for SwitchFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

struct FailAt(SwitchFaultPoint);

impl SwitchFaultInjector for FailAt {
    fn checkpoint(&self, point: SwitchFaultPoint) -> Result<(), UpgradeFilesystemError> {
        if point == self.0 {
            Err(error(UpgradeFilesystemErrorCode::SwitchFailed))
        } else {
            Ok(())
        }
    }
}

#[test]
fn exact_switch_preserves_old_inode_and_is_idempotent() {
    let mut fixture = SwitchFixture::new();
    let source_inode = fs::metadata(&fixture.active_path)
        .expect("source metadata")
        .ino();
    let candidate_inode = fs::metadata(&fixture.candidate_path)
        .expect("candidate metadata")
        .ino();
    let guard = fixture.store.acquire_guard().expect("guard");

    let summary = fixture
        .store
        .switch_userdb_candidate(&guard, &mut fixture.receipt)
        .expect("switch");
    assert_eq!(summary.disposition(), UpgradeSwitchDisposition::Switched);
    assert_eq!(fixture.receipt.state(), UpgradeState::Switched);
    assert_eq!(
        fs::metadata(&fixture.active_path)
            .expect("active metadata")
            .ino(),
        candidate_inode
    );
    assert_eq!(
        fs::metadata(&fixture.backup_path)
            .expect("backup metadata")
            .ino(),
        source_inode
    );
    fixture.assert_switched_files();

    let replay = fixture
        .store
        .switch_userdb_candidate(&guard, &mut fixture.receipt)
        .expect("idempotent replay");
    assert_eq!(
        replay.disposition(),
        UpgradeSwitchDisposition::AlreadySwitched
    );
    fixture.assert_switched_files();
}

#[test]
fn every_persisted_switch_boundary_recovers_to_the_exact_same_result() {
    let fault_points = [
        SwitchFaultPoint::BackupIdentityPersisted,
        SwitchFaultPoint::SwitchPreparedPersisted,
        SwitchFaultPoint::BackupRenamed,
        SwitchFaultPoint::BeforeBackupDestinationSync,
        SwitchFaultPoint::BackupDestinationSynced,
        SwitchFaultPoint::BeforeBackupSourceSync,
        SwitchFaultPoint::BackupSourceSynced,
        SwitchFaultPoint::CandidateRenamed,
        SwitchFaultPoint::BeforeCandidateDestinationSync,
        SwitchFaultPoint::CandidateDestinationSynced,
        SwitchFaultPoint::BeforeCandidateSourceSync,
        SwitchFaultPoint::CandidateSourceSynced,
        SwitchFaultPoint::SwitchedReceiptPersisted,
    ];

    for point in fault_points {
        let mut fixture = SwitchFixture::new();
        let guard = fixture.store.acquire_guard().expect("guard");
        assert_eq!(
            fixture
                .store
                .switch_userdb_candidate_with_faults(&guard, &mut fixture.receipt, &FailAt(point),)
                .expect_err("fault is injected")
                .code(),
            UpgradeFilesystemErrorCode::SwitchFailed,
            "fault point: {point:?}"
        );
        drop(guard);

        let mut recovered = fixture
            .store
            .load()
            .expect("interrupted scene is recognized")
            .expect("receipt remains");
        let guard = fixture.store.acquire_guard().expect("recovery guard");
        fixture
            .store
            .switch_userdb_candidate(&guard, &mut recovered)
            .expect("recovery completes");
        assert_eq!(recovered.state(), UpgradeState::Switched);
        fixture.assert_switched_files();
    }
}

#[test]
fn sidecars_and_unproven_backup_fail_before_any_path_change() {
    let mut sidecar = SwitchFixture::new();
    fs::write(
        sidecar
            .active_path
            .as_os_str()
            .to_string_lossy()
            .into_owned()
            + "-wal",
        b"unexpected",
    )
    .expect("sidecar");
    let guard = sidecar.store.acquire_guard().expect("guard");
    assert_eq!(
        sidecar
            .store
            .switch_userdb_candidate(&guard, &mut sidecar.receipt)
            .expect_err("sidecar fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InterruptedSwitch
    );
    assert_eq!(sidecar.receipt.state(), UpgradeState::CandidateVerified);
    assert!(sidecar.active_path.exists());
    assert!(sidecar.candidate_path.exists());
    assert!(!sidecar.backup_path.exists());

    let mut unexpected_backup = SwitchFixture::new();
    fs::write(&unexpected_backup.backup_path, b"unproven").expect("backup");
    fs::set_permissions(
        &unexpected_backup.backup_path,
        fs::Permissions::from_mode(0o600),
    )
    .expect("backup mode");
    let guard = unexpected_backup
        .store
        .acquire_guard()
        .expect("backup guard");
    assert_eq!(
        unexpected_backup
            .store
            .switch_userdb_candidate(&guard, &mut unexpected_backup.receipt)
            .expect_err("unproven backup fails closed")
            .code(),
        UpgradeFilesystemErrorCode::InvalidSwitchState
    );
    assert_eq!(
        unexpected_backup.receipt.state(),
        UpgradeState::CandidateVerified
    );
}

#[test]
fn missing_or_replaced_database_identity_never_advances_the_receipt() {
    let mut missing = SwitchFixture::new();
    fs::remove_file(&missing.active_path).expect("remove synthetic source");
    let guard = missing.store.acquire_guard().expect("guard");
    assert!(matches!(
        missing
            .store
            .switch_userdb_candidate(&guard, &mut missing.receipt)
            .expect_err("missing source"),
        UpgradeFilesystemError { .. }
    ));
    assert_eq!(missing.receipt.state(), UpgradeState::CandidateVerified);
    assert!(!missing.backup_path.exists());

    let mut replaced = SwitchFixture::new();
    fs::remove_file(&replaced.candidate_path).expect("remove synthetic candidate");
    fs::write(&replaced.candidate_path, &replaced.candidate_bytes).expect("replacement candidate");
    fs::set_permissions(&replaced.candidate_path, fs::Permissions::from_mode(0o600))
        .expect("replacement mode");
    let guard = replaced.store.acquire_guard().expect("replacement guard");
    assert!(matches!(
        replaced
            .store
            .switch_userdb_candidate(&guard, &mut replaced.receipt)
            .expect_err("replacement fails"),
        UpgradeFilesystemError { .. }
    ));
    assert_eq!(replaced.receipt.state(), UpgradeState::CandidateVerified);
    assert!(replaced.active_path.exists());
    assert!(!replaced.backup_path.exists());
}
