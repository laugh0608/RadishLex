//! Synthetic-only proof host. No source SQLite connection, production issuer,
//! reader integration, or release of the currently observed marker exists here.
use crate::{CancellationArchiveAuthority, CancellationProductAuthority};
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallFailureCode, InstallOperationKind,
    InstallProcessGuard, InstallReceipt, InstallReceiptStore, ProductArtifactIdentity,
    ProductRelease, ProgramBundleIdentity, ProgramComponent, VerifiedInstallRoot,
    INSTALL_PRODUCT_ID,
};
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationArchiveStore, CancellationReleaseCheckpoint,
    CancellationReleasePort, CancellationReleaseReceipt, CancellationReleaseRequirement,
    CancellationReleaseStore, CancellationSourceCheckpoint, CancellationSourcePort,
    CancellationSourceReceipt, PreparationBinding, PreparationCancellationCheckpoint,
    PreparationCancellationPort, PreparationCancellationRequest, PreparationCancellationStore,
    PreparationFamily, PreparationHasher, PreparationJournalStore, PreparationPhase,
    PreparationReceipt, ProductRelease as DataRelease, SourcePreparationError,
    TerminalReleaseBinding, TerminalReleaseCheckpoint, TerminalReleaseOuterRequirement,
    TerminalReleasePort, TerminalReleaseReceipt, TerminalReleaseStore, UpgradeArtifactIdentity,
    UpgradeArtifactSlot, UpgradeFailureCode, UpgradeProcessGuard, UpgradeReceipt,
    UpgradeReceiptStore, VerifiedDataRoot,
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OPERATION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DATA: &str = "cccccccccccccccccccccccccccccccc";
const PRIOR_CANCEL: &str = "dddddddddddddddddddddddddddddddd";
// This describes only the fresh-schema9 fixture's historical data receipt. It
// is never a current schema observation: Reserved has no snapshot schema and
// the observer must independently inspect its captured DB/WAL view.
const SYNTHETIC_HISTORICAL_SCHEMA: i64 = 9;
const CONTROL_ROOTS: [&str; 4] = [
    ".radishlex-install-v1",
    ".radishlex-install-history-v1",
    ".radishlex-upgrade-v1",
    ".radishlex-upgrade-history-v1",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProofError {
    EvidenceChanged,
    VerificationFailed,
    Replay,
    ControlRejected,
    UnsupportedFixture,
}
type Result<T> = std::result::Result<T, ProofError>;

/// Every variant is generated entirely in the current case. Historical marker
/// release occurs only during fixture construction, before observer entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyntheticPredecessor {
    NoIndex,
    LegacyData,
    TerminalV1,
    TerminalV2,
    CancellationV2,
    RepeatedCancellationV2,
}

/// No public fields, deserialization, arbitrary schema, or cross-call reuse.
#[derive(PartialEq, Eq)]
pub(crate) struct ProofBinding {
    canonical_proof: Vec<u8>,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    family: PreparationFamily,
    call_id: u64,
}
impl std::fmt::Debug for ProofBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProofBinding { <private synthetic binding> }")
    }
}

struct CoreHasher<'a> {
    check: RefCell<&'a mut dyn FnMut(u64) -> bool>,
}
impl PreparationHasher for CoreHasher<'_> {
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            // Reserve a complete block before IO. Short reads and EOF are
            // intentionally overcounted, so the shared cap is never exceeded.
            if !(self.check.borrow_mut())(buffer.len() as u64) {
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
            }
            let count = source.read(&mut buffer)?;
            if count == 0 {
                return Ok(hash.finalize().into());
            }
            hash.update(&buffer[..count]);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Material {
    relative: PathBuf,
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
    links: u64,
    len: u64,
    modified: (i64, i64),
    changed: (i64, i64),
    digest: Option<[u8; 32]>,
}

