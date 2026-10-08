use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationOuterBinding {
    pub(in super::super) state_directory: PreparationDirectoryIdentity,
    pub(in super::super) history_root: PreparationDirectoryIdentity,
    pub(in super::super) previous_directory: PreparationDirectoryIdentity,
    pub(in super::super) previous_identity: PreparationFileIdentity,
    pub(in super::super) previous_bytes: String,
    pub(in super::super) active_identity: PreparationFileIdentity,
    pub(in super::super) new_bytes: Option<String>,
}
impl CancellationOuterBinding {
    pub fn previous_bytes(&self) -> &[u8] {
        self.previous_bytes.as_bytes()
    }
    pub fn new_bytes(&self) -> Option<&[u8]> {
        self.new_bytes.as_deref().map(str::as_bytes)
    }
    pub fn state_directory(&self) -> &PreparationDirectoryIdentity {
        &self.state_directory
    }
    pub fn previous_install_evidence(&self) -> crate::PreparationInstallEvidence {
        crate::PreparationInstallEvidence {
            state_directory: self.state_directory.clone(),
            history_root: self.history_root.clone(),
            previous_directory: self.previous_directory.clone(),
            previous_receipt: self.previous_identity.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationArchiveSlot {
    Previous(Slot),
    Snapshot,
    SourceProgress,
    Preparation,
    NewOuter,
}

/// A preservation proof, not cancel_ready and not an authorization to release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationArchiveReceipt {
    pub(in super::super) format: String,
    pub(in super::super) request: PreparationCancellationRequest,
    pub(in super::super) source: CancellationSourceReceipt,
    pub(in super::super) source_identity: PreparationFileIdentity,
    pub(in super::super) outer: CancellationOuterBinding,
    pub(in super::super) directories: Vec<PreparationDirectoryIdentity>,
    pub(in super::super) history_entries: Vec<String>,
    pub(in super::super) settings: Option<PreparationFileIdentity>,
    pub(in super::super) rime: Option<PreparationDirectoryIdentity>,
    pub(in super::super) previous_files: [Option<PreparationFileIdentity>; 5],
    pub(in super::super) previous_directories: Option<[PreparationDirectoryIdentity; 3]>,
    pub(in super::super) moved: Vec<ArchiveSlot>,
    pub(in super::super) compatibility: Option<PreparationFileIdentity>,
    pub(in super::super) preserved: bool,
}
impl CancellationArchiveReceipt {
    pub fn request(&self) -> &PreparationCancellationRequest {
        &self.request
    }
    pub fn source(&self) -> &CancellationSourceReceipt {
        &self.source
    }
    pub fn outer(&self) -> &CancellationOuterBinding {
        &self.outer
    }
    pub fn preserved(&self) -> bool {
        self.preserved
    }
    pub fn moved_slots(&self) -> &[ArchiveSlot] {
        &self.moved
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
        if value.encode()? != bytes {
            return Err(Error::EvidenceChanged);
        }
        Ok(value)
    }
    pub(in super::super) fn expected_slots(&self) -> Vec<ArchiveSlot> {
        let mut result = Vec::new();
        if self
            .request
            .preparation()
            .previous_release_index_identity()
            .is_none()
        {
            for (index, slot) in SLOTS.into_iter().enumerate() {
                if self.previous_files[index].is_some() {
                    result.push(ArchiveSlot::Previous(slot));
                }
            }
        }
        if self.request.preparation().snapshot_identity().is_some() {
            result.push(ArchiveSlot::Snapshot);
        }
        result.extend([ArchiveSlot::SourceProgress, ArchiveSlot::Preparation]);
        if self.outer.new_bytes.is_some() {
            result.push(ArchiveSlot::NewOuter);
        }
        result
    }
    pub(in super::super) fn validate(&self) -> Result<()> {
        self.request.encode()?;
        self.source.validate_request(&self.request)?;
        let prep = self.request.preparation();
        let binding = prep.binding();
        if self.format != "radishlex-cancellation-archive-v1"
            || self.source.phase() != CancellationSourcePhase::SourceReady
            || self.directories.len() != 2
            || !self.expected_slots().starts_with(&self.moved)
            || self.previous_directories.is_some() != binding.previous_data_operation_id.is_some()
            || self.previous_files[0].is_some() != binding.previous_data_operation_id.is_some()
            || self.source_identity.byte_len != self.source.encode()?.len() as u64
            || Some(&self.outer.previous_identity.sha256)
                != binding.previous_install_receipt_sha256.as_ref()
            || self.outer.previous_identity.byte_len != self.outer.previous_bytes.len() as u64
            || self.outer.active_identity.byte_len
                != self
                    .outer
                    .new_bytes
                    .as_ref()
                    .unwrap_or(&self.outer.previous_bytes)
                    .len() as u64
            || self.outer.previous_bytes.is_empty()
            || self.outer.new_bytes.as_ref() == Some(&self.outer.previous_bytes)
            || (self.compatibility.is_some()
                && (self.outer.new_bytes.is_none() || self.moved != self.expected_slots()))
            || (self.preserved
                && (self.moved != self.expected_slots()
                    || (self.outer.new_bytes.is_some() && self.compatibility.is_none())))
        {
            return Err(Error::EvidenceChanged);
        }
        for directory in self
            .directories
            .iter()
            .chain([
                &self.outer.state_directory,
                &self.outer.history_root,
                &self.outer.previous_directory,
            ])
            .chain(self.previous_directories.iter().flatten())
            .chain(self.rime.iter())
        {
            if directory.inode == 0 || directory.mode != 0o700 {
                return Err(Error::EvidenceChanged);
            }
            if directory.device_id != binding.data_root.device_id
                || directory.owner_id != binding.data_root.owner_id
            {
                return Err(Error::EvidenceChanged);
            }
        }
        for identity in [
            &self.source_identity,
            self.source.request_identity(),
            &self.outer.previous_identity,
            &self.outer.active_identity,
        ]
        .into_iter()
        .chain(self.previous_files.iter().flatten())
        .chain(self.compatibility.iter())
        .chain(self.settings.iter())
        {
            identity.validate().map_err(|_| Error::EvidenceChanged)?;
            if identity.device_id != binding.data_root.device_id
                || identity.owner_id != binding.data_root.owner_id
            {
                return Err(Error::EvidenceChanged);
            }
        }
        if let Some(identity) = &self.compatibility {
            if identity.sha256 != self.outer.previous_identity.sha256
                || identity.byte_len != self.outer.previous_identity.byte_len
                || identity.inode == self.outer.previous_identity.inode
                || identity.inode == self.outer.active_identity.inode
            {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
    pub(in super::super) fn can_replace(&self, old: &Self) -> bool {
        if self == old {
            return true;
        }
        let mut expected = old.clone();
        if old.moved.len() < old.expected_slots().len() {
            expected.moved.push(old.expected_slots()[old.moved.len()]);
        } else if old.outer.new_bytes.is_some() && old.compatibility.is_none() {
            if self.compatibility.is_none() {
                return false;
            }
            expected.compatibility = self.compatibility.clone();
        } else if !old.preserved {
            expected.preserved = true;
        }
        &expected == self
    }
}
