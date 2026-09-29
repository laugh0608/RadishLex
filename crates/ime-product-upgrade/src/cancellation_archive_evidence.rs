use super::*;

impl CancellationArchiveStore {
    pub(super) fn optional_file(
        &self,
        path: &Path,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<PreparationFileIdentity>> {
        if path_exists(path)? {
            self.journal.evidence(path, hasher).map(Some)
        } else {
            Ok(None)
        }
    }
    pub(super) fn optional_directory(
        &self,
        path: &Path,
    ) -> Result<Option<PreparationDirectoryIdentity>> {
        if path_exists(path)? {
            self.journal.history_directory(path).map(Some)
        } else {
            Ok(None)
        }
    }
    pub(super) fn names(&self, path: &Path) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(path).map_err(|_| Error::Io)? {
            names.push(
                entry
                    .map_err(|_| Error::Io)?
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::EvidenceChanged)?,
            );
        }
        names.sort();
        Ok(names)
    }
    pub(super) fn verify_runtime_materials(
        &self,
        record: &CancellationArchiveReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self.optional_file(
            &self.journal.store.root.path.join("manager-settings.json"),
            hasher,
        )? != record.settings
            || self.optional_directory(&self.journal.store.root.path.join("Rime"))? != record.rime
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    pub(super) fn validate_entries(&self, temporary: Option<&Path>) -> Result<()> {
        self.journal.store.revalidate()?;
        let allowed = [REQUEST, SOURCE, JOURNAL, PREPARATION_SNAPSHOT, ARCHIVE];
        for entry in fs::read_dir(&self.journal.store.state_directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if !allowed.iter().any(|name| entry.file_name() == *name)
                && !SLOTS
                    .iter()
                    .any(|slot| entry.file_name() == slot_name(*slot))
                && temporary != Some(entry.path().as_path())
            {
                return Err(Error::EvidenceChanged);
            }
            self.journal.private_metadata(&entry.path())?;
        }
        Ok(())
    }

    pub(super) fn slot<'a>(
        &self,
        record: &'a CancellationArchiveReceipt,
        slot: ArchiveSlot,
    ) -> Result<(PathBuf, PathBuf, &'a PreparationFileIdentity)> {
        let directory = self.directory(record);
        let prep = record.request.preparation();
        Ok(match slot {
            ArchiveSlot::Previous(slot) => {
                let index = SLOTS
                    .iter()
                    .position(|value| *value == slot)
                    .ok_or(Error::EvidenceChanged)?;
                let previous = prep
                    .binding()
                    .previous_data_operation_id
                    .as_deref()
                    .ok_or(Error::EvidenceChanged)?;
                (
                    self.active(slot_name(slot)),
                    self.journal
                        .store
                        .root
                        .path
                        .join(HISTORY)
                        .join(previous)
                        .join("data")
                        .join(slot_name(slot)),
                    record.previous_files[index]
                        .as_ref()
                        .ok_or(Error::EvidenceChanged)?,
                )
            }
            ArchiveSlot::Snapshot => (
                self.active(PREPARATION_SNAPSHOT),
                directory.join(PREPARATION_SNAPSHOT),
                prep.snapshot_identity().ok_or(Error::EvidenceChanged)?,
            ),
            ArchiveSlot::SourceProgress => (
                self.active(SOURCE),
                directory.join(SOURCE),
                &record.source_identity,
            ),
            ArchiveSlot::Preparation => (
                self.active(JOURNAL),
                directory.join("preparation.json"),
                record.request.preparation_identity(),
            ),
            ArchiveSlot::NewOuter => (
                self.journal
                    .store
                    .root
                    .path
                    .join(OUTER_STATE)
                    .join(RECEIPT_FILE_NAME),
                directory.join(NEW_OUTER),
                &record.outer.active_identity,
            ),
        })
    }

    pub(super) fn verify(
        &self,
        guard: &UpgradeProcessGuard,
        record: &CancellationArchiveReceipt,
        hasher: &impl PreparationHasher,
        temporary: Option<(&Path, &PreparationFileIdentity)>,
    ) -> Result<()> {
        self.journal.verify_guard(guard)?;
        record.validate()?;
        self.validate_entries(temporary.map(|v| v.0))?;
        let prep = record.request.preparation();
        if prep.binding().data_root != self.journal.data_root_identity()
            || prep.binding().state_directory != self.journal.state_directory_identity()
            || self.journal.family(hasher)?
                != *record.source.source_ready().ok_or(Error::EvidenceChanged)?
            || self.journal.evidence(&self.active(REQUEST), hasher)?
                != *record.source.request_identity()
            || self.read_bytes(&self.active(REQUEST))? != record.request.encode()?
        {
            return Err(Error::EvidenceChanged);
        }
        let directory = self.directory(record);
        self.verify_runtime_materials(record, hasher)?;
        if self.names(directory.parent().ok_or(Error::EvidenceChanged)?)? != record.history_entries
        {
            return Err(Error::EvidenceChanged);
        }
        for (path, expected) in [
            directory.parent().ok_or(Error::EvidenceChanged)?,
            &directory,
        ]
        .into_iter()
        .zip(&record.directories)
        {
            if self.journal.history_directory(path)? != *expected {
                return Err(Error::EvidenceChanged);
            }
        }
        let allowed = [
            PREPARATION_SNAPSHOT,
            SOURCE,
            "preparation.json",
            NEW_OUTER,
            COMPATIBILITY,
        ];
        for entry in fs::read_dir(&directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if !allowed.iter().any(|name| entry.file_name() == *name) {
                return Err(Error::EvidenceChanged);
            }
            self.journal.private_metadata(&entry.path())?;
        }
        self.verify_previous(record, hasher)?;
        let expected = record.expected_slots();
        let mut actual = Vec::new();
        let mut active_seen = false;
        for slot in &expected {
            let (source, target, identity) = self.slot(record, *slot)?;
            // After new outer preservation, the active path can contain the
            // different, explicitly pinned compatibility projection.
            let location = if *slot == ArchiveSlot::NewOuter && record.moved.contains(slot) {
                target.clone()
            } else {
                self.unique(&source, &target)?
            };
            if self.journal.evidence(&location, hasher)? != *identity {
                return Err(Error::EvidenceChanged);
            }
            if location == target {
                if active_seen {
                    return Err(Error::EvidenceChanged);
                }
                actual.push(*slot);
            } else {
                active_seen = true;
            }
        }
        if !actual.starts_with(&record.moved) || actual.len() > record.moved.len() + 1 {
            return Err(Error::EvidenceChanged);
        }
        if record.preserved && actual != expected {
            return Err(Error::EvidenceChanged);
        }
        for (slot, bytes) in [
            (ArchiveSlot::SourceProgress, record.source.encode()?),
            (
                ArchiveSlot::Preparation,
                prep.encode().map_err(|_| Error::EvidenceChanged)?,
            ),
        ] {
            let (source, target, identity) = self.slot(record, slot)?;
            let path = self.unique(&source, &target)?;
            if self.read_bytes(&path)? != bytes
                || self.journal.evidence(&path, hasher)? != *identity
            {
                return Err(Error::EvidenceChanged);
            }
        }
        if let Some(snapshot) = prep.snapshot_identity() {
            let (source, target, _) = self.slot(record, ArchiveSlot::Snapshot)?;
            let location = self.unique(&source, &target)?;
            no_sidecars(&location)?;
            if self.journal.evidence(&location, hasher)? != *snapshot {
                return Err(Error::EvidenceChanged);
            }
        } else if path_exists(&self.active(PREPARATION_SNAPSHOT))?
            || path_exists(&directory.join(PREPARATION_SNAPSHOT))?
        {
            return Err(Error::EvidenceChanged);
        }
        let compatibility_temp =
            temporary.filter(|(path, _)| *path == directory.join(COMPATIBILITY));
        self.verify_outer(
            &record.request,
            &record.outer,
            record
                .compatibility
                .as_ref()
                .or(compatibility_temp.map(|v| v.1)),
            actual.contains(&ArchiveSlot::NewOuter),
            record.preserved,
            hasher,
        )?;
        if let Some((path, identity)) = temporary {
            if self.journal.evidence(path, hasher)? != *identity {
                return Err(Error::EvidenceChanged);
            }
        }
        self.journal.verify_guard(guard)?;
        Ok(())
    }

    pub(super) fn verify_source_equivalence(
        &self,
        record: &CancellationArchiveReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if record.request.preparation().phase() < PreparationPhase::MaintenanceIntent {
            return Ok(());
        }
        let (source, target, identity) = self.slot(record, ArchiveSlot::Snapshot)?;
        let snapshot = self.unique(&source, &target)?;
        let family = record.source.source_ready().ok_or(Error::EvidenceChanged)?;
        if self.journal.family(hasher)? != *family
            || self.journal.evidence(&snapshot, hasher)? != *identity
        {
            return Err(Error::EvidenceChanged);
        }
        no_sidecars(&self.journal.source_path())?;
        no_sidecars(&snapshot)?;
        if Some(UserDb::verify_prepared_source(
            self.journal.source_path(),
            &snapshot,
        )?) != record.source.preparation().snapshot_schema_version()
            || self.journal.family(hasher)? != *family
            || self.journal.evidence(&snapshot, hasher)? != *identity
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    pub(super) fn verify_previous(
        &self,
        record: &CancellationArchiveReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let prep = record.request.preparation();
        let inventory = if prep.binding().previous_data_operation_id.is_none() {
            PreviousInventory::empty(
                &self.journal,
                &prep.binding().operation_id,
                prep.binding().previous_install_operation_id.as_deref(),
            )?
        } else {
            self.journal.load_previous_inventory(prep, hasher)?
        };
        if inventory.files != record.previous_files
            || inventory.directories != record.previous_directories
        {
            return Err(Error::EvidenceChanged);
        }
        if inventory.released.is_some() {
            // load_previous_inventory resolves the exact v1 index and complete
            // frozen proof; unlike the normal preparer, our bound directory may exist.
            for (index, slot) in SLOTS.into_iter().enumerate() {
                if path_exists(&self.active(slot_name(slot)))? {
                    return Err(Error::EvidenceChanged);
                }
                let path = inventory.paths(&self.journal)?[2].join(slot_name(slot));
                match &record.previous_files[index] {
                    Some(expected) if self.journal.evidence(&path, hasher)? == *expected => {}
                    None if !path_exists(&path)? => {}
                    _ => return Err(Error::EvidenceChanged),
                }
            }
        } else {
            for (index, slot) in SLOTS.into_iter().enumerate() {
                if record.previous_files[index].is_none() {
                    if path_exists(&self.active(slot_name(slot)))? {
                        return Err(Error::EvidenceChanged);
                    }
                    if inventory.directories.is_some()
                        && path_exists(&inventory.paths(&self.journal)?[2].join(slot_name(slot)))?
                    {
                        return Err(Error::EvidenceChanged);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn verify_outer(
        &self,
        request: &PreparationCancellationRequest,
        outer: &CancellationOuterBinding,
        compatibility: Option<&PreparationFileIdentity>,
        moved: bool,
        preserved: bool,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let root = &self.journal.store.root.path;
        let prep = request.preparation();
        let previous = prep
            .binding()
            .previous_install_operation_id
            .as_deref()
            .ok_or(Error::EvidenceChanged)?;
        let history = root.join(OUTER_HISTORY);
        let old_directory = history.join(previous);
        let state = root.join(OUTER_STATE);
        for (path, expected) in [
            (&state, &outer.state_directory),
            (&history, &outer.history_root),
            (&old_directory, &outer.previous_directory),
        ] {
            if self.journal.history_directory(path)? != *expected {
                return Err(Error::EvidenceChanged);
            }
        }
        for entry in fs::read_dir(&old_directory).map_err(|_| Error::Io)? {
            if entry.map_err(|_| Error::Io)?.file_name() != PREVIOUS_OUTER {
                return Err(Error::EvidenceChanged);
            }
        }
        for entry in fs::read_dir(&state).map_err(|_| Error::Io)? {
            if entry.map_err(|_| Error::Io)?.file_name() != RECEIPT_FILE_NAME {
                return Err(Error::EvidenceChanged);
            }
        }
        let previous_path = old_directory.join(PREVIOUS_OUTER);
        if self.journal.evidence(&previous_path, hasher)? != outer.previous_identity
            || self.read_bytes(&previous_path)? != outer.previous_bytes.as_bytes()
            || self.journal.evidence(&previous_path, hasher)? != outer.previous_identity
            || Some(&outer.previous_identity.sha256)
                != prep.binding().previous_install_receipt_sha256.as_ref()
        {
            return Err(Error::EvidenceChanged);
        }
        let current = state.join(RECEIPT_FILE_NAME);
        let directory = root.join(HISTORY).join(&prep.binding().operation_id);
        let new_path = directory.join(NEW_OUTER);
        let temporary = directory.join(COMPATIBILITY);
        if let Some(bytes) = &outer.new_bytes {
            let location = if moved {
                new_path.clone()
            } else {
                current.clone()
            };
            if self.journal.evidence(&location, hasher)? != outer.active_identity
                || self.read_bytes(&location)? != bytes.as_bytes()
                || self.journal.evidence(&location, hasher)? != outer.active_identity
                || (!moved && path_exists(&new_path)?)
            {
                return Err(Error::EvidenceChanged);
            }
            if moved {
                if let Some(identity) = compatibility {
                    let projection = self.unique(&temporary, &current)?;
                    if self.journal.evidence(&projection, hasher)? != *identity
                        || (preserved && projection != current)
                    {
                        return Err(Error::EvidenceChanged);
                    }
                    // A just-created exclusive temporary can still be empty;
                    // publication is separately gated by its bound full digest.
                    if identity.byte_len != 0
                        && self.read_bytes(&projection)? != outer.previous_bytes.as_bytes()
                    {
                        return Err(Error::EvidenceChanged);
                    }
                } else if path_exists(&current)? || path_exists(&temporary)? {
                    return Err(Error::EvidenceChanged);
                }
            } else if compatibility.is_some() || path_exists(&temporary)? {
                return Err(Error::EvidenceChanged);
            }
        } else if moved
            || compatibility.is_some()
            || path_exists(&new_path)?
            || path_exists(&temporary)?
            || self.journal.evidence(&current, hasher)? != outer.active_identity
            || self.read_bytes(&current)? != outer.previous_bytes.as_bytes()
            || self.journal.evidence(&current, hasher)? != outer.active_identity
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
