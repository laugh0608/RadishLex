//! Exclusive creation before a handoff intention is durably bound.
use super::*;

struct CreationEvidence {
    directories: Vec<(PathBuf, PreparationDirectoryIdentity)>,
    receipt: Option<PreparationFileIdentity>,
}

impl PreparationJournalStore {
    pub(super) fn create_handoff_intent(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
    ) -> Result<PreparationReceipt> {
        let root = self.store.root.path.join(HISTORY);
        let directory = self.handoff_directory(record);
        if path_exists(&directory)? {
            // Even an empty directory must not be adopted without a durable pin.
            return Err(Error::EvidenceChanged);
        }
        let mut created = CreationEvidence {
            directories: Vec::new(),
            receipt: None,
        };
        for path in [&root, &directory] {
            if path != &root || !path_exists(path)? {
                DirBuilder::new()
                    .mode(0o700)
                    .create(path)
                    .map_err(|_| Error::Io)?;
            }
            created
                .directories
                .push((path.clone(), self.history_directory(path)?));
            let points = if path == &root {
                [
                    Point::HandoffHistoryRootReady,
                    Point::HandoffHistoryRootSynced,
                    Point::HandoffHistoryRootParentSynced,
                ]
            } else {
                [
                    Point::HandoffHistoryCreated,
                    Point::HandoffOperationSynced,
                    Point::HandoffOperationParentSynced,
                ]
            };
            self.creation_checkpoint(guard, port, hasher, record, &created, points[0])?;
            sync_directory(path)?;
            self.creation_checkpoint(guard, port, hasher, record, &created, points[1])?;
            sync_directory(path.parent().ok_or(Error::EvidenceChanged)?)?;
            self.creation_checkpoint(guard, port, hasher, record, &created, points[2])?;
        }
        let (artifacts, settings) = self.handoff_artifacts(hasher)?;
        let binding = record.binding();
        let receipt = UpgradeReceipt::new(
            &binding.operation_id,
            binding.previous_data_operation_id.clone(),
            binding.source_release.clone(),
            binding.target_release.clone(),
            record.snapshot_schema_version(),
            binding.target_schema_version,
            artifacts,
        )
        .map_err(|_| Error::EvidenceChanged)?;
        let bytes = receipt.encode().map_err(|_| Error::EvidenceChanged)?;
        let path = directory.join(RECEIPT_FILE_NAME);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        created.receipt = Some(self.evidence(&path, hasher)?);
        self.check_creation_file(
            &file,
            created.receipt.as_ref().ok_or(Error::EvidenceChanged)?,
        )?;
        self.creation_checkpoint(
            guard,
            port,
            hasher,
            record,
            &created,
            Point::HandoffReceiptCreated,
        )?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let identity = self.evidence(&path, hasher)?;
        if !same_object(
            created.receipt.as_ref().ok_or(Error::EvidenceChanged)?,
            &identity,
        ) {
            return Err(Error::EvidenceChanged);
        }
        created.receipt = Some(identity.clone());
        self.creation_checkpoint(
            guard,
            port,
            hasher,
            record,
            &created,
            Point::HandoffReceiptWritten,
        )?;
        self.check_creation_file(&file, &identity)?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.creation_checkpoint(
            guard,
            port,
            hasher,
            record,
            &created,
            Point::HandoffReceiptFileSynced,
        )?;
        sync_directory(&directory)?;
        self.creation_checkpoint(
            guard,
            port,
            hasher,
            record,
            &created,
            Point::HandoffHistorySynced,
        )?;
        let intent = PreparationHandoffIntent {
            directories: [
                created.directories[0].1.clone(),
                created.directories[1].1.clone(),
            ],
            receipt,
            receipt_identity: identity,
            settings,
        };
        if self.read_history_bytes(&path)? != bytes {
            return Err(Error::EvidenceChanged);
        }
        let mut next = record.clone();
        next.bind_handoff_intent(intent)
            .map_err(|_| Error::EvidenceChanged)?;
        self.persist(guard, Some(record), &next)?;
        Ok(next)
    }

    fn check_creation_file(&self, file: &File, identity: &PreparationFileIdentity) -> Result<()> {
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != identity.device_id || metadata.ino() != identity.inode {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn creation_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
        created: &CreationEvidence,
        point: Point,
    ) -> Result<()> {
        let verify = || -> Result<()> {
            self.verify_guard(guard)?;
            self.verify_binding(record)?;
            self.validate_entries()?;
            self.reject_staged_record()?;
            if self.load_guarded(guard)?.as_ref() != Some(record)
                || path_exists(&self.path(RECEIPT_FILE_NAME))?
            {
                return Err(Error::EvidenceChanged);
            }
            self.verify_prepared(record, hasher)?;
            self.verify_handoff_history(record, hasher)?;
            for (path, identity) in &created.directories {
                if self.history_directory(path)? != *identity {
                    return Err(Error::EvidenceChanged);
                }
            }
            let directory = self.handoff_directory(record);
            if created.directories.len() == 2 {
                for entry in fs::read_dir(&directory).map_err(|_| Error::Io)? {
                    if entry.map_err(|_| Error::Io)?.file_name() != RECEIPT_FILE_NAME {
                        return Err(Error::EvidenceChanged);
                    }
                }
            } else if path_exists(&directory)? {
                return Err(Error::EvidenceChanged);
            }
            let path = directory.join(RECEIPT_FILE_NAME);
            if let Some(identity) = &created.receipt {
                if self.evidence(&path, hasher)? != *identity {
                    return Err(Error::EvidenceChanged);
                }
            } else if path_exists(&path)? {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        };
        verify()?;
        let marker = self.evidence(&self.path(JOURNAL), hasher)?;
        if !port.confirm_authority_and_quiescence(record, point) {
            return Err(Error::AuthorityNotProven);
        }
        verify()?;
        if self.evidence(&self.path(JOURNAL), hasher)? != marker {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
