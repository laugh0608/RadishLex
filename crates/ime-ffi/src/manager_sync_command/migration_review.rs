use crate::error::FfiError;

use super::admission::ManagerSyncCommandHostTestAdmissionReviewDraft;
use super::{
    ensure_safe_summary_value, ManagerSyncCommandCAbiWrapperShapeReviewDraft,
    ManagerSyncCommandResultAccessorFieldSetReviewDraft, C_ABI_SYMBOL_NOT_EXPORTED,
    HOST_CONTRACT_TEST_GATE_CLOSED,
};

pub(crate) const HOST_EXPORT_APPROVAL_REVIEW_READY: &str =
    "native_symbol_export_approval_review_ready_no_native_symbol";
pub(crate) const HOST_EXPORT_APPROVAL_BLOCKED: &str =
    "native_symbol_export_blocked_until_adr_and_smoke_approval";
pub(crate) const DART_BINDING_MIGRATION_REVIEW_READY: &str =
    "dart_binding_migration_review_ready_no_native_symbol";
pub(crate) const DART_BINDING_MIGRATION_BLOCKED: &str =
    "dart_binding_migration_blocked_until_native_export";
pub(crate) const MANAGER_BRIDGE_MIGRATION_REVIEW_READY: &str =
    "manager_bridge_migration_review_ready_no_native_symbol";
pub(crate) const MANAGER_BRIDGE_MIGRATION_BLOCKED: &str =
    "manager_bridge_migration_blocked_until_binding_contract";

const EXPORT_REQUIRED_EVIDENCE: &[&str] = &[
    "adr_0006_current",
    "c_abi_wrapper_shape_review",
    "result_accessor_field_set_review",
    "result_handle_copy_then_free_review",
    "panic_boundary_review",
    "ffi_bridge_smoke_candidate_symbols_absent",
];

const BINDING_REVIEWED_CONDITIONS: &[&str] = &[
    "dart_copy_free_contract_reviewed",
    "unknown_native_status_mapping_reviewed",
    "ffi_command_error_mapping_reviewed",
    "forbidden_material_redaction_reviewed",
];

const BINDING_BLOCKING_CONDITIONS: &[&str] = &[
    "native_symbol_export_approved_by_adr",
    "dart_native_binding_approved",
    "manager_bridge_command_approved",
];

const BINDING_REQUIRED_EVIDENCE: &[&str] = &[
    "fake_native_binding_replay",
    "copy_free_call_order_test",
    "unknown_status_safe_downgrade_test",
    "ffi_error_mapping_allowlist_test",
    "forbidden_material_not_visible_to_manager_test",
];

const BRIDGE_REVIEWED_CONDITIONS: &[&str] = &[
    "action_intent_mapping_reviewed",
    "settings_draft_write_absent_reviewed",
    "diagnostics_redaction_reviewed",
];

const BRIDGE_BLOCKING_CONDITIONS: &[&str] = &[
    "manager_bridge_command_approved",
    "host_contract_test_file_approved",
    "real_sync_execution_approved_after_gate",
];

const BRIDGE_REQUIRED_EVIDENCE: &[&str] = &[
    "manager_sync_bridge_contract_current",
    "manager_sync_action_preview_current",
    "settings_diagnostics_redaction_current",
];

const EXECUTOR_FIELDS: &[&str] = &[
    "schema_version",
    "action_id",
    "command_status",
    "error_code",
    "retry_policy",
    "next_required_evidence",
];
const ACTION_ID_FIELDS: &[&str] = &["action_id"];
const STATUS_FIELDS: &[&str] = &["command_status"];
const ERROR_CODE_FIELDS: &[&str] = &["error_code"];
const RETRY_POLICY_FIELDS: &[&str] = &["retry_policy"];
const SUMMARY_FIELDS: &[&str] = &[
    "user_visible_summary_code",
    "diagnostics_summary_code",
    "next_required_evidence",
    "object_type_summary",
    "object_count_summary",
    "object_version_summary",
    "recorded_at_summary",
];
const RELEASE_FIELDS: &[&str] = &["result_handle_lifecycle"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostExportSymbolReviewDraft {
    pub symbol_name: &'static str,
    pub symbol_role: &'static str,
    pub required_contract_fields: &'static [&'static str],
    pub release_responsibility: &'static str,
    pub error_boundary: &'static str,
    pub panic_boundary: &'static str,
    pub smoke_requirement: &'static str,
    pub current_export_state: &'static str,
    pub export_approved: bool,
}

