//! Real core preparation/proof with synthetic authority and independent reader pack.
//! No installed program is launched and no final marker-release method is called.
use super::*;
use crate::{CancellationArchiveAuthority, CancellationProductAuthority};
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallFailureCode, InstallOperationKind,
    InstallProcessGuard, InstallReceipt, InstallReceiptStore, ProductRelease, VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationArchiveStore, CancellationReleaseCheckpoint,
    CancellationReleasePort, CancellationReleaseReceipt, CancellationReleaseRequirement,
    CancellationReleaseStore, CancellationSourceCheckpoint, CancellationSourcePort,
    CancellationSourceReceipt, PreparationBinding, PreparationCancellationCheckpoint,
    PreparationCancellationPort, PreparationCancellationRequest, PreparationCancellationStore,
    PreparationHasher, PreparationJournalStore, PreparationPhase, PreparationReceipt,
    ProductRelease as DataRelease, SourcePreparationCheckpoint, SourcePreparationPort,
    UpgradeReceiptStore, VerifiedDataRoot,
};
use radishlex_ime_userdb::UserDb;
use std::io::{Read, Write};

const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OPERATION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Hasher;
impl PreparationHasher for Hasher {
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                return Ok(hasher.finalize().into());
            }
            hasher.update(&buffer[..count]);
        }
    }
}

struct Admission {
    stop_after_snapshot: bool,
}
impl PreparationCancellationPort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: PreparationCancellationCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationSourcePort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: &CancellationSourceReceipt,
        _: CancellationSourceCheckpoint,
    ) -> bool {
        true
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
impl SourcePreparationPort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationReceipt,
        checkpoint: SourcePreparationCheckpoint,
    ) -> bool {
        !(self.stop_after_snapshot && checkpoint == SourcePreparationCheckpoint::SnapshotRecorded)
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
impl CancellationProductAuthority for Admission {
    fn confirm_source_and_quiescence(
        &mut self,
        _: &ProductArtifactIdentity,
        _: &ProductArtifactIdentity,
        _: CancellationArchiveCheckpoint,
    ) -> bool {
        true
    }
}

// This test port grants only preservation checkpoints; any qualification request
// is a test failure. It checks the live outer guard and complete typed v1 binding.
struct PreserveOnly<'a> {
    install: &'a InstallReceiptStore,
    guard: &'a InstallProcessGuard,
    source: &'a ProductArtifactIdentity,
    target: &'a ProductArtifactIdentity,
}
impl CancellationReleasePort for PreserveOnly<'_> {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        _: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool {
        assert_eq!(requirement, CancellationReleaseRequirement::Preserve);
        self.install.verify_guard(self.guard).is_ok()
            && validate_preparation_cancellation_outer(
                proof.archive().outer().previous_bytes(),
                proof.archive().outer().new_bytes(),
                proof.operation_id(),
                proof.legacy_outer_operation_id(),
                self.install.root_identity(),
                self.source,
                self.target,
            )
            .is_ok()
    }
}

