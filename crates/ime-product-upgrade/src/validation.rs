use radishlex_ime_userdb::UserDb;

use crate::{UpgradeFailureCode, UpgradeReceipt, UpgradeState};

use super::*;

pub const UPGRADE_VALIDATION_EVIDENCE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeManagerValidationEvidence {
    version: u32,
    schema_version: i64,
    management_queries_checked: u32,
    settings_checked: u32,
}

impl UpgradeManagerValidationEvidence {
    pub const fn new(
        version: u32,
        schema_version: i64,
        management_queries_checked: u32,
        settings_checked: u32,
    ) -> Self {
        Self {
            version,
            schema_version,
            management_queries_checked,
            settings_checked,
        }
    }

    const fn is_valid_for(self, target_schema_version: i64) -> bool {
        self.version == UPGRADE_VALIDATION_EVIDENCE_VERSION
            && self.schema_version == target_schema_version
            && self.management_queries_checked == 1
            && self.settings_checked == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeInputMethodValidationEvidence {
    version: u32,
    schema_version: i64,
    personalized_runtime_checked: u32,
    candidate_signals_read: u32,
}

impl UpgradeInputMethodValidationEvidence {
    pub const fn new(
        version: u32,
        schema_version: i64,
        personalized_runtime_checked: u32,
        candidate_signals_read: u32,
    ) -> Self {
        Self {
            version,
            schema_version,
            personalized_runtime_checked,
            candidate_signals_read,
        }
    }

    const fn is_valid_for(self, target_schema_version: i64) -> bool {
        self.version == UPGRADE_VALIDATION_EVIDENCE_VERSION
            && self.schema_version == target_schema_version
            && self.personalized_runtime_checked == 1
            && self.candidate_signals_read == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCandidateValidationReport {
    Passed {
        manager: UpgradeManagerValidationEvidence,
        input_method: UpgradeInputMethodValidationEvidence,
    },
    ManagerFailed,
    InputMethodFailed {
        manager: UpgradeManagerValidationEvidence,
    },
}

impl UpgradeCandidateValidationReport {
    pub const fn passed(
        manager: UpgradeManagerValidationEvidence,
        input_method: UpgradeInputMethodValidationEvidence,
    ) -> Self {
        Self::Passed {
            manager,
            input_method,
        }
    }

    pub const fn manager_failed() -> Self {
        Self::ManagerFailed
    }

    pub const fn input_method_failed(manager: UpgradeManagerValidationEvidence) -> Self {
        Self::InputMethodFailed { manager }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCandidateValidationDisposition {
    CandidateVerified,
    AbortedPreserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCandidateValidationSummary {
    disposition: UpgradeCandidateValidationDisposition,
    failure_code: Option<UpgradeFailureCode>,
}

impl UpgradeCandidateValidationSummary {
    pub const fn disposition(self) -> UpgradeCandidateValidationDisposition {
        self.disposition
    }

    pub const fn failure_code(self) -> Option<UpgradeFailureCode> {
        self.failure_code
    }
}

impl UpgradeReceiptStore {
    pub fn record_candidate_validation(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        report: UpgradeCandidateValidationReport,
    ) -> Result<UpgradeCandidateValidationSummary, UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if receipt.state() != UpgradeState::CandidateMigrated {
            return Err(error(
                UpgradeFilesystemErrorCode::InvalidCandidateValidation,
            ));
        }
        let current_receipt = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::InvalidCandidateValidation))?;
        if current_receipt != *receipt {
            return Err(error(
                UpgradeFilesystemErrorCode::InvalidCandidateValidation,
            ));
        }
        candidate::validate_candidate_state(self, Some(receipt))?;

        let failure_code = validation_failure(report, receipt.target_schema_version());
        if failure_code.is_none() && !candidate_is_still_current(self, receipt)? {
            return self.persist_validation_result(
                guard,
                receipt,
                Some(UpgradeFailureCode::ManagerValidationFailed),
            );
        }
        self.persist_validation_result(guard, receipt, failure_code)
    }

    fn persist_validation_result(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &mut UpgradeReceipt,
        failure_code: Option<UpgradeFailureCode>,
    ) -> Result<UpgradeCandidateValidationSummary, UpgradeFilesystemError> {
        let mut next_receipt = receipt.clone();
        let disposition = if let Some(failure_code) = failure_code {
            next_receipt
                .abort_preserved(failure_code, false)
                .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidCandidateValidation))?;
            UpgradeCandidateValidationDisposition::AbortedPreserved
        } else {
            next_receipt
                .advance(UpgradeState::CandidateVerified)
                .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidCandidateValidation))?;
            UpgradeCandidateValidationDisposition::CandidateVerified
        };
        self.persist(guard, &next_receipt)?;
        *receipt = next_receipt;
        Ok(UpgradeCandidateValidationSummary {
            disposition,
            failure_code,
        })
    }
}

const fn validation_failure(
    report: UpgradeCandidateValidationReport,
    target_schema_version: i64,
) -> Option<UpgradeFailureCode> {
    match report {
        UpgradeCandidateValidationReport::ManagerFailed => {
            Some(UpgradeFailureCode::ManagerValidationFailed)
        }
        UpgradeCandidateValidationReport::InputMethodFailed { manager } => {
            if manager.is_valid_for(target_schema_version) {
                Some(UpgradeFailureCode::InputMethodValidationFailed)
            } else {
                Some(UpgradeFailureCode::ManagerValidationFailed)
            }
        }
        UpgradeCandidateValidationReport::Passed {
            manager,
            input_method,
        } => {
            if !manager.is_valid_for(target_schema_version) {
                Some(UpgradeFailureCode::ManagerValidationFailed)
            } else if !input_method.is_valid_for(target_schema_version) {
                Some(UpgradeFailureCode::InputMethodValidationFailed)
            } else {
                None
            }
        }
    }
}

fn candidate_is_still_current(
    store: &UpgradeReceiptStore,
    receipt: &UpgradeReceipt,
) -> Result<bool, UpgradeFilesystemError> {
    let candidate_path = store.state_directory.join(CANDIDATE_FILE_NAME);
    let database = match UserDb::open_read_only_current(&candidate_path) {
        Ok(database) => database,
        Err(_) => return Ok(false),
    };
    let schema_matches = database
        .schema_version()
        .is_ok_and(|version| version == receipt.target_schema_version());
    drop(database);
    if !schema_matches {
        return Ok(false);
    }
    candidate::validate_candidate_state(store, Some(receipt))?;
    Ok(true)
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