pub(crate) const HOST_EXPORT_SYMBOL_REVIEWS: &[ManagerSyncCommandHostExportSymbolReviewDraft] = &[
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_execute_v1",
        symbol_role: "command_executor",
        required_contract_fields: EXECUTOR_FIELDS,
        release_responsibility: "returns_rust_owned_result_or_error_handle",
        error_boundary: "ffi_status_invalid_argument_invalid_state_sync_internal",
        panic_boundary: "catch_unwind_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_action_id",
        symbol_role: "result_accessor",
        required_contract_fields: ACTION_ID_FIELDS,
        release_responsibility: "caller_copies_before_result_free",
        error_boundary: "released_handle_returns_invalid_state",
        panic_boundary: "accessor_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_status",
        symbol_role: "result_accessor",
        required_contract_fields: STATUS_FIELDS,
        release_responsibility: "caller_copies_before_result_free",
        error_boundary: "released_handle_returns_invalid_state",
        panic_boundary: "accessor_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_error_code",
        symbol_role: "result_accessor",
        required_contract_fields: ERROR_CODE_FIELDS,
        release_responsibility: "caller_copies_before_result_free",
        error_boundary: "released_handle_returns_invalid_state",
        panic_boundary: "accessor_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_retry_policy",
        symbol_role: "result_accessor",
        required_contract_fields: RETRY_POLICY_FIELDS,
        release_responsibility: "caller_copies_before_result_free",
        error_boundary: "released_handle_returns_invalid_state",
        panic_boundary: "accessor_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_summary",
        symbol_role: "result_accessor",
        required_contract_fields: SUMMARY_FIELDS,
        release_responsibility: "caller_copies_before_result_free",
        error_boundary: "released_handle_returns_invalid_state",
        panic_boundary: "accessor_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandHostExportSymbolReviewDraft {
        symbol_name: "radishlex_manager_sync_command_result_free",
        symbol_role: "result_release",
        required_contract_fields: RELEASE_FIELDS,
        release_responsibility: "free_null_noop_and_double_release_invalidates_views",
        error_boundary: "release_never_exposes_provider_message",
        panic_boundary: "release_panic_maps_internal_error",
        smoke_requirement: "symbol_absent_until_export_approval",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostExportApprovalReviewDraft {
    pub review_state: &'static str,
    pub export_decision: &'static str,
    pub symbol_reviews: &'static [ManagerSyncCommandHostExportSymbolReviewDraft],
    pub required_evidence: &'static [&'static str],
    pub can_export_native_symbol: bool,
    pub can_create_host_contract_test_file: bool,
    pub can_modify_dart_native_binding: bool,
    pub can_modify_manager_bridge: bool,
}

impl ManagerSyncCommandHostExportApprovalReviewDraft {
    pub(crate) fn current_phase(
        admission: &ManagerSyncCommandHostTestAdmissionReviewDraft,
        wrapper_review: &ManagerSyncCommandCAbiWrapperShapeReviewDraft,
        result_fields: &ManagerSyncCommandResultAccessorFieldSetReviewDraft,
    ) -> Result<Self, FfiError> {
        admission.assert_safe_admission()?;
        wrapper_review.assert_current_phase_not_exportable()?;
        result_fields.assert_safe_field_set()?;
        for symbol in HOST_EXPORT_SYMBOL_REVIEWS {
            validate_symbol_review(symbol, wrapper_review, result_fields)?;
        }
        Ok(Self {
            review_state: HOST_EXPORT_APPROVAL_REVIEW_READY,
            export_decision: HOST_EXPORT_APPROVAL_BLOCKED,
            symbol_reviews: HOST_EXPORT_SYMBOL_REVIEWS,
            required_evidence: EXPORT_REQUIRED_EVIDENCE,
            can_export_native_symbol: false,
            can_create_host_contract_test_file: false,
            can_modify_dart_native_binding: false,
            can_modify_manager_bridge: false,
        })
    }

