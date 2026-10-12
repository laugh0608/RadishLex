//! Read-only persisted maintenance progress, never execution or startup authority.
use super::*;
use crate::PreparationFileIdentity;

const REQUEST: &str = "preparation-cancellation.json";
const SOURCE: &str = "cancellation-source.json";
const ARCHIVE: &str = "cancellation-archive.json";
const PROOF: &str = "cancellation-release.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenancePhase {
    Preparing,
    PreparationHandedOff,
    PreparationTerminalRelease,
    CancellationRequested,
    CancellationFinishingSource,
    CancellationPreserving,
    CancellationAwaitingQualification,
    CancellationReleased,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceInspection {
    phase: MaintenancePhase,
    preparation: PreparationReceipt,
    handed_off_receipt: Option<UpgradeReceipt>,
    terminal_release: Option<TerminalReleaseReceipt>,
}

impl MaintenanceInspection {
    pub fn phase(&self) -> MaintenancePhase {
        self.phase
    }

    pub fn preparation(&self) -> &PreparationReceipt {
        &self.preparation
    }

    pub fn handed_off_receipt(&self) -> Option<&UpgradeReceipt> {
        self.handed_off_receipt.as_ref()
    }
    pub fn terminal_release(&self) -> Option<&TerminalReleaseReceipt> {
        self.terminal_release.as_ref()
    }
}

/// Inspect fixed private records and their physical evidence without creating
/// directories, acquiring guards, opening SQLite or acknowledging durability.
/// A released result describes preserved history; it is not source qualification.
pub fn inspect_maintenance(
    root: impl AsRef<Path>,
    owner: u32,
    hasher: &impl PreparationHasher,
) -> Result<Option<MaintenanceInspection>> {
    let root = root.as_ref();
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(PreparationJournalError::IdentityChanged.into());
    }
    if !path_exists(root)? {
        return Ok(None);
    }
    let verified = VerifiedDataRoot::verify(root, owner)?;
    if !path_exists(&root.join(STATE_DIRECTORY_NAME))? {
        if path_exists(&root.join(HISTORY))? {
            return Err(Error::EvidenceChanged);
        }
        return Ok(None);
    }
    let journal = PreparationJournalStore {
        store: UpgradeReceiptStore::open_existing_directory(verified)?,
    };
    let guard = journal.store.inspection_guard_identity()?;
    let before = inspection_stamp(&journal, hasher)?;
    let result = inspect_existing(&journal, hasher)?;
    if inspect_existing(&journal, hasher)? != result
        || inspection_stamp(&journal, hasher)? != before
        || journal.store.inspection_guard_identity()? != guard
    {
        return Err(Error::EvidenceChanged);
    }
    journal.store.revalidate()?;
    Ok(result)
}

