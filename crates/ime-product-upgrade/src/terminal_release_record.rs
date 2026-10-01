//! Canonical release proof and locator. Neither is product authorization.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalReleaseBinding {
    pub operation_id: String,
    pub installed_release: crate::ProductRelease,
    pub installed_product_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalReleasePhase {
    Reserved,
    ReleaseReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalReleaseReceipt {
    pub(super) format: String,
    pub(super) phase: TerminalReleasePhase,
    pub(super) binding: TerminalReleaseBinding,
    pub(super) data_root: PreparationDirectoryIdentity,
    pub(super) state_directory: PreparationDirectoryIdentity,
    pub(super) directories: Vec<PreparationDirectoryIdentity>,
    pub(super) receipt: UpgradeReceipt,
    pub(super) files: [Option<PreparationFileIdentity>; 5],
    pub(super) preparation: [Option<PreparationFileIdentity>; 2],
    pub(super) database: PreparationFileIdentity,
    pub(super) settings: Option<PreparationFileIdentity>,
    pub(super) rime: Option<PreparationDirectoryIdentity>,
    pub(super) previous_index: Option<PreviousIndex>,
    pub(super) archived_slots: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviousIndex {
    pub identity: PreparationFileIdentity,
    pub index: ReleaseIndex,
}

impl TerminalReleaseReceipt {
    pub fn phase(&self) -> TerminalReleasePhase {
        self.phase
    }
    pub fn binding(&self) -> &TerminalReleaseBinding {
        &self.binding
    }
    pub fn data_receipt(&self) -> &UpgradeReceipt {
        &self.receipt
    }
    pub fn archived_slots(&self) -> &[Slot] {
        &self.archived_slots
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode(bytes)?;
        value.validate()?;
        Ok(value)
    }
    pub(super) fn validate(&self) -> Result<()> {
        if self.directories.len() != 3 {
            return Err(Error::EvidenceChanged);
        }
        self.validate_inputs()
    }
    pub(super) fn validate_inputs(&self) -> Result<()> {
        if !matches!(self.format.as_str(), LEGACY_FORMAT | FORMAT)
            || !valid_id(&self.binding.operation_id)
            || self.binding.operation_id != self.receipt.operation_id()
            || !valid_hash(&self.binding.installed_product_sha256)
            || !self.receipt.state().is_terminal()
            || self.receipt.manual_recovery_required()
            || self.files[0].is_none()
            || self.preparation[0].is_some() != self.preparation[1].is_some()
        {
            return Err(Error::EvidenceChanged);
        }
        self.receipt.encode().map_err(|_| Error::EvidenceChanged)?;
        let installed = if self.receipt.state() == UpgradeState::Completed {
            self.receipt.target_release()
        } else {
            self.receipt.source_release()
        };
        if installed != &self.binding.installed_release {
            return Err(Error::EvidenceChanged);
        }
        let root = &self.data_root;
        let mut inodes = std::collections::BTreeSet::new();
        for dir in [&self.data_root, &self.state_directory]
            .into_iter()
            .chain(&self.directories)
        {
            if dir.inode == 0
                || dir.mode != 0o700
                || dir.owner_id != root.owner_id
                || dir.device_id != root.device_id
                || !inodes.insert(dir.inode)
            {
                return Err(Error::EvidenceChanged);
            }
        }
        let root_artifact = self
            .receipt
            .artifacts()
            .iter()
            .find(|a| a.slot() == UpgradeArtifactSlot::DataRoot)
            .ok_or(Error::EvidenceChanged)?;
        if root_artifact.device_id() != root.device_id
            || root_artifact.inode() != root.inode
            || root_artifact.owner_id() != root.owner_id
            || root_artifact.mode() != root.mode
        {
            return Err(Error::EvidenceChanged);
        }
        let expected_slots: Vec<_> = SLOTS
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| self.files[i].as_ref().map(|_| *slot))
            .collect();
        if !expected_slots.starts_with(&self.archived_slots)
            || (self.phase == TerminalReleasePhase::ReleaseReady
                && expected_slots != self.archived_slots)
        {
            return Err(Error::EvidenceChanged);
        }
        for file in self
            .files
            .iter()
            .chain(&self.preparation)
            .flatten()
            .chain([&self.database])
            .chain(self.settings.iter())
        {
            if file.inode == 0
                || file.mode != 0o600
                || file.link_count != 1
                || file.device_id != root.device_id
                || file.owner_id != root.owner_id
                || !valid_hash(&file.sha256)
                || !inodes.insert(file.inode)
            {
                return Err(Error::EvidenceChanged);
            }
        }
        for (i, slot) in SLOTS.iter().copied().enumerate().skip(1) {
            match (&self.files[i], expected_private(&self.receipt, slot)) {
                (Some(file), Some(artifact)) if matches_artifact(file, artifact) => {}
                (None, None) => {}
                _ => return Err(Error::EvidenceChanged),
            }
        }
        let active = if self.receipt.state() == UpgradeState::Completed {
            UpgradeArtifactSlot::CandidateDatabase
        } else {
            UpgradeArtifactSlot::SourceDatabase
        };
        if !self
            .receipt
            .artifacts()
            .iter()
            .find(|a| a.slot() == active)
            .is_some_and(|a| matches_artifact(&self.database, a))
        {
            return Err(Error::EvidenceChanged);
        }
        let settings = self
            .receipt
            .artifacts()
            .iter()
            .find(|a| a.slot() == UpgradeArtifactSlot::SourceSettings);
        match (&self.settings, settings) {
            (Some(file), Some(artifact)) if matches_artifact(file, artifact) => {}
            (None, None) => {}
            _ => return Err(Error::EvidenceChanged),
        }
        let rime = self
            .receipt
            .artifacts()
            .iter()
            .find(|a| a.slot() == UpgradeArtifactSlot::RimeRoot);
        match (&self.rime, rime) {
            (Some(dir), Some(artifact))
                if dir.mode == 0o700
                    && dir.device_id == root.device_id
                    && dir.owner_id == root.owner_id
                    && dir.inode == artifact.inode()
                    && dir.device_id == artifact.device_id()
                    && dir.owner_id == artifact.owner_id()
                    && inodes.insert(dir.inode) => {}
            (None, None) => {}
            _ => return Err(Error::EvidenceChanged),
        }
        if let Some(previous) = &self.previous_index {
            previous.index.validate()?;
            if (self.format == LEGACY_FORMAT && !previous.index.is_legacy())
                || previous.index.operation_id() == self.binding.operation_id
                || previous.index.data_root() != &self.data_root
                || previous.identity.mode != 0o600
                || previous.identity.link_count != 1
                || previous.identity.inode == 0
                || previous.identity.device_id != root.device_id
                || previous.identity.owner_id != root.owner_id
                || !valid_hash(&previous.identity.sha256)
            {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
    pub(super) fn can_replace(&self, previous: &Self) -> bool {
        if self.validate().is_err() || previous.validate().is_err() {
            return false;
        }
        if self == previous {
            return true;
        }
        let mut expected = previous.clone();
        if previous.phase == TerminalReleasePhase::Reserved {
            if self.phase == TerminalReleasePhase::ReleaseReady {
                expected.phase = TerminalReleasePhase::ReleaseReady;
            } else if self.archived_slots.len() == previous.archived_slots.len() + 1
                && self.archived_slots.starts_with(&previous.archived_slots)
            {
                expected.archived_slots = self.archived_slots.clone();
            }
        }
        self == &expected
    }
}

pub(super) fn valid_id(id: &str) -> bool {
    hex(id, 32)
}
pub(super) fn valid_hash(hash: &str) -> bool {
    hex(hash, 64)
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| Error::EvidenceChanged)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
        return Err(Error::EvidenceChanged);
    }
    Ok(bytes)
}
pub(super) fn decode<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    if bytes.is_empty() || bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
        return Err(Error::EvidenceChanged);
    }
    let value: T = serde_json::from_slice(bytes).map_err(|_| Error::EvidenceChanged)?;
    if encode(&value)? != bytes {
        return Err(Error::EvidenceChanged);
    }
    Ok(value)
}
