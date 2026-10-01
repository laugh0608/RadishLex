use super::*;

type StoredSource = (CancellationSourceReceipt, PreparationFileIdentity);

// Exact path identities are kept across callbacks, including before publication.
struct SourceWrite<'a> {
    request: &'a PreparationCancellationRequest,
    next: &'a CancellationSourceReceipt,
    current: Option<StoredSource>,
    slot: Option<(&'static str, PreparationFileIdentity)>,
}

impl PreparationCancellationStore {
    pub(super) fn read_source_progress(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<StoredSource>> {
        let path = self.journal.path(SOURCE_PROGRESS);
        if !path_exists(&path)? {
            return Ok(None);
        }
        if self.journal.private_metadata(&path)?.len() > MAX_PREPARATION_RECEIPT_BYTES as u64 {
            return Err(Error::EvidenceChanged);
        }
        let identity = self.journal.evidence(&path, hasher)?;
        let receipt = CancellationSourceReceipt::decode(&self.read_bytes(&path)?)?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some((receipt, identity)))
    }

    pub(super) fn persist_source(
        &self,
        guard: &UpgradeProcessGuard,
        request: &PreparationCancellationRequest,
        previous: Option<&CancellationSourceReceipt>,
        next: &CancellationSourceReceipt,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.reject_source_staged()?;
        self.verify_source_progress(guard, request, next, hasher)?;
        let current = self.read_source_progress(hasher)?;
        if current.as_ref().map(|v| &v.0) != previous {
            return Err(Error::EvidenceChanged);
        }
        match previous {
            None => {
                self.verify_request(guard, request, hasher)?;
                if next != &CancellationSourceReceipt::new(request, next.request_identity().clone())
                {
                    return Err(Error::InvalidPhase);
                }
            }
            Some(old) if old == next => {}
            Some(old) if old.phase() == CancellationSourcePhase::Finishing => {
                let expected = if let Some(family) = next.source_ready() {
                    old.ready(family.clone())?
                } else {
                    old.with_recovery(
                        next.preparation()
                            .journal_recovery()
                            .ok_or(Error::InvalidPhase)?
                            .clone(),
                    )?
                };
                if &expected != next {
                    return Err(Error::InvalidPhase);
                }
            }
            _ => return Err(Error::InvalidPhase),
        }
        let mut write = SourceWrite {
            request,
            next,
            current,
            slot: None,
        };
        self.source_write_checkpoint(guard, &write, port, hasher, Point::Begin)?;
        if previous == Some(next) {
            let identity = &write.current.as_ref().ok_or(Error::EvidenceChanged)?.1;
            let file = File::open(self.journal.path(SOURCE_PROGRESS)).map_err(|_| Error::Io)?;
            let metadata = file.metadata().map_err(|_| Error::Io)?;
            if metadata.dev() != identity.device_id || metadata.ino() != identity.inode {
                return Err(Error::EvidenceChanged);
            }
            self.source_write_checkpoint(guard, &write, port, hasher, Point::BeforeFileSync)?;
            file.sync_all().map_err(|_| Error::Io)?;
            self.source_write_checkpoint(guard, &write, port, hasher, Point::FileSynced)?;
        } else {
            self.source_write_checkpoint(guard, &write, port, hasher, Point::BeforeCreate)?;
            let path = self.journal.path(STAGED_SOURCE_PROGRESS);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(|_| Error::Io)?;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::Io)?;
            let empty = self.journal.evidence(&path, hasher)?;
            let metadata = file.metadata().map_err(|_| Error::Io)?;
            if metadata.dev() != empty.device_id
                || metadata.ino() != empty.inode
                || empty.byte_len != 0
            {
                return Err(Error::EvidenceChanged);
            }
            write.slot = Some((STAGED_SOURCE_PROGRESS, empty.clone()));
            self.source_write_checkpoint(guard, &write, port, hasher, Point::Created)?;
            let bytes = next.encode()?;
            file.write_all(&bytes).map_err(|_| Error::Io)?;
            let written = self.journal.evidence(&path, hasher)?;
            if !same_object(&empty, &written)
                || self.read_bytes(&path)? != bytes
                || self.journal.evidence(&path, hasher)? != written
            {
                return Err(Error::EvidenceChanged);
            }
            write.slot = Some((STAGED_SOURCE_PROGRESS, written.clone()));
            for point in [Point::Written, Point::BeforeFileSync] {
                self.source_write_checkpoint(guard, &write, port, hasher, point)?;
            }
            file.sync_all().map_err(|_| Error::Io)?;
            for point in [Point::FileSynced, Point::BeforeRename] {
                self.source_write_checkpoint(guard, &write, port, hasher, point)?;
            }
            fs::rename(&path, self.journal.path(SOURCE_PROGRESS)).map_err(|_| Error::Io)?;
            write.current = Some((next.clone(), written));
            write.slot = None;
            self.source_write_checkpoint(guard, &write, port, hasher, Point::Renamed)?;
        }
        self.source_write_checkpoint(guard, &write, port, hasher, Point::BeforeDirectorySync)?;
        sync_directory(&self.journal.store.state_directory)?;
        self.source_write_checkpoint(guard, &write, port, hasher, Point::DirectorySynced)?;
        self.source_write_checkpoint(guard, &write, port, hasher, Point::Recorded)
    }

    fn source_write_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        write: &SourceWrite<'_>,
        port: &mut impl CancellationSourcePort,
        hasher: &impl PreparationHasher,
        point: Point,
    ) -> Result<()> {
        self.verify_source_write(guard, write, hasher)?;
        let family = self.journal.read_family(hasher)?;
        if !port.confirm_authority_and_quiescence(
            write.request,
            write.next,
            SourcePoint::Write(write.next.phase(), point),
        ) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_source_write(guard, write, hasher)?;
        if self.journal.read_family(hasher)? != family {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    fn verify_source_write(
        &self,
        guard: &UpgradeProcessGuard,
        write: &SourceWrite<'_>,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify_source_progress(guard, write.request, write.next, hasher)?;
        if write.current.is_none()
            && self.journal.family(hasher)? != *write.request.observed_source()
        {
            return Err(Error::EvidenceChanged);
        }
        if self.read_source_progress(hasher)? != write.current {
            return Err(Error::EvidenceChanged);
        }
        match &write.slot {
            Some((name, identity)) => {
                if self.journal.evidence(&self.journal.path(name), hasher)? != *identity {
                    return Err(Error::EvidenceChanged);
                }
            }
            None if path_exists(&self.journal.path(STAGED_SOURCE_PROGRESS))? => {
                return Err(Error::EvidenceChanged)
            }
            None => {}
        }
        Ok(())
    }
}
