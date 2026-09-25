//! Durable source-preparation contract. No paths or filesystem authority.

use crate::{ProductRelease, UpgradeArtifactSlot, UpgradeReceipt, UpgradeState};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const PREPARATION_RECEIPT_FORMAT: &str = "radishlex-source-preparation-v1";
pub const MAX_PREPARATION_RECEIPT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationReceiptError {
    InvalidEncoding,
    InvalidIdentity,
    InvalidBinding,
    InvalidPhase,
    EvidenceChanged,
}
impl fmt::Display for PreparationReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEncoding => "invalid_preparation_encoding",
            Self::InvalidIdentity => "invalid_preparation_identity",
            Self::InvalidBinding => "invalid_preparation_binding",
            Self::InvalidPhase => "invalid_preparation_phase",
            Self::EvidenceChanged => "preparation_evidence_changed",
        })
    }
}
impl std::error::Error for PreparationReceiptError {}
type Error = PreparationReceiptError;

#[path = "preparation_handoff_contract.rs"]
mod handoff;
pub(crate) use handoff::PreparationHandoffIntent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationDirectoryIdentity {
    pub device_id: u64,
    pub inode: u64,
    pub owner_id: u32,
    pub mode: u32,
}
impl PreparationDirectoryIdentity {
    fn validate(&self) -> Result<(), Error> {
        if self.inode == 0 || self.mode != 0o700 {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationFileIdentity {
    pub device_id: u64,
    pub inode: u64,
    pub owner_id: u32,
    pub mode: u32,
    pub link_count: u64,
    pub byte_len: u64,
    pub sha256: String,
}
impl PreparationFileIdentity {
    fn validate(&self) -> Result<(), Error> {
        if self.inode == 0 || self.mode != 0o600 || self.link_count != 1 || !hex(&self.sha256, 64) {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
    fn same_object(&self, other: &Self) -> bool {
        self.device_id == other.device_id
            && self.inode == other.inode
            && self.owner_id == other.owner_id
            && self.mode == other.mode
            && self.link_count == other.link_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationFamily {
    pub database: PreparationFileIdentity,
    pub wal: Option<PreparationFileIdentity>,
    pub shm: Option<PreparationFileIdentity>,
    pub journal: Option<PreparationFileIdentity>,
}
impl PreparationFamily {
    fn validate(&self, root: &PreparationDirectoryIdentity) -> Result<(), Error> {
        let mut identities = std::collections::BTreeSet::new();
        for file in [
            Some(&self.database),
            self.wal.as_ref(),
            self.shm.as_ref(),
            self.journal.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            file.validate()?;
            if file.device_id != root.device_id
                || file.owner_id != root.owner_id
                || !identities.insert((file.device_id, file.inode))
            {
                return Err(Error::InvalidIdentity);
            }
        }
        // No maintenance connection may begin on an unqualified hot journal.
        if self.journal.is_some() {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationBinding {
    pub operation_id: String,
    pub previous_install_operation_id: Option<String>,
    pub previous_data_operation_id: Option<String>,
    pub source_release: ProductRelease,
    pub target_release: ProductRelease,
    pub source_product_sha256: String,
    pub target_product_sha256: String,
    pub previous_install_receipt_sha256: Option<String>,
    pub previous_data_receipt_sha256: Option<String>,
    pub previous_inventory_sha256: Option<String>,
    pub target_schema_version: i64,
    pub data_root: PreparationDirectoryIdentity,
    pub state_directory: PreparationDirectoryIdentity,
}
impl PreparationBinding {
    fn validate(&self) -> Result<(), Error> {
        if !hex(&self.operation_id, 32)
            || self.target_schema_version < 1
            || self.source_release == self.target_release
            || !hex(&self.source_product_sha256, 64)
            || !hex(&self.target_product_sha256, 64)
        {
            return Err(Error::InvalidBinding);
        }
        for previous in [
            &self.previous_install_operation_id,
            &self.previous_data_operation_id,
        ]
        .into_iter()
        .flatten()
        {
            if !hex(previous, 32) || previous == &self.operation_id {
                return Err(Error::InvalidBinding);
            }
        }
        for (previous, digest) in [
            (
                &self.previous_install_operation_id,
                &self.previous_install_receipt_sha256,
            ),
            (
                &self.previous_data_operation_id,
                &self.previous_data_receipt_sha256,
            ),
            (
                &self.previous_data_operation_id,
                &self.previous_inventory_sha256,
            ),
        ] {
            if previous.is_some() != digest.is_some()
                || digest.as_ref().is_some_and(|hash| !hex(hash, 64))
            {
                return Err(Error::InvalidBinding);
            }
        }
        self.data_root.validate()?;
        self.state_directory.validate()?;
        if self.data_root.device_id != self.state_directory.device_id
            || self.data_root.owner_id != self.state_directory.owner_id
            || self.data_root.inode == self.state_directory.inode
        {
            return Err(Error::InvalidIdentity);
        }
        // Deserialization does not bypass the release constructor's validation.
        for release in [&self.source_release, &self.target_release] {
            ProductRelease::new(release.product_version(), release.build_number())
                .map_err(|_| Error::InvalidBinding)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationPhase {
    Reserved,
    SnapshotReady,
    MaintenanceIntent,
    SourcePrepared,
    PreviousArchived,
    HandoffReady,
}

/// Fixed private v1 slots, in archive order. Runtime data never enters this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationArchiveSlot {
    Receipt,
    Snapshot,
    Candidate,
    Backup,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotEvidence {
    identity: PreparationFileIdentity,
    source_family: PreparationFamily,
    schema_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationReceipt {
    format: String,
    phase: PreparationPhase,
    binding: PreparationBinding,
    initial_source: PreparationFamily,
    snapshot: Option<SnapshotEvidence>,
    prepared_source: Option<PreparationFileIdentity>,
    archived_inventory_sha256: Option<String>,
    handoff_receipt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    journal_recovery: Option<PreparationFamily>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    archived_slots: Vec<PreparationArchiveSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_inventory_identity: Option<PreparationFileIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_release_index_identity: Option<PreparationFileIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handoff_intent: Option<PreparationHandoffIntent>,
}
impl PreparationReceipt {
    pub fn new(
        binding: PreparationBinding,
        initial_source: PreparationFamily,
    ) -> Result<Self, Error> {
        let receipt = Self {
            format: PREPARATION_RECEIPT_FORMAT.to_owned(),
            phase: PreparationPhase::Reserved,
            binding,
            initial_source,
            snapshot: None,
            prepared_source: None,
            archived_inventory_sha256: None,
            handoff_receipt_sha256: None,
            journal_recovery: None,
            archived_slots: Vec::new(),
            previous_inventory_identity: None,
            previous_release_index_identity: None,
            handoff_intent: None,
        };
        receipt.validate()?;
        Ok(receipt)
    }
    pub fn phase(&self) -> PreparationPhase {
        self.phase
    }
    pub fn binding(&self) -> &PreparationBinding {
        &self.binding
    }
    pub fn initial_source(&self) -> &PreparationFamily {
        &self.initial_source
    }
    pub fn prepared_source(&self) -> Option<&PreparationFileIdentity> {
        self.prepared_source.as_ref()
    }
    pub fn snapshot_identity(&self) -> Option<&PreparationFileIdentity> {
        self.snapshot.as_ref().map(|value| &value.identity)
    }
    pub fn snapshot_schema_version(&self) -> Option<i64> {
        self.snapshot.as_ref().map(|value| value.schema_version)
    }
    pub fn maintenance_source(&self) -> Option<&PreparationFamily> {
        self.snapshot.as_ref().map(|value| &value.source_family)
    }
    pub fn journal_recovery(&self) -> Option<&PreparationFamily> {
        self.journal_recovery.as_ref()
    }

    pub fn archived_slots(&self) -> &[PreparationArchiveSlot] {
        &self.archived_slots
    }

    pub fn previous_inventory_identity(&self) -> Option<&PreparationFileIdentity> {
        self.previous_inventory_identity.as_ref()
    }

    pub fn previous_release_index_identity(&self) -> Option<&PreparationFileIdentity> {
        self.previous_release_index_identity.as_ref()
    }

    /// Bind a released predecessor before persisting the reservation. The proof
    /// supplies the inventory; the index must retain its exact physical identity.
    pub fn bind_released_predecessor(
        &mut self,
        proof: PreparationFileIdentity,
        index: PreparationFileIdentity,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::Reserved || self.previous_inventory_identity.is_some() {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.previous_inventory_identity = Some(proof);
        next.previous_release_index_identity = Some(index);
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn handoff_intent(&self) -> Option<&PreparationHandoffIntent> {
        self.handoff_intent.as_ref()
    }

    pub(crate) fn archived_inventory_sha256(&self) -> Option<&str> {
        self.archived_inventory_sha256.as_deref()
    }

    pub(crate) fn bind_handoff_intent(
        &mut self,
        intent: PreparationHandoffIntent,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::PreviousArchived || self.handoff_intent.is_some() {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.handoff_intent = Some(intent);
        self.replace_with(next)
    }

    /// Bind before the first reservation is persisted; later replacements cannot change it.
    pub fn bind_previous_inventory(
        &mut self,
        identity: PreparationFileIdentity,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::Reserved || self.previous_inventory_identity.is_some() {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.previous_inventory_identity = Some(identity);
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Append only after the unique historical slot and both directories are durable.
    pub fn record_archived_slot(&mut self, slot: PreparationArchiveSlot) -> Result<(), Error> {
        if self.phase != PreparationPhase::SourcePrepared
            || self.archived_slots.last().is_some_and(|last| *last >= slot)
        {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.archived_slots.push(slot);
        self.replace_with(next)
    }

    /// Append exactly one recovery identity before any SQLite recovery write.
    /// The coordinator separately qualifies the journal and proves both guards.
    pub fn record_journal_recovery(&mut self, family: PreparationFamily) -> Result<(), Error> {
        if self.phase != PreparationPhase::MaintenanceIntent || self.journal_recovery.is_some() {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.journal_recovery = Some(family);
        self.replace_with(next)
    }

    pub fn record_snapshot(
        &mut self,
        identity: PreparationFileIdentity,
        source_family: PreparationFamily,
        schema_version: i64,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::Reserved {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.snapshot = Some(SnapshotEvidence {
            identity,
            source_family,
            schema_version,
        });
        next.phase = PreparationPhase::SnapshotReady;
        self.replace_with(next)
    }
    pub fn begin_maintenance(&mut self) -> Result<(), Error> {
        if self.phase != PreparationPhase::SnapshotReady {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.phase = PreparationPhase::MaintenanceIntent;
        self.replace_with(next)
    }
    pub fn record_prepared_source(
        &mut self,
        identity: PreparationFileIdentity,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::MaintenanceIntent {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.prepared_source = Some(identity);
        next.phase = PreparationPhase::SourcePrepared;
        self.replace_with(next)
    }
    pub fn record_previous_archive(&mut self, inventory_sha256: String) -> Result<(), Error> {
        if self.phase != PreparationPhase::SourcePrepared {
            return Err(Error::InvalidPhase);
        }
        let mut next = self.clone();
        next.archived_inventory_sha256 = Some(inventory_sha256);
        next.phase = PreparationPhase::PreviousArchived;
        self.replace_with(next)
    }
    /// This checks the v1 identity binding; the store must independently verify
    /// canonical receipt bytes, actual files, both guards and quiescence.
    pub fn record_handoff(
        &mut self,
        receipt: &UpgradeReceipt,
        sha256: String,
    ) -> Result<(), Error> {
        if self.phase != PreparationPhase::PreviousArchived {
            return Err(Error::InvalidPhase);
        }
        self.check_handoff_receipt(receipt)?;
        let mut next = self.clone();
        next.handoff_receipt_sha256 = Some(sha256);
        next.phase = PreparationPhase::HandoffReady;
        self.replace_with(next)
    }

    fn check_handoff_receipt(&self, receipt: &UpgradeReceipt) -> Result<(), Error> {
        let source = self.prepared_source.as_ref().ok_or(Error::InvalidPhase)?;
        let snapshot = self.snapshot.as_ref().ok_or(Error::InvalidPhase)?;
        let source_matches = receipt.artifacts().iter().any(|artifact| {
            artifact.slot() == UpgradeArtifactSlot::SourceDatabase
                && artifact.device_id() == source.device_id
                && artifact.inode() == source.inode
                && artifact.owner_id() == source.owner_id
                && artifact.mode() == source.mode
                && artifact.link_count() == source.link_count
                && artifact.byte_len() == source.byte_len
        });
        let root_matches = receipt.artifacts().iter().any(|artifact| {
            artifact.slot() == UpgradeArtifactSlot::DataRoot
                && artifact.device_id() == self.binding.data_root.device_id
                && artifact.inode() == self.binding.data_root.inode
                && artifact.owner_id() == self.binding.data_root.owner_id
                && artifact.mode() == self.binding.data_root.mode
        });
        if receipt.state() != UpgradeState::Preflighted
            || receipt.operation_id() != self.binding.operation_id
            || receipt.previous_operation_id() != self.binding.previous_data_operation_id.as_deref()
            || receipt.source_release() != &self.binding.source_release
            || receipt.target_release() != &self.binding.target_release
            || receipt.source_schema_version() != Some(snapshot.schema_version)
            || receipt.target_schema_version() != self.binding.target_schema_version
            || !source_matches
            || !root_matches
        {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self).map_err(|_| Error::InvalidEncoding)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::InvalidEncoding);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty()
            || bytes.len() > MAX_PREPARATION_RECEIPT_BYTES
            || !bytes.ends_with(b"\n")
        {
            return Err(Error::InvalidEncoding);
        }
        let receipt: Self = serde_json::from_slice(bytes).map_err(|_| Error::InvalidEncoding)?;
        if receipt.encode()?.as_slice() != bytes {
            return Err(Error::InvalidEncoding);
        }
        Ok(receipt)
    }
    pub fn can_replace(&self, previous: &Self) -> bool {
        if self.validate().is_err()
            || previous.validate().is_err()
            || self.binding != previous.binding
            || self.initial_source != previous.initial_source
            || self.previous_inventory_identity != previous.previous_inventory_identity
            || self.previous_release_index_identity != previous.previous_release_index_identity
        {
            return false;
        }
        if self == previous {
            return true;
        }
        let successor = matches!(
            (previous.phase, self.phase),
            (PreparationPhase::Reserved, PreparationPhase::SnapshotReady)
                | (
                    PreparationPhase::SnapshotReady,
                    PreparationPhase::MaintenanceIntent
                )
                | (
                    PreparationPhase::MaintenanceIntent,
                    PreparationPhase::SourcePrepared
                )
                | (
                    PreparationPhase::SourcePrepared,
                    PreparationPhase::PreviousArchived
                )
                | (
                    PreparationPhase::PreviousArchived,
                    PreparationPhase::HandoffReady
                )
        );
        let recovery_append = previous.phase == PreparationPhase::MaintenanceIntent
            && self.phase == PreparationPhase::MaintenanceIntent
            && previous.journal_recovery.is_none()
            && self.journal_recovery.is_some();
        let archive_append = previous.phase == PreparationPhase::SourcePrepared
            && self.phase == PreparationPhase::SourcePrepared
            && self.archived_slots.len() == previous.archived_slots.len() + 1
            && self.archived_slots.starts_with(&previous.archived_slots);
        let handoff_append = previous.phase == PreparationPhase::PreviousArchived
            && self.phase == PreparationPhase::PreviousArchived
            && previous.handoff_intent.is_none()
            && self.handoff_intent.is_some();
        ((recovery_append && self.archived_slots == previous.archived_slots)
            || (archive_append && self.journal_recovery == previous.journal_recovery)
            || (handoff_append
                && self.journal_recovery == previous.journal_recovery
                && self.archived_slots == previous.archived_slots)
            || (successor
                && self.journal_recovery == previous.journal_recovery
                && self.archived_slots == previous.archived_slots))
            && (self.handoff_intent == previous.handoff_intent || handoff_append)
            && preserves(&previous.snapshot, &self.snapshot)
            && preserves(&previous.prepared_source, &self.prepared_source)
            && preserves(
                &previous.archived_inventory_sha256,
                &self.archived_inventory_sha256,
            )
            && preserves(
                &previous.handoff_receipt_sha256,
                &self.handoff_receipt_sha256,
            )
    }
    fn replace_with(&mut self, next: Self) -> Result<(), Error> {
        next.validate()?;
        if !next.can_replace(self) {
            return Err(Error::EvidenceChanged);
        }
        *self = next;
        Ok(())
    }
    fn validate(&self) -> Result<(), Error> {
        if self.format != PREPARATION_RECEIPT_FORMAT {
            return Err(Error::InvalidEncoding);
        }
        self.binding.validate()?;
        if let Some(intent) = &self.handoff_intent {
            intent.validate(self)?;
        }
        self.initial_source.validate(&self.binding.data_root)?;
        if let Some(index) = &self.previous_release_index_identity {
            index.validate()?;
            if self.previous_inventory_identity.is_none()
                || self.binding.previous_data_operation_id.is_none()
                || index.device_id != self.binding.data_root.device_id
                || index.owner_id != self.binding.data_root.owner_id
                || self
                    .previous_inventory_identity
                    .as_ref()
                    .is_some_and(|proof| proof.inode == index.inode)
                || !self.archived_slots.is_empty()
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if let Some(identity) = &self.previous_inventory_identity {
            identity.validate()?;
            if self.binding.previous_inventory_sha256.as_ref() != Some(&identity.sha256)
                || identity.device_id != self.binding.data_root.device_id
                || identity.owner_id != self.binding.data_root.owner_id
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if (!self.archived_slots.is_empty()
            && (self.phase < PreparationPhase::SourcePrepared
                || self.binding.previous_data_operation_id.is_none()
                || self.previous_inventory_identity.is_none()
                || self.archived_slots.first() != Some(&PreparationArchiveSlot::Receipt)))
            || self
                .archived_slots
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::InvalidPhase);
        }
        if let (Some(expected), Some(actual)) = (
            &self.binding.previous_inventory_sha256,
            &self.archived_inventory_sha256,
        ) {
            if expected != actual {
                return Err(Error::EvidenceChanged);
            }
        }
        if let Some(family) = &self.journal_recovery {
            if self.phase < PreparationPhase::MaintenanceIntent
                || family.wal.is_some()
                || family.shm.is_some()
            {
                return Err(Error::InvalidPhase);
            }
            let journal = family.journal.as_ref().ok_or(Error::InvalidIdentity)?;
            journal.validate()?;
            family.database.validate()?;
            if !family.database.same_object(&self.initial_source.database)
                || journal.device_id != self.binding.data_root.device_id
                || journal.owner_id != self.binding.data_root.owner_id
                || journal.byte_len <= 512
                || journal.inode == family.database.inode
                || self
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| journal.inode == snapshot.identity.inode)
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.snapshot.is_some() != (self.phase >= PreparationPhase::SnapshotReady)
            || self.prepared_source.is_some() != (self.phase >= PreparationPhase::SourcePrepared)
            || self.archived_inventory_sha256.is_some()
                != (self.phase >= PreparationPhase::PreviousArchived)
            || self.handoff_receipt_sha256.is_some()
                != (self.phase >= PreparationPhase::HandoffReady)
        {
            return Err(Error::InvalidPhase);
        }
        if let Some(snapshot) = &self.snapshot {
            snapshot.identity.validate()?;
            snapshot.source_family.validate(&self.binding.data_root)?;
            if snapshot.schema_version < 0
                || snapshot.schema_version > self.binding.target_schema_version
                || snapshot.identity.owner_id != self.binding.data_root.owner_id
                || snapshot.identity.device_id != self.binding.data_root.device_id
                || snapshot.identity.byte_len == 0
                || [
                    Some(&snapshot.source_family.database),
                    snapshot.source_family.wal.as_ref(),
                    snapshot.source_family.shm.as_ref(),
                ]
                .into_iter()
                .flatten()
                .any(|file| snapshot.identity.same_object(file))
                || snapshot.source_family.database != self.initial_source.database
                || self
                    .initial_source
                    .wal
                    .as_ref()
                    .is_some_and(|wal| snapshot.source_family.wal.as_ref() != Some(wal))
            {
                return Err(Error::InvalidIdentity);
            }
            if self.initial_source.wal.is_none()
                && snapshot
                    .source_family
                    .wal
                    .as_ref()
                    .is_some_and(|wal| wal.byte_len != 0)
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if let Some(source) = &self.prepared_source {
            source.validate()?;
            if !source.same_object(&self.initial_source.database) {
                return Err(Error::InvalidIdentity);
            }
        }
        for hash in [
            &self.archived_inventory_sha256,
            &self.handoff_receipt_sha256,
        ]
        .into_iter()
        .flatten()
        {
            if !hex(hash, 64) {
                return Err(Error::InvalidIdentity);
            }
        }
        Ok(())
    }
}

fn preserves<T: PartialEq>(previous: &Option<T>, next: &Option<T>) -> bool {
    previous
        .as_ref()
        .map_or(true, |value| next.as_ref() == Some(value))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;
