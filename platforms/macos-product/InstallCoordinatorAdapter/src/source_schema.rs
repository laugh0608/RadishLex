//! Guard-bound source-schema observation for qualification review. An observation
//! never grants cancellation authority and cannot be serialized or reconstructed.

#[path = "source_schema_fs.rs"]
pub(crate) mod filesystem;
#[path = "source_schema_native.rs"]
mod native;

use crate::qualification_reader::{
    self, CancellationEvidenceRequest, EvidenceEnvironment, EvidenceReview, ReadControl,
};
pub use crate::schema_observer_budget::BudgetError;
use crate::schema_observer_budget::{ObservationControl, ObserverBudget, Phase, BLOCK_SIZE};
use crate::CancellationReleaseProductAuthority;
pub use filesystem::FsError;
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallProcessGuard, InstallReceiptStore,
    ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    CancellationReleaseCheckpoint, CancellationReleaseReceipt, CancellationReleaseStore,
    PreparationFamily, PreparationHasher, UpgradeProcessGuard,
};
use radishlex_ime_userdb::schema_observer::{inspect_private_copy, CopyObservation, ObserverError};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Revocation/deadline supplied by a current Installer task. This alone grants
/// no source access: the authority must also revalidate the exact proof,
/// installed product identities and both programs' native quiescence.
pub struct SchemaTaskControl {
    deadline: Instant,
    active: Arc<dyn Fn() -> bool + Send + Sync>,
}
impl SchemaTaskControl {
    pub fn new(deadline: Instant, active: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        Self { deadline, active }
    }
}

/// Trusted product-side authority, implemented by the native Installer bridge.
/// A task liveness callback must remain bound to the supplied immutable proof
/// and source; revocation or owner drop must be permanent for that task.
pub trait SourceSchemaAuthority: CancellationReleaseProductAuthority {
    fn schema_task_control(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
    ) -> Result<SchemaTaskControl, ObservationError>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum ObservationError {
    Budget(BudgetError),
    Filesystem(FsError),
    Sqlite(ObserverError),
    ProofChanged,
    TaskAuthorization,
    UnsupportedPreparation,
    Reader(qualification_reader::ReadError),
    ScratchRetained {
        original: Box<ObservationError>,
        cleanup: FsError,
    },
}
impl std::fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "source schema observation rejected: {self:?}")
    }
}
impl std::error::Error for ObservationError {}
impl From<BudgetError> for ObservationError {
    fn from(value: BudgetError) -> Self {
        Self::Budget(value)
    }
}
impl From<FsError> for ObservationError {
    fn from(value: FsError) -> Self {
        Self::Filesystem(value)
    }
}
impl From<ObserverError> for ObservationError {
    fn from(value: ObserverError) -> Self {
        Self::Sqlite(value)
    }
}