fn inspection_stamp(
    journal: &PreparationJournalStore,
    hasher: &impl PreparationHasher,
) -> Result<Vec<(PathBuf, PreparationFileIdentity)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(&journal.store.state_directory).map_err(|_| Error::Io)? {
        let path = entry.map_err(|_| Error::Io)?.path();
        entries.push((path.clone(), journal.evidence(&path, hasher)?));
    }
    let index = journal
        .store
        .root
        .path
        .join(HISTORY)
        .join("latest-release.json");
    if path_exists(&index)? {
        entries.push((index.clone(), journal.evidence(&index, hasher)?));
    }
    let active = journal.path(RECEIPT_FILE_NAME);
    if path_exists(&active)? {
        let receipt = UpgradeReceipt::decode(&journal.read_history_bytes(&active)?)
            .map_err(|_| Error::EvidenceChanged)?;
        let proof = journal
            .store
            .root
            .path
            .join(HISTORY)
            .join(receipt.operation_id())
            .join("preparation.json");
        if path_exists(&proof)? {
            entries.push((proof.clone(), journal.evidence(&proof, hasher)?));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn inspect_existing(
    journal: &PreparationJournalStore,
    hasher: &impl PreparationHasher,
) -> Result<Option<MaintenanceInspection>> {
    let verified = || {
        VerifiedDataRoot::verify(
            &journal.store.root.path,
            journal.store.root.expected_owner_id,
        )
    };
    let result = if path_exists(&journal.path(REQUEST))? {
        let identity = journal.evidence(&journal.path(REQUEST), hasher)?;
        let request = PreparationCancellationRequest::decode(
            &journal.read_history_bytes(&journal.path(REQUEST))?,
        )?;
        let operation = &request.preparation().binding().operation_id;
        let proof = journal
            .store
            .root
            .path
            .join(HISTORY)
            .join(operation)
            .join(PROOF);
        let result = if path_exists(&proof)? {
            let store = CancellationReleaseStore::open_existing(verified()?)?;
            let (proof, awaiting) = store.inspect_pending(operation, hasher)?;
            MaintenanceInspection {
                phase: if awaiting {
                    MaintenancePhase::CancellationAwaitingQualification
                } else {
                    MaintenancePhase::CancellationPreserving
                },
                preparation: proof.preparation().clone(),
                handed_off_receipt: None,
                terminal_release: None,
            }
        } else if path_exists(&journal.path(ARCHIVE))? {
            let store = CancellationArchiveStore::open_existing(verified()?)?;
            let record = store
                .inspect_record(hasher)?
                .ok_or(Error::EvidenceChanged)?;
            MaintenanceInspection {
                phase: MaintenancePhase::CancellationPreserving,
                preparation: record.request().preparation().clone(),
                handed_off_receipt: None,
                terminal_release: None,
            }
        } else {
            let store = PreparationCancellationStore::open_existing(verified()?)?;
            let (request, source) = store.inspect_request_and_source(hasher)?;
            let phase = match source.as_ref().map(|value| value.phase()) {
                None => MaintenancePhase::CancellationRequested,
                Some(CancellationSourcePhase::Finishing) => {
                    MaintenancePhase::CancellationFinishingSource
                }
                Some(CancellationSourcePhase::SourceReady) => {
                    MaintenancePhase::CancellationPreserving
                }
            };
            MaintenanceInspection {
                phase,
                preparation: request.preparation().clone(),
                handed_off_receipt: None,
                terminal_release: None,
            }
        };
        if journal.evidence(&journal.path(REQUEST), hasher)? != identity {
            return Err(Error::EvidenceChanged);
        }
        result
    } else if path_exists(&journal.path(JOURNAL))? {
        journal.validate_entries()?;
        journal.reject_staged_record()?;
        let record = journal
            .read_record()?
            .ok_or(Error::EvidenceChanged)?
            .receipt;
        if record.handoff_intent().is_some() {
            journal.verify_handoff_files(hasher, &record)?;
            return Ok(Some(MaintenanceInspection {
                phase: MaintenancePhase::Preparing,
                preparation: record,
                handed_off_receipt: None,
                terminal_release: None,
            }));
        }
        let inventory = journal.load_previous_inventory(&record, hasher)?;
        journal.verify_inventory_objects(&inventory, hasher, Some(&record))?;
        if record.snapshot_identity().is_some() {
            journal.verify_snapshot(&record, hasher)?;
        } else if path_exists(&journal.path(PREPARATION_SNAPSHOT))? {
            return Err(Error::EvidenceChanged);
        }
        // Never open a pager while observing preparation. Physical checks do
        // not stand in for the coordinator's logical equivalence verification.
        let family = journal.read_family(hasher)?;
        if !files::same_object(&family.database, &record.initial_source().database) {
            return Err(Error::EvidenceChanged);
        }
        match record.phase() {
            PreparationPhase::Reserved if family != *record.initial_source() => {
                return Err(Error::EvidenceChanged)
            }
            PreparationPhase::SnapshotReady if Some(&family) != record.maintenance_source() => {
                return Err(Error::EvidenceChanged)
            }
            PreparationPhase::SourcePrepared
            | PreparationPhase::PreviousArchived
            | PreparationPhase::HandoffReady
                if record.prepared_source() != Some(&family.database)
                    || family.wal.is_some()
                    || family.shm.is_some()
                    || family.journal.is_some() =>
            {
                return Err(Error::EvidenceChanged);
            }
            _ => {}
        }
        MaintenanceInspection {
            phase: MaintenancePhase::Preparing,
            preparation: record,
            handed_off_receipt: None,
            terminal_release: None,
        }
    } else if path_exists(&journal.path("terminal-release.json"))? {
        let record = TerminalReleaseReceipt::decode(
            &journal.read_history_bytes(&journal.path("terminal-release.json"))?,
        )?;
        let store = TerminalReleaseStore::open_existing(verified()?)?;
        let (release, preparation) =
            store.inspect_terminal_release_readonly(&record.binding().operation_id, hasher)?;
        MaintenanceInspection {
            phase: MaintenancePhase::PreparationTerminalRelease,
            preparation,
            handed_off_receipt: Some(release.data_receipt().clone()),
            terminal_release: Some(release),
        }
    } else {
        // No record is not a first-use fallback for orphaned cancellation slots.
        if path_exists(&journal.path(ARCHIVE))?
            || path_exists(&journal.path(SOURCE))?
            || path_exists(&journal.path("terminal-release.json"))?
        {
            return Err(Error::EvidenceChanged);
        }
        if !path_exists(&journal.store.root.path.join(HISTORY))? {
            if journal
                .store
                .load_for_recovery_inspection()?
                .is_some_and(|receipt| receipt.previous_operation_id().is_some())
            {
                return Err(Error::EvidenceChanged);
            }
            return Ok(None);
        }
        if path_exists(&journal.path(RECEIPT_FILE_NAME))? {
            let current = journal
                .store
                .load_for_recovery_inspection()?
                .ok_or(Error::EvidenceChanged)?;
            let proof = journal
                .store
                .root
                .path
                .join(HISTORY)
                .join(current.operation_id())
                .join("preparation.json");
            if path_exists(&proof)? {
                let record = journal.read_handoff_proof(current.operation_id(), hasher)?;
                journal.verify_completed_handoff(&record, &current, hasher)?;
                return Ok(Some(MaintenanceInspection {
                    phase: MaintenancePhase::PreparationHandedOff,
                    preparation: record,
                    handed_off_receipt: Some(current),
                    terminal_release: None,
                }));
            }
        }
        let store = TerminalReleaseStore::open_existing(verified()?)?;
        let latest = store.inspect_latest_lifecycle(hasher)?;
        if path_exists(&journal.path(RECEIPT_FILE_NAME))? {
            let current = journal
                .store
                .load_for_recovery_inspection()?
                .ok_or(Error::EvidenceChanged)?;
            if !current.state().is_terminal()
                || latest
                    .as_ref()
                    .and_then(LifecycleReleaseReceipt::data_receipt)
                    != Some(&current)
            {
                return Err(Error::EvidenceChanged);
            }
            return Ok(None);
        }
        if fs::read_dir(&journal.store.state_directory)
            .map_err(|_| Error::Io)?
            .next()
            .is_some()
        {
            return Err(Error::EvidenceChanged);
        }
        match latest {
            Some(LifecycleReleaseReceipt::Cancellation(proof)) => MaintenanceInspection {
                phase: MaintenancePhase::CancellationReleased,
                preparation: proof.preparation().clone(),
                handed_off_receipt: None,
                terminal_release: None,
            },
            _ => return Ok(None),
        }
    };
    Ok(Some(result))
}
