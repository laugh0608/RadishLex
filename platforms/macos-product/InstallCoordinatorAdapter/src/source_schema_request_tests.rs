//! Production observer, real core guards/proofs and synthetic-only data/catalog.
use super::*;
use crate::source_schema::{
    observe_at_for_test, GuardedCancellationSource, ObservationError, SchemaTaskControl,
    SourceSchemaAuthority,
};
use crate::CancellationReleaseProductAuthority;
use std::cell::Cell;

struct Authority {
    proof: CancellationReleaseReceipt,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    active: Arc<AtomicBool>,
    deadline: Instant,
    calls: usize,
    revoke_at: Option<usize>,
}
impl Authority {
    fn new(core: &CoreProof, source: &ProductArtifactIdentity) -> Self {
        Self {
            proof: core.proof.clone(),
            source: source.clone(),
            target: core.target.clone(),
            active: Arc::new(AtomicBool::new(true)),
            deadline: Instant::now() + Duration::from_secs(120),
            calls: 0,
            revoke_at: None,
        }
    }
}
impl Drop for Authority {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}
impl CancellationReleaseProductAuthority for Authority {
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        _: CancellationReleaseCheckpoint,
    ) -> bool {
        self.calls += 1;
        if self.revoke_at == Some(self.calls) {
            self.active.store(false, Ordering::Release);
        }
        self.active.load(Ordering::Acquire)
            && proof == &self.proof
            && source == &self.source
            && target == &self.target
    }
}
impl SourceSchemaAuthority for Authority {
    fn schema_task_control(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
    ) -> Result<SchemaTaskControl, ObservationError> {
        if proof != &self.proof || source != &self.source || !self.active.load(Ordering::Acquire) {
            return Err(ObservationError::TaskAuthorization);
        }
        let active = self.active.clone();
        Ok(SchemaTaskControl::new(
            self.deadline,
            Arc::new(move || active.load(Ordering::Acquire)),
        ))
    }
}

struct Caches {
    path: PathBuf,
    node: (u64, u64),
}

#[test]
fn empty_production_catalog_rejects_before_task_or_copy_access() {
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    let core = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    let before = snapshot(&core.path);
    let mut authority = Authority::new(&core, source);
    // Even an unavailable task cannot be consulted before catalog rejection.
    authority.active.store(false, Ordering::Release);
    guarded(&core, source, |context, _, _| {
        let (control, _) = ReadControl::new(Duration::from_secs(30)).unwrap();
        let review = crate::source_schema::review_evidence_with_source_schema(
            context,
            &mut authority,
            "synthetic-reviewed-entry",
            &fixture.path,
            &fixture.manifest.environment,
            &control,
        )
        .unwrap();
        assert_eq!(review.rejection(), Some(ReadError::UntrustedSource));
        assert!(review.schema_observation().is_none());
    });
    assert_eq!(authority.calls, 0);
    assert_eq!(snapshot(&core.path), before);
    assert!(core.marker().exists());
}
impl Caches {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/radishlex-observer-request-caches-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        Self {
            path,
            node: (meta.dev(), meta.ino()),
        }
    }
    fn assert_disposed(&self) {
        assert!(!self
            .path
            .join("RadishLex/SchemaObservation-v1/active")
            .exists());
    }
    fn finish(self) {
        self.assert_disposed();
        let meta = fs::symlink_metadata(&self.path).unwrap();
        assert_eq!((meta.dev(), meta.ino()), self.node);
        for path in [
            self.path.join("RadishLex/SchemaObservation-v1"),
            self.path.join("RadishLex"),
            self.path,
        ] {
            match fs::remove_dir(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => panic!("synthetic empty cache cleanup: {error}"),
            }
        }
    }
}