/// Borrowed continuous guards and immutable request. The source path is always
/// derived from the verified install store, never from the evidence package.
pub struct GuardedCancellationSource<'a> {
    install: &'a InstallReceiptStore,
    outer_guard: &'a InstallProcessGuard,
    release: &'a CancellationReleaseStore,
    inner_guard: &'a UpgradeProcessGuard,
    proof: &'a CancellationReleaseReceipt,
    source: &'a ProductArtifactIdentity,
    target: &'a ProductArtifactIdentity,
}
impl<'a> GuardedCancellationSource<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        install: &'a InstallReceiptStore,
        outer_guard: &'a InstallProcessGuard,
        release: &'a CancellationReleaseStore,
        inner_guard: &'a UpgradeProcessGuard,
        proof: &'a CancellationReleaseReceipt,
        source: &'a ProductArtifactIdentity,
        target: &'a ProductArtifactIdentity,
    ) -> Self {
        Self {
            install,
            outer_guard,
            release,
            inner_guard,
            proof,
            source,
            target,
        }
    }
    fn verify(
        &self,
        authority: &mut impl SourceSchemaAuthority,
        budget: &mut ObserverBudget,
    ) -> Result<(), ObservationError> {
        budget.check_io()?;
        self.install
            .verify_guard(self.outer_guard)
            .map_err(|_| ObservationError::ProofChanged)?;
        validate_preparation_cancellation_outer(
            self.proof.archive().outer().previous_bytes(),
            self.proof.archive().outer().new_bytes(),
            self.proof.operation_id(),
            self.proof.legacy_outer_operation_id(),
            self.install.root_identity(),
            self.source,
            self.target,
        )
        .map_err(|_| ObservationError::ProofChanged)?;
        // The native authority checks current task, both source bundles and
        // sealed target around a fresh quiescence observation. No cached bool.
        if !authority.confirm_source_and_quiescence(
            self.proof,
            self.source,
            self.target,
            CancellationReleaseCheckpoint::Begin,
        ) {
            return Err(ObservationError::TaskAuthorization);
        }
        budget.check_io()?;
        let hasher = BoundedHasher {
            budget: RefCell::new(budget),
            failure: Cell::new(None),
            steps: Cell::new(0),
        };
        let verified =
            self.release
                .verify_cancellation_observation(self.inner_guard, self.proof, &hasher);
        if let Some(error) = hasher.failure.get() {
            return Err(error.into());
        }
        verified.map_err(|_| ObservationError::ProofChanged)?;
        hasher.budget.borrow().check_io()?;
        self.install
            .verify_guard(self.outer_guard)
            .map_err(|_| ObservationError::ProofChanged)?;
        Ok(())
    }
}

struct BoundedHasher<'a> {
    budget: RefCell<&'a mut ObserverBudget>,
    failure: Cell<Option<BudgetError>>,
    steps: Cell<u32>,
}
impl BoundedHasher<'_> {
    fn charge(&self, bytes: u64) -> std::io::Result<()> {
        if self.failure.get().is_some() {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        if let Err(error) = self.budget.borrow_mut().charge_read(bytes) {
            self.failure.set(Some(error));
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        Ok(())
    }
}
impl PreparationHasher for BoundedHasher<'_> {
    fn before_read(&self, maximum_bytes: u64) -> std::io::Result<()> {
        // Bound metadata/history traversal independently of byte lengths.
        if self.steps.get() >= 16_384 {
            self.failure.set(Some(BudgetError::LimitExceeded));
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        self.steps.set(self.steps.get() + 1);
        self.charge(maximum_bytes)
    }
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
        let mut digest = Sha256::new();
        let mut buffer = [0u8; BLOCK_SIZE];
        loop {
            self.charge(buffer.len() as u64)?;
            let count = source.read(&mut buffer)?;
            if count == 0 {
                return Ok(digest.finalize().into());
            }
            digest.update(&buffer[..count]);
        }
    }
}

struct FullBinding {
    canonical_proof: Vec<u8>,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    family: PreparationFamily,
}
impl FullBinding {
    fn new(context: &GuardedCancellationSource<'_>) -> Result<Self, ObservationError> {
        let preparation = context.proof.preparation();
        if preparation.snapshot_identity().is_some()
            || preparation.snapshot_schema_version().is_some()
            || preparation.phase()
                >= radishlex_ime_product_upgrade::PreparationPhase::MaintenanceIntent
            || preparation.handoff_started()
        {
            return Err(ObservationError::UnsupportedPreparation);
        }
        CancellationEvidenceRequest::new(context.proof, context.source, context.target)
            .map_err(ObservationError::Reader)?;
        Ok(Self {
            canonical_proof: context
                .proof
                .encode()
                .map_err(|_| ObservationError::ProofChanged)?,
            source: context.source.clone(),
            target: context.target.clone(),
            family: context
                .proof
                .archive()
                .source()
                .source_ready()
                .ok_or(ObservationError::ProofChanged)?
                .clone(),
        })
    }
    fn matches(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
    ) -> bool {
        proof.encode().as_ref() == Ok(&self.canonical_proof)
            && source == &self.source
            && target == &self.target
            && proof.archive().source().source_ready() == Some(&self.family)
    }
}

