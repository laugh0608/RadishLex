use super::*;
use PreparationCancellationCheckpoint as WritePoint;

impl CancellationArchiveStore {
    pub(super) fn read_record(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<(CancellationArchiveReceipt, PreparationFileIdentity)>> {
        let path = self.active(ARCHIVE);
        if !path_exists(&path)? {
            return Ok(None);
        }
        let identity = self.journal.evidence(&path, hasher)?;
        let record = CancellationArchiveReceipt::decode(&self.read_bytes(&path)?)?;
        if self.journal.evidence(&path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(Some((record, identity)))
    }

    pub(super) fn persist(
        &self,
        guard: &UpgradeProcessGuard,
        previous: Option<&CancellationArchiveReceipt>,
        next: &CancellationArchiveReceipt,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify(guard, next, hasher, None)?;
        let mut current = self.read_record(hasher)?;
        if current.as_ref().map(|v| &v.0) != previous
            || previous.is_some_and(|old| !next.can_replace(old))
        {
            return Err(Error::EvidenceChanged);
        }
        let path = self.active(TEMP);
        if path_exists(&path)? {
            return Err(Error::EvidenceChanged);
        }
        let mut temporary: Option<PreparationFileIdentity> = None;
        let checkpoint =
            |point,
             current: &Option<(CancellationArchiveReceipt, PreparationFileIdentity)>,
             temporary: &Option<PreparationFileIdentity>,
             port: &mut _|
             -> Result<()> {
                let verify = || -> Result<()> {
                    self.verify(
                        guard,
                        next,
                        hasher,
                        temporary
                            .as_ref()
                            .map(|identity| (path.as_path(), identity)),
                    )?;
                    if &self.read_record(hasher)? != current {
                        return Err(Error::EvidenceChanged);
                    }
                    Ok(())
                };
                verify()?;
                if !CancellationArchivePort::confirm_authority_and_quiescence(
                    port,
                    next,
                    Point::Record(point),
                ) {
                    return Err(Error::AuthorityNotProven);
                }
                verify()
            };
        checkpoint(WritePoint::Begin, &current, &temporary, port)?;
        if previous == Some(next) {
            checkpoint(WritePoint::BeforeFileSync, &current, &temporary, port)?;
            self.sync_file(
                &self.active(ARCHIVE),
                &current.as_ref().ok_or(Error::EvidenceChanged)?.1,
                hasher,
            )?;
            checkpoint(WritePoint::FileSynced, &current, &temporary, port)?;
        } else {
            checkpoint(WritePoint::BeforeCreate, &current, &temporary, port)?;
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
            temporary = Some(empty.clone());
            checkpoint(WritePoint::Created, &current, &temporary, port)?;
            let bytes = next.encode()?;
            file.write_all(&bytes).map_err(|_| Error::Io)?;
            let written = self.journal.evidence(&path, hasher)?;
            if !same_object(&empty, &written) || self.read_bytes(&path)? != bytes {
                return Err(Error::EvidenceChanged);
            }
            temporary = Some(written.clone());
            checkpoint(WritePoint::Written, &current, &temporary, port)?;
            checkpoint(WritePoint::BeforeFileSync, &current, &temporary, port)?;
            file.sync_all().map_err(|_| Error::Io)?;
            checkpoint(WritePoint::FileSynced, &current, &temporary, port)?;
            checkpoint(WritePoint::BeforeRename, &current, &temporary, port)?;
            fs::rename(&path, self.active(ARCHIVE)).map_err(|_| Error::Io)?;
            temporary = None;
            current = Some((next.clone(), written));
            checkpoint(WritePoint::Renamed, &current, &temporary, port)?;
        }
        checkpoint(WritePoint::BeforeDirectorySync, &current, &temporary, port)?;
        sync_directory(&self.journal.store.state_directory)?;
        checkpoint(WritePoint::DirectorySynced, &current, &temporary, port)?;
        checkpoint(WritePoint::Recorded, &current, &temporary, port)
    }

    pub(super) fn restore_compatibility(
        &self,
        guard: &UpgradeProcessGuard,
        record: &mut CancellationArchiveReceipt,
        port: &mut impl CancellationArchivePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if record.outer.new_bytes.is_none() {
            return Ok(());
        }
        let path = self.directory(record).join(COMPATIBILITY);
        let target = self
            .journal
            .store
            .root
            .path
            .join(OUTER_STATE)
            .join(RECEIPT_FILE_NAME);
        if record.compatibility.is_none() {
            self.checkpoint(
                guard,
                record,
                port,
                hasher,
                Point::BeforeCompatibilityCreate,
            )?;
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
            let stored = self.read_record(hasher)?.ok_or(Error::EvidenceChanged)?;
            let checkpoint = |point,
                              identity: &PreparationFileIdentity,
                              port: &mut _|
             -> Result<()> {
                let verify = || -> Result<()> {
                    self.verify(guard, record, hasher, Some((&path, identity)))?;
                    if self.read_record(hasher)?.as_ref() != Some(&stored) {
                        return Err(Error::EvidenceChanged);
                    }
                    Ok(())
                };
                verify()?;
                if !CancellationArchivePort::confirm_authority_and_quiescence(port, record, point) {
                    return Err(Error::AuthorityNotProven);
                }
                verify()
            };
            checkpoint(Point::CompatibilityCreated, &empty, port)?;
            file.write_all(record.outer.previous_bytes.as_bytes())
                .map_err(|_| Error::Io)?;
            let identity = self.journal.evidence(&path, hasher)?;
            if !same_object(&empty, &identity)
                || identity.sha256 != record.outer.previous_identity.sha256
            {
                return Err(Error::EvidenceChanged);
            }
            checkpoint(Point::CompatibilityWritten, &identity, port)?;
            file.sync_all().map_err(|_| Error::Io)?;
            checkpoint(Point::CompatibilityFileSynced, &identity, port)?;
            sync_directory(&self.directory(record))?;
            checkpoint(Point::CompatibilityDirectorySynced, &identity, port)?;
            let mut next = record.clone();
            next.compatibility = Some(identity);
            self.persist(guard, Some(record), &next, port, hasher)?;
            *record = next;
            self.checkpoint(guard, record, port, hasher, Point::CompatibilityBound)?;
        }
        self.checkpoint(
            guard,
            record,
            port,
            hasher,
            Point::BeforeCompatibilityPublish,
        )?;
        let location = self.unique(&path, &target)?;
        self.sync_file(
            &location,
            record
                .compatibility
                .as_ref()
                .ok_or(Error::EvidenceChanged)?,
            hasher,
        )?;
        self.checkpoint(
            guard,
            record,
            port,
            hasher,
            Point::BeforeCompatibilityPublish,
        )?;
        if location == path {
            if path_exists(&target)? {
                return Err(Error::EvidenceChanged);
            }
            fs::rename(&path, &target).map_err(|_| Error::Io)?;
        }
        self.checkpoint(guard, record, port, hasher, Point::CompatibilityPublished)?;
        sync_directory(target.parent().ok_or(Error::EvidenceChanged)?)?;
        self.checkpoint(
            guard,
            record,
            port,
            hasher,
            Point::CompatibilityTargetSynced,
        )?;
        sync_directory(&self.directory(record))?;
        self.checkpoint(
            guard,
            record,
            port,
            hasher,
            Point::CompatibilitySourceSynced,
        )
    }
}
