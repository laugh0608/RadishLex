//! An immutable cancellation request, not a cancellation completion proof.
use super::*;
use serde::{Deserialize, Serialize};

const FORMAT: &str = "radishlex-preparation-cancellation-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Requested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Reason {
    UserRequested,
}

/// The request freezes the original preparation and observed family. It does
/// not assert logical equivalence, restored outer state or permission to start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationCancellationRequest {
    format: String,
    phase: Phase,
    reason: Reason,
    preparation: PreparationReceipt,
    preparation_identity: crate::PreparationFileIdentity,
    observed_source: PreparationFamily,
}

impl PreparationCancellationRequest {
    pub(super) fn new(
        preparation: PreparationReceipt,
        preparation_identity: crate::PreparationFileIdentity,
        observed_source: PreparationFamily,
    ) -> Result<Self> {
        let value = Self {
            format: FORMAT.to_owned(),
            phase: Phase::Requested,
            reason: Reason::UserRequested,
            preparation,
            preparation_identity,
            observed_source,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn preparation(&self) -> &PreparationReceipt {
        &self.preparation
    }

    pub fn preparation_identity(&self) -> &crate::PreparationFileIdentity {
        &self.preparation_identity
    }

    pub fn observed_source(&self) -> &PreparationFamily {
        &self.observed_source
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self).map_err(|_| Error::EvidenceChanged)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::EvidenceChanged)?;
        if value.encode()?.as_slice() != bytes {
            return Err(Error::EvidenceChanged);
        }
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        let prep = &self.preparation;
        let binding = prep.binding();
        let bytes = prep.encode().map_err(|_| Error::EvidenceChanged)?;
        if self.format != FORMAT
            || binding.previous_install_operation_id.is_none()
            || prep.handoff_intent().is_some()
            || prep.phase() >= PreparationPhase::HandoffReady
            || self.preparation_identity.byte_len != bytes.len() as u64
        {
            return Err(Error::EvidenceChanged);
        }
        self.observed_source
            .validate(&binding.data_root)
            .map_err(|_| Error::EvidenceChanged)?;
        self.preparation_identity
            .validate()
            .map_err(|_| Error::EvidenceChanged)?;
        let file = &self.preparation_identity;
        if file.device_id != binding.data_root.device_id
            || file.owner_id != binding.data_root.owner_id
            || [
                Some(&self.observed_source.database),
                self.observed_source.wal.as_ref(),
                self.observed_source.shm.as_ref(),
                prep.snapshot_identity(),
                prep.previous_inventory_identity(),
                prep.previous_release_index_identity(),
            ]
            .into_iter()
            .flatten()
            .any(|other| file.inode == other.inode)
            || file.inode == binding.data_root.inode
            || file.inode == binding.state_directory.inode
        {
            return Err(Error::EvidenceChanged);
        }
        validate_source(prep, &self.observed_source)
    }
}

pub(super) fn validate_source(prep: &PreparationReceipt, family: &PreparationFamily) -> Result<()> {
    if family.journal.is_some() {
        return Err(Error::EvidenceChanged);
    }
    match prep.phase() {
        PreparationPhase::Reserved => readonly_family(prep.initial_source(), family),
        PreparationPhase::SnapshotReady => {
            if prep.maintenance_source() != Some(family) {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        }
        PreparationPhase::MaintenanceIntent => {
            // A request is safe while maintenance is interrupted, but this is
            // not evidence of content equivalence or qualification to release.
            if !same_object(&prep.initial_source().database, &family.database) {
                return Err(Error::EvidenceChanged);
            }
            if let Some(wal) = &family.wal {
                if wal.byte_len != 0
                    && prep
                        .maintenance_source()
                        .and_then(|old| old.wal.as_ref())
                        .map_or(true, |old| !same_object(old, wal))
                {
                    return Err(Error::EvidenceChanged);
                }
            }
            if prep.journal_recovery().is_some() && (family.wal.is_some() || family.shm.is_some()) {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        }
        PreparationPhase::SourcePrepared | PreparationPhase::PreviousArchived => {
            if family.wal.is_some()
                || family.shm.is_some()
                || prep.prepared_source() != Some(&family.database)
            {
                return Err(Error::EvidenceChanged);
            }
            Ok(())
        }
        PreparationPhase::HandoffReady => Err(Error::InvalidPhase),
    }
}
