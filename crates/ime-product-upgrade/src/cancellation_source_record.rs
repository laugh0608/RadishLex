//! Source-only progress; it never denotes outer restoration or startup release.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationSourcePhase {
    Finishing,
    SourceReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationSourceReceipt {
    format: String,
    phase: CancellationSourcePhase,
    request_identity: PreparationFileIdentity,
    preparation: PreparationReceipt,
    source_ready: Option<PreparationFamily>,
}

impl CancellationSourceReceipt {
    pub(super) fn new(
        request: &PreparationCancellationRequest,
        identity: PreparationFileIdentity,
    ) -> Self {
        Self {
            format: "radishlex-cancellation-source-v1".into(),
            phase: CancellationSourcePhase::Finishing,
            request_identity: identity,
            preparation: request.preparation().clone(),
            source_ready: None,
        }
    }
    pub fn phase(&self) -> CancellationSourcePhase {
        self.phase
    }
    pub fn request_identity(&self) -> &PreparationFileIdentity {
        &self.request_identity
    }
    pub fn preparation(&self) -> &PreparationReceipt {
        &self.preparation
    }
    pub fn source_ready(&self) -> Option<&PreparationFamily> {
        self.source_ready.as_ref()
    }

    pub(super) fn with_recovery(&self, family: PreparationFamily) -> Result<Self> {
        let mut next = self.clone();
        next.preparation
            .record_journal_recovery(family)
            .map_err(|_| Error::EvidenceChanged)?;
        Ok(next)
    }
    pub(super) fn ready(&self, family: PreparationFamily) -> Result<Self> {
        let mut next = self.clone();
        if next.preparation.phase() == PreparationPhase::MaintenanceIntent {
            next.preparation
                .record_prepared_source(family.database.clone())
                .map_err(|_| Error::EvidenceChanged)?;
        }
        next.phase = CancellationSourcePhase::SourceReady;
        next.source_ready = Some(family);
        Ok(next)
    }
    pub(super) fn validate_request(&self, request: &PreparationCancellationRequest) -> Result<()> {
        self.encode()?;
        let original = request.preparation();
        let mut expected = original.clone();
        if expected.phase() == PreparationPhase::MaintenanceIntent {
            if expected.journal_recovery().is_none() {
                if let Some(recovery) = self.preparation.journal_recovery() {
                    expected
                        .record_journal_recovery(recovery.clone())
                        .map_err(|_| Error::EvidenceChanged)?;
                }
            }
            if let Some(family) = &self.source_ready {
                expected
                    .record_prepared_source(family.database.clone())
                    .map_err(|_| Error::EvidenceChanged)?;
            }
        }
        if expected != self.preparation
            || self.request_identity.byte_len != request.encode()?.len() as u64
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(family) = &self.source_ready {
            if original.phase() < PreparationPhase::MaintenanceIntent
                && family != request.observed_source()
            {
                return Err(Error::EvidenceChanged);
            }
            record::validate_source(&self.preparation, family)?;
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.preparation
            .encode()
            .map_err(|_| Error::EvidenceChanged)?;
        self.request_identity
            .validate()
            .map_err(|_| Error::EvidenceChanged)?;
        let root = &self.preparation.binding().data_root;
        if self.format != "radishlex-cancellation-source-v1"
            || (self.phase == CancellationSourcePhase::SourceReady) != self.source_ready.is_some()
            || self
                .preparation
                .binding()
                .previous_install_operation_id
                .is_none()
            || self.preparation.handoff_intent().is_some()
            || self.preparation.phase() >= PreparationPhase::HandoffReady
            || self.request_identity.device_id != root.device_id
            || self.request_identity.owner_id != root.owner_id
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(family) = &self.source_ready {
            family.validate(root).map_err(|_| Error::EvidenceChanged)?;
            record::validate_source(&self.preparation, family)?;
        }
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
}
