//! Read-only inspection of independently reviewed evidence. Reports are never
//! cancellation authority; production has no approved evidence catalog entries.

use radishlex_ime_product_install::ProductArtifactIdentity;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[path = "qualification_reader_binding.rs"]
mod binding;
#[allow(unsafe_code)]
#[path = "qualification_reader_fs.rs"]
mod filesystem;
#[path = "qualification_reader_model.rs"]
mod model;
#[path = "qualification_reader_validation.rs"]
mod validation;
pub use binding::CancellationEvidenceRequest;
pub use model::{
    Architecture, CaseSemantics, Environment as EvidenceEnvironment, ExecutionMode, IsolationMethod,
};
#[cfg(test)]
#[path = "qualification_reader_model_tests.rs"]
mod model_tests;
#[cfg(test)]
#[path = "qualification_reader_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    UntrustedSource,
    MissingEvidence,
    CoverageMissing,
    EnvironmentUnsupported,
    PolicyUnavailable,
    Revoked,
    SchemaUnsupported,
    SchemaObservationUnavailable,
    IdentityMismatch,
    EvidenceChanged,
    Malformed,
    ResourceLimit,
    IoFailure,
    AssertionFailed,
    ContradictoryRun,
    Cancelled,
    DeadlineExceeded,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "qualification evidence rejected: {self:?}")
    }
}

impl std::error::Error for ReadError {}

