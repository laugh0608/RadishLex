//! Synthetic qualification only. Real dual guards, canonical v1 receipts,
//! SQLite preparation and persistent cancellation proof are used throughout.
use super::*;
use radishlex_ime_product_install::InstallProcessGuard;
use radishlex_ime_product_upgrade::{
    CancellationReleaseCheckpoint as CP, CancellationReleasePort, CancellationReleaseReceipt,
    CancellationReleaseRequirement as Requirement, CancellationReleaseStore,
    SourcePreparationCheckpoint, SourcePreparationPort, UpgradeProcessGuard,
};
use radishlex_macos_product_install_coordinator::{
    CancellationQualificationError as QE, CancellationReleaseAuthority as Authority,
    CancellationReleaseAuthorityFailure as Failure, CancellationReleaseProductAuthority,
    CancellationSourceQualification as Qualification, CancellationSourceQualificationAuthority,
};

mod failures;

impl SourcePreparationPort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationReceipt,
        _: SourcePreparationCheckpoint,
    ) -> bool {
        true
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}

struct Preserved {
    install: InstallReceiptStore,
    guard: InstallProcessGuard,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    old: InstallReceipt,
    fixture: Fixture,
}
impl Preserved {
    fn new(maintained: bool, new_outer: bool) -> Self {
        let fixture = Fixture::new();
        drop(UserDb::open(fixture.0.join("userdb.sqlite3")).unwrap());
        let source = product(39, 'a');
        let target = product(41, 'b');
        let history = fixture.0.join(".radishlex-install-history-v1");
        let previous = history.join(OLD);
        for path in [&history, &previous] {
            DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        let install = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&fixture.0, fs::metadata(&fixture.0).unwrap().uid())
                .unwrap(),
        )
        .unwrap();
        let guard = install.acquire_guard().unwrap();
        let mut old = InstallReceipt::new(
            OLD,
            None,
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .unwrap();
        install.persist(&guard, &old).unwrap();
        old.abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .unwrap();
        install.persist(&guard, &old).unwrap();
        let bytes = old.encode().unwrap();
        put(&previous.join("receipt.json"), &bytes);
        if new_outer {
            let next = InstallReceipt::new(
                NEW,
                Some(OLD.into()),
                InstallOperationKind::Upgrade,
                install.root_identity().clone(),
                Some(source.clone()),
                Some(target.clone()),
            )
            .unwrap();
            install.persist(&guard, &next).unwrap();
        }
        let data = UpgradeReceiptStore::open(fixture.data()).unwrap();
        let inner = data.acquire_guard().unwrap();
        let journal = PreparationJournalStore::attach(data, &inner).unwrap();
        let prep = PreparationReceipt::new(
            PreparationBinding {
                operation_id: NEW.into(),
                previous_install_operation_id: Some(OLD.into()),
                previous_data_operation_id: None,
                source_release: DataRelease::new("26.7.1", 39).unwrap(),
                target_release: DataRelease::new("26.7.1", 41).unwrap(),
                source_product_sha256: source.product_manifest_sha256().into(),
                target_product_sha256: target.product_manifest_sha256().into(),
                previous_install_receipt_sha256: Some(hash(&bytes)),
                previous_data_receipt_sha256: None,
                previous_inventory_sha256: None,
                target_schema_version: 9,
                data_root: journal.data_root_identity(),
                state_directory: journal.state_directory_identity(),
            },
            journal.observe_source_family(&inner, &Hasher).unwrap(),
        )
        .unwrap();
        journal.persist(&inner, None, &prep).unwrap();
        if maintained {
            journal
                .prepare_userdb_source(&inner, &mut Admission, &Hasher)
                .unwrap();
        }
        let cancel = PreparationCancellationStore::attach(journal, &inner).unwrap();
        cancel
            .request_cancellation(&inner, NEW, &mut Admission, &Hasher)
            .unwrap();
        cancel
            .finish_source(&inner, &mut Admission, &Hasher)
            .unwrap();
        drop(inner);
        let archive = CancellationArchiveStore::open_existing(fixture.data()).unwrap();
        let inner = archive.acquire_guard().unwrap();
        let outer = archive.observe_outer(&inner, &Hasher).unwrap();
        let mut products = Products {
            calls: 0,
            reject: false,
        };
        let mut authority = CancellationArchiveAuthority {
            install: &install,
            guard: &guard,
            source: &source,
            target: &target,
            product: &mut products,
        };
        archive
            .preserve_cancellation(&inner, &outer, &mut authority, &Hasher)
            .unwrap();
        drop(inner);
        Self {
            install,
            guard,
            source,
            target,
            old,
            fixture,
        }
    }
    fn store(&self) -> (CancellationReleaseStore, UpgradeProcessGuard) {
        let store = CancellationReleaseStore::open_existing(self.fixture.data()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (store, guard)
    }
    fn marker(&self) -> PathBuf {
        self.fixture
            .0
            .join(".radishlex-upgrade-v1/preparation-cancellation.json")
    }
    fn outer(&self) -> PathBuf {
        self.fixture.0.join(".radishlex-install-v1/receipt.json")
    }
    fn prepare(&self) -> CancellationReleaseReceipt {
        let (store, inner) = self.store();
        let mut products = ReleaseProducts::default();
        let mut qualification = Qualifier::default();
        let mut authority = Authority::new(
            &self.install,
            &self.guard,
            &self.source,
            &self.target,
            &mut products,
            &mut qualification,
        );
        let proof = store
            .prepare_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .unwrap();
        assert_eq!(qualification.calls, 0);
        assert!(self.marker().exists());
        proof
    }
}

#[derive(Default)]
struct ReleaseProducts {
    calls: usize,
    reject_at: Option<usize>,
    drift: Option<(PathBuf, Vec<u8>)>,
}
impl CancellationReleaseProductAuthority for ReleaseProducts {
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        _: CP,
    ) -> bool {
        assert_eq!(proof.operation_id(), NEW);
        assert_eq!(source.release().build_number(), 39);
        assert_eq!(target.release().build_number(), 41);
        self.calls += 1;
        if let Some((path, bytes)) = self.drift.take() {
            put(&path, &bytes);
        }
        self.reject_at != Some(self.calls)
    }
}

