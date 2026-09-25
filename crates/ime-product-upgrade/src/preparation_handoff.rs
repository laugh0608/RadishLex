//! Narrow transfer to a new v1 receipt; ordinary receipt readers stay strict.
use super::*;
use crate::preparation::PreparationHandoffIntent;
use crate::PreparationFileIdentity;
use SourcePreparationCheckpoint as Point;

const PROOF: &str = "preparation.json";

#[path = "preparation_handoff_creation.rs"]
mod creation;
#[path = "preparation_handoff_evidence.rs"]
mod evidence;

impl PreparationJournalStore {
    /// Consume the preparation store and return the same guarded v1 store.
    /// The caller must supply the expected NEW outer operation on every retry.
    /// Success leaves a nonterminal preflighted receipt: it never allows startup.
    /// Errors preserve all files, including unproven interrupted creations.
    pub fn handoff_userdb_source(
        self,
        guard: &UpgradeProcessGuard,
        operation_id: &str,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
    ) -> Result<(UpgradeReceiptStore, UpgradeReceipt)> {
        if operation_id.len() != 32
            || !operation_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::EvidenceChanged);
        }
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        let mut record = match self.read_record()? {
            Some(stored) => stored.receipt,
            None => self.read_handoff_proof(operation_id, hasher)?,
        };
        if record.binding().operation_id != operation_id
            || !matches!(
                record.phase(),
                PreparationPhase::PreviousArchived | PreparationPhase::HandoffReady
            )
        {
            return Err(Error::InvalidPhase);
        }
        if record.handoff_intent().is_none() {
            // This also rejects a prior unrecorded history directory or file.
            self.archive_checkpoint(guard, port, hasher, &record, Point::HandoffBegin)?;
            self.persist(guard, Some(&record), &record)?;
            record = self.create_handoff_intent(guard, port, hasher, &record)?;
        }
        self.handoff_checkpoint(guard, port, hasher, &record, Point::HandoffIntentRecorded)?;
        let intent = record
            .handoff_intent()
            .ok_or(Error::EvidenceChanged)?
            .clone();
        let history = self.handoff_directory(&record);
        if record.phase() == PreparationPhase::PreviousArchived {
            self.persist(guard, Some(&record), &record)?;
            self.handoff_move(
                guard,
                port,
                hasher,
                &record,
                (
                    &history.join(RECEIPT_FILE_NAME),
                    &self.path(RECEIPT_FILE_NAME),
                ),
                &intent.receipt_identity,
                [
                    Point::HandoffBeforeReceiptRename,
                    Point::HandoffReceiptRenamed,
                    Point::HandoffReceiptTargetSynced,
                    Point::HandoffReceiptSourceSynced,
                ],
            )?;
            let mut next = record.clone();
            next.record_handoff(&intent.receipt, intent.receipt_identity.sha256.clone())
                .map_err(|_| Error::EvidenceChanged)?;
            self.persist(guard, Some(&record), &next)?;
            record = next;
        }
        self.handoff_checkpoint(guard, port, hasher, &record, Point::HandoffReadyRecorded)?;
        if path_exists(&self.path(JOURNAL))? {
            // Repeat durability after an interrupted journal replacement.
            self.persist(guard, Some(&record), &record)?;
        }
        self.handoff_move(
            guard,
            port,
            hasher,
            &record,
            (
                &self.path(PREPARATION_SNAPSHOT),
                &history.join(PREPARATION_SNAPSHOT),
            ),
            record.snapshot_identity().ok_or(Error::EvidenceChanged)?,
            [
                Point::HandoffBeforeSnapshotRename,
                Point::HandoffSnapshotRenamed,
                Point::HandoffSnapshotTargetSynced,
                Point::HandoffSnapshotSourceSynced,
            ],
        )?;
        let proof_location = self.unique_location(&self.path(JOURNAL), &history.join(PROOF))?;
        let proof_identity = self.evidence(&proof_location, hasher)?;
        self.handoff_move(
            guard,
            port,
            hasher,
            &record,
            (&self.path(JOURNAL), &history.join(PROOF)),
            &proof_identity,
            [
                Point::HandoffBeforeMarkerRename,
                Point::HandoffMarkerRenamed,
                Point::HandoffMarkerTargetSynced,
                Point::HandoffMarkerSourceSynced,
            ],
        )?;
        self.handoff_checkpoint(guard, port, hasher, &record, Point::HandoffComplete)?;
        if self.store.load_guarded(guard)?.as_ref() != Some(&intent.receipt) {
            return Err(Error::EvidenceChanged);
        }
        Ok((self.store, intent.receipt))
    }

    #[allow(clippy::too_many_arguments)]
    fn handoff_move(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
        paths: (&Path, &Path),
        identity: &PreparationFileIdentity,
        points: [Point; 4],
    ) -> Result<()> {
        let (source, target) = paths;
        self.handoff_checkpoint(guard, port, hasher, record, points[0])?;
        let location = self.unique_location(source, target)?;
        if self.evidence(&location, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        let file = File::open(&location).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.dev() != identity.device_id || metadata.ino() != identity.inode {
            return Err(Error::EvidenceChanged);
        }
        file.sync_all().map_err(|_| Error::Io)?;
        if self.evidence(&location, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        if location == source {
            if path_exists(target)? {
                return Err(Error::EvidenceChanged);
            }
            fs::rename(source, target).map_err(|_| Error::Io)?;
        }
        if self.evidence(target, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        self.handoff_checkpoint(guard, port, hasher, record, points[1])?;
        sync_directory(target.parent().ok_or(Error::EvidenceChanged)?)?;
        self.handoff_checkpoint(guard, port, hasher, record, points[2])?;
        sync_directory(source.parent().ok_or(Error::EvidenceChanged)?)?;
        self.handoff_checkpoint(guard, port, hasher, record, points[3])?;
        Ok(())
    }

    fn handoff_checkpoint(
        &self,
        guard: &UpgradeProcessGuard,
        port: &mut impl SourcePreparationPort,
        hasher: &impl PreparationHasher,
        record: &PreparationReceipt,
        point: Point,
    ) -> Result<()> {
        self.verify_handoff(guard, hasher, record)?;
        let proof_path = self.unique_location(
            &self.path(JOURNAL),
            &self.handoff_directory(record).join(PROOF),
        )?;
        let proof = self.evidence(&proof_path, hasher)?;
        if !port.confirm_authority_and_quiescence(record, point) {
            return Err(Error::AuthorityNotProven);
        }
        self.verify_handoff(guard, hasher, record)?;
        if self.evidence(&proof_path, hasher)? != proof {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    fn handoff_directory(&self, record: &PreparationReceipt) -> PathBuf {
        self.store
            .root
            .path
            .join(HISTORY)
            .join(&record.binding().operation_id)
    }

    fn unique_location(&self, source: &Path, target: &Path) -> Result<PathBuf> {
        match (path_exists(source)?, path_exists(target)?) {
            (true, false) => Ok(source.to_path_buf()),
            (false, true) => Ok(target.to_path_buf()),
            _ => Err(Error::EvidenceChanged),
        }
    }
}