/// Bounded, revocable read budget; it is not an Installer task authorization.
pub struct ReadControl {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct ReadCancellation(Arc<AtomicBool>);

impl ReadCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

impl ReadControl {
    pub fn new(budget: Duration) -> Result<(Self, ReadCancellation), ReadError> {
        if budget.is_zero() || budget > Duration::from_secs(300) {
            return Err(ReadError::ResourceLimit);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now()
            .checked_add(budget)
            .ok_or(ReadError::ResourceLimit)?;
        Ok((
            Self {
                deadline,
                cancelled: cancelled.clone(),
            },
            ReadCancellation(cancelled),
        ))
    }

    pub(super) fn check(&self) -> Result<(), ReadError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(ReadError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(ReadError::DeadlineExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Rejected,
    ReviewedUnbound,
    ReviewedMatchingRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseReviewStatus {
    Covered,
    Missing,
}

#[derive(Debug)]
pub struct CaseReview {
    pub case_id: String,
    pub status: CaseReviewStatus,
}

#[derive(Debug)]
pub struct EvidenceFileObservation {
    pub relative_path: String,
    pub device: u64,
    pub inode: u64,
    pub owner: u32,
    pub group: u32,
    pub mode: u32,
    pub links: u64,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub struct RequestReviewBinding {
    pub proof_sha256: String,
    pub family_sha256: String,
    pub source_manifest_sha256: String,
    pub target_manifest_sha256: String,
    pub root_device: u64,
    pub root_inode: u64,
    pub root_owner: u32,
    pub root_mode: u32,
}

/// An audit result, deliberately not serializable or convertible into a
/// CancellationSourceQualification. It carries no mutation or task authority.
#[derive(Debug)]
pub struct EvidenceReview {
    status: ReviewStatus,
    rejection: Option<ReadError>,
    policy_revision: Option<String>,
    entry_id: Option<String>,
    manifest_sha256: Option<String>,
    review_record_sha256: Option<String>,
    source: Option<ProductArtifactIdentity>,
    environment: Option<EvidenceEnvironment>,
    source_schema: Option<i64>,
    files: Vec<EvidenceFileObservation>,
    cases: Vec<CaseReview>,
    request: Option<RequestReviewBinding>,
}

impl EvidenceReview {
    fn rejected(error: ReadError) -> Self {
        Self {
            status: ReviewStatus::Rejected,
            rejection: Some(error),
            policy_revision: None,
            entry_id: None,
            manifest_sha256: None,
            review_record_sha256: None,
            source: None,
            environment: None,
            source_schema: None,
            files: Vec::new(),
            cases: Vec::new(),
            request: None,
        }
    }
    pub fn status(&self) -> ReviewStatus {
        self.status
    }
    pub fn rejection(&self) -> Option<ReadError> {
        self.rejection
    }
    pub fn policy_revision(&self) -> Option<&str> {
        self.policy_revision.as_deref()
    }
    pub fn entry_id(&self) -> Option<&str> {
        self.entry_id.as_deref()
    }
    pub fn manifest_sha256(&self) -> Option<&str> {
        self.manifest_sha256.as_deref()
    }
    pub fn review_record_sha256(&self) -> Option<&str> {
        self.review_record_sha256.as_deref()
    }
    pub fn source(&self) -> Option<&ProductArtifactIdentity> {
        self.source.as_ref()
    }
    pub fn environment(&self) -> Option<&EvidenceEnvironment> {
        self.environment.as_ref()
    }
    pub fn source_schema(&self) -> Option<i64> {
        self.source_schema
    }
    pub fn files(&self) -> &[EvidenceFileObservation] {
        &self.files
    }
    pub fn cases(&self) -> &[CaseReview] {
        &self.cases
    }
    pub fn request_binding(&self) -> Option<&RequestReviewBinding> {
        self.request.as_ref()
    }
}

// These records have no public constructors and cannot be loaded from JSON.
// A future catalog requires separately reviewed product assembly and freshness.
struct Catalog {
    revision: String,
    entries: Vec<ApprovedEntry>,
    revoked: BTreeSet<String>,
}

struct ApprovedEntry {
    id: String,
    manifest_sha256: String,
    review_record_sha256: String,
    source: model::ExactSource,
    target: ProductArtifactIdentity,
    environment: EvidenceEnvironment,
    schema: model::SourceSchema,
    collector: model::Collector,
    profile: String,
    cases: Vec<model::CaseDefinition>,
}

/// Inspect against the product's fixed catalog. It is intentionally empty:
/// no source 39 program qualification or production issuer has been approved.
/// An untrusted entry is rejected before accessing its proposed filesystem root.
pub fn review_evidence(
    entry_id: &str,
    root: &Path,
    source: &ProductArtifactIdentity,
    environment: &EvidenceEnvironment,
    request: Option<&CancellationEvidenceRequest>,
    control: &ReadControl,
) -> EvidenceReview {
    let catalog = Catalog {
        revision: "no-approved-qualification-v1".into(),
        entries: Vec::new(),
        revoked: BTreeSet::new(),
    };
    review_with_catalog(
        &catalog,
        entry_id,
        root,
        source,
        environment,
        request,
        control,
    )
}

#[allow(clippy::too_many_arguments)]
fn review_with_catalog(
    catalog: &Catalog,
    entry_id: &str,
    root: &Path,
    source: &ProductArtifactIdentity,
    environment: &EvidenceEnvironment,
    request: Option<&CancellationEvidenceRequest>,
    control: &ReadControl,
) -> EvidenceReview {
    match inspect(
        catalog,
        entry_id,
        root,
        source,
        environment,
        request,
        control,
    ) {
        Ok(report) => report,
        Err(error) => EvidenceReview::rejected(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect(
    catalog: &Catalog,
    entry_id: &str,
    root: &Path,
    source: &ProductArtifactIdentity,
    environment: &EvidenceEnvironment,
    request: Option<&CancellationEvidenceRequest>,
    control: &ReadControl,
) -> Result<EvidenceReview, ReadError> {
    control.check()?;
    if catalog.revision.is_empty() {
        return Err(ReadError::PolicyUnavailable);
    }
    if catalog.revoked.contains(entry_id) {
        return Err(ReadError::Revoked);
    }
    let mut candidates = catalog.entries.iter().filter(|entry| entry.id == entry_id);
    let entry = candidates.next().ok_or(ReadError::UntrustedSource)?;
    if candidates.next().is_some()
        || !validation::valid_digest(&entry.manifest_sha256)
        || !validation::valid_digest(&entry.review_record_sha256)
    {
        return Err(ReadError::PolicyUnavailable);
    }
    if &entry.source.product != source {
        return Err(ReadError::IdentityMismatch);
    }
    if &entry.environment != environment {
        return Err(ReadError::EnvironmentUnsupported);
    }
    let evidence = filesystem::EvidenceRoot::open(root, control)?;
    let manifest_file =
        evidence.read_file("manifest.json", model::MAX_MANIFEST_BYTES, true, control)?;
    let manifest_hash = hex(&manifest_file.sha256);
    if manifest_hash != entry.manifest_sha256 {
        return Err(ReadError::EvidenceChanged);
    }
    let manifest =
        validation::parse_manifest(manifest_file.bytes.as_deref().ok_or(ReadError::IoFailure)?)?;
    control.check()?;
    if manifest.source != entry.source
        || manifest.collector != entry.collector
        || manifest.profile != entry.profile
    {
        return Err(ReadError::IdentityMismatch);
    }
    if manifest.environment != entry.environment {
        return Err(ReadError::EnvironmentUnsupported);
    }
    if manifest.schema.source != entry.schema {
        return Err(ReadError::SchemaUnsupported);
    }
    verify_closed_inventory(&manifest, evidence.inventory())?;
    let mut files = vec![file_observation("manifest.json", &manifest_file)];
    let mut total = manifest_file.identity.size;
    for expected in &manifest.files {
        control.check()?;
        total = total
            .checked_add(expected.bytes)
            .ok_or(ReadError::ResourceLimit)?;
        if total > model::MAX_TOTAL_BYTES {
            return Err(ReadError::ResourceLimit);
        }
        let actual = evidence.read_file(
            &expected.path,
            model::MAX_FILE_BYTES as usize,
            false,
            control,
        )?;
        if actual.identity.size != expected.bytes || hex(&actual.sha256) != expected.sha256 {
            return Err(ReadError::EvidenceChanged);
        }
        files.push(file_observation(&expected.path, &actual));
    }
    let missing = validation::validate_coverage(&manifest, &entry.cases)?;
    let required_groups = [
        model::CoverageGroup::CompatiblePostcondition,
        model::CoverageGroup::HistoricalCombination,
        model::CoverageGroup::NormalPrograms,
        model::CoverageGroup::DataSemantics,
        model::CoverageGroup::BlockingState,
        model::CoverageGroup::ReaderRejection,
        model::CoverageGroup::SubsequentRecovery,
    ];
    let complete_profile = required_groups
        .iter()
        .all(|group| entry.cases.iter().any(|case| &case.group == group));
    let cases = entry
        .cases
        .iter()
        .map(|case| CaseReview {
            case_id: case.case_id.clone(),
            status: if missing.contains(&case.case_id) {
                CaseReviewStatus::Missing
            } else {
                CaseReviewStatus::Covered
            },
        })
        .collect();
    let request_binding = if let Some(request) = request {
        if request.source != *source || request.target != entry.target {
            return Err(ReadError::IdentityMismatch);
        }
        let schema = request
            .schema
            .ok_or(ReadError::SchemaObservationUnavailable)?;
        if schema != entry.schema.version {
            return Err(ReadError::SchemaUnsupported);
        }
        if !required_groups.iter().all(|group| {
            entry
                .cases
                .iter()
                .any(|case| &case.group == group && same_context(&case.scenario, &request.scenario))
        }) {
            return Err(ReadError::CoverageMissing);
        }
        Some(RequestReviewBinding {
            proof_sha256: request.proof_sha256.clone(),
            family_sha256: request.family_sha256.clone(),
            source_manifest_sha256: source.product_manifest_sha256().into(),
            target_manifest_sha256: request.target.product_manifest_sha256().into(),
            root_device: request.root.device_id(),
            root_inode: request.root.inode(),
            root_owner: request.root.owner_id(),
            root_mode: request.root.mode(),
        })
    } else {
        None
    };
    evidence.verify_unchanged(control)?;
    control.check()?;
    let rejected = !missing.is_empty() || !complete_profile;
    Ok(EvidenceReview {
        status: if rejected {
            ReviewStatus::Rejected
        } else if request_binding.is_some() {
            ReviewStatus::ReviewedMatchingRequest
        } else {
            ReviewStatus::ReviewedUnbound
        },
        rejection: rejected.then_some(ReadError::CoverageMissing),
        policy_revision: Some(catalog.revision.clone()),
        entry_id: Some(entry.id.clone()),
        manifest_sha256: Some(manifest_hash),
        review_record_sha256: Some(entry.review_record_sha256.clone()),
        source: Some(source.clone()),
        environment: Some(environment.clone()),
        source_schema: Some(entry.schema.version),
        files,
        cases,
        request: request_binding,
    })
}

fn verify_closed_inventory(
    manifest: &model::Manifest,
    inventory: &BTreeMap<String, filesystem::EntryIdentity>,
) -> Result<(), ReadError> {
    let mut expected = BTreeMap::from([("manifest.json".to_owned(), filesystem::EntryKind::File)]);
    for file in &manifest.files {
        if file.path == "manifest.json" {
            return Err(ReadError::Malformed);
        }
        if !filesystem::valid_relative_path(&file.path) {
            return Err(ReadError::Malformed);
        }
        expected.insert(file.path.clone(), filesystem::EntryKind::File);
        let mut directory = PathBuf::from(&file.path);
        directory.pop();
        while !directory.as_os_str().is_empty() {
            let name = directory.to_str().ok_or(ReadError::Malformed)?.to_owned();
            if expected.insert(name, filesystem::EntryKind::Directory)
                == Some(filesystem::EntryKind::File)
            {
                return Err(ReadError::Malformed);
            }
            directory.pop();
        }
    }
    if expected.len() != inventory.len() {
        return Err(ReadError::EvidenceChanged);
    }
    for (name, kind) in expected {
        if inventory.get(&name).map(|identity| identity.kind) != Some(kind) {
            return Err(ReadError::EvidenceChanged);
        }
    }
    Ok(())
}

fn same_context(a: &model::Scenario, b: &model::Scenario) -> bool {
    a.maintenance == b.maintenance
        && a.outer_state == b.outer_state
        && a.data_predecessor == b.data_predecessor
        && a.lifecycle == b.lifecycle
        && a.family == b.family
        && a.repeated_cancellation == b.repeated_cancellation
}

fn file_observation(path: &str, read: &filesystem::FileRead) -> EvidenceFileObservation {
    let identity = &read.identity;
    EvidenceFileObservation {
        relative_path: path.into(),
        device: identity.device,
        inode: identity.inode,
        owner: identity.owner,
        group: identity.group,
        mode: identity.mode,
        links: identity.links,
        bytes: identity.size,
        sha256: hex(&read.sha256),
    }
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
