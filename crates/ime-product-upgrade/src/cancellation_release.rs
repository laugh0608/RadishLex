//! Cancellation finalization is separate from material preservation. Only the
//! qualified-source step may move the last active request out of the data gate.
use super::*;

pub(super) const CANCELLATION_RELEASE: &str = "cancellation-release.json";
const PROOF_TEMP: &str = "cancellation-release.json.tmp";
const REQUEST: &str = "preparation-cancellation.json";
const ARCHIVE: &str = "cancellation-archive.json";
const OUTER_STATE: &str = ".radishlex-install-v1";
const OUTER_HISTORY: &str = ".radishlex-install-history-v1";

#[path = "cancellation_release_record.rs"]
mod record;
pub use record::CancellationReleaseReceipt;
#[path = "cancellation_release_evidence.rs"]
pub(super) mod evidence;
#[path = "cancellation_release_persistence.rs"]
mod persistence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReleaseCheckpoint {
    Begin,
    SourceVerified,
    ProofCreated,
    ProofWritten,
    ProofFileSynced,
    ProofPublished,
    ProofDirectorySynced,
    BeforeArchiveMove,
    ArchiveFileSynced,
    ArchiveMoved,
    ArchiveTargetSynced,
    ArchiveSourceSynced,
    BeforeIndexWrite,
    IndexCreated,
    IndexWritten,
    IndexFileSynced,
    IndexPublished,
    IndexDirectorySynced,
    Ready,
    BeforeMarkerMove,
    MarkerFileSynced,
    MarkerMoved,
    MarkerTargetSynced,
    MarkerSourceSynced,
    Released,
}
use CancellationReleaseCheckpoint as CP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReleaseRequirement {
    Preserve,
    QualifiedSource,
}

/// Hold the real outer guard before the data guard. At every call revalidate
/// canonical old/new outer receipts, exact source programs and quiescence.
/// QualifiedSource additionally requires independent, identity-bound old-binary
/// gate/open/learning/restart qualification for this source schema. A UI flag or
/// synthetic qualification is not production authority. No production port is
/// supplied by the core, and a preserved archive alone cannot satisfy this port.
pub trait CancellationReleasePort {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        checkpoint: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool;
}

