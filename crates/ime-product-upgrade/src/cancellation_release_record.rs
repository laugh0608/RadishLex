//! Immutable cancel_ready proof. The original request and archive stay unchanged.
use super::*;
use serde::{Deserialize, Serialize};

/// Read-only classification of the already validated predecessor locator.
/// This is not a new persisted state or an authorization to release anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationPredecessorKind {
    NoIndex,
    TerminalV1,
    TerminalV2,
    CancellationV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationReleaseReceipt {
    format: String,
    pub(in super::super) archive: CancellationArchiveReceipt,
    pub(in super::super) archive_identity: PreparationFileIdentity,
    pub(in super::super) data_receipt: Option<UpgradeReceipt>,
    pub(in super::super) previous_index: Option<PreviousIndex>,
}

/// Validated immutable value and canonical bytes for one verification call.
/// This contains no filesystem observations and must never waive fresh reads.
/// Private fields prevent pairing an unchecked value with unrelated bytes.
pub(in super::super) struct EncodedCancellationProof<'a> {
    receipt: std::borrow::Cow<'a, CancellationReleaseReceipt>,
    bytes: Vec<u8>,
}

impl EncodedCancellationProof<'_> {
    pub(in super::super) fn receipt(&self) -> &CancellationReleaseReceipt {
        &self.receipt
    }

    pub(in super::super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(in super::super) fn into_receipt(self) -> CancellationReleaseReceipt {
        self.receipt.into_owned()
    }
}

impl EncodedCancellationProof<'static> {
    pub(in super::super) fn decode(bytes: Vec<u8>) -> Result<Self> {
        // The public strict decoder checks canonical bytes and the full model.
        // Keep both results together instead of revalidating immediately after
        // this read. Subsequent reads still use the strict decoder again.
        let receipt = CancellationReleaseReceipt::decode(&bytes)?;
        Ok(Self {
            receipt: std::borrow::Cow::Owned(receipt),
            bytes,
        })
    }
}

impl CancellationReleaseReceipt {
    pub fn operation_id(&self) -> &str {
        &self.preparation().binding().operation_id
    }
    pub fn legacy_outer_operation_id(&self) -> &str {
        // validate() requires this predecessor; do not use the cancellation ID.
        self.preparation()
            .binding()
            .previous_install_operation_id
            .as_deref()
            .expect("validated cancellation has an outer predecessor")
    }
    pub fn archive(&self) -> &CancellationArchiveReceipt {
        &self.archive
    }
    pub fn preparation(&self) -> &PreparationReceipt {
        self.archive.request.preparation()
    }
    pub fn data_receipt(&self) -> Option<&UpgradeReceipt> {
        self.data_receipt.as_ref()
    }
    pub fn predecessor_kind(&self) -> CancellationPredecessorKind {
        match self.previous_index.as_ref().map(|value| &value.index) {
            None => CancellationPredecessorKind::NoIndex,
            Some(ReleaseIndex::V1 { .. }) => CancellationPredecessorKind::TerminalV1,
            Some(ReleaseIndex::V2 {
                proof: LifecycleProofReference::TerminalRelease { .. },
                ..
            }) => CancellationPredecessorKind::TerminalV2,
            Some(ReleaseIndex::V2 {
                proof: LifecycleProofReference::PreparationCancellation { .. },
                ..
            }) => CancellationPredecessorKind::CancellationV2,
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(self)
    }
    pub(in super::super) fn encoded(&self) -> Result<EncodedCancellationProof<'_>> {
        Ok(EncodedCancellationProof {
            receipt: std::borrow::Cow::Borrowed(self),
            bytes: self.encode()?,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode(bytes)?;
        value.validate()?;
        Ok(value)
    }
    pub(super) fn new(
        archive: CancellationArchiveReceipt,
        archive_identity: PreparationFileIdentity,
        data_receipt: Option<UpgradeReceipt>,
        previous_index: Option<PreviousIndex>,
    ) -> Result<Self> {
        let value = Self {
            format: "radishlex-cancellation-release-v1".into(),
            archive,
            archive_identity,
            data_receipt,
            previous_index,
        };
        value.validate()?;
        Ok(value)
    }
    pub(in super::super) fn validate(&self) -> Result<()> {
        // encode() already validates the complete archive. Do not repeat its
        // nested request/source/preparation validation solely to get a length.
        let archive_bytes = self.archive.encode()?;
        let prep = self.preparation();
        let binding = prep.binding();
        self.archive_identity
            .validate()
            .map_err(|_| Error::EvidenceChanged)?;
        if self.format != "radishlex-cancellation-release-v1"
            || !self.archive.preserved
            || !valid_id(self.legacy_outer_operation_id())
            || self.legacy_outer_operation_id() == self.operation_id()
            || self.archive_identity.device_id != binding.data_root.device_id
            || self.archive_identity.owner_id != binding.data_root.owner_id
            || self.archive_identity.byte_len != archive_bytes.len() as u64
            || self.data_receipt.as_ref().map(UpgradeReceipt::operation_id)
                != binding.previous_data_operation_id.as_deref()
            || self.previous_index.as_ref().map(|v| &v.identity)
                != prep.previous_release_index_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(receipt) = &self.data_receipt {
            let receipt_bytes = receipt.encode().map_err(|_| Error::EvidenceChanged)?;
            let installed = if receipt.state() == UpgradeState::Completed {
                receipt.target_release()
            } else {
                receipt.source_release()
            };
            if !receipt.state().is_terminal()
                || receipt.manual_recovery_required()
                || receipt.operation_id() == self.operation_id()
                || installed != &binding.source_release
                || self.archive.previous_files[0].as_ref().map(|v| v.byte_len)
                    != Some(receipt_bytes.len() as u64)
                || self.archive.previous_files[0].as_ref().map(|v| &v.sha256)
                    != binding.previous_data_receipt_sha256.as_ref()
            {
                return Err(Error::EvidenceChanged);
            }
            for (i, slot) in SLOTS.into_iter().enumerate().skip(1) {
                match (
                    &self.archive.previous_files[i],
                    expected_private(receipt, slot),
                ) {
                    (Some(file), Some(artifact)) if matches_artifact(file, artifact) => {}
                    (None, None) => {}
                    _ => return Err(Error::EvidenceChanged),
                }
            }
        } else if self.archive.previous_files.iter().any(Option::is_some)
            || self.archive.previous_directories.is_some()
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(previous) = &self.previous_index {
            previous.index.validate()?;
            previous
                .identity
                .validate()
                .map_err(|_| Error::EvidenceChanged)?;
            if previous.index.operation_id() == self.operation_id()
                || previous.index.data_root() != &binding.data_root
                || previous.index.data_operation_id()
                    != binding.previous_data_operation_id.as_deref()
                || previous.index.installed_release() != &binding.source_release
                || previous.identity.device_id != binding.data_root.device_id
                || previous.identity.owner_id != binding.data_root.owner_id
            {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
}
