//! Durable cancellation admission. No SQLite calls, artifact moves, outer
//! replacement or startup release occur here. Ordinary readers remain blocked.
use super::*;
use crate::PreparationFileIdentity;

const REQUEST: &str = "preparation-cancellation.json";
const STAGED_REQUEST: &str = "preparation-cancellation.json.tmp";

#[path = "preparation_cancellation_record.rs"]
mod record;
pub use record::PreparationCancellationRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationCancellationCheckpoint {
    Begin,
    BeforeCreate,
    Created,
    Written,
    BeforeFileSync,
    FileSynced,
    BeforeRename,
    Renamed,
    BeforeDirectorySync,
    DirectorySynced,
    Recorded,
}
use PreparationCancellationCheckpoint as Point;

/// Must hold and revalidate the outer guard, the sealed previous outer bytes,
/// exact source/target products, source programs and fresh quiescence on EVERY
/// call. The new outer must be absent (old terminal still active) or precisely
/// the bound, artifact-free Prepared operation. The previous outer must be a
/// valid terminal matching installed source. A cached/UI flag is not authority.
/// The callback acknowledges only a cancellation REQUEST, never its completion.
pub trait PreparationCancellationPort {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        checkpoint: PreparationCancellationCheckpoint,
    ) -> bool;
}

/// Only this store recognizes the cancellation marker. It cannot be converted
/// back to an ordinary preparation store, or erase/replace an existing request.
pub struct PreparationCancellationStore {
    journal: PreparationJournalStore,
}

impl PreparationCancellationStore {
    pub fn attach(journal: PreparationJournalStore, guard: &UpgradeProcessGuard) -> Result<Self> {
        journal.verify_guard(guard)?;
        journal.validate_entries()?;
        journal.reject_staged_record()?;
        Ok(Self { journal })
    }

    /// Read-only construction; acquiring a guard and loading remain explicit.
    pub fn open_existing(root: VerifiedDataRoot) -> Result<Self> {
        let value = Self {
            journal: PreparationJournalStore {
                store: UpgradeReceiptStore::open_existing_directory(root)?,
            },
        };
        value.validate_entries()?;
        Ok(value)
    }

    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard> {
        self.validate_entries()?;
        Ok(self.journal.store.acquire_directory_guard()?)
    }

    /// Reloads an unchanged request with its bound preparation, source family
    /// and predecessor inventory. No outer/product authorization is inferred.
    pub fn load_guarded(
        &self,
        guard: &UpgradeProcessGuard,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<PreparationCancellationRequest>> {
        self.journal.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged()?;
        let Some((request, identity)) = self.read_request(hasher)? else {
            return Ok(None);
        };
        self.verify_request(guard, &request, hasher)?;
        if self.journal.evidence(&self.journal.path(REQUEST), hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some(request))
    }

    /// Persist one immutable request. Success means "cancellation requested";
    /// startup and all ordinary preparation/handoff entry points stay blocked.
    /// An interrupted temp file is preserved and never adopted automatically.
    pub fn request_cancellation(
        &self,
        guard: &UpgradeProcessGuard,
        operation_id: &str,
        port: &mut impl PreparationCancellationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationCancellationRequest> {
        self.journal.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged()?;
        if let Some((request, identity)) = self.read_request(hasher)? {
            if request.preparation().binding().operation_id != operation_id {
                return Err(Error::EvidenceChanged);
            }
            self.confirm_durable(guard, &request, &identity, port, hasher)?;
            return Ok(request);
        }
        let prep = self
            .journal
            .read_record()?
            .ok_or(Error::InvalidPhase)?
            .receipt;
        if prep.binding().operation_id != operation_id {
            return Err(Error::EvidenceChanged);
        }
        let request = PreparationCancellationRequest::new(
            prep,
            self.journal.evidence(&self.journal.path(JOURNAL), hasher)?,
            self.journal.family(hasher)?,
        )?;
        self.checkpoint(guard, &request, port, hasher, Point::Begin, None)?;
        let bytes = request.encode()?;
        self.checkpoint(guard, &request, port, hasher, Point::BeforeCreate, None)?;
        let path = self.journal.path(STAGED_REQUEST);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        let initial = self.journal.evidence(&path, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != initial.device_id
            || metadata.ino() != initial.inode
            || initial.byte_len != 0
        {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            &request,
            port,
            hasher,
            Point::Created,
            Some((STAGED_REQUEST, &initial)),
        )?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let written = self.journal.evidence(&path, hasher)?;
        if !same_object(&initial, &written)
            || self.read_bytes(&path)? != bytes
            || self.journal.evidence(&path, hasher)? != written
        {
            return Err(Error::EvidenceChanged);
        }
        for point in [Point::Written, Point::BeforeFileSync] {
            self.checkpoint(
                guard,
                &request,
                port,
                hasher,
                point,
                Some((STAGED_REQUEST, &written)),
            )?;
        }
        file.sync_all().map_err(|_| Error::Io)?;
        for point in [Point::FileSynced, Point::BeforeRename] {
            self.checkpoint(
                guard,
                &request,
                port,
                hasher,
                point,
                Some((STAGED_REQUEST, &written)),
            )?;
        }
        fs::rename(&path, self.journal.path(REQUEST)).map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            &request,
            port,
            hasher,
            Point::Renamed,
            Some((REQUEST, &written)),
        )?;
        self.finish_directory_sync(guard, &request, &written, port, hasher)?;
        Ok(request)
    }

    fn confirm_durable(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        identity: &PreparationFileIdentity,
        port: &mut impl PreparationCancellationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::Begin,
            Some((REQUEST, identity)),
        )?;
        let file = File::open(self.journal.path(REQUEST)).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != identity.device_id || metadata.ino() != identity.inode {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::BeforeFileSync,
            Some((REQUEST, identity)),
        )?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::FileSynced,
            Some((REQUEST, identity)),
        )?;
        self.finish_directory_sync(guard, request, identity, port, hasher)
    }