    pub(crate) fn assert_safe_export_review(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "host_export_review_state")?;
        ensure_safe_summary_value(self.export_decision, "host_export_decision")?;
        if self.can_export_native_symbol
            || self.can_create_host_contract_test_file
            || self.can_modify_dart_native_binding
            || self.can_modify_manager_bridge
            || self.symbol_reviews.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        for evidence in self.required_evidence {
            ensure_safe_summary_value(evidence, "host_export_required_evidence")?;
        }
        for symbol in self.symbol_reviews {
            ensure_safe_symbol_review(symbol)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandDartBindingMigrationReviewDraft {
    pub review_state: &'static str,
    pub migration_decision: &'static str,
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
    pub can_modify_dart_native_binding: bool,
    pub can_call_native_command: bool,
    pub can_hold_native_pointer_after_copy: bool,
    pub can_write_settings_action: bool,
    pub can_modify_manager_bridge: bool,
}

impl ManagerSyncCommandDartBindingMigrationReviewDraft {
    pub(crate) fn current_phase(
        export_review: &ManagerSyncCommandHostExportApprovalReviewDraft,
    ) -> Result<Self, FfiError> {
        export_review.assert_safe_export_review()?;
        Ok(Self {
            review_state: DART_BINDING_MIGRATION_REVIEW_READY,
            migration_decision: DART_BINDING_MIGRATION_BLOCKED,
            reviewed_conditions: BINDING_REVIEWED_CONDITIONS,
            blocking_conditions: BINDING_BLOCKING_CONDITIONS,
            required_evidence: BINDING_REQUIRED_EVIDENCE,
            can_modify_dart_native_binding: false,
            can_call_native_command: false,
            can_hold_native_pointer_after_copy: false,
            can_write_settings_action: false,
            can_modify_manager_bridge: false,
        })
    }

    pub(crate) fn assert_safe_binding_migration(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "dart_binding_review_state")?;
        ensure_safe_summary_value(self.migration_decision, "dart_binding_migration_decision")?;
        if self.can_modify_dart_native_binding
            || self.can_call_native_command
            || self.can_hold_native_pointer_after_copy
            || self.can_write_settings_action
            || self.can_modify_manager_bridge
            || self.blocking_conditions.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        ensure_safe_conditions(self.reviewed_conditions, "dart_binding_reviewed_condition")?;
        ensure_safe_conditions(self.blocking_conditions, "dart_binding_blocking_condition")?;
        ensure_safe_conditions(self.required_evidence, "dart_binding_required_evidence")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandManagerBridgeMigrationReviewDraft {
    pub review_state: &'static str,
    pub migration_decision: &'static str,
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
    pub can_modify_manager_bridge: bool,
    pub can_create_bridge_command_request: bool,
    pub can_write_settings_action: bool,
    pub can_execute_real_sync: bool,
}

impl ManagerSyncCommandManagerBridgeMigrationReviewDraft {
    pub(crate) fn current_phase(
        binding_review: &ManagerSyncCommandDartBindingMigrationReviewDraft,
    ) -> Result<Self, FfiError> {
        binding_review.assert_safe_binding_migration()?;
        Ok(Self {
            review_state: MANAGER_BRIDGE_MIGRATION_REVIEW_READY,
            migration_decision: MANAGER_BRIDGE_MIGRATION_BLOCKED,
            reviewed_conditions: BRIDGE_REVIEWED_CONDITIONS,
            blocking_conditions: BRIDGE_BLOCKING_CONDITIONS,
            required_evidence: BRIDGE_REQUIRED_EVIDENCE,
            can_modify_manager_bridge: false,
            can_create_bridge_command_request: false,
            can_write_settings_action: false,
            can_execute_real_sync: false,
        })
    }

    pub(crate) fn assert_safe_bridge_migration(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "manager_bridge_review_state")?;
        ensure_safe_summary_value(self.migration_decision, "manager_bridge_migration_decision")?;
        if self.can_modify_manager_bridge
            || self.can_create_bridge_command_request
            || self.can_write_settings_action
            || self.can_execute_real_sync
            || self.blocking_conditions.is_empty()
        {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        ensure_safe_conditions(
            self.reviewed_conditions,
            "manager_bridge_reviewed_condition",
        )?;
        ensure_safe_conditions(
            self.blocking_conditions,
            "manager_bridge_blocking_condition",
        )?;
        ensure_safe_conditions(self.required_evidence, "manager_bridge_required_evidence")?;
        Ok(())
    }
}

fn validate_symbol_review(
    review: &ManagerSyncCommandHostExportSymbolReviewDraft,
    wrapper_review: &ManagerSyncCommandCAbiWrapperShapeReviewDraft,
    result_fields: &ManagerSyncCommandResultAccessorFieldSetReviewDraft,
) -> Result<(), FfiError> {
    ensure_safe_symbol_review(review)?;
    if !wrapper_review
        .candidate_symbols
        .iter()
        .any(|symbol| symbol.name == review.symbol_name)
    {
        return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
    }
    if review.symbol_role == "result_accessor" {
        for field in review.required_contract_fields {
            if !result_fields
                .fields
                .iter()
                .any(|result_field| result_field.field_name == *field)
            {
                return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
            }
        }
    }
    Ok(())
}

fn ensure_safe_symbol_review(
    review: &ManagerSyncCommandHostExportSymbolReviewDraft,
) -> Result<(), FfiError> {
    ensure_safe_summary_value(review.symbol_name, "host_export_symbol_name")?;
    ensure_safe_summary_value(review.symbol_role, "host_export_symbol_role")?;
    ensure_safe_summary_value(
        review.release_responsibility,
        "host_export_release_responsibility",
    )?;
    ensure_safe_summary_value(review.error_boundary, "host_export_error_boundary")?;
    ensure_safe_summary_value(review.panic_boundary, "host_export_panic_boundary")?;
    ensure_safe_summary_value(review.smoke_requirement, "host_export_smoke_requirement")?;
    ensure_safe_summary_value(review.current_export_state, "host_export_current_state")?;
    if review.export_approved
        || review.current_export_state != C_ABI_SYMBOL_NOT_EXPORTED
        || review.required_contract_fields.is_empty()
    {
        return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
    }
    ensure_safe_conditions(
        review.required_contract_fields,
        "host_export_required_contract_field",
    )?;
    Ok(())
}

fn ensure_safe_conditions(
    conditions: &[&'static str],
    field: &'static str,
) -> Result<(), FfiError> {
    if conditions.is_empty() {
        return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
    }
    for condition in conditions {
        ensure_safe_summary_value(condition, field)?;
    }
    Ok(())
}
