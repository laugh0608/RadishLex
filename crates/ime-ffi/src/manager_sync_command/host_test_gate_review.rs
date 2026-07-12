use crate::error::FfiError;

use super::migration_review::ManagerSyncCommandManagerBridgeMigrationReviewDraft;
use super::{ensure_safe_summary_value, HOST_CONTRACT_TEST_GATE_CLOSED};

pub(crate) const HOST_TEST_FILE_APPROVAL_REVIEW_READY: &str =
    "host_contract_test_file_approval_review_ready_no_native_symbol";
pub(crate) const HOST_TEST_FILE_APPROVAL_BLOCKED: &str =
    "host_contract_test_file_blocked_until_symbol_and_binding_approval";
pub(crate) const REAL_SYNC_EXECUTION_GATE_REVIEW_READY: &str =
    "real_sync_execution_gate_review_ready_no_remote_sync";
pub(crate) const REAL_SYNC_EXECUTION_BLOCKED: &str =
    "real_sync_execution_blocked_until_platform_and_deployment_evidence";
pub(crate) const REAL_SYNC_EVIDENCE_BUNDLE_REVIEW_READY: &str =
    "real_sync_execution_evidence_bundle_review_ready_no_remote_sync";
pub(crate) const REAL_SYNC_EVIDENCE_BUNDLE_BLOCKED: &str =
    "real_sync_execution_evidence_bundle_blocked_until_release_evidence";

const HOST_TEST_TARGET_FILE: &str = "crates/ime-ffi/tests/manager_sync_command_boundary.rs";

const HOST_TEST_PLANNED_CASES: &[&str] = &[
    "contract_reports_command_capability_closed",
    "invalid_request_inputs_return_stable_status",
    "result_handle_copy_then_free",
    "envelope_allowlist_only",
    "forbidden_material_absent_from_native_outputs",
    "panic_boundary_returns_internal_error",
    "sync_domain_command_serialization",
];

const HOST_TEST_FILE_REVIEWED_CONDITIONS: &[&str] = &[
    "rust_host_contract_cases_mapped",
    "c_abi_symbol_lookup_strategy_reviewed",
    "capability_missing_behavior_reviewed",
    "result_handle_lifecycle_reviewed",
    "error_handle_lifecycle_reviewed",
    "panic_boundary_reviewed",
    "forbidden_material_assertions_reviewed",
    "dynamic_library_smoke_absence_reviewed",
];

const HOST_TEST_FILE_BLOCKING_CONDITIONS: &[&str] = &[
    "native_symbol_export_approved_by_adr",
    "dart_native_binding_approved",
    "manager_bridge_command_approved",
    "host_contract_test_file_approved",
];

const HOST_TEST_FILE_REQUIRED_EVIDENCE: &[&str] = &[
    "host_contract_catalog_current",
    "host_test_design_package_current",
    "c_abi_contract_review_matrix_current",
    "manager_sync_command_internal_draft_tests",
    "ffi_bridge_smoke_candidate_symbols_absent",
];

const REAL_SYNC_REVIEWED_CONDITIONS: &[&str] = &[
    "command_context_owner_scope_reviewed",
    "command_worker_thread_policy_reviewed",
    "sync_domain_serialization_reviewed",
    "operation_id_idempotency_reviewed",
    "readiness_snapshot_binding_reviewed",
    "deployment_evidence_summary_reviewed",
    "forbidden_material_redaction_reviewed",
];

const REAL_SYNC_BLOCKING_CONDITIONS: &[&str] = &[
    "host_contract_test_file_approved",
    "native_symbol_export_approved_by_adr",
    "dart_native_binding_approved",
    "manager_bridge_command_approved",
    "platform_private_key_backend_production_ready",
    "recovery_authorization_interaction_tests_passed",
    "deployment_evidence_summary_approved",
    "real_sync_execution_approved_after_gate",
];