    fn finish_directory_sync(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        identity: &PreparationFileIdentity,
        port: &mut impl PreparationCancellationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::BeforeDirectorySync,
            Some((REQUEST, identity)),
        )?;
        sync_directory(&self.journal.store.state_directory)?;
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::DirectorySynced,
            Some((REQUEST, identity)),
        )?;
        if self.load_guarded(guard, hasher)?.as_ref() != Some(request) {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            request,
            port,
            hasher,
            Point::Recorded,
            Some((REQUEST, identity)),
        )
    }

    fn checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        port: &mut impl PreparationCancellationPort,
        hasher: &impl PreparationHasher,
        point: Point,
        slot: Option<(&str, &PreparationFileIdentity)>,
    ) -> Result<()> {
        self.verify_request(guard, request, hasher)?;
        self.verify_slot(slot, hasher)?;
        if !port.confirm_authority_and_quiescence(request, point) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_request(guard, request, hasher)?;
        self.verify_slot(slot, hasher)
    }

    fn verify_request(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.journal.verify_guard(guard)?;
        self.validate_entries()?;
        self.journal.reject_staged_record()?;
        request.encode()?;
        let prep = request.preparation();
        if self
            .journal
            .read_record()?
            .as_ref()
            .map(|record| &record.receipt)
            != Some(prep)
            || self.journal.evidence(&self.journal.path(JOURNAL), hasher)?
                != *request.preparation_identity()
            || self.journal.family(hasher)? != *request.observed_source()
        {
            return Err(Error::EvidenceChanged);
        }
        if prep.snapshot_identity().is_some() {
            self.journal.verify_snapshot(prep, hasher)?;
        } else if path_exists(&self.journal.path(PREPARATION_SNAPSHOT))? {
            return Err(Error::EvidenceChanged);
        }
        let inventory = self.journal.load_previous_inventory(prep, hasher)?;
        self.journal
            .verify_inventory_objects(&inventory, hasher, Some(prep))?;
        if self.journal.family(hasher)? != *request.observed_source()
            || self.journal.evidence(&self.journal.path(JOURNAL), hasher)?
                != *request.preparation_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        self.journal.verify_guard(guard)?;
        Ok(())
    }

    fn verify_slot(
        &self,
        slot: Option<(&str, &PreparationFileIdentity)>,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        for name in [REQUEST, STAGED_REQUEST] {
            match slot {
                Some((expected, identity)) if name == expected => {
                    if self.journal.evidence(&self.journal.path(name), hasher)? != *identity {
                        return Err(Error::EvidenceChanged);
                    }
                }
                _ if path_exists(&self.journal.path(name))? => return Err(Error::EvidenceChanged),
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_entries(&self) -> Result<()> {
        self.journal.store.revalidate()?;
        self.journal
            .validate_entries_with_extra(&[REQUEST, STAGED_REQUEST])?;
        Ok(())
    }

    fn reject_staged(&self) -> Result<()> {
        self.journal.reject_staged_record()?;
        if path_exists(&self.journal.path(STAGED_REQUEST))? {
            return Err(Error::Journal(PreparationJournalError::InterruptedWrite));
        }
        Ok(())
    }

    fn read_request(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<(PreparationCancellationRequest, PreparationFileIdentity)>> {
        let path = self.journal.path(REQUEST);
        if !path_exists(&path)? {
            return Ok(None);
        }
        if self.journal.private_metadata(&path)?.len() > MAX_PREPARATION_RECEIPT_BYTES as u64 {
            return Err(Error::EvidenceChanged);
        }
        let identity = self.journal.evidence(&path, hasher)?;
        let request = PreparationCancellationRequest::decode(&self.read_bytes(&path)?)?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some((request, identity)))
    }

    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| Error::Io)?
            .take(MAX_PREPARATION_RECEIPT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Io)?;
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        Ok(bytes)
    }
}