/// Both real guards remain owned for the entire observer window. Destruction
/// closes only these guards; the case capability owns exact fixture teardown.
pub(crate) struct SyntheticProofSession {
    root: PathBuf,
    install: InstallReceiptStore,
    outer_guard: InstallProcessGuard,
    release: CancellationReleaseStore,
    inner_guard: UpgradeProcessGuard,
    guard_store: UpgradeReceiptStore,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    proof: CancellationReleaseReceipt,
    material: Vec<Material>,
    issued: RefCell<BTreeSet<u64>>,
    consumed: RefCell<BTreeSet<u64>>,
}
impl SyntheticProofSession {
    pub(crate) fn new(root: &Path, source: &ProductArtifactIdentity) -> Result<Self> {
        Self::with_predecessor(root, source, SyntheticPredecessor::NoIndex)
    }

    pub(crate) fn synthetic_source() -> Result<ProductArtifactIdentity> {
        let bundle = |component, name: &str| {
            ProgramBundleIdentity::new(
                component,
                format!("dev.radishlex.synthetic.{name}"),
                digest(name.as_bytes()),
                digest(format!("synthetic-signature-{name}").as_bytes()),
            )
            .map_err(|_| ProofError::VerificationFailed)
        };
        ProductArtifactIdentity::new(
            INSTALL_PRODUCT_ID,
            ProductRelease::new("0.1.0", 39).map_err(|_| ProofError::VerificationFailed)?,
            digest(b"schema observer synthetic source manifest"),
            bundle(ProgramComponent::Manager, "manager")?,
            bundle(ProgramComponent::InputMethod, "inputmethod")?,
        )
        .map_err(|_| ProofError::VerificationFailed)
    }