pub struct CancellationReleaseStore {
    journal: PreparationJournalStore,
}
impl CancellationReleaseStore {
    pub fn open_existing(root: VerifiedDataRoot) -> Result<Self> {
        let value = Self {
            journal: PreparationJournalStore {
                store: UpgradeReceiptStore::open_existing_directory(root)?,
            },
        };
        value.validate_active()?;
        Ok(value)
    }
    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard> {
        self.validate_active()?;
        Ok(self.journal.store.acquire_directory_guard()?)
    }
    fn history(&self) -> ReleaseHistory<'_> {
        ReleaseHistory {
            journal: &self.journal,
        }
    }
    fn active(&self, name: &str) -> PathBuf {
        self.journal.store.state_directory.join(name)
    }
    fn directory(&self, operation: &str) -> PathBuf {
        self.journal.store.root.path.join(HISTORY).join(operation)
    }
    fn unique(&self, source: &Path, target: &Path) -> Result<PathBuf> {
        match (path_exists(source)?, path_exists(target)?) {
            (true, false) => Ok(source.to_owned()),
            (false, true) => Ok(target.to_owned()),
            _ => Err(Error::EvidenceChanged),
        }
    }
    fn validate_active(&self) -> Result<()> {
        self.journal.store.revalidate()?;
        for entry in fs::read_dir(&self.journal.store.state_directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if entry.file_name() != REQUEST && entry.file_name() != ARCHIVE {
                return Err(Error::EvidenceChanged);
            }
            self.journal.private_metadata(&entry.path())?;
        }
        Ok(())
    }
    /// Persist a complete immutable proof, seal the archive and publish the one
    /// lifecycle locator. The request remains active and startup stays blocked.
    pub fn prepare_cancellation_release(
        &self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationReleaseReceipt> {
        if !valid_id(operation) {
            return Err(Error::EvidenceChanged);
        }
        self.journal.verify_guard(guard)?;
        self.validate_active()?;
        if !path_exists(&self.active(REQUEST))? {
            return Err(Error::InvalidPhase);
        }
        let path = self.directory(operation).join(CANCELLATION_RELEASE);
        let (proof, mut identity) =
            if let Some(identity) = self.history().optional_file(&path, hasher)? {
                let proof = self.history().read_cancellation(&path, hasher)?;
                (proof, Some(identity))
            } else {
                let archive_path = self.active(ARCHIVE);
                let archive_identity = self.journal.evidence(&archive_path, hasher)?;
                // Existing preservation reader verifies all bound materials before
                // any finalization file is introduced into its strict namespace.
                let root = VerifiedDataRoot::verify(
                    &self.journal.store.root.path,
                    self.journal.store.root.expected_owner_id,
                )?;
                let store = CancellationArchiveStore::open_existing(root)?;
                let archive = store
                    .load_guarded(guard, hasher)?
                    .ok_or(Error::InvalidPhase)?;
                let prep = archive.request.preparation();
                let inventory = if prep.binding().previous_data_operation_id.is_none()
                    && prep.previous_release_index_identity().is_none()
                {
                    PreviousInventory::empty(
                        &self.journal,
                        operation,
                        prep.binding().previous_install_operation_id.as_deref(),
                    )?
                } else {
                    self.journal.load_previous_inventory(prep, hasher)?
                };
                let previous = self.history().load_index(hasher)?;
                let proof = CancellationReleaseReceipt::new(
                    archive,
                    archive_identity,
                    inventory.receipt,
                    previous,
                )?;
                (proof, None)
            };
        if proof.operation_id() != operation {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            &proof,
            identity.as_ref(),
            None,
            port,
            hasher,
            CP::Begin,
            CancellationReleaseRequirement::Preserve,
        )?;
        self.verify_source_equivalence(&proof)?;
        self.checkpoint(
            guard,
            &proof,
            identity.as_ref(),
            None,
            port,
            hasher,
            CP::SourceVerified,
            CancellationReleaseRequirement::Preserve,
        )?;
        if identity.is_none() {
            identity = Some(self.write_proof(guard, &proof, port, hasher)?);
        }
        let identity = identity.ok_or(Error::EvidenceChanged)?;
        self.sync_file(&path, &identity, hasher)?;
        sync_directory(&self.directory(operation))?;
        self.move_material(guard, &proof, &identity, false, port, hasher)?;
        self.publish_index(guard, &proof, &identity, port, hasher)?;
        self.checkpoint(
            guard,
            &proof,
            Some(&identity),
            None,
            port,
            hasher,
            CP::Ready,
            CancellationReleaseRequirement::Preserve,
        )?;
        Ok(proof)
    }
    /// Separate final step. Without qualified source authority this retains the
    /// last marker, even when a cancel_ready proof and index have been published.
    pub fn finish_cancellation_release(
        self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<(UpgradeReceiptStore, CancellationReleaseReceipt)> {
        if !valid_id(operation) {
            return Err(Error::EvidenceChanged);
        }
        self.journal.verify_guard(guard)?;
        self.validate_active()?;
        let path = self.directory(operation).join(CANCELLATION_RELEASE);
        let identity = self.journal.evidence(&path, hasher)?;
        let proof = self.history().read_cancellation(&path, hasher)?;
        if proof.operation_id() != operation {
            return Err(Error::EvidenceChanged);
        }
        self.require_index(&proof, hasher)?;
        if path_exists(&self.active(ARCHIVE))? {
            return Err(Error::InvalidPhase);
        }
        self.checkpoint(
            guard,
            &proof,
            Some(&identity),
            None,
            port,
            hasher,
            CP::Begin,
            CancellationReleaseRequirement::QualifiedSource,
        )?;
        self.verify_source_equivalence(&proof)?;
        self.checkpoint(
            guard,
            &proof,
            Some(&identity),
            None,
            port,
            hasher,
            CP::SourceVerified,
            CancellationReleaseRequirement::QualifiedSource,
        )?;
        self.move_material(guard, &proof, &identity, true, port, hasher)?;
        self.checkpoint(
            guard,
            &proof,
            Some(&identity),
            None,
            port,
            hasher,
            CP::Released,
            CancellationReleaseRequirement::QualifiedSource,
        )?;
        if self.journal.store.load_guarded(guard)?.is_some() {
            return Err(Error::EvidenceChanged);
        }
        Ok((self.journal.store, proof))
    }
    fn require_index(
        &self,
        proof: &CancellationReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self
            .history()
            .load_index(hasher)?
            .as_ref()
            .map(|v| &v.index)
            != Some(&self.history().cancellation_index(proof, hasher)?)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        proof: &CancellationReleaseReceipt,
        identity: Option<&PreparationFileIdentity>,
        temporary: Option<(&Path, &PreparationFileIdentity)>,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
        point: CP,
        requirement: CancellationReleaseRequirement,
    ) -> Result<()> {
        self.verify_live(guard, proof, identity, temporary, hasher)?;
        let index = self.history().optional_file(
            &self.journal.store.root.path.join(HISTORY).join(INDEX),
            hasher,
        )?;
        if !port.confirm_authority_and_quiescence(proof, point, requirement) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_live(guard, proof, identity, temporary, hasher)?;
        if self.history().optional_file(
            &self.journal.store.root.path.join(HISTORY).join(INDEX),
            hasher,
        )? != index
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
