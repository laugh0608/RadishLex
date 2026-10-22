//! Bind a completed preparation to its evolving active v1 transaction.
//! Persisted observations do not confer execution authority or durability.
use super::*;

impl PreparationJournalStore {
    /// Reopen the existing v1 chain under the already-held data guard. The
    /// caller must retain its outer guard and separately authorize execution.
    /// This does not create a receipt, open SQLite, sync files or release gates.
    pub fn rebind_handed_off_upgrade(
        self,
        guard: &UpgradeProcessGuard,
        expected: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<(UpgradeReceiptStore, UpgradeReceipt)> {
        self.verify_guard(guard)?;
        let operation = &expected.binding().operation_id;
        let proof_path = self.handoff_directory(expected).join(PROOF);
        let proof_identity = self.evidence(&proof_path, hasher)?;
        let receipt_identity = self.evidence(&self.path(RECEIPT_FILE_NAME), hasher)?;
        let receipt = self
            .store
            .load_guarded(guard)?
            .ok_or(Error::EvidenceChanged)?;
        let record = self.read_handoff_proof(operation, hasher)?;
        if record != *expected {
            return Err(Error::EvidenceChanged);
        }
        self.verify_completed_handoff(&record, &receipt, hasher)?;
        if self.read_handoff_proof(operation, hasher)? != record
            || self.evidence(&proof_path, hasher)? != proof_identity
            || self.evidence(&self.path(RECEIPT_FILE_NAME), hasher)? != receipt_identity
        {
            return Err(Error::EvidenceChanged);
        }
        self.store.verify_current(guard, &receipt)?;
        self.verify_guard(guard)?;
        Ok((self.store, receipt))
    }

    pub(in super::super) fn verify_completed_handoff(
        &self,
        record: &PreparationReceipt,
        current: &UpgradeReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.verify_binding(record)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        if record.phase() != PreparationPhase::HandoffReady
            || path_exists(&self.path(JOURNAL))?
            || path_exists(&self.path(PREPARATION_SNAPSHOT))?
        {
            return Err(Error::EvidenceChanged);
        }
        let intent = record.handoff_intent().ok_or(Error::EvidenceChanged)?;
        let original = &intent.receipt;
        if current.operation_id() != original.operation_id()
            || current.previous_operation_id() != original.previous_operation_id()
            || current.data_layout() != original.data_layout()
            || current.source_release() != original.source_release()
            || current.target_release() != original.target_release()
            || current.source_schema_version() != original.source_schema_version()
            || current.target_schema_version() != original.target_schema_version()
            || original
                .artifacts()
                .iter()
                .any(|item| !current.artifacts().contains(item))
        {
            return Err(Error::EvidenceChanged);
        }
        let directory = self.handoff_directory(record);
        if self.history_directory(&self.store.root.path.join(HISTORY))? != intent.directories[0]
            || self.history_directory(&directory)? != intent.directories[1]
        {
            return Err(Error::EvidenceChanged);
        }
        // Terminal release owns its later directory grammar. An active v1
        // rebind cannot adopt partial release material or unknown files.
        for entry in fs::read_dir(&directory).map_err(|_| Error::Io)? {
            let entry = entry.map_err(|_| Error::Io)?;
            if entry.file_name() != PROOF && entry.file_name() != PREPARATION_SNAPSHOT {
                return Err(Error::EvidenceChanged);
            }
            self.private_metadata(&entry.path())?;
        }
        if self.read_handoff_proof(current.operation_id(), hasher)? != *record {
            return Err(Error::EvidenceChanged);
        }
        let snapshot = directory.join(PREPARATION_SNAPSHOT);
        no_sidecars(&snapshot)?;
        if record.snapshot_identity() != Some(&self.evidence(&snapshot, hasher)?) {
            return Err(Error::EvidenceChanged);
        }
        self.verify_preserved_predecessor(record, hasher)?;
        if current.state() == UpgradeState::Preflighted
            && (current != original
                || self.evidence(&self.path(RECEIPT_FILE_NAME), hasher)? != intent.receipt_identity)
        {
            return Err(Error::EvidenceChanged);
        }
        // Before switch preparation, the exact source is still authoritative.
        // SwitchPrepared may already have moved either file; v1 validates its
        // exact scene, including the temporarily absent active database.
        if matches!(
            current.state(),
            UpgradeState::Preflighted
                | UpgradeState::Quiesced
                | UpgradeState::SnapshotReady
                | UpgradeState::CandidateMigrated
                | UpgradeState::CandidateVerified
                | UpgradeState::AbortedPreserved
        ) {
            no_sidecars(&self.source_path())?;
            if record.prepared_source() != Some(&self.family(hasher)?.database) {
                return Err(Error::EvidenceChanged);
            }
            self.verify_handoff_runtime_artifacts(record, hasher)?;
        }
        Ok(())
    }
}
