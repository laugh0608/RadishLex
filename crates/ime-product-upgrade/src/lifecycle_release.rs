//! A lifecycle result can preserve an older data transaction or no data at all.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleReleaseReceipt {
    Terminal(Box<TerminalReleaseReceipt>),
    Cancellation(Box<CancellationReleaseReceipt>),
}

impl LifecycleReleaseReceipt {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Terminal(value) => &value.binding.operation_id,
            Self::Cancellation(value) => value.operation_id(),
        }
    }
    pub fn legacy_outer_operation_id(&self) -> &str {
        match self {
            Self::Terminal(value) => &value.binding.operation_id,
            Self::Cancellation(value) => value.legacy_outer_operation_id(),
        }
    }
    pub fn data_receipt(&self) -> Option<&UpgradeReceipt> {
        match self {
            Self::Terminal(value) => Some(&value.receipt),
            Self::Cancellation(value) => value.data_receipt.as_ref(),
        }
    }
    pub fn installed_release(&self) -> &crate::ProductRelease {
        match self {
            Self::Terminal(value) => &value.binding.installed_release,
            Self::Cancellation(value) => &value.preparation().binding().source_release,
        }
    }
    pub(super) fn product_sha256(&self) -> &str {
        match self {
            Self::Terminal(value) => &value.binding.installed_product_sha256,
            Self::Cancellation(value) => &value.preparation().binding().source_product_sha256,
        }
    }
    pub(super) fn previous_index(&self) -> Option<&PreviousIndex> {
        match self {
            Self::Terminal(value) => value.previous_index.as_ref(),
            Self::Cancellation(value) => value.previous_index.as_ref(),
        }
    }
    pub(super) fn previous_data_id(&self) -> Option<&str> {
        match self {
            Self::Terminal(value) => value.receipt.previous_operation_id(),
            Self::Cancellation(value) => value
                .preparation()
                .binding()
                .previous_data_operation_id
                .as_deref(),
        }
    }
    pub(super) fn source_release(&self) -> &crate::ProductRelease {
        match self {
            Self::Terminal(value) => value.receipt.source_release(),
            Self::Cancellation(value) => &value.preparation().binding().source_release,
        }
    }
    pub(super) fn files(&self) -> &[Option<PreparationFileIdentity>; 5] {
        match self {
            Self::Terminal(value) => &value.files,
            Self::Cancellation(value) => &value.archive.previous_files,
        }
    }
    pub(super) fn data_directories(&self) -> Result<Option<[PreparationDirectoryIdentity; 3]>> {
        match self {
            Self::Terminal(value) => Ok(Some(
                value
                    .directories
                    .clone()
                    .try_into()
                    .map_err(|_| Error::EvidenceChanged)?,
            )),
            Self::Cancellation(value) => Ok(value.archive.previous_directories.clone()),
        }
    }
}