const REAL_SYNC_REQUIRED_EVIDENCE: &[&str] = &[
    "platform_private_key_backend_strategy_current",
    "manager_recovery_device_auth_flow_current",
    "sync_server_production_deployment_runbook_current",
    "manager_sync_entry_boundary_current",
    "local_docker_https_smoke_development_only",
];

const REAL_SYNC_EVIDENCE_BUNDLE_ITEMS: &[ManagerSyncCommandRealSyncEvidenceItemDraft] = &[
    ManagerSyncCommandRealSyncEvidenceItemDraft {
        evidence_id: "platform_private_key_backend",
        evidence_domain: "platform_signing_backend",
        current_state: "production_backend_not_ready_current_phase",
        blocking_condition: "platform_private_key_backend_production_ready",
        required_source: "platform_private_key_backend_strategy",
        safe_summary: "backend_status_codes_only_no_key_material",
        can_unlock_real_sync: false,
    },
    ManagerSyncCommandRealSyncEvidenceItemDraft {
        evidence_id: "recovery_authorization_interaction",
        evidence_domain: "recovery_and_device_authorization",
        current_state: "interaction_tests_not_passed_current_phase",
        blocking_condition: "recovery_authorization_interaction_tests_passed",
        required_source: "manager_recovery_device_auth_flow",
        safe_summary: "confirmation_and_transient_lifecycle_codes_only",
        can_unlock_real_sync: false,
    },
    ManagerSyncCommandRealSyncEvidenceItemDraft {
        evidence_id: "deployment_evidence_summary",
        evidence_domain: "release_deployment_evidence",
        current_state: "release_deployment_evidence_missing_current_phase",
        blocking_condition: "deployment_evidence_summary_approved",
        required_source: "sync_server_production_deployment_runbook",
        safe_summary: "deployment_evidence_summary_v1_required_local_smoke_only",
        can_unlock_real_sync: false,
    },
];

const REAL_SYNC_EVIDENCE_BUNDLE_REVIEWED_CONDITIONS: &[&str] = &[
    "platform_private_key_backend_summary_mapped",
    "recovery_authorization_interaction_summary_mapped",
    "deployment_evidence_summary_source_mapped",
    "local_smoke_kept_development_only",
    "forbidden_material_redaction_reviewed",
    "current_phase_real_sync_unlock_blocked",
];

const REAL_SYNC_EVIDENCE_BUNDLE_BLOCKING_CONDITIONS: &[&str] = &[
    "platform_private_key_backend_production_ready",
    "recovery_authorization_interaction_tests_passed",
    "deployment_evidence_summary_approved",
    "release_evidence_bundle_approved_after_gate",
    "real_sync_execution_approved_after_gate",
];

const REAL_SYNC_EVIDENCE_BUNDLE_REQUIRED_EVIDENCE: &[&str] = &[
    "platform_private_key_backend_strategy_current",
    "manager_recovery_device_auth_flow_current",
    "sync_server_production_deployment_runbook_current",
    "deployment_evidence_summary_v1_release_target",
    "manager_sync_entry_gate_visible_layer_tests",
    "real_sync_execution_evidence_bundle_replay",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostTestFileApprovalReviewDraft {
    pub review_state: &'static str,
    pub approval_decision: &'static str,
    pub target_test_file: &'static str,
    pub planned_test_cases: &'static [&'static str],
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
    pub can_create_host_contract_test_file: bool,
    pub can_export_native_symbol: bool,
    pub can_call_dynamic_library_symbol: bool,
    pub can_modify_dart_native_binding: bool,
}

impl ManagerSyncCommandHostTestFileApprovalReviewDraft {
    pub(crate) fn current_phase(
        bridge_review: &ManagerSyncCommandManagerBridgeMigrationReviewDraft,
    ) -> Result<Self, FfiError> {
        bridge_review.assert_safe_bridge_migration()?;
        if !bridge_review
            .blocking_conditions
            .contains(&"host_contract_test_file_approved")
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(Self {
            review_state: HOST_TEST_FILE_APPROVAL_REVIEW_READY,
            approval_decision: HOST_TEST_FILE_APPROVAL_BLOCKED,
            target_test_file: HOST_TEST_TARGET_FILE,
            planned_test_cases: HOST_TEST_PLANNED_CASES,
            reviewed_conditions: HOST_TEST_FILE_REVIEWED_CONDITIONS,
            blocking_conditions: HOST_TEST_FILE_BLOCKING_CONDITIONS,
            required_evidence: HOST_TEST_FILE_REQUIRED_EVIDENCE,
            can_create_host_contract_test_file: false,
            can_export_native_symbol: false,
            can_call_dynamic_library_symbol: false,
            can_modify_dart_native_binding: false,
        })
    }

