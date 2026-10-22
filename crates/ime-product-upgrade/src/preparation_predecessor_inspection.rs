//! Read-only admission evidence for a new preparation. No guard creation,
//! durability acknowledgement, inventory capture or SQLite access occurs here.
use super::*;

impl PreparationJournalStore {
    /// Inspect a terminal data predecessor and its exact lifecycle without
    /// taking a process guard. Execution must observe again under both guards.
    pub fn inspect_preparation_predecessor(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<(Option<UpgradeReceipt>, Option<LifecycleReleaseReceipt>)> {
        let guard = self.store.inspection_guard_identity()?;
        let before = maintenance_inspection::inspection_stamp(self, hasher)?;
        let observe = || self.inspect_new_predecessor(hasher);
        let result = observe()?;
        if observe()? != result
            || maintenance_inspection::inspection_stamp(self, hasher)? != before
            || self.store.inspection_guard_identity()? != guard
        {
            return Err(Error::EvidenceChanged);
        }
        self.store.revalidate()?;
        Ok(result)
    }

    fn inspect_new_predecessor(
        &self,
        hasher: &impl PreparationHasher,
    ) -> Result<(Option<UpgradeReceipt>, Option<LifecycleReleaseReceipt>)> {
        self.validate_entries()?;
        self.reject_staged_record()?;
        if self.read_record()?.is_some() {
            return Err(Error::InvalidPhase);
        }
        // No active preparation authorizes leftover snapshots or partial slots.
        // Only exact terminal receipt artifacts may remain in the active area.
        for entry in fs::read_dir(&self.store.state_directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if !SLOTS
                .iter()
                .any(|slot| entry.file_name() == slot_name(*slot))
            {
                return Err(Error::EvidenceChanged);
            }
        }
        let verified =
            VerifiedDataRoot::verify(&self.store.root.path, self.store.root.expected_owner_id)?;
        let lifecycle =
            TerminalReleaseStore::open_existing(verified)?.inspect_latest_lifecycle(hasher)?;
        let path = self.path(RECEIPT_FILE_NAME);
        if !path_exists(&path)? {
            if fs::read_dir(&self.store.state_directory)
                .map_err(|_| Error::Io)?
                .next()
                .is_some()
            {
                return Err(Error::EvidenceChanged);
            }
            let data = lifecycle
                .as_ref()
                .and_then(LifecycleReleaseReceipt::data_receipt)
                .cloned();
            if let Some(receipt) = &data {
                self.verify_readonly_predecessor(receipt)?;
            }
            return Ok((data, lifecycle));
        }
        let bytes = self.read_history_bytes(&path)?;
        let receipt = UpgradeReceipt::decode(&bytes).map_err(|_| Error::EvidenceChanged)?;
        self.verify_readonly_predecessor(&receipt)?;
        if receipt.encode().map_err(|_| Error::EvidenceChanged)? != bytes
            || lifecycle
                .as_ref()
                .is_some_and(|proof| proof.data_receipt() != Some(&receipt))
            || (lifecycle.is_none() && receipt.previous_operation_id().is_some())
        {
            return Err(Error::EvidenceChanged);
        }
        for slot in SLOTS.iter().copied().skip(1) {
            let path = self.path(slot_name(slot));
            match (expected_private(&receipt, slot), path_exists(&path)?) {
                (Some(artifact), true)
                    if matches_artifact(&self.evidence(&path, hasher)?, artifact) => {}
                (None, false) => {}
                _ => return Err(Error::EvidenceChanged),
            }
        }
        // Match observe_preparation_predecessor: an active receipt is the data
        // predecessor; a lifecycle value is returned for released empty slots.
        Ok((Some(receipt), None))
    }

    fn verify_readonly_predecessor(&self, receipt: &UpgradeReceipt) -> Result<()> {
        // A view does not allocate the future operation. Use a distinct ID only
        // to reuse the existing terminal/root validator; execution allocates and
        // binds its real operation while holding both guards.
        let distinct = if receipt.operation_id() == "11111111111111111111111111111111" {
            "22222222222222222222222222222222"
        } else {
            "11111111111111111111111111111111"
        };
        self.verify_previous_receipt(receipt, distinct)
    }
}