/// Audit metadata, deliberately not a qualification or a reusable capability.
#[derive(Debug)]
pub struct SourceSchemaReview {
    profile: &'static str,
    sqlite_source_id: String,
    compile_options: Vec<String>,
    call_id: u64,
}
impl SourceSchemaReview {
    pub fn profile(&self) -> &'static str {
        self.profile
    }
    pub fn sqlite_source_id(&self) -> &str {
        &self.sqlite_source_id
    }
    pub fn compile_options(&self) -> &[String] {
        &self.compile_options
    }
    pub fn call_id(&self) -> u64 {
        self.call_id
    }
}

/// Created only after confirmed scratch disposition and final live verification.
/// It has no public constructor, Clone, serialization or integer conversion.
pub(crate) struct SourceSchemaObservation {
    binding: FullBinding,
    sqlite: CopyObservation,
    control: ObservationControl,
}
impl SourceSchemaObservation {
    pub(crate) fn consume(
        self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        call_id: u64,
    ) -> Result<(i64, SourceSchemaReview), qualification_reader::ReadError> {
        self.control.check_work().map_err(|error| match error {
            BudgetError::Cancelled => qualification_reader::ReadError::Cancelled,
            BudgetError::Deadline | BudgetError::EntryTooShort => {
                qualification_reader::ReadError::DeadlineExceeded
            }
            BudgetError::LimitExceeded | BudgetError::Overflow => {
                qualification_reader::ReadError::ResourceLimit
            }
        })?;
        if self.control.call_id() != call_id || !self.binding.matches(proof, source, target) {
            return Err(qualification_reader::ReadError::IdentityMismatch);
        }
        Ok((
            self.sqlite.schema_version(),
            SourceSchemaReview {
                profile: self.sqlite.profile(),
                sqlite_source_id: self.sqlite.sqlite_source_id().into(),
                compile_options: self.sqlite.compile_options().to_vec(),
                call_id,
            },
        ))
    }
}

fn sqlite_budget(error: BudgetError) -> ObserverError {
    match error {
        BudgetError::Cancelled => ObserverError::Cancelled,
        BudgetError::Deadline | BudgetError::EntryTooShort => ObserverError::Deadline,
        BudgetError::LimitExceeded | BudgetError::Overflow => ObserverError::LimitExceeded,
    }
}

fn inspect_owned_copy(
    context: &GuardedCancellationSource<'_>,
    authority: &mut impl SourceSchemaAuthority,
    mut scratch: filesystem::Scratch,
    budget: &mut ObserverBudget,
) -> Result<CopyObservation, ObservationError> {
    let work = (|| {
        scratch.capture(budget)?;
        context.verify(authority, budget)?;
        budget.checkpoint(Phase::Sqlite)?;
        scratch.mark_sqlite_started(budget)?;
        let path = scratch.database_path();
        let control = budget.control();
        let shared = RefCell::new(&mut *budget);
        let mut filesystem_failure = None;
        let observed = inspect_private_copy(
            &path,
            Arc::new(move || control.check_work().map_err(sqlite_budget)),
            &mut |bytes| {
                shared
                    .borrow_mut()
                    .charge_read(bytes)
                    .map_err(sqlite_budget)
            },
            &mut || {
                scratch
                    .verify_copy(&mut shared.borrow_mut())
                    .map_err(|error| {
                        filesystem_failure = Some(error);
                        ObserverError::EvidenceChanged
                    })
            },
        );
        if matches!(observed, Err(ObserverError::CloseUnconfirmed)) {
            return Err(ObserverError::CloseUnconfirmed.into());
        }
        if let Some(error) = filesystem_failure {
            return Err(error.into());
        }
        let observed = observed?;
        budget.checkpoint(Phase::Verify)?;
        scratch.revalidate_source(budget)?;
        Ok(observed)
    })();
    dispose_observation(work, scratch, budget)
}