fn guarded<T>(
    core: &CoreProof,
    source: &ProductArtifactIdentity,
    run: impl FnOnce(
        GuardedCancellationSource<'_>,
        &CancellationReleaseStore,
        &radishlex_ime_product_upgrade::UpgradeProcessGuard,
    ) -> T,
) -> T {
    let owner = fs::symlink_metadata(&core.path).unwrap().uid();
    let install =
        InstallReceiptStore::open(VerifiedInstallRoot::verify(&core.path, owner).unwrap()).unwrap();
    let outer_guard = install.acquire_guard().unwrap();
    let release = CancellationReleaseStore::open_existing(
        VerifiedDataRoot::verify(&core.path, owner).unwrap(),
    )
    .unwrap();
    let inner_guard = release.acquire_guard().unwrap();
    run(
        GuardedCancellationSource::new(
            &install,
            &outer_guard,
            &release,
            &inner_guard,
            &core.proof,
            source,
            &core.target,
        ),
        &release,
        &inner_guard,
    )
}

#[test]
fn observed_reserved_proof_matches_reader_only_after_disposal() {
    let mut fixture = Fixture::new();
    let source = fixture.manifest.source.product.clone();
    let core = CoreProof::with_observer_fixture(&source, PreparationPoint::Reserved, true);
    let caches = Caches::new();
    let before = snapshot(&core.path);
    let mut authority = Authority::new(&core, &source);
    guarded(&core, &source, |context, _, _| {
        let unobserved =
            CancellationEvidenceRequest::new(&core.proof, &source, &core.target).unwrap();
        assert!(unobserved.schema.is_none());
        let (observation, budget) =
            observe_at_for_test(&context, &mut authority, &caches.path).unwrap();
        caches.assert_disposed();
        let (request, metadata) = CancellationEvidenceRequest::with_observation(
            &core.proof,
            &source,
            &core.target,
            observation,
            budget.control().call_id(),
        )
        .unwrap();
        assert_eq!(request.schema, Some(9));
        assert_eq!(metadata.profile(), "compiled-fresh-schema9-exact-v1");
        assert!(!metadata.sqlite_source_id().is_empty());
        assert!(!metadata.compile_options().is_empty());
        bind_test_catalog(&mut fixture, &request);
        let mut reviewed = review_request(&fixture, &request);
        assert_eq!(
            reviewed.status(),
            ReviewStatus::ReviewedMatchingRequest,
            "{:?}",
            reviewed.rejection()
        );
        reviewed.attach_schema_observation(metadata);
        assert!(reviewed.schema_observation().is_some());
        let (control, _) = ReadControl::new(Duration::from_secs(30)).unwrap();
        assert_eq!(
            review_evidence(
                "synthetic-reviewed-entry",
                &fixture.path,
                &source,
                &fixture.manifest.environment,
                Some(&request),
                &control
            )
            .rejection(),
            Some(ReadError::UntrustedSource)
        );
    });
    assert_eq!(authority.calls, 3);
    assert_eq!(snapshot(&core.path), before);
    assert!(core.marker().exists());
    caches.finish();
}

#[test]
fn observed_capability_rejects_other_root_task_products_and_revocation() {
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    let first = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    let second = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    assert_eq!(first.proof.operation_id(), second.proof.operation_id());
    let before = snapshot(&first.path);
    let caches = Caches::new();
    for fault in 0..4 {
        let mut authority = Authority::new(&first, source);
        guarded(&first, source, |context, _, _| {
            let (observation, budget) =
                observe_at_for_test(&context, &mut authority, &caches.path).unwrap();
            caches.assert_disposed();
            let call = budget.control().call_id();
            let result = match fault {
                0 => CancellationEvidenceRequest::with_observation(
                    &second.proof,
                    source,
                    &second.target,
                    observation,
                    call,
                ),
                1 => CancellationEvidenceRequest::with_observation(
                    &first.proof,
                    source,
                    &first.target,
                    observation,
                    call + 1,
                ),
                2 => CancellationEvidenceRequest::with_observation(
                    &first.proof,
                    &different_manifest(source),
                    &first.target,
                    observation,
                    call,
                ),
                _ => {
                    authority.active.store(false, Ordering::Release);
                    CancellationEvidenceRequest::with_observation(
                        &first.proof,
                        source,
                        &first.target,
                        observation,
                        call,
                    )
                }
            };
            assert!(matches!(
                result,
                Err(ReadError::IdentityMismatch | ReadError::Cancelled)
            ));
        });
    }
    assert_eq!(snapshot(&first.path), before);
    assert!(first.marker().exists() && second.marker().exists());
    caches.finish();
}