    pub(crate) fn with_predecessor(
        root: &Path,
        source: &ProductArtifactIdentity,
        predecessor: SyntheticPredecessor,
    ) -> Result<Self> {
        // The caller must pass SyntheticCase::source_path(), whose private DB
        // was generated by the userdb fixture. This host never creates a DB.
        if !root.is_absolute() || !root.join("userdb.sqlite3").is_file() {
            return Err(ProofError::EvidenceChanged);
        }
        for name in CONTROL_ROOTS {
            if fs::symlink_metadata(root.join(name)).is_ok() {
                return Err(ProofError::EvidenceChanged);
            }
        }
        let owner = fs::symlink_metadata(root)
            .map_err(|_| ProofError::VerificationFailed)?
            .uid();
        let target = ProductArtifactIdentity::new(
            source.product_id(),
            ProductRelease::new(source.release().product_version(), 41)
                .map_err(|_| ProofError::VerificationFailed)?,
            digest(b"schema observer synthetic target manifest"),
            source.program(ProgramComponent::Manager).clone(),
            source.program(ProgramComponent::InputMethod).clone(),
        )
        .map_err(|_| ProofError::VerificationFailed)?;
        let history = root.join(CONTROL_ROOTS[1]);
        for path in [&history, &history.join(OLD)] {
            DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| ProofError::VerificationFailed)?;
        }
        let install = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(root, owner).map_err(|_| ProofError::EvidenceChanged)?,
        )
        .map_err(|_| ProofError::VerificationFailed)?;
        let outer_guard = install
            .acquire_guard()
            .map_err(|_| ProofError::VerificationFailed)?;
        let mut old = InstallReceipt::new(
            OLD,
            None,
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .map_err(|_| ProofError::VerificationFailed)?;
        install
            .persist(&outer_guard, &old)
            .map_err(|_| ProofError::VerificationFailed)?;
        old.abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .map_err(|_| ProofError::VerificationFailed)?;
        install
            .persist(&outer_guard, &old)
            .map_err(|_| ProofError::VerificationFailed)?;
        let outer_bytes = old.encode().map_err(|_| ProofError::VerificationFailed)?;
        put(&history.join(OLD).join("receipt.json"), &outer_bytes)?;
        let data = UpgradeReceiptStore::open(verified(root, owner)?)
            .map_err(|_| ProofError::VerificationFailed)?;
        let inner_guard = data
            .acquire_guard()
            .map_err(|_| ProofError::VerificationFailed)?;
        let guard_store = UpgradeReceiptStore::open_existing(verified(root, owner)?)
            .map_err(|_| ProofError::VerificationFailed)?;
        let mut allow = |_| true;
        let hasher = CoreHasher {
            check: RefCell::new(&mut allow),
        };
        let mut journal = PreparationJournalStore::attach(data, &inner_guard)
            .map_err(|_| ProofError::VerificationFailed)?;
        let mut previous_data = None;
        if matches!(
            predecessor,
            SyntheticPredecessor::LegacyData
                | SyntheticPredecessor::TerminalV1
                | SyntheticPredecessor::TerminalV2
        ) {
            // An actual persisted terminal v1 data receipt, with distinct outer
            // and data operations. No SQLite migration or source write occurs.
            drop(journal);
            let data = UpgradeReceiptStore::open_existing(verified(root, owner)?)
                .map_err(|_| ProofError::VerificationFailed)?;
            let meta = fs::symlink_metadata(root.join("userdb.sqlite3"))
                .map_err(|_| ProofError::VerificationFailed)?;
            let database = UpgradeArtifactIdentity::private_file(
                UpgradeArtifactSlot::SourceDatabase,
                meta.dev(),
                meta.ino(),
                meta.uid(),
                meta.len(),
            )
            .map_err(|_| ProofError::VerificationFailed)?;
            let mut receipt = UpgradeReceipt::new(
                DATA,
                None,
                data_release(source)?,
                data_release(&target)?,
                Some(SYNTHETIC_HISTORICAL_SCHEMA),
                9,
                vec![data.data_root_identity().clone(), database],
            )
            .map_err(|_| ProofError::VerificationFailed)?;
            data.persist(&inner_guard, &receipt)
                .map_err(|_| ProofError::VerificationFailed)?;
            receipt
                .abort_preserved(UpgradeFailureCode::ProcessNotQuiescent, false)
                .map_err(|_| ProofError::VerificationFailed)?;
            data.persist(&inner_guard, &receipt)
                .map_err(|_| ProofError::VerificationFailed)?;
            previous_data = Some(DATA);
            if matches!(
                predecessor,
                SyntheticPredecessor::TerminalV1 | SyntheticPredecessor::TerminalV2
            ) {
                for suffix in ["-wal", "-shm", "-journal"] {
                    if fs::symlink_metadata(root.join(format!("userdb.sqlite3{suffix}"))).is_ok() {
                        return Err(ProofError::UnsupportedFixture);
                    }
                }
                let terminal = TerminalReleaseStore::attach(data, &inner_guard)
                    .map_err(|_| ProofError::VerificationFailed)?;
                let binding = TerminalReleaseBinding {
                    operation_id: DATA.into(),
                    installed_release: data_release(source)?,
                    installed_product_sha256: source.product_manifest_sha256().into(),
                };
                let mut admission = SyntheticTerminal {
                    stop_at_intent: predecessor == SyntheticPredecessor::TerminalV1,
                };
                if admission.stop_at_intent {
                    // Same compatibility fixture as release history tests: seed
                    // canonical pre-v2 bytes at Reserved, before any index or
                    // archived slot exists; then real core completes the old
                    // writer lifecycle and validates the full historical chain.
                    let failed = terminal.prepare_terminal_release(
                        &inner_guard,
                        &binding,
                        &mut admission,
                        &hasher,
                    );
                    if !matches!(failed, Err(SourcePreparationError::AuthorityNotProven)) {
                        return Err(ProofError::VerificationFailed);
                    }
                    let path = root.join(".radishlex-upgrade-v1/terminal-release.json");
                    let bytes = fs::read(&path).map_err(|_| ProofError::VerificationFailed)?;
                    let text =
                        String::from_utf8(bytes).map_err(|_| ProofError::VerificationFailed)?;
                    let legacy = text.replacen(
                        "radishlex-terminal-release-v2",
                        "radishlex-terminal-release-v1",
                        1,
                    );
                    let proof = TerminalReleaseReceipt::decode(legacy.as_bytes())
                        .map_err(|_| ProofError::VerificationFailed)?;
                    let mut file = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&path)
                        .map_err(|_| ProofError::VerificationFailed)?;
                    file.write_all(&proof.encode().map_err(|_| ProofError::VerificationFailed)?)
                        .map_err(|_| ProofError::VerificationFailed)?;
                    file.sync_all()
                        .map_err(|_| ProofError::VerificationFailed)?;
                    admission.stop_at_intent = false;
                }
                terminal
                    .prepare_terminal_release(&inner_guard, &binding, &mut admission, &hasher)
                    .map_err(|_| ProofError::VerificationFailed)?;
                terminal
                    .finish_terminal_release(&inner_guard, DATA, &mut admission, &hasher)
                    .map_err(|_| ProofError::VerificationFailed)?;
            }
            journal = PreparationJournalStore::open_existing(verified(root, owner)?)
                .map_err(|_| ProofError::VerificationFailed)?;
        }
        if matches!(
            predecessor,
            SyntheticPredecessor::CancellationV2 | SyntheticPredecessor::RepeatedCancellationV2
        ) {
            let count = if predecessor == SyntheticPredecessor::RepeatedCancellationV2 {
                2
            } else {
                1
            };
            for operation in [PRIOR_CANCEL, DATA].into_iter().take(count) {
                reserve(
                    &journal,
                    &inner_guard,
                    operation,
                    previous_data,
                    source,
                    &target,
                    &outer_bytes,
                    &hasher,
                )?;
                let (release, proof) = preserve(
                    root,
                    journal,
                    &inner_guard,
                    operation,
                    &install,
                    &outer_guard,
                    source,
                    &target,
                    &hasher,
                )?;
                let mut historical = HistoricalCancellation {
                    install: &install,
                    guard: &outer_guard,
                    source,
                    target: &target,
                    allow_final: true,
                };
                release
                    .finish_cancellation_release(
                        &inner_guard,
                        proof.operation_id(),
                        &mut historical,
                        &hasher,
                    )
                    .map_err(|_| ProofError::VerificationFailed)?;
                journal = PreparationJournalStore::open_existing(verified(root, owner)?)
                    .map_err(|_| ProofError::VerificationFailed)?;
            }
        }
        reserve(
            &journal,
            &inner_guard,
            OPERATION,
            previous_data,
            source,
            &target,
            &outer_bytes,
            &hasher,
        )?;
        let (release, proof) = preserve(
            root,
            journal,
            &inner_guard,
            OPERATION,
            &install,
            &outer_guard,
            source,
            &target,
            &hasher,
        )?;
        let proof = CancellationReleaseReceipt::decode(
            &proof.encode().map_err(|_| ProofError::VerificationFailed)?,
        )
        .map_err(|_| ProofError::VerificationFailed)?;
        if proof.preparation().snapshot_identity().is_some()
            || proof.preparation().snapshot_schema_version().is_some()
            || proof.preparation().phase() != PreparationPhase::Reserved
            || proof.preparation().handoff_started()
        {
            return Err(ProofError::EvidenceChanged);
        }
        let material = materials(root, &hasher)?;
        let observed: BTreeSet<_> = material.iter().map(|item| item.relative.clone()).collect();
        // This is an explicit creation plan, not ownership inferred by sweeping
        // whatever happens to be in a directory after construction.
        if observed != expected_paths(predecessor) {
            return Err(ProofError::EvidenceChanged);
        }
        Ok(Self {
            root: root.to_owned(),
            install,
            outer_guard,
            release,
            inner_guard,
            guard_store,
            source: source.clone(),
            target,
            proof,
            material,
            issued: RefCell::new(BTreeSet::new()),
            consumed: RefCell::new(BTreeSet::new()),
        })
    }

    pub(crate) fn proof(&self) -> &CancellationReleaseReceipt {
        &self.proof
    }
    pub(crate) fn source(&self) -> &ProductArtifactIdentity {
        &self.source
    }
    pub(crate) fn target(&self) -> &ProductArtifactIdentity {
        &self.target
    }
    pub(crate) fn family(&self) -> Result<&PreparationFamily> {
        self.proof
            .archive()
            .source()
            .source_ready()
            .ok_or(ProofError::EvidenceChanged)
    }
    /// Relative paths to register with SyntheticCase::register_fixture_paths.
    /// Guards live in the core's separate temporary socket namespace and are
    /// removed by the retained guard owners before case cleanup.
    pub(crate) fn created_paths(&self) -> Vec<PathBuf> {
        self.material
            .iter()
            .map(|item| item.relative.clone())
            .collect()
    }
    pub(crate) fn marker(&self) -> PathBuf {
        self.root
            .join(".radishlex-upgrade-v1/preparation-cancellation.json")
    }
    pub(crate) fn validate_current_with(&self, check: &mut dyn FnMut(u64) -> bool) -> Result<()> {
        if !check(0) {
            return Err(ProofError::ControlRejected);
        }
        let hasher = CoreHasher {
            check: RefCell::new(check),
        };
        self.guard_store
            .verify_process_guard(&self.inner_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        self.install
            .verify_guard(&self.outer_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        if materials(&self.root, &hasher)? != self.material {
            return Err(ProofError::EvidenceChanged);
        }
        // Core raw JSON reads bypass PreparationHasher. Reserve a conservative
        // envelope before entering it (bounded fixture tree, no arbitrary
        // ancestor input). Hash reads are charged separately per full block.
        let raw_read_bound = self
            .material
            .iter()
            .try_fold(0u64, |sum, item| {
                sum.checked_add(if item.digest.is_some() { item.len } else { 0 })
            })
            .and_then(|sum| sum.checked_mul(256))
            .ok_or(ProofError::VerificationFailed)?;
        if !(hasher.check.borrow_mut())(raw_read_bound) {
            return Err(ProofError::ControlRejected);
        }
        // In the existing core Begin is reached only AFTER verify_live, which
        // checks inner guard, originals, index/ancestors and the full family.
        // Refusing this exact first checkpoint exits before writes or fsync.
        let mut probe = ReadOnlyProbe {
            install: &self.install,
            guard: &self.outer_guard,
            source: &self.source,
            target: &self.target,
            expected: &self.proof,
            visits: 0,
            matched: false,
        };
        let outcome = self.release.prepare_cancellation_release(
            &self.inner_guard,
            self.proof.operation_id(),
            &mut probe,
            &hasher,
        );
        if !matches!(outcome, Err(SourcePreparationError::AuthorityNotProven))
            || probe.visits != 1
            || !probe.matched
        {
            return Err(ProofError::EvidenceChanged);
        }
        self.guard_store
            .verify_process_guard(&self.inner_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        self.install
            .verify_guard(&self.outer_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        if materials(&self.root, &hasher)? != self.material {
            return Err(ProofError::EvidenceChanged);
        }
        if !(hasher.check.borrow_mut())(0) {
            return Err(ProofError::ControlRejected);
        }
        self.guard_store
            .verify_process_guard(&self.inner_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        self.install
            .verify_guard(&self.outer_guard)
            .map_err(|_| ProofError::EvidenceChanged)?;
        Ok(())
    }
    pub(crate) fn bind(&self, call_id: u64) -> Result<ProofBinding> {
        if call_id == 0 || !self.issued.borrow_mut().insert(call_id) {
            return Err(ProofError::Replay);
        }
        Ok(ProofBinding {
            canonical_proof: self
                .proof
                .encode()
                .map_err(|_| ProofError::EvidenceChanged)?,
            source: self.source.clone(),
            target: self.target.clone(),
            family: self.family()?.clone(),
            call_id,
        })
    }
    pub(crate) fn validate_binding(&self, binding: &ProofBinding, call_id: u64) -> Result<()> {
        if call_id == 0
            || call_id != binding.call_id
            || !self.issued.borrow().contains(&call_id)
            || self.consumed.borrow().contains(&call_id)
        {
            return Err(ProofError::Replay);
        }
        if binding.canonical_proof
            != self
                .proof
                .encode()
                .map_err(|_| ProofError::EvidenceChanged)?
            || binding.source != self.source
            || binding.target != self.target
            || binding.family != *self.family()?
        {
            return Err(ProofError::EvidenceChanged);
        }
        Ok(())
    }
    /// Parent validates live controls and current material immediately before
    /// this one-shot consume; no callback can reuse an observation afterwards.
    pub(crate) fn consume_binding(&self, binding: &ProofBinding, call_id: u64) -> Result<()> {
        self.validate_binding(binding, call_id)?;
        self.consumed.borrow_mut().insert(call_id);
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn verified(root: &Path, owner: u32) -> Result<VerifiedDataRoot> {
    VerifiedDataRoot::verify(root, owner).map_err(|_| ProofError::EvidenceChanged)
}
fn data_release(product: &ProductArtifactIdentity) -> Result<DataRelease> {
    DataRelease::new(
        product.release().product_version(),
        product.release().build_number(),
    )
    .map_err(|_| ProofError::VerificationFailed)
}
fn put(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| ProofError::VerificationFailed)?;
    file.write_all(bytes)
        .map_err(|_| ProofError::VerificationFailed)?;
    file.sync_all().map_err(|_| ProofError::VerificationFailed)
}
#[allow(clippy::too_many_arguments)]
fn reserve(
    journal: &PreparationJournalStore,
    guard: &UpgradeProcessGuard,
    operation: &str,
    previous_data: Option<&str>,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
    outer_bytes: &[u8],
    hasher: &impl PreparationHasher,
) -> Result<()> {
    let inventory = journal
        .capture_previous_inventory(guard, operation, Some(OLD), previous_data, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    let identity = journal
        .previous_inventory_identity(guard, &inventory, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    let mut prep = PreparationReceipt::new(
        PreparationBinding {
            operation_id: operation.into(),
            previous_install_operation_id: Some(OLD.into()),
            previous_data_operation_id: previous_data.map(str::to_owned),
            source_release: data_release(source)?,
            target_release: data_release(target)?,
            source_product_sha256: source.product_manifest_sha256().into(),
            target_product_sha256: target.product_manifest_sha256().into(),
            previous_install_receipt_sha256: Some(digest(outer_bytes)),
            previous_data_receipt_sha256: inventory.receipt_sha256().map(str::to_owned),
            previous_inventory_sha256: identity.as_ref().map(|value| value.sha256.clone()),
            target_schema_version: 9,
            data_root: journal.data_root_identity(),
            state_directory: journal.state_directory_identity(),
        },
        journal
            .observe_source_family(guard, hasher)
            .map_err(|_| ProofError::VerificationFailed)?,
    )
    .map_err(|_| ProofError::VerificationFailed)?;
    if let Some(identity) = identity {
        if let Some(index) = inventory.release_index_identity() {
            prep.bind_released_predecessor(identity, index.clone())
                .map_err(|_| ProofError::VerificationFailed)?;
        } else {
            prep.bind_previous_inventory(identity)
                .map_err(|_| ProofError::VerificationFailed)?;
        }
    }
    journal
        .persist(guard, None, &prep)
        .map_err(|_| ProofError::VerificationFailed)
}

struct Admission;
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
#[allow(clippy::too_many_arguments)]
fn preserve(
    path: &Path,
    journal: PreparationJournalStore,
    inner: &UpgradeProcessGuard,
    operation: &str,
    install: &InstallReceiptStore,
    outer: &InstallProcessGuard,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
    hasher: &impl PreparationHasher,
) -> Result<(CancellationReleaseStore, CancellationReleaseReceipt)> {
    let root = journal.data_root_identity();
    let cancel = PreparationCancellationStore::attach(journal, inner)
        .map_err(|_| ProofError::VerificationFailed)?;
    cancel
        .request_cancellation(inner, operation, &mut Admission, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    cancel
        .finish_source(inner, &mut Admission, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    let archive = CancellationArchiveStore::open_existing(verified(path, root.owner_id)?)
        .map_err(|_| ProofError::VerificationFailed)?;
    let binding = archive
        .observe_outer(inner, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    let mut authority = CancellationArchiveAuthority {
        install,
        guard: outer,
        source,
        target,
        product: &mut Admission,
    };
    archive
        .preserve_cancellation(inner, &binding, &mut authority, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    let release = CancellationReleaseStore::open_existing(verified(path, root.owner_id)?)
        .map_err(|_| ProofError::VerificationFailed)?;
    let mut authority = HistoricalCancellation {
        install,
        guard: outer,
        source,
        target,
        allow_final: false,
    };
    let proof = release
        .prepare_cancellation_release(inner, operation, &mut authority, hasher)
        .map_err(|_| ProofError::VerificationFailed)?;
    Ok((release, proof))
}

struct HistoricalCancellation<'a> {
    allow_final: bool,
    install: &'a InstallReceiptStore,
    guard: &'a InstallProcessGuard,
    source: &'a ProductArtifactIdentity,
    target: &'a ProductArtifactIdentity,
}
impl CancellationReleasePort for HistoricalCancellation<'_> {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        _: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool {
        (self.allow_final || requirement == CancellationReleaseRequirement::Preserve)
            && self.install.verify_guard(self.guard).is_ok()
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
struct SyntheticTerminal {
    stop_at_intent: bool,
}
impl TerminalReleasePort for SyntheticTerminal {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &TerminalReleaseReceipt,
        checkpoint: TerminalReleaseCheckpoint,
        _: TerminalReleaseOuterRequirement,
    ) -> bool {
        !(self.stop_at_intent && checkpoint == TerminalReleaseCheckpoint::IntentRecorded)
    }
}
struct ReadOnlyProbe<'a> {
    install: &'a InstallReceiptStore,
    guard: &'a InstallProcessGuard,
    source: &'a ProductArtifactIdentity,
    target: &'a ProductArtifactIdentity,
    expected: &'a CancellationReleaseReceipt,
    visits: usize,
    matched: bool,
}
impl CancellationReleasePort for ReadOnlyProbe<'_> {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        checkpoint: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool {
        self.visits += 1;
        self.matched = self.visits == 1
            && checkpoint == CancellationReleaseCheckpoint::Begin
            && requirement == CancellationReleaseRequirement::Preserve
            && proof == self.expected
            && self.install.verify_guard(self.guard).is_ok()
            && validate_preparation_cancellation_outer(
                proof.archive().outer().previous_bytes(),
                proof.archive().outer().new_bytes(),
                proof.operation_id(),
                proof.legacy_outer_operation_id(),
                self.install.root_identity(),
                self.source,
                self.target,
            )
            .is_ok();
        false
    }
}

fn materials(root: &Path, hasher: &impl PreparationHasher) -> Result<Vec<Material>> {
    let mut pending: Vec<_> = CONTROL_ROOTS.into_iter().map(PathBuf::from).collect();
    let mut result = Vec::new();
    while let Some(relative) = pending.pop() {
        if result.len() >= 128 {
            return Err(ProofError::EvidenceChanged);
        }
        let path = root.join(&relative);
        let meta = fs::symlink_metadata(&path).map_err(|_| ProofError::EvidenceChanged)?;
        if !meta.is_file() && !meta.is_dir() {
            return Err(ProofError::EvidenceChanged);
        }
        let digest = if meta.is_file() {
            let mut file = File::open(&path).map_err(|_| ProofError::VerificationFailed)?;
            let held = file
                .metadata()
                .map_err(|_| ProofError::VerificationFailed)?;
            if held.dev() != meta.dev() || held.ino() != meta.ino() {
                return Err(ProofError::EvidenceChanged);
            }
            Some(
                hasher
                    .sha256(&mut file)
                    .map_err(|_| ProofError::VerificationFailed)?,
            )
        } else {
            for entry in fs::read_dir(&path).map_err(|_| ProofError::VerificationFailed)? {
                let entry = entry.map_err(|_| ProofError::VerificationFailed)?;
                pending.push(relative.join(entry.file_name()));
            }
            None
        };
        let after = fs::symlink_metadata(&path).map_err(|_| ProofError::EvidenceChanged)?;
        let identity = |value: &fs::Metadata| {
            (
                value.dev(),
                value.ino(),
                value.uid(),
                value.mode(),
                value.nlink(),
                value.len(),
                value.mtime(),
                value.mtime_nsec(),
                value.ctime(),
                value.ctime_nsec(),
            )
        };
        if identity(&meta) != identity(&after) {
            return Err(ProofError::EvidenceChanged);
        }
        result.push(Material {
            relative,
            device: meta.dev(),
            inode: meta.ino(),
            owner: meta.uid(),
            mode: meta.mode(),
            links: meta.nlink(),
            len: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
            digest,
        });
    }
    result.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(result)
}

#[path = "schema_observer_proof_tests.rs"]
mod tests;
pub(crate) use tests::check_binding_negatives;

fn expected_paths(predecessor: SyntheticPredecessor) -> BTreeSet<PathBuf> {
    let mut paths: BTreeSet<_> = CONTROL_ROOTS.into_iter().map(PathBuf::from).collect();
    for relative in [
        ".radishlex-install-v1/receipt.json".to_owned(),
        format!(".radishlex-install-history-v1/{OLD}"),
        format!(".radishlex-install-history-v1/{OLD}/receipt.json"),
        ".radishlex-upgrade-v1/preparation-cancellation.json".to_owned(),
        ".radishlex-upgrade-history-v1/latest-release.json".to_owned(),
    ] {
        paths.insert(PathBuf::from(relative));
    }
    let mut add_cancellation = |operation: &str, historical: bool| {
        let base = PathBuf::from(".radishlex-upgrade-history-v1").join(operation);
        paths.insert(base.clone());
        for name in [
            "preparation.json",
            "cancellation-source.json",
            "cancellation-archive.json",
            "cancellation-release.json",
        ] {
            paths.insert(base.join(name));
        }
        if historical {
            paths.insert(base.join("preparation-cancellation.json"));
        }
    };
    add_cancellation(OPERATION, false);
    if matches!(
        predecessor,
        SyntheticPredecessor::CancellationV2 | SyntheticPredecessor::RepeatedCancellationV2
    ) {
        add_cancellation(PRIOR_CANCEL, true);
        if predecessor == SyntheticPredecessor::RepeatedCancellationV2 {
            add_cancellation(DATA, true);
        }
    }
    if matches!(
        predecessor,
        SyntheticPredecessor::LegacyData
            | SyntheticPredecessor::TerminalV1
            | SyntheticPredecessor::TerminalV2
    ) {
        let base = PathBuf::from(".radishlex-upgrade-history-v1").join(DATA);
        paths.insert(base.clone());
        paths.insert(base.join("data"));
        paths.insert(base.join("data/receipt.json"));
        paths.insert(
            base.join(if predecessor == SyntheticPredecessor::LegacyData {
                "inventory.json"
            } else {
                "terminal-release.json"
            }),
        );
    }
    paths
}
