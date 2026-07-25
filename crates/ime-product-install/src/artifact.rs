use serde::{Deserialize, Serialize};

use crate::{InstallReceiptError, ProgramBundleIdentity, ProgramComponent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramFilesystemIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    mode: u32,
}

impl ProgramFilesystemIdentity {
    pub fn new(
        device_id: u64,
        inode: u64,
        owner_id: u32,
        mode: u32,
    ) -> Result<Self, InstallReceiptError> {
        let identity = Self {
            device_id,
            inode,
            owner_id,
            mode,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn device_id(&self) -> u64 {
        self.device_id
    }

    pub const fn inode(&self) -> u64 {
        self.inode
    }

    pub const fn owner_id(&self) -> u32 {
        self.owner_id
    }

    pub const fn mode(&self) -> u32 {
        self.mode
    }

    pub(crate) fn validate(&self) -> Result<(), InstallReceiptError> {
        if self.inode == 0 || !matches!(self.mode, 0o700 | 0o755) {
            return Err(InstallReceiptError::invalid(
                "program_filesystem_identity",
                "program filesystem identity requires a positive inode and mode 0700 or 0755",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallArtifactSlot {
    SourceManager,
    SourceInputMethod,
    StagedManager,
    StagedInputMethod,
    BackupManager,
    BackupInputMethod,
    InstalledManager,
    InstalledInputMethod,
}

impl InstallArtifactSlot {
    pub(crate) const fn component(self) -> ProgramComponent {
        match self {
            Self::SourceManager
            | Self::StagedManager
            | Self::BackupManager
            | Self::InstalledManager => ProgramComponent::Manager,
            Self::SourceInputMethod
            | Self::StagedInputMethod
            | Self::BackupInputMethod
            | Self::InstalledInputMethod => ProgramComponent::InputMethod,
        }
    }

    pub(crate) const fn is_target(self) -> bool {
        matches!(
            self,
            Self::StagedManager
                | Self::StagedInputMethod
                | Self::InstalledManager
                | Self::InstalledInputMethod
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallArtifactEvidence {
    pub(crate) slot: InstallArtifactSlot,
    pub(crate) identity: ProgramBundleIdentity,
    pub(crate) filesystem_identity: ProgramFilesystemIdentity,
}

impl InstallArtifactEvidence {
    pub fn new(
        slot: InstallArtifactSlot,
        identity: ProgramBundleIdentity,
        filesystem_identity: ProgramFilesystemIdentity,
    ) -> Result<Self, InstallReceiptError> {
        if slot.component() != identity.component {
            return Err(InstallReceiptError::invalid(
                "artifact_evidence",
                "artifact slot and component differ",
            ));
        }
        identity.validate()?;
        filesystem_identity.validate()?;
        Ok(Self {
            slot,
            identity,
            filesystem_identity,
        })
    }

    pub const fn slot(&self) -> InstallArtifactSlot {
        self.slot
    }

    pub fn identity(&self) -> &ProgramBundleIdentity {
        &self.identity
    }

    pub fn filesystem_identity(&self) -> &ProgramFilesystemIdentity {
        &self.filesystem_identity
    }
}
