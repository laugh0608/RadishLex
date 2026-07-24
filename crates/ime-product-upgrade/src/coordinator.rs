use std::fmt;

use crate::ProductRelease;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCoordinatorCheckpoint {
    BeforeQuiesced,
    BeforeSettingsBackup,
    BeforeSnapshot,
    BeforeCandidateMigration,
    BeforeCandidateValidation,
    AfterCandidateValidation,
    BeforeSwitch,
    BeforePostSwitchValidation,
    AfterPostSwitchValidation,
    BeforeCompletion,
    BeforeRollbackRestore,
    BeforeSourceValidation,
    AfterSourceValidation,
}

pub trait UpgradeCoordinatorPort {
    fn confirm_quiescence(&mut self, checkpoint: UpgradeCoordinatorCheckpoint) -> bool;

    fn validate_candidate(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport;

    fn validate_post_switch(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport;

    fn validate_restored_source(
        &mut self,
        source_release: &ProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCoordinatorDisposition {
    Completed,
    AbortedPreserved,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCoordinatorSummary {
    disposition: UpgradeCoordinatorDisposition,
}

impl UpgradeCoordinatorSummary {
    pub const fn disposition(self) -> UpgradeCoordinatorDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCoordinatorError {
    Filesystem(UpgradeFilesystemErrorCode),
    QuiescenceNotProven(UpgradeCoordinatorCheckpoint),
    SourceValidationNotProven,
}

impl fmt::Display for UpgradeCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem(_) => formatter.write_str("upgrade coordination filesystem failure"),
            Self::QuiescenceNotProven(_) => {
                formatter.write_str("upgrade coordination quiescence was not proven")
            }
            Self::SourceValidationNotProven => {
                formatter.write_str("upgrade rollback source validation was not proven")
            }
        }
    }
}

impl std::error::Error for UpgradeCoordinatorError {}

impl From<UpgradeFilesystemError> for UpgradeCoordinatorError {
    fn from(error: UpgradeFilesystemError) -> Self {
        Self::Filesystem(error.code())
    }
}

impl UpgradeReceiptStore {
    pub fn resume_userdb_upgrade<P: UpgradeCoordinatorPort>(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        reported_available_bytes: u64,
        port: &mut P,
    ) -> Result<UpgradeCoordinatorSummary, UpgradeCoordinatorError> {
        self.validate_coordinator_entry(guard, receipt)?;
        loop {
            match receipt.state() {
                UpgradeState::Preflighted => {
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeQuiesced)?;
                    let mut next_receipt = receipt.clone();
                    next_receipt.advance(UpgradeState::Quiesced).map_err(|_| {
                        UpgradeCoordinatorError::Filesystem(
                            UpgradeFilesystemErrorCode::InvalidReceiptReplacement,
                        )
                    })?;
                    self.persist(guard, &next_receipt)?;
                    *receipt = next_receipt;
                }
                UpgradeState::Quiesced => {
                    let source_settings_bytes = source_settings_bytes(receipt);
                    if source_settings_bytes.is_some() {
                        require_quiescence(
                            port,
                            UpgradeCoordinatorCheckpoint::BeforeSettingsBackup,
                        )?;
                        self.create_settings_backup(guard, receipt)?;
                    }
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeSnapshot)?;
                    let snapshot_available = reported_available_bytes
                        .checked_sub(source_settings_bytes.unwrap_or(0))
                        .ok_or(UpgradeCoordinatorError::Filesystem(
                            UpgradeFilesystemErrorCode::InsufficientSpace,
                        ))?;
                    self.create_userdb_snapshot(guard, receipt, snapshot_available)?;
                }
                UpgradeState::SnapshotReady => {
                    require_quiescence(
                        port,
                        UpgradeCoordinatorCheckpoint::BeforeCandidateMigration,
                    )?;
                    self.create_userdb_candidate(guard, receipt)?;
                }
                UpgradeState::CandidateMigrated => {
                    require_quiescence(
                        port,
                        UpgradeCoordinatorCheckpoint::BeforeCandidateValidation,
                    )?;
                    let report = port.validate_candidate(
                        receipt.target_release(),
                        receipt.target_schema_version(),
                    );
                    require_quiescence(
                        port,
                        UpgradeCoordinatorCheckpoint::AfterCandidateValidation,
                    )?;
                    self.record_candidate_validation(guard, receipt, report)?;
                }
                UpgradeState::CandidateVerified | UpgradeState::SwitchPrepared => {
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeSwitch)?;
                    self.switch_userdb_candidate(guard, receipt)?;
                }
                UpgradeState::Switched => {
                    require_quiescence(
                        port,
                        UpgradeCoordinatorCheckpoint::BeforePostSwitchValidation,
                    )?;
                    let report = port.validate_post_switch(
                        receipt.target_release(),
                        receipt.target_schema_version(),
                    );
                    require_quiescence(
                        port,
                        UpgradeCoordinatorCheckpoint::AfterPostSwitchValidation,
                    )?;
                    self.record_post_switch_validation(guard, receipt, report)?;
                }
                UpgradeState::PostSwitchVerified => {
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeCompletion)?;
                    self.complete_post_switch_validation(guard, receipt)?;
                }
                UpgradeState::RollbackRequired => {
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeRollbackRestore)?;
                    self.restore_userdb_backup(guard, receipt)?;
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::BeforeSourceValidation)?;
                    let source_schema_version = receipt.source_schema_version().ok_or(
                        UpgradeCoordinatorError::Filesystem(
                            UpgradeFilesystemErrorCode::InvalidSwitchState,
                        ),
                    )?;
                    let evidence = port
                        .validate_restored_source(receipt.source_release(), source_schema_version)
                        .ok_or(UpgradeCoordinatorError::SourceValidationNotProven)?;
                    require_quiescence(port, UpgradeCoordinatorCheckpoint::AfterSourceValidation)?;
                    self.record_rollback_validation(guard, receipt, evidence)?;
                }
                UpgradeState::Completed => {
                    return Ok(UpgradeCoordinatorSummary {
                        disposition: UpgradeCoordinatorDisposition::Completed,
                    });
                }
                UpgradeState::AbortedPreserved => {
                    return Ok(UpgradeCoordinatorSummary {
                        disposition: UpgradeCoordinatorDisposition::AbortedPreserved,
                    });
                }
                UpgradeState::RolledBack => {
                    return Ok(UpgradeCoordinatorSummary {
                        disposition: UpgradeCoordinatorDisposition::RolledBack,
                    });
                }
            }
        }
    }

    fn validate_coordinator_entry(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &UpgradeReceipt,
    ) -> Result<(), UpgradeCoordinatorError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(UpgradeCoordinatorError::Filesystem(
                UpgradeFilesystemErrorCode::IdentityChanged,
            ));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        let stored = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or(UpgradeCoordinatorError::Filesystem(
                UpgradeFilesystemErrorCode::InvalidReceipt,
            ))?;
        if stored != *receipt {
            return Err(UpgradeCoordinatorError::Filesystem(
                UpgradeFilesystemErrorCode::InvalidReceiptReplacement,
            ));
        }
        Ok(())
    }
}

fn require_quiescence<P: UpgradeCoordinatorPort>(
    port: &mut P,
    checkpoint: UpgradeCoordinatorCheckpoint,
) -> Result<(), UpgradeCoordinatorError> {
    if port.confirm_quiescence(checkpoint) {
        Ok(())
    } else {
        Err(UpgradeCoordinatorError::QuiescenceNotProven(checkpoint))
    }
}

fn source_settings_bytes(receipt: &UpgradeReceipt) -> Option<u64> {
    receipt
        .artifacts()
        .iter()
        .find(|artifact| artifact.slot() == UpgradeArtifactSlot::SourceSettings)
        .map(UpgradeArtifactIdentity::byte_len)
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;