#[test]
fn revoked_tasks_at_each_live_checkpoint_return_no_observation() {
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    let core = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    let before = snapshot(&core.path);
    for checkpoint in 1..=3 {
        let caches = Caches::new();
        let mut authority = Authority::new(&core, source);
        authority.revoke_at = Some(checkpoint);
        guarded(&core, source, |context, _, _| {
            assert!(matches!(
                observe_at_for_test(&context, &mut authority, &caches.path),
                Err(ObservationError::TaskAuthorization)
            ));
        });
        caches.finish();
    }
    assert_eq!(snapshot(&core.path), before);
    assert!(core.marker().exists());
}

#[test]
fn readonly_core_respects_raw_read_budget_without_mutation_or_sqlite() {
    struct DenyRaw {
        denied: Cell<bool>,
        hashes: Cell<usize>,
    }
    impl PreparationHasher for DenyRaw {
        fn before_read(&self, bytes: u64) -> std::io::Result<()> {
            if bytes > 0 {
                self.denied.set(true);
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            Ok(())
        }
        fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
            assert!(
                !self.denied.get(),
                "must stop immediately after raw-read denial"
            );
            self.hashes.set(self.hashes.get() + 1);
            Hasher.sha256(source)
        }
    }
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    let core = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    let before = snapshot(&core.path);
    guarded(&core, source, |_, release, guard| {
        let hasher = DenyRaw {
            denied: Cell::new(false),
            hashes: Cell::new(0),
        };
        assert!(release
            .verify_cancellation_observation(guard, &core.proof, &hasher)
            .is_err());
        assert!(hasher.denied.get());
        assert!(hasher.hashes.get() > 0);
        release
            .verify_cancellation_observation(guard, &core.proof, &Hasher)
            .unwrap();
    });
    assert_eq!(snapshot(&core.path), before);
    assert!(core.marker().exists());
}

#[test]
fn source_identity_drift_is_rejected_before_creating_a_copy() {
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    let core = CoreProof::with_observer_fixture(source, PreparationPoint::Reserved, true);
    let caches = Caches::new();
    let path = core.path.join("userdb.sqlite3");
    let before = snapshot(&core.path);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let mut authority = Authority::new(&core, source);
    guarded(&core, source, |context, _, _| {
        assert!(matches!(
            observe_at_for_test(&context, &mut authority, &caches.path),
            Err(ObservationError::ProofChanged)
        ));
    });
    assert!(!caches.path.join("RadishLex").exists());
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(snapshot(&core.path), before);
    caches.finish();
}

#[test]
fn snapshot_and_maintenance_branches_cannot_enter_the_no_snapshot_observer() {
    let fixture = Fixture::new();
    let source = &fixture.manifest.source.product;
    for point in [
        PreparationPoint::SnapshotReady,
        PreparationPoint::SourcePrepared,
    ] {
        let core = CoreProof::with_observer_fixture(source, point, true);
        let caches = Caches::new();
        let mut authority = Authority::new(&core, source);
        guarded(&core, source, |context, _, _| {
            assert!(matches!(
                observe_at_for_test(&context, &mut authority, &caches.path),
                Err(ObservationError::UnsupportedPreparation)
            ));
        });
        assert_eq!(authority.calls, 0);
        assert!(!caches.path.join("RadishLex").exists());
        caches.finish();
    }
}
