//! Versioned locator for the single latest-release.json slot. A locator is not
//! release authority: readers must resolve its matching complete proof first.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", deny_unknown_fields)]
pub(super) enum ReleaseIndex {
    // Field order is the original v1 canonical order. Never retag old bytes.
    #[serde(rename = "radishlex-latest-release-v1")]
    V1 {
        operation_id: String,
        data_operation_id: String,
        installed_release: crate::ProductRelease,
        data_root: PreparationDirectoryIdentity,
        release_sha256: String,
    },
    #[serde(rename = "radishlex-latest-release-v2")]
    V2 {
        operation_id: String,
        legacy_outer_operation_id: String,
        data_operation_id: Option<String>,
        installed_release: crate::ProductRelease,
        data_root: PreparationDirectoryIdentity,
        proof: LifecycleProofReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum LifecycleProofReference {
    TerminalRelease {
        sha256: String,
    },
    PreparationCancellation {
        sha256: String,
        file_identity: PreparationFileIdentity,
    },
}

impl ReleaseIndex {
    pub(super) fn for_cancellation(
        encoded: &cancellation_release::EncodedCancellationProof<'_>,
        identity: PreparationFileIdentity,
    ) -> Result<Self> {
        let record = encoded.receipt();
        let result = Self::V2 {
            operation_id: record.operation_id().to_owned(),
            legacy_outer_operation_id: record.legacy_outer_operation_id().to_owned(),
            data_operation_id: record.data_receipt().map(|v| v.operation_id().to_owned()),
            installed_release: record.preparation().binding().source_release.clone(),
            data_root: record.preparation().binding().data_root.clone(),
            proof: LifecycleProofReference::PreparationCancellation {
                sha256: identity.sha256.clone(),
                file_identity: identity,
            },
        };
        result.validate()?;
        Ok(result)
    }
    pub(super) fn proof_file_name(&self) -> &'static str {
        match self {
            Self::V2 {
                proof: LifecycleProofReference::PreparationCancellation { .. },
                ..
            } => cancellation_release::CANCELLATION_RELEASE,
            _ => RELEASE,
        }
    }
    pub(super) fn legacy_outer_operation_id(&self) -> &str {
        match self {
            Self::V1 { operation_id, .. } => operation_id,
            Self::V2 {
                legacy_outer_operation_id,
                ..
            } => legacy_outer_operation_id,
        }
    }
    pub(super) fn for_terminal(record: &TerminalReleaseReceipt, digest: String) -> Result<Self> {
        let result = match record.format.as_str() {
            LEGACY_FORMAT => Self::V1 {
                operation_id: record.binding.operation_id.clone(),
                data_operation_id: record.receipt.operation_id().to_owned(),
                installed_release: record.binding.installed_release.clone(),
                data_root: record.data_root.clone(),
                release_sha256: digest,
            },
            FORMAT => Self::V2 {
                operation_id: record.binding.operation_id.clone(),
                legacy_outer_operation_id: record.binding.operation_id.clone(),
                data_operation_id: Some(record.receipt.operation_id().to_owned()),
                installed_release: record.binding.installed_release.clone(),
                data_root: record.data_root.clone(),
                proof: LifecycleProofReference::TerminalRelease { sha256: digest },
            },
            _ => return Err(Error::EvidenceChanged),
        };
        result.validate()?;
        Ok(result)
    }

    pub(super) fn is_legacy(&self) -> bool {
        matches!(self, Self::V1 { .. })
    }
    pub(super) fn operation_id(&self) -> &str {
        match self {
            Self::V1 { operation_id, .. } | Self::V2 { operation_id, .. } => operation_id,
        }
    }
    pub(super) fn data_operation_id(&self) -> Option<&str> {
        match self {
            Self::V1 {
                data_operation_id, ..
            } => Some(data_operation_id),
            Self::V2 {
                data_operation_id, ..
            } => data_operation_id.as_deref(),
        }
    }
    pub(super) fn installed_release(&self) -> &crate::ProductRelease {
        match self {
            Self::V1 {
                installed_release, ..
            }
            | Self::V2 {
                installed_release, ..
            } => installed_release,
        }
    }
    pub(super) fn data_root(&self) -> &PreparationDirectoryIdentity {
        match self {
            Self::V1 { data_root, .. } | Self::V2 { data_root, .. } => data_root,
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        let digest = match self {
            Self::V1 { release_sha256, .. } => release_sha256,
            Self::V2 {
                legacy_outer_operation_id,
                proof,
                ..
            } => match proof {
                LifecycleProofReference::TerminalRelease { sha256 } => {
                    if legacy_outer_operation_id != self.operation_id()
                        || self.data_operation_id() != Some(self.operation_id())
                    {
                        return Err(Error::EvidenceChanged);
                    }
                    sha256
                }
                LifecycleProofReference::PreparationCancellation {
                    sha256,
                    file_identity,
                } => {
                    file_identity
                        .validate()
                        .map_err(|_| Error::EvidenceChanged)?;
                    if !valid_id(legacy_outer_operation_id)
                        || legacy_outer_operation_id == self.operation_id()
                        || self
                            .data_operation_id()
                            .is_some_and(|id| !valid_id(id) || id == self.operation_id())
                        || file_identity.sha256 != *sha256
                        || file_identity.owner_id != self.data_root().owner_id
                        || file_identity.device_id != self.data_root().device_id
                    {
                        return Err(Error::EvidenceChanged);
                    }
                    sha256
                }
            },
        };
        if !valid_id(self.operation_id())
            || (self.is_legacy() && self.data_operation_id() != Some(self.operation_id()))
            || !record::valid_hash(digest)
            || self.data_root().inode == 0
            || self.data_root().mode != 0o700
        {
            return Err(Error::EvidenceChanged);
        }
        let release = self.installed_release();
        crate::ProductRelease::new(release.product_version(), release.build_number())
            .map_err(|_| Error::EvidenceChanged)?;
        Ok(())
    }
}