fn dispose_observation(
    work: Result<CopyObservation, ObservationError>,
    scratch: filesystem::Scratch,
    budget: &mut ObserverBudget,
) -> Result<CopyObservation, ObservationError> {
    if matches!(
        work,
        Err(ObservationError::Sqlite(ObserverError::CloseUnconfirmed))
            | Err(ObservationError::Filesystem(FsError::Close))
    ) {
        let cleanup = scratch.retain().err().unwrap_or(FsError::Close);
        return Err(ObservationError::ScratchRetained {
            original: Box::new(work.err().ok_or(ObservationError::ProofChanged)?),
            cleanup,
        });
    }
    match (work, scratch.dispose(budget)) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (result, Err(cleanup)) => Err(ObservationError::ScratchRetained {
            original: Box::new(
                result
                    .err()
                    .unwrap_or(ObservationError::Filesystem(FsError::ScratchRetained)),
            ),
            cleanup,
        }),
    }
}

fn observe_with_factory(
    context: &GuardedCancellationSource<'_>,
    authority: &mut impl SourceSchemaAuthority,
    create: impl FnOnce(&PreparationFamily, &mut ObserverBudget) -> Result<filesystem::Scratch, FsError>,
) -> Result<(SourceSchemaObservation, ObserverBudget), ObservationError> {
    let task = authority.schema_task_control(context.proof, context.source)?;
    let control = ObservationControl::enter_task(task.deadline, task.active)?;
    let mut budget = ObserverBudget::new(control.clone());
    let binding = FullBinding::new(context)?;
    budget.admit_family([
        Some(binding.family.database.byte_len),
        binding.family.wal.as_ref().map(|v| v.byte_len),
        binding.family.shm.as_ref().map(|v| v.byte_len),
        binding.family.journal.as_ref().map(|v| v.byte_len),
    ])?;
    context.verify(authority, &mut budget)?;
    let scratch = create(&binding.family, &mut budget)?;
    let sqlite = inspect_owned_copy(context, authority, scratch, &mut budget)?;
    budget.checkpoint(Phase::Return)?;
    context.verify(authority, &mut budget)?;
    Ok((
        SourceSchemaObservation {
            binding,
            sqlite,
            control,
        },
        budget,
    ))
}

#[cfg(test)]
pub(crate) fn observe_at_for_test(
    context: &GuardedCancellationSource<'_>,
    authority: &mut impl SourceSchemaAuthority,
    caches: &Path,
) -> Result<(SourceSchemaObservation, ObserverBudget), ObservationError> {
    observe_with_factory(context, authority, |family, budget| {
        filesystem::Scratch::create_at(
            caches,
            context.install.data_root_path(),
            &context.proof.preparation().binding().data_root,
            family,
            budget,
        )
    })
}

/// Observe the exact source on a disposable private copy, then consume that
/// observation once while reading qualification evidence. The fixed production
/// catalog remains empty, so this cannot enable final cancellation.
#[allow(clippy::too_many_arguments)]
pub fn review_evidence_with_source_schema(
    context: GuardedCancellationSource<'_>,
    authority: &mut impl SourceSchemaAuthority,
    entry_id: &str,
    evidence_root: &Path,
    environment: &EvidenceEnvironment,
    read_control: &ReadControl,
) -> Result<EvidenceReview, ObservationError> {
    // A rejected or incomplete package cannot justify making a sensitive copy.
    // The current empty catalog therefore exits before any source/cache access.
    let initial = qualification_reader::review_evidence(
        entry_id,
        evidence_root,
        context.source,
        environment,
        None,
        read_control,
    );
    if initial.status() != qualification_reader::ReviewStatus::ReviewedUnbound {
        return Ok(initial);
    }
    let (observation, mut budget) = observe_with_factory(&context, authority, |family, budget| {
        filesystem::Scratch::create(
            context.install.data_root_path(),
            &context.proof.preparation().binding().data_root,
            family,
            evidence_root,
            budget,
        )
    })?;
    let control = budget.control();
    let (request, metadata) = CancellationEvidenceRequest::with_observation(
        context.proof,
        context.source,
        context.target,
        observation,
        control.call_id(),
    )
    .map_err(ObservationError::Reader)?;
    let mut review = qualification_reader::review_evidence(
        entry_id,
        evidence_root,
        context.source,
        environment,
        Some(&request),
        read_control,
    );
    budget.checkpoint(Phase::Return)?;
    context.verify(authority, &mut budget)?;
    control.check_work()?;
    review.attach_schema_observation(metadata);
    Ok(review)
}
