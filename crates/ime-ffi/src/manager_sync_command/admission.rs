use crate::error::FfiError;

use super::{
    ensure_safe_summary_value, ManagerSyncCommandHostGateReadinessReviewDraft,
    HOST_CONTRACT_TEST_GATE_CLOSED, HOST_TEST_ADMISSION_BLOCKED, HOST_TEST_ADMISSION_NEXT_DECISION,
    HOST_TEST_ADMISSION_REVIEW_READY, MANAGER_SYNC_COMMAND_HOST_GATE_BLOCKING_CONDITIONS_DRAFT,
    MANAGER_SYNC_COMMAND_HOST_GATE_REVIEWED_CONDITIONS_DRAFT,
    MANAGER_SYNC_COMMAND_HOST_TEST_EVIDENCE_DRAFT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostTestAdmissionReviewDraft {
    pub review_state: &'static str,
    pub admission_decision: &'static str,
    pub ready_for_host_contract_test: bool,
    pub satisfied_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub evidence_sources: &'static [&'static str],
    pub next_decision_required: &'static str,
    pub can_create_host_contract_test_file: bool,
    pub can_export_native_symbol: bool,
    pub can_modify_manager_bridge: bool,
    pub can_execute_real_sync: bool,
}

impl ManagerSyncCommandHostTestAdmissionReviewDraft {
    pub(crate) fn current_phase(
        readiness: &ManagerSyncCommandHostGateReadinessReviewDraft,
    ) -> Result<Self, FfiError> {
        readiness.assert_safe_readiness()?;
        Ok(Self {
            review_state: HOST_TEST_ADMISSION_REVIEW_READY,
            admission_decision: HOST_TEST_ADMISSION_BLOCKED,
            ready_for_host_contract_test: false,
            satisfied_conditions: MANAGER_SYNC_COMMAND_HOST_GATE_REVIEWED_CONDITIONS_DRAFT,
            blocking_conditions: MANAGER_SYNC_COMMAND_HOST_GATE_BLOCKING_CONDITIONS_DRAFT,
            evidence_sources: MANAGER_SYNC_COMMAND_HOST_TEST_EVIDENCE_DRAFT,
            next_decision_required: HOST_TEST_ADMISSION_NEXT_DECISION,
            can_create_host_contract_test_file: false,
            can_export_native_symbol: false,
            can_modify_manager_bridge: false,
            can_execute_real_sync: false,
        })
    }

    pub(crate) fn assert_safe_admission(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "host_test_admission_review_state")?;
        ensure_safe_summary_value(self.admission_decision, "host_test_admission_decision")?;
        ensure_safe_summary_value(
            self.next_decision_required,
            "host_test_admission_next_decision",
        )?;
        for condition in self.satisfied_conditions {
            ensure_safe_summary_value(condition, "host_test_admission_satisfied_condition")?;
            if self.blocking_conditions.contains(condition) {
                return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
            }
        }
        for condition in self.blocking_conditions {
            ensure_safe_summary_value(condition, "host_test_admission_blocking_condition")?;
        }
        for source in self.evidence_sources {
            ensure_safe_summary_value(source, "host_test_admission_evidence_source")?;
        }
        if self.ready_for_host_contract_test
            || self.can_create_host_contract_test_file
            || self.can_export_native_symbol
            || self.can_modify_manager_bridge
            || self.can_execute_real_sync
            || self.blocking_conditions.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}