#[derive(Default)]
struct Qualifier {
    calls: usize,
    reject: Option<CP>,
    unavailable: bool,
    evidence: Option<Qualification>,
    drift: Option<(PathBuf, Vec<u8>)>,
}
impl CancellationSourceQualificationAuthority for Qualifier {
    fn verify_source_qualification(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        checkpoint: CP,
    ) -> Result<Qualification, QE> {
        self.calls += 1;
        if self.unavailable || self.reject == Some(checkpoint) {
            return Err(QE::Unavailable);
        }
        if let Some((path, bytes)) = self.drift.take() {
            put(&path, &bytes);
        }
        self.evidence.clone().map(Ok).unwrap_or_else(|| {
            // Explicitly synthetic. No actual old binaries are qualified here.
            Qualification::from_verified_source(source.clone(), 9, proof.clone())
        })
    }
}

#[test]
fn release_requires_fresh_bound_qualification_and_reobserves_source() {
    for maintained in [false, true] {
        for new_outer in [false, true] {
            let fixture = Preserved::new(maintained, new_outer);
            let proof = fixture.prepare();
            let marker = fs::metadata(fixture.marker()).unwrap().ino();
            let original = fs::read(fixture.marker()).unwrap();
            let (store, inner) = fixture.store();
            let mut product = ReleaseProducts::default();
            let mut qualifier = Qualifier {
                unavailable: true,
                ..Default::default()
            };
            let mut authority = Authority::new(
                &fixture.install,
                &fixture.guard,
                &fixture.source,
                &fixture.target,
                &mut product,
                &mut qualifier,
            );
            assert!(store
                .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
                .is_err());
            assert_eq!(
                authority.failure(),
                Some(Failure::SourceQualification(QE::Unavailable))
            );
            assert_eq!(fs::metadata(fixture.marker()).unwrap().ino(), marker);
            assert_eq!(fs::read(fixture.marker()).unwrap(), original);
            drop(inner);
            let (store, inner) = fixture.store();
            let mut product = ReleaseProducts::default();
            let mut qualifier = Qualifier::default();
            let mut authority = Authority::new(
                &fixture.install,
                &fixture.guard,
                &fixture.source,
                &fixture.target,
                &mut product,
                &mut qualifier,
            );
            let (_, result) = store
                .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
                .unwrap();
            assert_eq!(result, proof);
            assert_eq!(authority.failure(), None);
            assert!(qualifier.calls >= 7);
            assert_eq!(product.calls, qualifier.calls * 2);
            assert!(!fixture.marker().exists());
            let archived = fixture
                .fixture
                .0
                .join(".radishlex-upgrade-history-v1")
                .join(NEW)
                .join("preparation-cancellation.json");
            assert_eq!(fs::metadata(&archived).unwrap().ino(), marker);
            assert_eq!(fs::read(archived).unwrap(), original);
            assert_eq!(
                fixture.install.load_guarded(&fixture.guard).unwrap(),
                Some(fixture.old.clone())
            );
        }
    }
}
