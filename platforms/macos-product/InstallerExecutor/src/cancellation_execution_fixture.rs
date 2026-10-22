use super::*;
use radishlex_ime_product_install::{
    InstallFailureCode, InstallOperationKind, InstallReceipt, VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::{
    PreparationBinding, PreparationJournalStore, SourcePreparationCheckpoint,
    SourcePreparationPort, UpgradeReceiptStore,
};
use radishlex_ime_userdb::{SelectionEventDraft, UserDb};
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const NEW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub(super) const STATE: &str = ".radishlex-upgrade-v1";

pub(super) struct Fixture {
    pub(super) path: PathBuf,
    pub(super) source: ProductArtifactIdentity,
    pub(super) target: ProductArtifactIdentity,
    pub(super) previous: InstallReceipt,
}
impl Fixture {
    pub(super) fn new(stage: u8, new_outer: bool) -> Self {
        let mut nonce = [0; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-cancellation-execution-{}-{}",
                std::process::id(),
                u64::from_ne_bytes(nonce)
            ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let mut db = UserDb::open(path.join("userdb.sqlite3")).unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-cancel-execution",
            "ci",
            "词",
            0,
            1,
        ))
        .unwrap();
        drop(db);
        let owner = fs::metadata(&path).unwrap().uid();
        let install =
            InstallReceiptStore::open(VerifiedInstallRoot::verify(&path, owner).unwrap()).unwrap();
        let guard = install.acquire_guard().unwrap();
        let source = crate::cancellation_fixture::product(39, 'a');
        let target = crate::cancellation_fixture::product(41, 'b');
        let mut previous = InstallReceipt::new(
            OLD,
            None,
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .unwrap();
        install.persist(&guard, &previous).unwrap();
        previous
            .abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .unwrap();
        install.persist(&guard, &previous).unwrap();
        let history = path.join(".radishlex-install-history-v1");
        let old_dir = history.join(OLD);
        for dir in [&history, &old_dir] {
            DirBuilder::new().mode(0o700).create(dir).unwrap();
        }
        let bytes = previous.encode().unwrap();
        put(&old_dir.join("receipt.json"), &bytes);
        let data =
            UpgradeReceiptStore::open(VerifiedDataRoot::verify(&path, owner).unwrap()).unwrap();
        let inner = data.acquire_guard().unwrap();
        let journal = PreparationJournalStore::attach(data, &inner).unwrap();
        use radishlex_ime_product_upgrade::PreparationHasher;
        let digest = MacOsPreparationHasher
            .sha256(&mut &bytes[..])
            .unwrap()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let prep = PreparationReceipt::new(
            PreparationBinding {
                operation_id: NEW.into(),
                previous_install_operation_id: Some(OLD.into()),
                previous_data_operation_id: None,
                source_release: radishlex_ime_product_upgrade::ProductRelease::new("26.7.1", 39)
                    .unwrap(),
                target_release: radishlex_ime_product_upgrade::ProductRelease::new("26.7.1", 41)
                    .unwrap(),
                source_product_sha256: source.product_manifest_sha256().into(),
                target_product_sha256: target.product_manifest_sha256().into(),
                previous_install_receipt_sha256: Some(digest),
                previous_data_receipt_sha256: None,
                previous_inventory_sha256: None,
                target_schema_version: UserDb::supported_schema_version(),
                data_root: journal.data_root_identity(),
                state_directory: journal.state_directory_identity(),
            },
            journal
                .observe_source_family(&inner, &MacOsPreparationHasher)
                .unwrap(),
        )
        .unwrap();
        journal.persist(&inner, None, &prep).unwrap();
        if stage != 0 {
            let fail = match stage {
                1 => Some(SourcePreparationCheckpoint::SnapshotRecorded),
                2 => Some(SourcePreparationCheckpoint::BeforeMaintenance),
                _ => None,
            };
            let result = journal.prepare_userdb_source(
                &inner,
                &mut Prepare { fail },
                &MacOsPreparationHasher,
            );
            assert_eq!(result.is_ok(), stage >= 3);
            if stage == 4 {
                journal
                    .archive_previous_upgrade(
                        &inner,
                        &mut Prepare { fail: None },
                        &MacOsPreparationHasher,
                    )
                    .unwrap();
            }
        }
        if new_outer {
            let new = InstallReceipt::new(
                NEW,
                Some(OLD.into()),
                InstallOperationKind::Upgrade,
                install.root_identity().clone(),
                Some(source.clone()),
                Some(target.clone()),
            )
            .unwrap();
            install.persist(&guard, &new).unwrap();
        }
        Self {
            path,
            source,
            target,
            previous,
        }
    }
    pub(super) fn begin_handoff(&self) {
        let install = self.install();
        let _outer = install.acquire_guard().unwrap();
        let journal = PreparationJournalStore::open_existing(
            VerifiedDataRoot::verify(&self.path, self.owner()).unwrap(),
        )
        .unwrap();
        let inner = journal.acquire_guard().unwrap();
        assert!(journal
            .handoff_userdb_source(
                &inner,
                NEW,
                &mut Prepare {
                    fail: Some(SourcePreparationCheckpoint::HandoffIntentRecorded)
                },
                &MacOsPreparationHasher
            )
            .is_err());
    }
    pub(super) fn reopen(path: PathBuf) -> Self {
        let source = crate::cancellation_fixture::product(39, 'a');
        let target = crate::cancellation_fixture::product(41, 'b');
        let bytes = fs::read(
            path.join(".radishlex-install-history-v1")
                .join(OLD)
                .join("receipt.json"),
        )
        .unwrap();
        let previous = InstallReceipt::decode(&bytes).unwrap();
        Self {
            path,
            source,
            target,
            previous,
        }
    }
    pub(super) fn install(&self) -> InstallReceiptStore {
        InstallReceiptStore::open_existing(
            VerifiedInstallRoot::verify(&self.path, self.owner()).unwrap(),
        )
        .unwrap()
    }
    pub(super) fn owner(&self) -> u32 {
        fs::metadata(&self.path).unwrap().uid()
    }
    pub(super) fn observe(&self) -> MaintenanceInspection {
        inspect_maintenance(&self.path, self.owner(), &MacOsPreparationHasher)
            .unwrap()
            .unwrap()
    }
    pub(super) fn grant(
        &self,
    ) -> (
        CancellationPreservationAuthorization,
        CancellationTaskControl,
    ) {
        authorize_cancellation_preservation(
            &self.observe(),
            &self.source,
            &self.target,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
struct Prepare {
    fail: Option<SourcePreparationCheckpoint>,
}
impl SourcePreparationPort for Prepare {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationReceipt,
        point: SourcePreparationCheckpoint,
    ) -> bool {
        self.fail != Some(point)
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
pub(super) fn confirmation() -> InstallerUserAuthorization {
    InstallerUserAuthorization {
        explicit_action_confirmed: true,
        data_retention_acknowledged: true,
        neutral_input_source_selected: true,
        manager_closed: true,
    }
}
pub(super) fn put(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
pub(super) fn tree(root: &Path) -> Vec<(PathBuf, u64, u32, Vec<u8>)> {
    let mut nodes = vec![root.to_owned()];
    let mut result = vec![];
    while let Some(path) = nodes.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(&path).unwrap()
        } else {
            vec![]
        };
        result.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.ino(),
            metadata.mode(),
            bytes,
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).unwrap() {
                nodes.push(entry.unwrap().path());
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}