#[derive(Clone, Copy)]
enum PreparationPoint {
    Reserved,
    SnapshotReady,
    SourcePrepared,
}
struct CoreProof {
    path: PathBuf,
    proof: CancellationReleaseReceipt,
    target: ProductArtifactIdentity,
}
impl CoreProof {
    fn new(source: &ProductArtifactIdentity, point: PreparationPoint) -> Self {
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-reader-core-proof-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        drop(UserDb::open(path.join("userdb.sqlite3")).unwrap());
        assert_eq!(UserDb::supported_schema_version(), 9);
        let target = ProductArtifactIdentity::new(
            source.product_id(),
            ProductRelease::new(source.release().product_version(), 41).unwrap(),
            digest(b"synthetic target product manifest"),
            source
                .program(radishlex_ime_product_install::ProgramComponent::Manager)
                .clone(),
            source
                .program(radishlex_ime_product_install::ProgramComponent::InputMethod)
                .clone(),
        )
        .unwrap();
        let history = path.join(".radishlex-install-history-v1");
        let previous = history.join(OLD);
        DirBuilder::new().mode(0o700).create(&history).unwrap();
        DirBuilder::new().mode(0o700).create(&previous).unwrap();
        let owner = fs::metadata(&path).unwrap().uid();
        let install =
            InstallReceiptStore::open(VerifiedInstallRoot::verify(&path, owner).unwrap()).unwrap();
        let outer_guard = install.acquire_guard().unwrap();
        let mut old = InstallReceipt::new(
            OLD,
            None,
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .unwrap();
        install.persist(&outer_guard, &old).unwrap();
        old.abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .unwrap();
        install.persist(&outer_guard, &old).unwrap();
        let old_bytes = old.encode().unwrap();
        let mut previous_file = fs::File::create(previous.join("receipt.json")).unwrap();
        previous_file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
        previous_file.write_all(&old_bytes).unwrap();
        previous_file.sync_all().unwrap();
        drop(previous_file);
        let data =
            UpgradeReceiptStore::open(VerifiedDataRoot::verify(&path, owner).unwrap()).unwrap();
        let inner_guard = data.acquire_guard().unwrap();
        let journal = PreparationJournalStore::attach(data, &inner_guard).unwrap();
        let preparation = PreparationReceipt::new(
            PreparationBinding {
                operation_id: OPERATION.into(),
                previous_install_operation_id: Some(OLD.into()),
                previous_data_operation_id: None,
                source_release: DataRelease::new(
                    source.release().product_version(),
                    source.release().build_number(),
                )
                .unwrap(),
                target_release: DataRelease::new(
                    target.release().product_version(),
                    target.release().build_number(),
                )
                .unwrap(),
                source_product_sha256: source.product_manifest_sha256().into(),
                target_product_sha256: target.product_manifest_sha256().into(),
                previous_install_receipt_sha256: Some(digest(&old_bytes)),
                previous_data_receipt_sha256: None,
                previous_inventory_sha256: None,
                target_schema_version: 9,
                data_root: journal.data_root_identity(),
                state_directory: journal.state_directory_identity(),
            },
            journal
                .observe_source_family(&inner_guard, &Hasher)
                .unwrap(),
        )
        .unwrap();
        journal.persist(&inner_guard, None, &preparation).unwrap();
        let mut admission = Admission {
            stop_after_snapshot: matches!(point, PreparationPoint::SnapshotReady),
        };
        match point {
            PreparationPoint::Reserved => {}
            PreparationPoint::SnapshotReady => {
                assert!(journal
                    .prepare_userdb_source(&inner_guard, &mut admission, &Hasher)
                    .is_err());
                assert_eq!(
                    journal.load_guarded(&inner_guard).unwrap().unwrap().phase(),
                    PreparationPhase::SnapshotReady
                );
            }
            PreparationPoint::SourcePrepared => {
                journal
                    .prepare_userdb_source(&inner_guard, &mut admission, &Hasher)
                    .unwrap();
            }
        }
        let cancellation = PreparationCancellationStore::attach(journal, &inner_guard).unwrap();
        cancellation
            .request_cancellation(&inner_guard, OPERATION, &mut admission, &Hasher)
            .unwrap();
        cancellation
            .finish_source(&inner_guard, &mut admission, &Hasher)
            .unwrap();
        drop(inner_guard);
        let archive = CancellationArchiveStore::open_existing(
            VerifiedDataRoot::verify(&path, owner).unwrap(),
        )
        .unwrap();
        let inner_guard = archive.acquire_guard().unwrap();
        let outer = archive.observe_outer(&inner_guard, &Hasher).unwrap();
        let mut authority = CancellationArchiveAuthority {
            install: &install,
            guard: &outer_guard,
            source,
            target: &target,
            product: &mut admission,
        };
        archive
            .preserve_cancellation(&inner_guard, &outer, &mut authority, &Hasher)
            .unwrap();
        drop(inner_guard);
        let release = CancellationReleaseStore::open_existing(
            VerifiedDataRoot::verify(&path, owner).unwrap(),
        )
        .unwrap();
        let inner_guard = release.acquire_guard().unwrap();
        let mut preserve = PreserveOnly {
            install: &install,
            guard: &outer_guard,
            source,
            target: &target,
        };
        let proof = release
            .prepare_cancellation_release(&inner_guard, OPERATION, &mut preserve, &Hasher)
            .unwrap();
        // Exercise strict decoding instead of passing a field-assembled proof.
        let proof = CancellationReleaseReceipt::decode(&proof.encode().unwrap()).unwrap();
        drop(inner_guard);
        drop(outer_guard);
        let result = Self {
            path,
            proof,
            target,
        };
        assert!(result.marker().exists());
        result
    }
    fn marker(&self) -> PathBuf {
        self.path
            .join(".radishlex-upgrade-v1/preparation-cancellation.json")
    }
}
impl Drop for CoreProof {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn bind_test_catalog(fixture: &mut Fixture, request: &CancellationEvidenceRequest) {
    fixture.catalog.entries[0].target = request.target.clone();
    for row in &mut fixture.manifest.rows {
        let blocking = row.scenario.blocking_object;
        let fault = row.scenario.rejection_fault;
        row.scenario = request.scenario.clone();
        row.scenario.blocking_object = blocking;
        row.scenario.rejection_fault = fault;
        for observation in &mut row.observations {
            if let model::Event::ReceiptState { outer, .. } = &mut observation.event {
                *outer = request.scenario.outer_state;
            }
        }
    }
    for (case, row) in fixture.catalog.entries[0]
        .cases
        .iter_mut()
        .zip(&fixture.manifest.rows)
    {
        case.scenario = row.scenario.clone();
    }
    fixture.publish_test_manifest();
}
fn review_request(fixture: &Fixture, request: &CancellationEvidenceRequest) -> EvidenceReview {
    let (control, _) = ReadControl::new(Duration::from_secs(30)).unwrap();
    review_with_catalog(
        &fixture.catalog,
        "synthetic-reviewed-entry",
        &fixture.path,
        &fixture.manifest.source.product,
        &fixture.manifest.environment,
        Some(request),
        &control,
    )
}
fn different_manifest(product: &ProductArtifactIdentity) -> ProductArtifactIdentity {
    ProductArtifactIdentity::new(
        product.product_id(),
        product.release().clone(),
        digest(b"different manifest with same release"),
        product
            .program(radishlex_ime_product_install::ProgramComponent::Manager)
            .clone(),
        product
            .program(radishlex_ime_product_install::ProgramComponent::InputMethod)
            .clone(),
    )
    .unwrap()
}

#[test]
fn real_maintained_proof_matches_without_releasing_marker_and_wrong_products_fail() {
    let mut fixture = Fixture::new();
    let core = CoreProof::new(
        &fixture.manifest.source.product,
        PreparationPoint::SourcePrepared,
    );
    let request = CancellationEvidenceRequest::new(
        &core.proof,
        &fixture.manifest.source.product,
        &core.target,
    )
    .unwrap();
    assert_eq!(
        core.proof.predecessor_kind(),
        radishlex_ime_product_upgrade::CancellationPredecessorKind::NoIndex
    );
    assert_eq!(
        request.scenario.maintenance,
        model::Maintenance::AfterEquivalent
    );
    assert_eq!(request.schema, Some(9));
    bind_test_catalog(&mut fixture, &request);
    let pack_before = snapshot(&fixture.path);
    let core_before = snapshot(&core.path);
    let report = review_request(&fixture, &request);
    assert_eq!(
        report.status(),
        ReviewStatus::ReviewedMatchingRequest,
        "{:?}",
        report.rejection()
    );
    assert_eq!(
        report.request_binding().unwrap().proof_sha256,
        request.proof_sha256
    );
    assert_eq!(
        report.request_binding().unwrap().family_sha256,
        request.family_sha256
    );
    assert_eq!(snapshot(&fixture.path), pack_before);
    assert_eq!(snapshot(&core.path), core_before);
    assert!(core.marker().exists());
    let wrong_source = different_manifest(&fixture.manifest.source.product);
    let wrong_target = different_manifest(&core.target);
    assert!(matches!(
        CancellationEvidenceRequest::new(&core.proof, &wrong_source, &core.target),
        Err(ReadError::IdentityMismatch)
    ));
    assert!(matches!(
        CancellationEvidenceRequest::new(
            &core.proof,
            &fixture.manifest.source.product,
            &wrong_target
        ),
        Err(ReadError::IdentityMismatch)
    ));
    let (control, _) = ReadControl::new(Duration::from_secs(30)).unwrap();
    let rejected = review_with_catalog(
        &fixture.catalog,
        "synthetic-reviewed-entry",
        &fixture.path,
        &wrong_source,
        &fixture.manifest.environment,
        Some(&request),
        &control,
    );
    assert_eq!(rejected.rejection(), Some(ReadError::IdentityMismatch));
    assert_eq!(snapshot(&core.path), core_before);
}

#[test]
fn no_snapshot_cannot_borrow_target_schema_or_matching_catalog_schema() {
    let mut fixture = Fixture::new();
    let core = CoreProof::new(&fixture.manifest.source.product, PreparationPoint::Reserved);
    assert_eq!(core.proof.preparation().binding().target_schema_version, 9);
    let request = CancellationEvidenceRequest::new(
        &core.proof,
        &fixture.manifest.source.product,
        &core.target,
    )
    .unwrap();
    assert_eq!(request.schema, None);
    assert_eq!(request.scenario.maintenance, model::Maintenance::Before);
    bind_test_catalog(&mut fixture, &request);
    let pack_before = snapshot(&fixture.path);
    let core_before = snapshot(&core.path);
    assert_eq!(
        review_request(&fixture, &request).rejection(),
        Some(ReadError::SchemaObservationUnavailable)
    );
    assert_eq!(snapshot(&fixture.path), pack_before);
    assert_eq!(snapshot(&core.path), core_before);
    assert!(core.marker().exists());
}

#[test]
fn snapshot_ready_has_schema_but_remains_before_maintenance() {
    let mut fixture = Fixture::new();
    let core = CoreProof::new(
        &fixture.manifest.source.product,
        PreparationPoint::SnapshotReady,
    );
    assert_eq!(
        core.proof.preparation().phase(),
        PreparationPhase::SnapshotReady
    );
    let request = CancellationEvidenceRequest::new(
        &core.proof,
        &fixture.manifest.source.product,
        &core.target,
    )
    .unwrap();
    assert_eq!(request.schema, Some(9));
    assert_eq!(request.scenario.maintenance, model::Maintenance::Before);
    assert_eq!(request.scenario.family, model::SourceFamily::Original);
    bind_test_catalog(&mut fixture, &request);
    let pack_before = snapshot(&fixture.path);
    let core_before = snapshot(&core.path);
    let report = review_request(&fixture, &request);
    assert_eq!(
        report.status(),
        ReviewStatus::ReviewedMatchingRequest,
        "{:?}",
        report.rejection()
    );
    assert_eq!(snapshot(&fixture.path), pack_before);
    assert_eq!(snapshot(&core.path), core_before);
    assert!(core.marker().exists());
}

#[test]
fn same_operation_distinct_roots_get_fresh_distinct_report_bindings() {
    let mut fixture = Fixture::new();
    let first = CoreProof::new(
        &fixture.manifest.source.product,
        PreparationPoint::SourcePrepared,
    );
    let second = CoreProof::new(
        &fixture.manifest.source.product,
        PreparationPoint::SourcePrepared,
    );
    assert_eq!(first.proof.operation_id(), second.proof.operation_id());
    let first_request = CancellationEvidenceRequest::new(
        &first.proof,
        &fixture.manifest.source.product,
        &first.target,
    )
    .unwrap();
    let second_request = CancellationEvidenceRequest::new(
        &second.proof,
        &fixture.manifest.source.product,
        &second.target,
    )
    .unwrap();
    assert_ne!(first_request.proof_sha256, second_request.proof_sha256);
    assert_ne!(first_request.family_sha256, second_request.family_sha256);
    bind_test_catalog(&mut fixture, &first_request);
    let pack_before = snapshot(&fixture.path);
    let first_before = snapshot(&first.path);
    let second_before = snapshot(&second.path);
    let first_report = review_request(&fixture, &first_request);
    let second_report = review_request(&fixture, &second_request);
    assert_eq!(first_report.status(), ReviewStatus::ReviewedMatchingRequest);
    assert_eq!(
        second_report.status(),
        ReviewStatus::ReviewedMatchingRequest
    );
    let first_binding = first_report.request_binding().unwrap();
    let second_binding = second_report.request_binding().unwrap();
    assert_ne!(first_binding.proof_sha256, second_binding.proof_sha256);
    assert_ne!(first_binding.family_sha256, second_binding.family_sha256);
    assert_ne!(first_binding.root_inode, second_binding.root_inode);
    assert_eq!(first_binding.proof_sha256, first_request.proof_sha256);
    assert_eq!(second_binding.proof_sha256, second_request.proof_sha256);
    assert_eq!(snapshot(&fixture.path), pack_before);
    assert_eq!(snapshot(&first.path), first_before);
    assert_eq!(snapshot(&second.path), second_before);
    // A prior matching report cannot cause the next invocation to skip its pack.
    let raw = fixture.path.join("raw.log");
    let mut bytes = fs::read(&raw).unwrap();
    bytes[0] ^= 1;
    fs::write(&raw, bytes).unwrap();
    assert_eq!(
        review_request(&fixture, &second_request).rejection(),
        Some(ReadError::EvidenceChanged)
    );
    assert_eq!(snapshot(&first.path), first_before);
    assert_eq!(snapshot(&second.path), second_before);
    assert!(first.marker().exists() && second.marker().exists());
}