    pub(crate) fn assert_safe_file_approval(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "host_test_file_approval_review_state")?;
        ensure_safe_summary_value(self.approval_decision, "host_test_file_approval_decision")?;
        ensure_safe_summary_value(self.target_test_file, "host_test_file_approval_target")?;
        ensure_safe_values(self.planned_test_cases, "host_test_file_planned_case")?;
        ensure_safe_values(
            self.reviewed_conditions,
            "host_test_file_reviewed_condition",
        )?;
        ensure_safe_values(
            self.blocking_conditions,
            "host_test_file_blocking_condition",
        )?;
        ensure_safe_values(self.required_evidence, "host_test_file_required_evidence")?;
        if self.can_create_host_contract_test_file
            || self.can_export_native_symbol
            || self.can_call_dynamic_library_symbol
            || self.can_modify_dart_native_binding
            || self.planned_test_cases.is_empty()
            || self.blocking_conditions.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandRealSyncExecutionGateReviewDraft {
    pub review_state: &'static str,
    pub execution_decision: &'static str,
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
    pub can_execute_real_sync: bool,
    pub can_connect_go_server: bool,
    pub can_touch_platform_key_backend: bool,
    pub can_generate_recovery_code: bool,
    pub can_create_join_request: bool,
    pub can_revoke_device: bool,
}

impl ManagerSyncCommandRealSyncExecutionGateReviewDraft {
    pub(crate) fn current_phase(
        host_test_file: &ManagerSyncCommandHostTestFileApprovalReviewDraft,
    ) -> Result<Self, FfiError> {
        host_test_file.assert_safe_file_approval()?;
        Ok(Self {
            review_state: REAL_SYNC_EXECUTION_GATE_REVIEW_READY,
            execution_decision: REAL_SYNC_EXECUTION_BLOCKED,
            reviewed_conditions: REAL_SYNC_REVIEWED_CONDITIONS,
            blocking_conditions: REAL_SYNC_BLOCKING_CONDITIONS,
            required_evidence: REAL_SYNC_REQUIRED_EVIDENCE,
            can_execute_real_sync: false,
            can_connect_go_server: false,
            can_touch_platform_key_backend: false,
            can_generate_recovery_code: false,
            can_create_join_request: false,
            can_revoke_device: false,
        })
    }

