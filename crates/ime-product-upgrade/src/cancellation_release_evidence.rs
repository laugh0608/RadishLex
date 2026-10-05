//! Read immutable cancellation materials independently of the current runtime.
use super::*;

impl ReleaseHistory<'_> {
    pub(in super::super) fn read_cancellation(
        &self,
        path: &Path,
        hasher: &impl PreparationHasher,
    ) -> Result<CancellationReleaseReceipt> {
        let identity = self.journal.evidence(path, hasher)?;
        let proof = CancellationReleaseReceipt::decode(&self.journal.read_history_bytes(path)?)?;
        if self.journal.evidence(path, hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(proof)
    }
    pub(in super::super) fn cancellation_index(
        &self,
        proof: &CancellationReleaseReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<ReleaseIndex> {
        let bytes = proof.encode()?;
        let digest = hash_bytes(&bytes, hasher)?;
        let path = self
            .journal
            .store
            .root
            .path
            .join(HISTORY)
            .join(proof.operation_id())
            .join(CANCELLATION_RELEASE);
        let identity = self.journal.evidence(&path, hasher)?;
        if identity.sha256 != digest || identity.byte_len != bytes.len() as u64 {
            return Err(Error::EvidenceChanged);
        }
        ReleaseIndex::for_cancellation(proof, identity)
    }
    pub(in super::super) fn verify_cancellation_materials(
        &self,
        proof: &CancellationReleaseReceipt,
        historical: bool,
        temporary: Option<(&Path, &PreparationFileIdentity)>,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        proof.validate()?;
        let archive = &proof.archive;
        let prep = proof.preparation();
        let binding = prep.binding();
        if binding.data_root != self.journal.data_root_identity()
            || binding.state_directory != self.journal.state_directory_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        let root = self.journal.store.root.path.join(HISTORY);
        let directory = root.join(proof.operation_id());
        for (path, identity) in [&root, &directory].into_iter().zip(&archive.directories) {
            if self.journal.history_directory(path)? != *identity {
                return Err(Error::EvidenceChanged);
            }
        }
        for entry in fs::read_dir(&root).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name();
            if name == INDEX || name == INDEX_TEMP {
                self.journal.private_metadata(&entry.path())?;
            } else if name.to_str().is_some_and(valid_id) {
                self.journal.history_directory(&entry.path())?;
            } else {
                return Err(Error::EvidenceChanged);
            }
        }
        let allowed = [
            CANCELLATION_RELEASE,
            ARCHIVE,
            REQUEST,
            "preparation.json",
            PREPARATION_SNAPSHOT,
            "cancellation-source.json",
            "cancelled-outer.json",
        ];
        for entry in fs::read_dir(&directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if !allowed.iter().any(|v| entry.file_name() == *v)
                && temporary.map(|v| v.0) != Some(entry.path().as_path())
            {
                return Err(Error::EvidenceChanged);
            }
            self.journal.private_metadata(&entry.path())?;
        }
        for (name, identity, bytes) in [
            (
                REQUEST,
                archive.source.request_identity(),
                archive.request.encode()?,
            ),
            (ARCHIVE, &proof.archive_identity, archive.encode()?),
        ] {
            let target = directory.join(name);
            let active = self.journal.store.state_directory.join(name);
            let path = if historical {
                target
            } else {
                match (path_exists(&active)?, path_exists(&target)?) {
                    (true, false) => active,
                    (false, true) => target,
                    _ => return Err(Error::EvidenceChanged),
                }
            };
            self.verify_bytes(&path, identity, &bytes, hasher)?;
        }
        for (name, identity, bytes) in [
            (
                "preparation.json",
                archive.request.preparation_identity(),
                prep.encode().map_err(|_| Error::EvidenceChanged)?,
            ),
            (
                "cancellation-source.json",
                &archive.source_identity,
                archive.source.encode()?,
            ),
        ] {
            self.verify_bytes(&directory.join(name), identity, &bytes, hasher)?;
        }
        let snapshot = directory.join(PREPARATION_SNAPSHOT);
        if self.optional_file(&snapshot, hasher)?.as_ref() != prep.snapshot_identity() {
            return Err(Error::EvidenceChanged);
        }
        no_sidecars(&snapshot)?;
        let outer = &archive.outer;
        let outer_root = self.journal.store.root.path.join(OUTER_HISTORY);
        let outer_directory = outer_root.join(proof.legacy_outer_operation_id());
        let state = self.journal.store.root.path.join(OUTER_STATE);
        for (path, identity) in [
            (&outer_root, &outer.history_root),
            (&outer_directory, &outer.previous_directory),
            (&state, &outer.state_directory),
        ] {
            if self.journal.history_directory(path)? != *identity {
                return Err(Error::EvidenceChanged);
            }
        }
        for entry in fs::read_dir(&outer_directory).map_err(|_| Error::Io)? {
            if entry.map_err(|_| Error::Io)?.file_name() != RECEIPT_FILE_NAME {
                return Err(Error::EvidenceChanged);
            }
        }
        self.verify_bytes(
            &outer_directory.join(RECEIPT_FILE_NAME),
            &outer.previous_identity,
            outer.previous_bytes.as_bytes(),
            hasher,
        )?;
        let new_outer = directory.join("cancelled-outer.json");
        match &outer.new_bytes {
            Some(bytes) => {
                self.verify_bytes(&new_outer, &outer.active_identity, bytes.as_bytes(), hasher)?
            }
            None if path_exists(&new_outer)? => return Err(Error::EvidenceChanged),
            None => {}
        }
        if let Some(receipt) = &proof.data_receipt {
            self.journal
                .verify_previous_receipt(receipt, proof.operation_id())?;
            let paths = [
                root.clone(),
                root.join(receipt.operation_id()),
                root.join(receipt.operation_id()).join("data"),
            ];
            for (path, expected) in paths.iter().zip(
                archive
                    .previous_directories
                    .as_ref()
                    .ok_or(Error::EvidenceChanged)?,
            ) {
                if self.journal.history_directory(path)? != *expected {
                    return Err(Error::EvidenceChanged);
                }
            }
            for entry in fs::read_dir(&paths[2]).map_err(|_| Error::Io)? {
                let entry = entry.map_err(|_| Error::Io)?;
                if !SLOTS
                    .iter()
                    .any(|slot| entry.file_name() == slot_name(*slot))
                {
                    return Err(Error::EvidenceChanged);
                }
            }
            for (slot, identity) in SLOTS.iter().zip(&archive.previous_files) {
                let path = paths[2].join(slot_name(*slot));
                if self.optional_file(&path, hasher)? != *identity {
                    return Err(Error::EvidenceChanged);
                }
                if *slot == Slot::Receipt {
                    self.verify_bytes(
                        &path,
                        identity.as_ref().ok_or(Error::EvidenceChanged)?,
                        &receipt.encode().map_err(|_| Error::EvidenceChanged)?,
                        hasher,
                    )?;
                }
            }
            if proof.previous_index.is_none() {
                // This old data operation never had a terminal-release proof.
                // Its original inventory is the authority, now bound by this proof.
                let mut inventory = PreviousInventory::empty(
                    self.journal,
                    proof.operation_id(),
                    Some(proof.legacy_outer_operation_id()),
                )?;
                inventory.receipt = Some(receipt.clone());
                inventory.files = archive.previous_files.clone();
                inventory.directories = archive.previous_directories.clone();
                inventory.verify_context(self.journal, prep)?;
                self.journal.verify_history_directories(&inventory, true)?;
                self.verify_bytes(
                    &paths[1].join(INVENTORY),
                    prep.previous_inventory_identity()
                        .ok_or(Error::EvidenceChanged)?,
                    &inventory.encode()?,
                    hasher,
                )?;
            }
        }
        if let Some(previous) = &proof.previous_index {
            let path = root
                .join(previous.index.operation_id())
                .join(previous.index.proof_file_name());
            if prep.previous_inventory_identity() != Some(&self.journal.evidence(&path, hasher)?) {
                return Err(Error::EvidenceChanged);
            }
        } else if proof.data_receipt.is_none() && prep.previous_inventory_identity().is_some() {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    fn verify_bytes(
        &self,
        path: &Path,
        identity: &PreparationFileIdentity,
        bytes: &[u8],
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self.journal.evidence(path, hasher)? != *identity
            || self.journal.read_history_bytes(path)? != bytes
            || self.journal.evidence(path, hasher)? != *identity
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}

pub(in super::super) fn hash_bytes(
    bytes: &[u8],
    hasher: &impl PreparationHasher,
) -> Result<String> {
    Ok(hasher
        .sha256(&mut &*bytes)
        .map_err(|_| Error::Io)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

impl CancellationReleaseStore {
    pub(super) fn verify_source_equivalence(
        &self,
        proof: &CancellationReleaseReceipt,
    ) -> Result<()> {
        let archive = &proof.archive;
        if proof.preparation().phase() >= PreparationPhase::MaintenanceIntent {
            let snapshot = self
                .directory(proof.operation_id())
                .join(PREPARATION_SNAPSHOT);
            no_sidecars(&self.journal.source_path())?;
            no_sidecars(&snapshot)?;
            if Some(UserDb::verify_prepared_source(
                self.journal.source_path(),
                &snapshot,
            )?) != archive.source.preparation().snapshot_schema_version()
            {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
    pub(super) fn verify_live(
        &self,
        guard: &UpgradeProcessGuard,
        proof: &CancellationReleaseReceipt,
        identity: Option<&PreparationFileIdentity>,
        temporary: Option<(&Path, &PreparationFileIdentity)>,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.journal.verify_guard(guard)?;
        self.validate_active()?;
        let directory = self.directory(proof.operation_id());
        let path = directory.join(CANCELLATION_RELEASE);
        if self.history().optional_file(&path, hasher)?.as_ref() != identity {
            return Err(Error::EvidenceChanged);
        }
        if let Some(identity) = identity {
            self.history()
                .verify_bytes(&path, identity, &proof.encode()?, hasher)?;
        }
        for path in [
            directory.join(PROOF_TEMP),
            self.journal.store.root.path.join(HISTORY).join(INDEX_TEMP),
        ] {
            let expected = temporary.filter(|v| v.0 == path).map(|v| v.1);
            if self.history().optional_file(&path, hasher)?.as_ref() != expected {
                return Err(Error::EvidenceChanged);
            }
        }
        self.history()
            .verify_cancellation_materials(proof, false, temporary, hasher)?;
        self.history().verify_lifecycle_ancestors(
            &LifecycleReleaseReceipt::Cancellation(Box::new(proof.clone())),
            hasher,
        )?;
        let archive = &proof.archive;
        if self.journal.family(hasher)?
            != *archive
                .source
                .source_ready()
                .ok_or(Error::EvidenceChanged)?
            || self.history().optional_file(
                &self.journal.store.root.path.join("manager-settings.json"),
                hasher,
            )? != archive.settings
        {
            return Err(Error::EvidenceChanged);
        }
        let rime = self.journal.store.root.path.join("Rime");
        let actual_rime = if path_exists(&rime)? {
            Some(self.journal.history_directory(&rime)?)
        } else {
            None
        };
        if actual_rime != archive.rime {
            return Err(Error::EvidenceChanged);
        }
        let outer_path = self.journal.store.root.path.join(OUTER_STATE);
        for entry in fs::read_dir(&outer_path).map_err(|_| Error::Io)? {
            if entry.map_err(|_| Error::Io)?.file_name() != RECEIPT_FILE_NAME {
                return Err(Error::EvidenceChanged);
            }
        }
        let compatibility = archive
            .compatibility
            .as_ref()
            .unwrap_or(&archive.outer.active_identity);
        self.history().verify_bytes(
            &outer_path.join(RECEIPT_FILE_NAME),
            compatibility,
            archive.outer.previous_bytes.as_bytes(),
            hasher,
        )?;
        let index_temp = temporary
            .filter(|v| v.0 == self.journal.store.root.path.join(HISTORY).join(INDEX_TEMP))
            .map(|v| v.1);
        let actual = self.history().load_index_with_temp(hasher, index_temp)?;
        let published = identity.is_some()
            && actual.as_ref().map(|v| &v.index)
                == Some(&self.history().cancellation_index(proof, hasher)?);
        if actual != proof.previous_index && !(identity.is_some() && published) {
            return Err(Error::EvidenceChanged);
        }
        if proof.previous_index.is_none() {
            for entry in
                fs::read_dir(self.journal.store.root.path.join(HISTORY)).map_err(|_| Error::Io)?
            {
                let name = entry.map_err(|_| Error::Io)?.file_name();
                if name != INDEX
                    && name != INDEX_TEMP
                    && name != proof.operation_id()
                    && !proof
                        .data_receipt
                        .as_ref()
                        .is_some_and(|v| name == v.operation_id())
                {
                    return Err(Error::EvidenceChanged);
                }
            }
        }
        if path_exists(&directory.join(ARCHIVE))? && identity.is_none() {
            return Err(Error::EvidenceChanged);
        }
        if path_exists(&directory.join(REQUEST))?
            && (!published || path_exists(&self.active(ARCHIVE))? || identity.is_none())
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(self.journal.verify_guard(guard)?)
    }
}
