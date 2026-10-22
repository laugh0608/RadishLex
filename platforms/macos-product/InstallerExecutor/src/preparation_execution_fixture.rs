use super::*;
use radishlex_ime_product_install::{InstallFailureCode, VerifiedInstallRoot};
use radishlex_ime_product_upgrade::{
    UpgradeArtifactIdentity, UpgradeArtifactSlot, UpgradeFailureCode, UpgradeState,
};
use radishlex_ime_userdb::SelectionEventDraft;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const DATA: &str = "dddddddddddddddddddddddddddddddd";
pub(crate) const NEW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub(super) const STATE: &str = ".radishlex-upgrade-v1";
pub(crate) struct Fixture {
    pub(crate) path: PathBuf,
    pub(super) source: ProductArtifactIdentity,
    pub(crate) target: ProductArtifactIdentity,
    pub(super) previous: InstallReceipt,
}
impl Fixture {
    pub(crate) fn new() -> Self {
        let mut nonce = [0; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-preparation-task-{}-{}",
                std::process::id(),
                u64::from_ne_bytes(nonce)
            ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let mut db = UserDb::open(path.join("userdb.sqlite3")).unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-prepare",
            "ci",
            "词",
            0,
            1,
        ))
        .unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-prepare",
            "shan",
            "删",
            0,
            2,
        ))
        .unwrap();
        db.delete_term("shan", "删", None).unwrap();
        drop(db);
        put(&path.join("manager-settings.json"), b"synthetic-settings");
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
        Self {
            path,
            source,
            target,
            previous,
        }
    }
    pub(crate) fn reopen(path: PathBuf) -> Self {
        let owner = fs::metadata(&path).unwrap().uid();
        let store =
            InstallReceiptStore::open_existing(VerifiedInstallRoot::verify(&path, owner).unwrap())
                .unwrap();
        let previous = if path
            .join(".radishlex-install-history-v1")
            .join(OLD)
            .join("receipt.json")
            .exists()
        {
            InstallReceipt::decode(
                &fs::read(
                    path.join(".radishlex-install-history-v1")
                        .join(OLD)
                        .join("receipt.json"),
                )
                .unwrap(),
            )
            .unwrap()
        } else {
            store.load_for_recovery_inspection().unwrap().unwrap()
        };
        Self {
            path,
            source: crate::cancellation_fixture::product(39, 'a'),
            target: crate::cancellation_fixture::product(41, 'b'),
            previous,
        }
    }
    pub(crate) fn install(&self) -> InstallReceiptStore {
        InstallReceiptStore::open_existing(
            VerifiedInstallRoot::verify(&self.path, self.owner()).unwrap(),
        )
        .unwrap()
    }
    pub(crate) fn owner(&self) -> u32 {
        fs::metadata(&self.path).unwrap().uid()
    }
    pub(super) fn grant(&self) -> (PreparationTaskAuthorization, PreparationTaskControl) {
        authorize_source_preparation(
            &self.previous,
            &self.target,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap()
    }
    pub(crate) fn preparation(&self) -> PreparationReceipt {
        let journal = PreparationJournalStore::open_existing(
            VerifiedDataRoot::verify(&self.path, self.owner()).unwrap(),
        )
        .unwrap();
        let guard = journal.acquire_guard().unwrap();
        journal
            .load_preparation_for_operation(&guard, NEW, &MacOsPreparationHasher)
            .unwrap()
    }
    pub(super) fn resume_grant(&self) -> (PreparationTaskAuthorization, PreparationTaskControl) {
        authorize_preparation_resume(
            &self.preparation(),
            &self.source,
            &self.target,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap()
    }
    pub(crate) fn run(&self, observer: &mut Observer) -> Result<UpgradeReceipt> {
        let (auth, _control) = self.grant();
        execute(auth, &self.install(), observer, &mut Ids)
    }
    pub(super) fn resume(&self, observer: &mut Observer) -> Result<UpgradeReceipt> {
        let (auth, _control) = self.resume_grant();
        execute(auth, &self.install(), observer, &mut NoIds)
    }
    pub(super) fn legacy(&self, state: UpgradeState) {
        let store =
            UpgradeReceiptStore::open(VerifiedDataRoot::verify(&self.path, self.owner()).unwrap())
                .unwrap();
        let _guard = store.acquire_guard().unwrap();
        let at = |name| self.path.join(STATE).join(name);
        put(&at("source-snapshot.sqlite3"), b"synthetic-snapshot");
        let source = if state == UpgradeState::Completed {
            put(&at("source-backup.sqlite3"), b"synthetic-old-source");
            at("source-backup.sqlite3")
        } else {
            self.path.join("userdb.sqlite3")
        };
        let candidate = if state == UpgradeState::Completed {
            self.path.join("userdb.sqlite3")
        } else {
            put(
                &at("migration-candidate.sqlite3"),
                b"synthetic-old-candidate",
            );
            at("migration-candidate.sqlite3")
        };
        let schema = UserDb::supported_schema_version();
        let mut receipt = UpgradeReceipt::new(
            DATA,
            None,
            ProductRelease::new(
                "26.7.1",
                if state == UpgradeState::Completed {
                    38
                } else {
                    39
                },
            )
            .unwrap(),
            ProductRelease::new(
                "26.7.1",
                if state == UpgradeState::Completed {
                    39
                } else {
                    40
                },
            )
            .unwrap(),
            Some(schema),
            schema,
            vec![
                store.data_root_identity().clone(),
                artifact(UpgradeArtifactSlot::SourceDatabase, &source),
            ],
        )
        .unwrap();
        receipt.advance(UpgradeState::Quiesced).unwrap();
        receipt
            .record_artifact(artifact(
                UpgradeArtifactSlot::SnapshotDatabase,
                &at("source-snapshot.sqlite3"),
            ))
            .unwrap();
        receipt.advance(UpgradeState::SnapshotReady).unwrap();
        receipt
            .record_artifact(artifact(UpgradeArtifactSlot::CandidateDatabase, &candidate))
            .unwrap();
        receipt.advance(UpgradeState::CandidateMigrated).unwrap();
        receipt.advance(UpgradeState::CandidateVerified).unwrap();
        if state == UpgradeState::AbortedPreserved {
            receipt
                .abort_preserved(UpgradeFailureCode::InputMethodValidationFailed, false)
                .unwrap();
        } else if state != UpgradeState::CandidateVerified {
            receipt
                .record_artifact(artifact(UpgradeArtifactSlot::BackupDatabase, &source))
                .unwrap();
            receipt.advance(UpgradeState::SwitchPrepared).unwrap();
            receipt.advance(UpgradeState::Switched).unwrap();
            if state == UpgradeState::Completed {
                receipt.advance(UpgradeState::PostSwitchVerified).unwrap();
                receipt.advance(UpgradeState::Completed).unwrap();
            } else {
                receipt
                    .require_rollback(UpgradeFailureCode::PostSwitchValidationFailed)
                    .unwrap();
                receipt.mark_rolled_back().unwrap();
            }
        }
        put(&at("receipt.json"), &receipt.encode().unwrap());
        let mut db = UserDb::open(self.path.join("userdb.sqlite3")).unwrap();
        for n in 0..20 {
            db.record_selection(SelectionEventDraft::new(
                "synthetic-later",
                "xin",
                format!("新词{n}"),
                0,
                3,
            ))
            .unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
pub(super) fn artifact(slot: UpgradeArtifactSlot, path: &Path) -> UpgradeArtifactIdentity {
    let m = fs::metadata(path).unwrap();
    UpgradeArtifactIdentity::private_file(slot, m.dev(), m.ino(), m.uid(), m.len()).unwrap()
}
pub(crate) fn confirmation() -> InstallerUserAuthorization {
    InstallerUserAuthorization {
        explicit_action_confirmed: true,
        data_retention_acknowledged: true,
        neutral_input_source_selected: true,
        manager_closed: true,
    }
}
pub(super) fn put(path: &Path, bytes: &[u8]) {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
pub(super) struct Ids;
impl InstallerOperationIdSource for Ids {
    fn next_operation_id(&mut self) -> std::result::Result<String, crate::InstallerExecutionError> {
        Ok(NEW.into())
    }
}
pub(super) struct NoIds;
impl InstallerOperationIdSource for NoIds {
    fn next_operation_id(&mut self) -> std::result::Result<String, crate::InstallerExecutionError> {
        panic!("resume must not allocate a new operation")
    }
}
#[derive(Default)]
pub(crate) struct Observer {
    pub(super) seen: Vec<ExecutionPoint>,
    pub(super) fail: Option<ExecutionPoint>,
    pub(super) mutation: Option<(ExecutionPoint, Box<dyn FnOnce()>)>,
    pub(super) available: Option<u64>,
    pub(super) exit: bool,
}
impl PreparationObserver for Observer {
    fn inspect(
        &mut self,
        _: &PreparationDirectoryIdentity,
        _: &ProductArtifactIdentity,
        _: &ProductArtifactIdentity,
        point: ExecutionPoint,
    ) -> Result<u64> {
        self.seen.push(point);
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.fail == Some(point) {
            if self.exit {
                std::process::exit(79);
            }
            return Err(Error::NativeObservation);
        }
        Ok(self.available.unwrap_or(u64::MAX))
    }
}