    pub(crate) fn assert_safe_execution_gate(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "real_sync_execution_gate_review_state")?;
        ensure_safe_summary_value(self.execution_decision, "real_sync_execution_decision")?;
        ensure_safe_values(
            self.reviewed_conditions,
            "real_sync_execution_reviewed_condition",
        )?;
        ensure_safe_values(
            self.blocking_conditions,
            "real_sync_execution_blocking_condition",
        )?;
        ensure_safe_values(
            self.required_evidence,
            "real_sync_execution_required_evidence",
        )?;
        if self.can_execute_real_sync
            || self.can_connect_go_server
            || self.can_touch_platform_key_backend
            || self.can_generate_recovery_code
            || self.can_create_join_request
            || self.can_revoke_device
            || self.blocking_conditions.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandRealSyncEvidenceItemDraft {
    pub evidence_id: &'static str,
    pub evidence_domain: &'static str,
    pub current_state: &'static str,
    pub blocking_condition: &'static str,
    pub required_source: &'static str,
    pub safe_summary: &'static str,
    pub can_unlock_real_sync: bool,
}

impl ManagerSyncCommandRealSyncEvidenceItemDraft {
    fn assert_safe_evidence_item(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.evidence_id, "real_sync_evidence_item_id")?;
        ensure_safe_summary_value(self.evidence_domain, "real_sync_evidence_domain")?;
        ensure_safe_summary_value(self.current_state, "real_sync_evidence_current_state")?;
        ensure_safe_summary_value(
            self.blocking_condition,
            "real_sync_evidence_blocking_condition",
        )?;
        ensure_safe_summary_value(self.required_source, "real_sync_evidence_required_source")?;
        ensure_safe_summary_value(self.safe_summary, "real_sync_evidence_safe_summary")?;
        if self.can_unlock_real_sync {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandRealSyncExecutionEvidenceBundleReviewDraft {
    pub review_state: &'static str,
    pub bundle_decision: &'static str,
    pub evidence_items: &'static [ManagerSyncCommandRealSyncEvidenceItemDraft],
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
    pub can_mark_platform_backend_ready: bool,
    pub can_mark_recovery_authorization_ready: bool,
    pub can_mark_deployment_summary_ready: bool,
    pub can_unlock_real_sync: bool,
    pub can_execute_remote_call: bool,
    pub can_persist_secret_material: bool,
}

impl ManagerSyncCommandRealSyncExecutionEvidenceBundleReviewDraft {
    pub(crate) fn current_phase(
        execution_gate: &ManagerSyncCommandRealSyncExecutionGateReviewDraft,
    ) -> Result<Self, FfiError> {
        execution_gate.assert_safe_execution_gate()?;
        for condition in [
            "platform_private_key_backend_production_ready",
            "recovery_authorization_interaction_tests_passed",
            "deployment_evidence_summary_approved",
        ] {
            if !execution_gate.blocking_conditions.contains(&condition) {
                return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
            }
        }
        Ok(Self {
            review_state: REAL_SYNC_EVIDENCE_BUNDLE_REVIEW_READY,
            bundle_decision: REAL_SYNC_EVIDENCE_BUNDLE_BLOCKED,
            evidence_items: REAL_SYNC_EVIDENCE_BUNDLE_ITEMS,
            reviewed_conditions: REAL_SYNC_EVIDENCE_BUNDLE_REVIEWED_CONDITIONS,
            blocking_conditions: REAL_SYNC_EVIDENCE_BUNDLE_BLOCKING_CONDITIONS,
            required_evidence: REAL_SYNC_EVIDENCE_BUNDLE_REQUIRED_EVIDENCE,
            can_mark_platform_backend_ready: false,
            can_mark_recovery_authorization_ready: false,
            can_mark_deployment_summary_ready: false,
            can_unlock_real_sync: false,
            can_execute_remote_call: false,
            can_persist_secret_material: false,
        })
    }

    pub(crate) fn assert_safe_evidence_bundle(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "real_sync_evidence_bundle_review_state")?;
        ensure_safe_summary_value(self.bundle_decision, "real_sync_evidence_bundle_decision")?;
        if self.evidence_items.is_empty() {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        for item in self.evidence_items {
            item.assert_safe_evidence_item()?;
        }
        ensure_safe_values(
            self.reviewed_conditions,
            "real_sync_evidence_bundle_reviewed_condition",
        )?;
        ensure_safe_values(
            self.blocking_conditions,
            "real_sync_evidence_bundle_blocking_condition",
        )?;
        ensure_safe_values(
            self.required_evidence,
            "real_sync_evidence_bundle_required_evidence",
        )?;
        if self.can_mark_platform_backend_ready
            || self.can_mark_recovery_authorization_ready
            || self.can_mark_deployment_summary_ready
            || self.can_unlock_real_sync
            || self.can_execute_remote_call
            || self.can_persist_secret_material
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

fn ensure_safe_values(values: &[&'static str], field: &'static str) -> Result<(), FfiError> {
    if values.is_empty() {
        return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
    }
    for value in values {
        ensure_safe_summary_value(value, field)?;
    }
    Ok(())
}
