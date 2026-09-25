//! Append-only physical intention for transferring a prepared source to v1.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparationHandoffIntent {
    pub directories: [PreparationDirectoryIdentity; 2],
    pub receipt: UpgradeReceipt,
    pub receipt_identity: PreparationFileIdentity,
    pub settings: Option<PreparationFileIdentity>,
}

impl PreparationHandoffIntent {
    pub(super) fn validate(&self, record: &PreparationReceipt) -> Result<(), Error> {
        if record.phase < PreparationPhase::PreviousArchived {
            return Err(Error::InvalidPhase);
        }
        self.receipt.encode().map_err(|_| Error::InvalidBinding)?;
        record.check_handoff_receipt(&self.receipt)?;
        for directory in &self.directories {
            directory.validate()?;
            if directory.device_id != record.binding.data_root.device_id
                || directory.owner_id != record.binding.data_root.owner_id
                || directory.inode == record.binding.data_root.inode
                || directory.inode == record.binding.state_directory.inode
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if self.directories[0] == self.directories[1] {
            return Err(Error::InvalidIdentity);
        }
        for file in [Some(&self.receipt_identity), self.settings.as_ref()]
            .into_iter()
            .flatten()
        {
            file.validate()?;
            if file.device_id != record.binding.data_root.device_id
                || file.owner_id != record.binding.data_root.owner_id
            {
                return Err(Error::InvalidIdentity);
            }
        }
        if record
            .handoff_receipt_sha256
            .as_ref()
            .is_some_and(|hash| hash != &self.receipt_identity.sha256)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
}
