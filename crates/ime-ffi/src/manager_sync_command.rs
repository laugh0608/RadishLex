#![allow(dead_code)]

use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;
use std::str;
use std::sync::Mutex;
use std::thread::{self, ThreadId};

use crate::error::{FfiError, RadishLexStatusCode};
use crate::snapshot::RadishLexStringView;

mod admission;
mod migration_review;

pub(crate) const MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1: u32 = 1;
pub(crate) const MANAGER_SYNC_ACTION_RECOVERY_SETUP: u32 = 1;
pub(crate) const MANAGER_SYNC_ACTION_RECOVERY_RESTORE: u32 = 2;
pub(crate) const MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION: u32 = 3;
pub(crate) const MANAGER_SYNC_ACTION_DEVICE_REVOCATION: u32 = 4;

const CURRENT_PHASE_COMMAND_ERROR: &str = "sync_command_not_enabled_current_phase";
const CURRENT_PHASE_USER_SUMMARY: &str = "user_sync_entry_closed_current_phase";
const CURRENT_PHASE_DIAGNOSTICS_SUMMARY: &str = "not_executable_current_phase";
const RESULT_HANDLE_RELEASED_ERROR: &str = "manager_sync_command_result_handle_released";
const ERROR_HANDLE_RELEASED_ERROR: &str = "manager_sync_command_error_handle_released";
const ERROR_HANDLE_FORBIDDEN_MESSAGE: &str =
    "manager_sync_command_error_contains_forbidden_material";
const PANIC_BOUNDARY_ERROR: &str = "manager_sync_command_panic_boundary";
const SYNC_COMMAND_CONTEXT_LOCK_ERROR: &str = "manager_sync_command_context_lock_poisoned";
const SYNC_COMMAND_DOMAIN_DRAFT: &str = "manager_sync_write_domain";
const SYNC_COMMAND_DOMAIN_BUSY_ERROR: &str = "sync_domain_command_in_progress";
const ACTION_SECTION_MISMATCH_ERROR: &str = "manager_sync_action_section_mismatch";
const ACTION_SECTION_VALUE_ERROR: &str = "manager_sync_action_section_value_not_allowed";
const ACTION_CONFIRMATION_UNAVAILABLE_ERROR: &str =
    "manager_sync_action_confirmation_not_available_current_phase";
const C_ABI_WRAPPER_REVIEW_READY: &str = "c_abi_wrapper_shape_review_ready_no_native_symbol";
const HOST_CONTRACT_TEST_GATE_CLOSED: &str = "host_contract_test_gate_closed_no_native_symbol";
const C_ABI_SYMBOL_NOT_EXPORTED: &str = "planned_not_exported_current_phase";
const C_ABI_SYMBOL_EXPORT_UNAPPROVED_ERROR: &str =
    "manager_sync_c_abi_symbol_export_not_approved_current_phase";
const RESULT_ACCESSOR_FIELD_SET_REVIEW_READY: &str =
    "result_accessor_field_set_review_ready_no_native_symbol";
const RESULT_ACCESSOR_FIELD_SET_ERROR: &str = "manager_sync_result_accessor_field_set_not_reviewed";
const COMMAND_CONTEXT_OWNER_SCOPE_REVIEW_READY: &str =
    "command_context_owner_scope_review_ready_no_native_symbol";
const COMMAND_CONTEXT_OWNER_SCOPE_ERROR: &str =
    "manager_sync_command_context_owner_scope_not_reviewed";
const SYNC_COMMAND_CONTEXT_OWNER_MISMATCH_ERROR: &str =
    "manager_sync_command_context_owner_mismatch";
const HOST_CONTRACT_GATE_MIGRATION_REVIEW_READY: &str =
    "host_contract_gate_migration_review_ready_no_native_symbol";
const HOST_GATE_READINESS_REVIEW_READY: &str = "host_gate_readiness_review_ready_no_native_symbol";
const HOST_GATE_BLOCKED_NO_NATIVE_SYMBOL: &str = "host_gate_blocked_no_native_symbol";
const HOST_GATE_READINESS_REPLAY_READY: &str =
    "dart_fake_native_gate_migration_replay_ready_no_native_symbol";
const HOST_TEST_ADMISSION_REVIEW_READY: &str =
    "host_contract_test_admission_review_ready_no_native_symbol";
const HOST_TEST_ADMISSION_BLOCKED: &str = "blocked_before_real_host_contract_test_file";
const HOST_TEST_ADMISSION_NEXT_DECISION: &str =
    "native_symbol_export_and_binding_approval_required";
const SUMMARY_STORAGE_REVIEW_READY: &str = "summary_storage_review_ready_no_native_symbol";
const SUMMARY_STORAGE_REVIEW_ERROR: &str = "manager_sync_summary_storage_not_reviewed";
const COMMAND_WORKER_THREAD_POLICY_REVIEW_READY: &str =
    "command_worker_thread_policy_review_ready_no_native_symbol";
const COMMAND_WORKER_THREAD_POLICY_ERROR: &str =
    "manager_sync_command_worker_thread_policy_not_reviewed";
const DEBUG_REDACTION_TEST_SHAPE_REVIEW_READY: &str =
    "debug_redaction_test_shape_review_ready_no_native_symbol";
const DEBUG_REDACTION_TEST_SHAPE_ERROR: &str =
    "manager_sync_debug_redaction_test_shape_not_reviewed";

pub(crate) const MANAGER_SYNC_SECTION_RECOVERY_SETUP: u32 = 1;
pub(crate) const MANAGER_SYNC_SECTION_RECOVERY_RESTORE: u32 = 2;
pub(crate) const MANAGER_SYNC_SECTION_JOIN_REQUEST_AUTHORIZATION: u32 = 3;
pub(crate) const MANAGER_SYNC_SECTION_DEVICE_REVOCATION: u32 = 4;

const SETUP_CONFIRMATION_UNAVAILABLE: &str = "setup_save_confirmation_not_available_current_phase";
const SETUP_REQUIRED_BEFORE_UPLOAD: &str = "required_before_first_upload";
const SETUP_DISPLAY_UNAVAILABLE: &str = "display_not_available_current_phase";
const RESTORE_CONFIRMATION_UNAVAILABLE: &str = "restore_confirmation_not_available_current_phase";
const RESTORE_RECORD_NOT_CHECKED: &str = "recovery_record_not_checked_current_phase";
const RESTORE_INPUT_UNAVAILABLE: &str = "input_not_available_current_phase";
const JOIN_CONFIRMATION_UNAVAILABLE: &str =
    "join_authorization_confirmation_not_available_current_phase";
const JOIN_REQUEST_NOT_CREATED: &str = "join_request_not_created_current_phase";
const JOIN_SHORT_CODE_UNAVAILABLE: &str = "short_code_not_available_current_phase";
const REVOCATION_CONFIRMATION_UNAVAILABLE: &str =
    "device_revocation_confirmation_not_available_current_phase";
const REVOCATION_ACTIVE_DEVICE_REQUIRED: &str = "active_device_required_current_phase";
const REVOCATION_KEY_EPOCH_NOT_ROTATED: &str = "key_epoch_not_rotated_current_phase";

const FORBIDDEN_SUMMARY_FRAGMENTS: &[&str] = &[
    "secret-token",
    "Bearer ",
    "RADISHLEX-RECOVERY-CODE-SECRET",
    "short_code=",
    "signature_bytes",
    "wrapped_material_bytes",
    "payload_bytes=",
    "request_body",
    "response_body",
    "/synthetic/private",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandCAbiSymbolRoleDraft {
    CommandExecutor,
    ResultAccessor,
    ResultRelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandCAbiSymbolDraft {
    pub name: &'static str,
    pub role: ManagerSyncCommandCAbiSymbolRoleDraft,
    pub current_export_state: &'static str,
    pub export_approved: bool,
}

pub(crate) const MANAGER_SYNC_COMMAND_C_ABI_SYMBOLS_DRAFT: &[ManagerSyncCommandCAbiSymbolDraft] = &[
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_execute_v1",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::CommandExecutor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_action_id",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultAccessor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_status",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultAccessor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_error_code",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultAccessor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_retry_policy",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultAccessor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_summary",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultAccessor,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
    ManagerSyncCommandCAbiSymbolDraft {
        name: "radishlex_manager_sync_command_result_free",
        role: ManagerSyncCommandCAbiSymbolRoleDraft::ResultRelease,
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        export_approved: false,
    },
];

const MANAGER_SYNC_COMMAND_EXISTING_ERROR_SYMBOLS_DRAFT: &[&str] = &[
    "radishlex_error_code",
    "radishlex_error_message",
    "radishlex_error_free",
];

const MANAGER_SYNC_COMMAND_HOST_TEST_BLOCKERS_DRAFT: &[&str] = &[
    "native_symbol_export_not_approved",
    "dart_native_binding_not_approved",
    "manager_bridge_command_not_approved",
    "host_contract_test_file_not_approved",
    "real_sync_execution_not_approved",
];

const MANAGER_SYNC_COMMAND_HOST_TEST_EVIDENCE_DRAFT: &[&str] = &[
    "adr_0006_current",
    "manager_sync_ffi_command_boundary_current",
    "manager_sync_command_internal_draft_tests",
    "dart_fake_native_gate_migration_replay",
    "ffi_bridge_smoke_candidate_symbols_absent",
];

const MANAGER_SYNC_COMMAND_HOST_GATE_MIGRATION_CONDITIONS_DRAFT: &[&str] = &[
    "result_accessor_field_set_reviewed",
    "summary_storage_policy_reviewed",
    "object_summary_numeric_width_reviewed",
    "command_context_owner_scope_reviewed",
    "command_worker_thread_policy_reviewed",
    "debug_redaction_test_shape_reviewed",
    "forbidden_material_redaction_reviewed",
    "native_symbol_export_approved_by_adr",
    "dart_native_binding_approved",
    "manager_bridge_command_approved",
    "host_contract_test_file_approved",
    "ffi_smoke_candidate_symbols_absent_until_approval",
    "real_sync_execution_approved_after_gate",
];

const MANAGER_SYNC_COMMAND_HOST_GATE_REVIEWED_CONDITIONS_DRAFT: &[&str] = &[
    "result_accessor_field_set_reviewed",
    "summary_storage_policy_reviewed",
    "object_summary_numeric_width_reviewed",
    "command_context_owner_scope_reviewed",
    "command_worker_thread_policy_reviewed",
    "debug_redaction_test_shape_reviewed",
    "forbidden_material_redaction_reviewed",
    "ffi_smoke_candidate_symbols_absent_until_approval",
];

const MANAGER_SYNC_COMMAND_HOST_GATE_BLOCKING_CONDITIONS_DRAFT: &[&str] = &[
    "native_symbol_export_approved_by_adr",
    "dart_native_binding_approved",
    "manager_bridge_command_approved",
    "host_contract_test_file_approved",
    "real_sync_execution_approved_after_gate",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultAccessorFieldDraft {
    pub field_name: &'static str,
    pub accessor_symbol: &'static str,
    pub value_kind: &'static str,
    pub current_export_state: &'static str,
    pub copy_required_before_release: bool,
}

pub(crate) const MANAGER_SYNC_COMMAND_RESULT_ACCESSOR_FIELDS_DRAFT:
    &[ManagerSyncCommandResultAccessorFieldDraft] = &[
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "schema_version",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "u32",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "action_id",
        accessor_symbol: "radishlex_manager_sync_command_result_action_id",
        value_kind: "u32",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "command_status",
        accessor_symbol: "radishlex_manager_sync_command_result_status",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "error_code",
        accessor_symbol: "radishlex_manager_sync_command_result_error_code",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "retry_policy",
        accessor_symbol: "radishlex_manager_sync_command_result_retry_policy",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "user_visible_summary_code",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "diagnostics_summary_code",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "next_required_evidence",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "object_type_summary",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "object_count_summary",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "u64_count",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "object_version_summary",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
    ManagerSyncCommandResultAccessorFieldDraft {
        field_name: "recorded_at_summary",
        accessor_symbol: "radishlex_manager_sync_command_result_summary",
        value_kind: "summary_code",
        current_export_state: C_ABI_SYMBOL_NOT_EXPORTED,
        copy_required_before_release: true,
    },
];

const MANAGER_SYNC_COMMAND_DEBUG_REDACTION_TARGETS_DRAFT: &[&str] = &[
    "request_error",
    "command_result",
    "result_accessor_field",
    "owner_scope_review",
    "worker_policy_review",
    "gate_migration_review",
    "host_gate_readiness_review",
    "host_test_admission_review",
    "native_export_approval_review",
    "dart_binding_migration_review",
    "manager_bridge_migration_review",
];

const MANAGER_SYNC_COMMAND_FORBIDDEN_CATEGORIES_DRAFT: &[&str] = &[
    "token_material",
    "recovery_secret_material",
    "join_verifier_material",
    "signature_or_wrapped_sync_material",
    "opaque_transport_content",
    "local_path_material",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandCAbiWrapperShapeReviewDraft {
    pub review_state: &'static str,
    pub executor_strategy: &'static str,
    pub result_handle_strategy: &'static str,
    pub error_lifecycle_strategy: &'static str,
    pub candidate_symbols: &'static [ManagerSyncCommandCAbiSymbolDraft],
    pub existing_error_lifecycle_symbols: &'static [&'static str],
    pub native_symbol_export_approved: bool,
    pub dart_native_binding_approved: bool,
    pub manager_bridge_command_approved: bool,
}

impl ManagerSyncCommandCAbiWrapperShapeReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: C_ABI_WRAPPER_REVIEW_READY,
            executor_strategy: "single_versioned_manager_sync_command_executor",
            result_handle_strategy: "rust_owned_result_handle_copy_then_free",
            error_lifecycle_strategy: "reuse_existing_radishlex_error_handle",
            candidate_symbols: MANAGER_SYNC_COMMAND_C_ABI_SYMBOLS_DRAFT,
            existing_error_lifecycle_symbols: MANAGER_SYNC_COMMAND_EXISTING_ERROR_SYMBOLS_DRAFT,
            native_symbol_export_approved: false,
            dart_native_binding_approved: false,
            manager_bridge_command_approved: false,
        }
    }

    pub(crate) fn assert_current_phase_not_exportable(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "c_abi_wrapper_review_state")?;
        ensure_safe_summary_value(self.executor_strategy, "c_abi_executor_strategy")?;
        ensure_safe_summary_value(self.result_handle_strategy, "c_abi_result_handle_strategy")?;
        ensure_safe_summary_value(
            self.error_lifecycle_strategy,
            "c_abi_error_lifecycle_strategy",
        )?;

        if self.native_symbol_export_approved
            || self.dart_native_binding_approved
            || self.manager_bridge_command_approved
        {
            return Err(FfiError::invalid_state(
                C_ABI_SYMBOL_EXPORT_UNAPPROVED_ERROR,
            ));
        }

        for symbol in self.candidate_symbols {
            ensure_safe_summary_value(symbol.name, "c_abi_candidate_symbol")?;
            ensure_safe_summary_value(symbol.current_export_state, "c_abi_symbol_export_state")?;
            if symbol.export_approved {
                return Err(FfiError::invalid_state(
                    C_ABI_SYMBOL_EXPORT_UNAPPROVED_ERROR,
                ));
            }
        }

        for symbol in self.existing_error_lifecycle_symbols {
            ensure_safe_summary_value(symbol, "existing_error_lifecycle_symbol")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostContractTestGateDraft {
    pub gate_state: &'static str,
    pub ready_for_host_contract_test: bool,
    pub blockers: &'static [&'static str],
    pub required_evidence: &'static [&'static str],
}

impl ManagerSyncCommandHostContractTestGateDraft {
    pub(crate) fn current_phase(
        review: &ManagerSyncCommandCAbiWrapperShapeReviewDraft,
    ) -> Result<Self, FfiError> {
        review.assert_current_phase_not_exportable()?;
        Ok(Self {
            gate_state: HOST_CONTRACT_TEST_GATE_CLOSED,
            ready_for_host_contract_test: false,
            blockers: MANAGER_SYNC_COMMAND_HOST_TEST_BLOCKERS_DRAFT,
            required_evidence: MANAGER_SYNC_COMMAND_HOST_TEST_EVIDENCE_DRAFT,
        })
    }

    pub(crate) fn assert_safe_gate_summary(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.gate_state, "host_contract_test_gate_state")?;
        for blocker in self.blockers {
            ensure_safe_summary_value(blocker, "host_contract_test_blocker")?;
        }
        for evidence in self.required_evidence {
            ensure_safe_summary_value(evidence, "host_contract_test_required_evidence")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultAccessorFieldSetReviewDraft {
    pub review_state: &'static str,
    pub fields: &'static [ManagerSyncCommandResultAccessorFieldDraft],
}

impl ManagerSyncCommandResultAccessorFieldSetReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: RESULT_ACCESSOR_FIELD_SET_REVIEW_READY,
            fields: MANAGER_SYNC_COMMAND_RESULT_ACCESSOR_FIELDS_DRAFT,
        }
    }

    pub(crate) fn assert_safe_field_set(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "result_accessor_field_set_review_state")?;
        let mut names = HashSet::new();
        for field in self.fields {
            ensure_safe_summary_value(field.field_name, "result_accessor_field_name")?;
            ensure_safe_summary_value(field.accessor_symbol, "result_accessor_symbol")?;
            ensure_safe_summary_value(field.value_kind, "result_accessor_value_kind")?;
            ensure_safe_summary_value(field.current_export_state, "result_accessor_export_state")?;
            if !field.copy_required_before_release
                || field.current_export_state != C_ABI_SYMBOL_NOT_EXPORTED
                || !names.insert(field.field_name)
            {
                return Err(FfiError::invalid_state(RESULT_ACCESSOR_FIELD_SET_ERROR));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandContextOwnerScopeReviewDraft {
    pub review_state: &'static str,
    pub owner_scope: &'static str,
    pub domain_guard_storage_scope: &'static str,
    pub command_worker_scope: &'static str,
    pub owns_flutter_widget_state: bool,
    pub owns_settings_payload: bool,
    pub owns_dart_pointer: bool,
    pub owns_platform_ui_object: bool,
}

impl ManagerSyncCommandContextOwnerScopeReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: COMMAND_CONTEXT_OWNER_SCOPE_REVIEW_READY,
            owner_scope: "manager_sync_command_context_owner_thread",
            domain_guard_storage_scope: "rust_owned_context_mutex_active_domain_set",
            command_worker_scope: "not_started_current_phase",
            owns_flutter_widget_state: false,
            owns_settings_payload: false,
            owns_dart_pointer: false,
            owns_platform_ui_object: false,
        }
    }

    pub(crate) fn assert_safe_owner_scope(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(
            self.review_state,
            "command_context_owner_scope_review_state",
        )?;
        ensure_safe_summary_value(self.owner_scope, "command_context_owner_scope")?;
        ensure_safe_summary_value(
            self.domain_guard_storage_scope,
            "command_context_domain_guard_storage_scope",
        )?;
        ensure_safe_summary_value(self.command_worker_scope, "command_context_worker_scope")?;
        if self.owns_flutter_widget_state
            || self.owns_settings_payload
            || self.owns_dart_pointer
            || self.owns_platform_ui_object
        {
            return Err(FfiError::invalid_state(COMMAND_CONTEXT_OWNER_SCOPE_ERROR));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandSummaryStorageReviewDraft {
    pub review_state: &'static str,
    pub current_phase_storage: &'static str,
    pub future_dynamic_storage: &'static str,
    pub borrowed_view_lifetime: &'static str,
    pub stores_caller_pointer: bool,
    pub stores_provider_message: bool,
    pub stores_transport_payload: bool,
}

impl ManagerSyncCommandSummaryStorageReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: SUMMARY_STORAGE_REVIEW_READY,
            current_phase_storage: "static_summary_code_table",
            future_dynamic_storage: "rust_owned_result_handle_storage",
            borrowed_view_lifetime: "valid_until_result_release",
            stores_caller_pointer: false,
            stores_provider_message: false,
            stores_transport_payload: false,
        }
    }

    pub(crate) fn assert_safe_storage_policy(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "summary_storage_review_state")?;
        ensure_safe_summary_value(self.current_phase_storage, "current_phase_summary_storage")?;
        ensure_safe_summary_value(
            self.future_dynamic_storage,
            "future_dynamic_summary_storage",
        )?;
        ensure_safe_summary_value(
            self.borrowed_view_lifetime,
            "summary_borrowed_view_lifetime",
        )?;
        if self.stores_caller_pointer
            || self.stores_provider_message
            || self.stores_transport_payload
        {
            return Err(FfiError::invalid_state(SUMMARY_STORAGE_REVIEW_ERROR));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandWorkerThreadPolicyReviewDraft {
    pub review_state: &'static str,
    pub current_phase_policy: &'static str,
    pub future_worker_policy: &'static str,
    pub owner_migration_policy: &'static str,
    pub allows_background_remote_retry: bool,
    pub queues_secret_payload: bool,
    pub blocks_flutter_ui_isolate: bool,
}

impl ManagerSyncCommandWorkerThreadPolicyReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: COMMAND_WORKER_THREAD_POLICY_REVIEW_READY,
            current_phase_policy: "caller_thread_owner_checked_current_phase",
            future_worker_policy: "single_serial_manager_sync_worker_before_real_sync",
            owner_migration_policy: "must_be_reviewed_before_worker_enabled",
            allows_background_remote_retry: false,
            queues_secret_payload: false,
            blocks_flutter_ui_isolate: false,
        }
    }

    pub(crate) fn assert_safe_worker_policy(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(
            self.review_state,
            "command_worker_thread_policy_review_state",
        )?;
        ensure_safe_summary_value(self.current_phase_policy, "current_phase_worker_policy")?;
        ensure_safe_summary_value(self.future_worker_policy, "future_worker_policy")?;
        ensure_safe_summary_value(self.owner_migration_policy, "owner_migration_policy")?;
        if self.allows_background_remote_retry
            || self.queues_secret_payload
            || self.blocks_flutter_ui_isolate
        {
            return Err(FfiError::invalid_state(COMMAND_WORKER_THREAD_POLICY_ERROR));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandDebugRedactionReviewDraft {
    pub review_state: &'static str,
    pub debug_targets: &'static [&'static str],
    pub forbidden_categories: &'static [&'static str],
}

impl ManagerSyncCommandDebugRedactionReviewDraft {
    pub(crate) fn current_phase() -> Self {
        Self {
            review_state: DEBUG_REDACTION_TEST_SHAPE_REVIEW_READY,
            debug_targets: MANAGER_SYNC_COMMAND_DEBUG_REDACTION_TARGETS_DRAFT,
            forbidden_categories: MANAGER_SYNC_COMMAND_FORBIDDEN_CATEGORIES_DRAFT,
        }
    }

    pub(crate) fn assert_safe_debug_redaction_shape(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "debug_redaction_review_state")?;
        if self.debug_targets.is_empty() || self.forbidden_categories.is_empty() {
            return Err(FfiError::invalid_state(DEBUG_REDACTION_TEST_SHAPE_ERROR));
        }
        for target in self.debug_targets {
            ensure_safe_summary_value(target, "debug_redaction_target")?;
        }
        for category in self.forbidden_categories {
            ensure_safe_summary_value(category, "debug_redaction_forbidden_category")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostGateMigrationReviewDraft {
    pub review_state: &'static str,
    pub ready_for_host_contract_test: bool,
    pub required_conditions: &'static [&'static str],
}

impl ManagerSyncCommandHostGateMigrationReviewDraft {
    pub(crate) fn current_phase(
        gate: &ManagerSyncCommandHostContractTestGateDraft,
        result_fields: &ManagerSyncCommandResultAccessorFieldSetReviewDraft,
        owner_scope: &ManagerSyncCommandContextOwnerScopeReviewDraft,
        summary_storage: &ManagerSyncCommandSummaryStorageReviewDraft,
        worker_policy: &ManagerSyncCommandWorkerThreadPolicyReviewDraft,
        debug_redaction: &ManagerSyncCommandDebugRedactionReviewDraft,
    ) -> Result<Self, FfiError> {
        gate.assert_safe_gate_summary()?;
        result_fields.assert_safe_field_set()?;
        owner_scope.assert_safe_owner_scope()?;
        summary_storage.assert_safe_storage_policy()?;
        worker_policy.assert_safe_worker_policy()?;
        debug_redaction.assert_safe_debug_redaction_shape()?;
        Ok(Self {
            review_state: HOST_CONTRACT_GATE_MIGRATION_REVIEW_READY,
            ready_for_host_contract_test: false,
            required_conditions: MANAGER_SYNC_COMMAND_HOST_GATE_MIGRATION_CONDITIONS_DRAFT,
        })
    }

    pub(crate) fn assert_safe_migration_conditions(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "host_gate_migration_review_state")?;
        for condition in self.required_conditions {
            ensure_safe_summary_value(condition, "host_gate_migration_condition")?;
        }
        if self.ready_for_host_contract_test {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandHostGateReadinessReviewDraft {
    pub review_state: &'static str,
    pub readiness_state: &'static str,
    pub ready_for_host_contract_test: bool,
    pub reviewed_conditions: &'static [&'static str],
    pub blocking_conditions: &'static [&'static str],
    pub dart_fake_replay_status: &'static str,
}

impl ManagerSyncCommandHostGateReadinessReviewDraft {
    pub(crate) fn current_phase(
        migration: &ManagerSyncCommandHostGateMigrationReviewDraft,
    ) -> Result<Self, FfiError> {
        migration.assert_safe_migration_conditions()?;
        for condition in MANAGER_SYNC_COMMAND_HOST_GATE_REVIEWED_CONDITIONS_DRAFT {
            if !migration.required_conditions.contains(condition) {
                return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
            }
        }
        for condition in MANAGER_SYNC_COMMAND_HOST_GATE_BLOCKING_CONDITIONS_DRAFT {
            if !migration.required_conditions.contains(condition) {
                return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
            }
        }

        Ok(Self {
            review_state: HOST_GATE_READINESS_REVIEW_READY,
            readiness_state: HOST_GATE_BLOCKED_NO_NATIVE_SYMBOL,
            ready_for_host_contract_test: false,
            reviewed_conditions: MANAGER_SYNC_COMMAND_HOST_GATE_REVIEWED_CONDITIONS_DRAFT,
            blocking_conditions: MANAGER_SYNC_COMMAND_HOST_GATE_BLOCKING_CONDITIONS_DRAFT,
            dart_fake_replay_status: HOST_GATE_READINESS_REPLAY_READY,
        })
    }

    pub(crate) fn assert_safe_readiness(&self) -> Result<(), FfiError> {
        ensure_safe_summary_value(self.review_state, "host_gate_readiness_review_state")?;
        ensure_safe_summary_value(self.readiness_state, "host_gate_readiness_state")?;
        ensure_safe_summary_value(self.dart_fake_replay_status, "host_gate_replay_status")?;
        for condition in self.reviewed_conditions {
            ensure_safe_summary_value(condition, "host_gate_reviewed_condition")?;
        }
        for condition in self.blocking_conditions {
            ensure_safe_summary_value(condition, "host_gate_blocking_condition")?;
        }
        if self.ready_for_host_contract_test || self.blocking_conditions.is_empty() {
            return Err(FfiError::invalid_state(HOST_CONTRACT_TEST_GATE_CLOSED));
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ManagerSyncCommandRequestRawDraft {
    pub schema_version: u32,
    pub action_id: u32,
    pub operation_id: RadishLexStringView,
    pub readiness_snapshot_id: RadishLexStringView,
    pub deployment_evidence_source_tag: RadishLexStringView,
    pub device_backend_gate: RadishLexStringView,
    pub explicit_user_start: u8,
    pub action_section: ManagerSyncActionSectionRawDraft,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ManagerSyncActionSectionRawDraft {
    pub section_id: u32,
    pub confirmation_status_code: RadishLexStringView,
    pub prerequisite_evidence_code: RadishLexStringView,
    pub transient_material_status_code: RadishLexStringView,
    pub user_confirmation_present: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandActionDraft {
    RecoverySetup,
    RecoveryRestore,
    JoinRequestAuthorization,
    DeviceRevocation,
}

impl ManagerSyncCommandActionDraft {
    fn from_id(action_id: u32) -> Result<Self, FfiError> {
        match action_id {
            MANAGER_SYNC_ACTION_RECOVERY_SETUP => Ok(Self::RecoverySetup),
            MANAGER_SYNC_ACTION_RECOVERY_RESTORE => Ok(Self::RecoveryRestore),
            MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION => Ok(Self::JoinRequestAuthorization),
            MANAGER_SYNC_ACTION_DEVICE_REVOCATION => Ok(Self::DeviceRevocation),
            _ => Err(FfiError::invalid_argument("unknown manager sync action id")),
        }
    }

    fn as_summary_code(self) -> &'static str {
        match self {
            Self::RecoverySetup => "recovery_setup",
            Self::RecoveryRestore => "recovery_restore",
            Self::JoinRequestAuthorization => "join_request_authorization",
            Self::DeviceRevocation => "device_revocation",
        }
    }

    fn as_id(self) -> u32 {
        match self {
            Self::RecoverySetup => MANAGER_SYNC_ACTION_RECOVERY_SETUP,
            Self::RecoveryRestore => MANAGER_SYNC_ACTION_RECOVERY_RESTORE,
            Self::JoinRequestAuthorization => MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION,
            Self::DeviceRevocation => MANAGER_SYNC_ACTION_DEVICE_REVOCATION,
        }
    }

    fn section_kind(self) -> ManagerSyncActionSectionKindDraft {
        match self {
            Self::RecoverySetup => ManagerSyncActionSectionKindDraft::RecoverySetup,
            Self::RecoveryRestore => ManagerSyncActionSectionKindDraft::RecoveryRestore,
            Self::JoinRequestAuthorization => {
                ManagerSyncActionSectionKindDraft::JoinRequestAuthorization
            }
            Self::DeviceRevocation => ManagerSyncActionSectionKindDraft::DeviceRevocation,
        }
    }

    fn section_profile(self) -> ManagerSyncActionSectionProfileDraft {
        match self {
            Self::RecoverySetup => ManagerSyncActionSectionProfileDraft {
                confirmation_status_code: SETUP_CONFIRMATION_UNAVAILABLE,
                prerequisite_evidence_code: SETUP_REQUIRED_BEFORE_UPLOAD,
                transient_material_status_code: SETUP_DISPLAY_UNAVAILABLE,
            },
            Self::RecoveryRestore => ManagerSyncActionSectionProfileDraft {
                confirmation_status_code: RESTORE_CONFIRMATION_UNAVAILABLE,
                prerequisite_evidence_code: RESTORE_RECORD_NOT_CHECKED,
                transient_material_status_code: RESTORE_INPUT_UNAVAILABLE,
            },
            Self::JoinRequestAuthorization => ManagerSyncActionSectionProfileDraft {
                confirmation_status_code: JOIN_CONFIRMATION_UNAVAILABLE,
                prerequisite_evidence_code: JOIN_REQUEST_NOT_CREATED,
                transient_material_status_code: JOIN_SHORT_CODE_UNAVAILABLE,
            },
            Self::DeviceRevocation => ManagerSyncActionSectionProfileDraft {
                confirmation_status_code: REVOCATION_CONFIRMATION_UNAVAILABLE,
                prerequisite_evidence_code: REVOCATION_ACTIVE_DEVICE_REQUIRED,
                transient_material_status_code: REVOCATION_KEY_EPOCH_NOT_ROTATED,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncActionSectionKindDraft {
    RecoverySetup,
    RecoveryRestore,
    JoinRequestAuthorization,
    DeviceRevocation,
}

impl ManagerSyncActionSectionKindDraft {
    fn from_id(section_id: u32) -> Result<Self, FfiError> {
        match section_id {
            MANAGER_SYNC_SECTION_RECOVERY_SETUP => Ok(Self::RecoverySetup),
            MANAGER_SYNC_SECTION_RECOVERY_RESTORE => Ok(Self::RecoveryRestore),
            MANAGER_SYNC_SECTION_JOIN_REQUEST_AUTHORIZATION => Ok(Self::JoinRequestAuthorization),
            MANAGER_SYNC_SECTION_DEVICE_REVOCATION => Ok(Self::DeviceRevocation),
            _ => Err(FfiError::invalid_argument(
                "unknown manager sync action section id",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManagerSyncActionSectionProfileDraft {
    confirmation_status_code: &'static str,
    prerequisite_evidence_code: &'static str,
    transient_material_status_code: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncActionSectionDraft {
    kind: ManagerSyncActionSectionKindDraft,
    confirmation_status_code: &'static str,
    prerequisite_evidence_code: &'static str,
    transient_material_status_code: &'static str,
    user_confirmation_present: bool,
}

impl ManagerSyncActionSectionDraft {
    fn from_raw(
        action: ManagerSyncCommandActionDraft,
        raw: ManagerSyncActionSectionRawDraft,
    ) -> Result<Self, FfiError> {
        let kind = ManagerSyncActionSectionKindDraft::from_id(raw.section_id)?;
        if kind != action.section_kind() {
            return Err(FfiError::invalid_argument(ACTION_SECTION_MISMATCH_ERROR));
        }

        let user_confirmation_present =
            read_ffi_bool(raw.user_confirmation_present, "user_confirmation_present")?;
        if user_confirmation_present {
            return Err(FfiError::invalid_state(
                ACTION_CONFIRMATION_UNAVAILABLE_ERROR,
            ));
        }

        let profile = action.section_profile();
        let confirmation_status_code = read_expected_action_section_code(
            raw.confirmation_status_code,
            "confirmation_status_code",
            profile.confirmation_status_code,
        )?;
        let prerequisite_evidence_code = read_expected_action_section_code(
            raw.prerequisite_evidence_code,
            "prerequisite_evidence_code",
            profile.prerequisite_evidence_code,
        )?;
        let transient_material_status_code = read_expected_action_section_code(
            raw.transient_material_status_code,
            "transient_material_status_code",
            profile.transient_material_status_code,
        )?;

        Ok(Self {
            kind,
            confirmation_status_code,
            prerequisite_evidence_code,
            transient_material_status_code,
            user_confirmation_present,
        })
    }

    fn current_phase_for_action(action: ManagerSyncCommandActionDraft) -> Self {
        let profile = action.section_profile();
        Self {
            kind: action.section_kind(),
            confirmation_status_code: profile.confirmation_status_code,
            prerequisite_evidence_code: profile.prerequisite_evidence_code,
            transient_material_status_code: profile.transient_material_status_code,
            user_confirmation_present: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandRequestDraft {
    schema_version: u32,
    action: ManagerSyncCommandActionDraft,
    action_section: ManagerSyncActionSectionDraft,
    operation_id: String,
    readiness_snapshot_id: String,
    deployment_evidence_source_tag: String,
    device_backend_gate: String,
    explicit_user_start: bool,
}

impl ManagerSyncCommandRequestDraft {
    pub(crate) fn from_raw(raw: ManagerSyncCommandRequestRawDraft) -> Result<Self, FfiError> {
        if raw.schema_version != MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1 {
            return Err(FfiError::invalid_argument(
                "unknown manager sync command schema version",
            ));
        }

        let action = ManagerSyncCommandActionDraft::from_id(raw.action_id)?;
        let action_section = ManagerSyncActionSectionDraft::from_raw(action, raw.action_section)?;
        let explicit_user_start = read_ffi_bool(raw.explicit_user_start, "explicit_user_start")?;
        let operation_id = read_required_safe_utf8_view(raw.operation_id, "operation_id")?;
        let readiness_snapshot_id =
            read_required_safe_utf8_view(raw.readiness_snapshot_id, "readiness_snapshot_id")?;
        let deployment_evidence_source_tag = read_required_safe_utf8_view(
            raw.deployment_evidence_source_tag,
            "deployment_evidence_source_tag",
        )?;
        let device_backend_gate =
            read_required_safe_utf8_view(raw.device_backend_gate, "device_backend_gate")?;

        Ok(Self {
            schema_version: raw.schema_version,
            action,
            action_section,
            operation_id,
            readiness_snapshot_id,
            deployment_evidence_source_tag,
            device_backend_gate,
            explicit_user_start,
        })
    }

    pub(crate) fn action(&self) -> ManagerSyncCommandActionDraft {
        self.action
    }

    pub(crate) fn action_section(&self) -> ManagerSyncActionSectionDraft {
        self.action_section
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandStatusDraft {
    BlockedByReadiness,
}

impl ManagerSyncCommandStatusDraft {
    fn as_summary_code(self) -> &'static str {
        match self {
            Self::BlockedByReadiness => "blocked_by_readiness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncRetryPolicyDraft {
    NotRetryable,
}

impl ManagerSyncRetryPolicyDraft {
    fn as_summary_code(self) -> &'static str {
        match self {
            Self::NotRetryable => "not_retryable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandEnvelopeDraft {
    pub action_summary_code: &'static str,
    pub command_status: &'static str,
    pub error_code: &'static str,
    pub retry_policy: &'static str,
    pub user_visible_summary_code: &'static str,
    pub diagnostics_summary_code: &'static str,
    pub next_required_evidence: &'static str,
    pub object_type_summary: &'static str,
    pub object_count_summary: u64,
    pub object_version_summary: &'static str,
    pub recorded_at_summary: &'static str,
}

impl ManagerSyncCommandEnvelopeDraft {
    fn current_phase_closed(
        action: ManagerSyncCommandActionDraft,
        action_section: ManagerSyncActionSectionDraft,
    ) -> Self {
        Self {
            action_summary_code: action.as_summary_code(),
            command_status: ManagerSyncCommandStatusDraft::BlockedByReadiness.as_summary_code(),
            error_code: CURRENT_PHASE_COMMAND_ERROR,
            retry_policy: ManagerSyncRetryPolicyDraft::NotRetryable.as_summary_code(),
            user_visible_summary_code: CURRENT_PHASE_USER_SUMMARY,
            diagnostics_summary_code: CURRENT_PHASE_DIAGNOSTICS_SUMMARY,
            next_required_evidence: action_section.prerequisite_evidence_code,
            object_type_summary: "none",
            object_count_summary: 0,
            object_version_summary: "none",
            recorded_at_summary: "not_recorded_current_phase",
        }
    }

    fn summary_values(&self) -> [&str; 10] {
        [
            self.action_summary_code,
            self.command_status,
            self.error_code,
            self.retry_policy,
            self.user_visible_summary_code,
            self.diagnostics_summary_code,
            self.next_required_evidence,
            self.object_type_summary,
            self.object_version_summary,
            self.recorded_at_summary,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultDraft {
    pub schema_version: u32,
    pub action_id: u32,
    pub abi_status: RadishLexStatusCode,
    pub envelope: ManagerSyncCommandEnvelopeDraft,
}

impl ManagerSyncCommandResultDraft {
    pub(crate) fn current_phase_closed(action: ManagerSyncCommandActionDraft) -> Self {
        Self::current_phase_closed_with_section(
            action,
            ManagerSyncActionSectionDraft::current_phase_for_action(action),
        )
    }

    pub(crate) fn current_phase_closed_with_section(
        action: ManagerSyncCommandActionDraft,
        action_section: ManagerSyncActionSectionDraft,
    ) -> Self {
        Self {
            schema_version: MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1,
            action_id: action.as_id(),
            abi_status: RadishLexStatusCode::InvalidState,
            envelope: ManagerSyncCommandEnvelopeDraft::current_phase_closed(action, action_section),
        }
    }

    pub(crate) fn assert_safe_summary(&self) -> Result<(), FfiError> {
        for value in self.envelope.summary_values() {
            ensure_safe_summary_value(value, "manager_sync_command_result")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultViewDraft<'a> {
    pub schema_version: u32,
    pub action_id: u32,
    pub abi_status: RadishLexStatusCode,
    pub action_summary_code: &'a str,
    pub command_status: &'a str,
    pub error_code: &'a str,
    pub retry_policy: &'a str,
    pub user_visible_summary_code: &'a str,
    pub diagnostics_summary_code: &'a str,
    pub next_required_evidence: &'a str,
    pub object_type_summary: &'a str,
    pub object_count_summary: u64,
    pub object_version_summary: &'a str,
    pub recorded_at_summary: &'a str,
}

impl<'a> ManagerSyncCommandResultViewDraft<'a> {
    fn from_result(result: &'a ManagerSyncCommandResultDraft) -> Self {
        Self {
            schema_version: result.schema_version,
            action_id: result.action_id,
            abi_status: result.abi_status,
            action_summary_code: result.envelope.action_summary_code,
            command_status: result.envelope.command_status,
            error_code: result.envelope.error_code,
            retry_policy: result.envelope.retry_policy,
            user_visible_summary_code: result.envelope.user_visible_summary_code,
            diagnostics_summary_code: result.envelope.diagnostics_summary_code,
            next_required_evidence: result.envelope.next_required_evidence,
            object_type_summary: result.envelope.object_type_summary,
            object_count_summary: result.envelope.object_count_summary,
            object_version_summary: result.envelope.object_version_summary,
            recorded_at_summary: result.envelope.recorded_at_summary,
        }
    }

    pub(crate) fn accessor_value(
        &self,
        field_name: &str,
    ) -> Result<ManagerSyncCommandResultAccessorValueDraft<'a>, FfiError> {
        match field_name {
            "schema_version" => Ok(ManagerSyncCommandResultAccessorValueDraft::U32(
                self.schema_version,
            )),
            "action_id" => Ok(ManagerSyncCommandResultAccessorValueDraft::U32(
                self.action_id,
            )),
            "command_status" => Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                self.command_status,
            )),
            "error_code" => Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                self.error_code,
            )),
            "retry_policy" => Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                self.retry_policy,
            )),
            "user_visible_summary_code" => {
                Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                    self.user_visible_summary_code,
                ))
            }
            "diagnostics_summary_code" => {
                Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                    self.diagnostics_summary_code,
                ))
            }
            "next_required_evidence" => {
                Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                    self.next_required_evidence,
                ))
            }
            "object_type_summary" => Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                self.object_type_summary,
            )),
            "object_count_summary" => Ok(ManagerSyncCommandResultAccessorValueDraft::U64(
                self.object_count_summary,
            )),
            "object_version_summary" => {
                Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                    self.object_version_summary,
                ))
            }
            "recorded_at_summary" => Ok(ManagerSyncCommandResultAccessorValueDraft::SummaryCode(
                self.recorded_at_summary,
            )),
            _ => Err(FfiError::invalid_argument(RESULT_ACCESSOR_FIELD_SET_ERROR)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandResultAccessorValueDraft<'a> {
    U32(u32),
    U64(u64),
    SummaryCode(&'a str),
}

impl ManagerSyncCommandResultAccessorValueDraft<'_> {
    pub(crate) fn assert_safe_summary(self) -> Result<(), FfiError> {
        if let Self::SummaryCode(value) = self {
            ensure_safe_summary_value(value, "manager_sync_result_accessor_value")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultHandleDraft {
    result: Option<ManagerSyncCommandResultDraft>,
}

impl ManagerSyncCommandResultHandleDraft {
    pub(crate) fn new(result: ManagerSyncCommandResultDraft) -> Result<Self, FfiError> {
        result.assert_safe_summary()?;
        Ok(Self {
            result: Some(result),
        })
    }

    pub(crate) fn view(&self) -> Result<ManagerSyncCommandResultViewDraft<'_>, FfiError> {
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| FfiError::invalid_state(RESULT_HANDLE_RELEASED_ERROR))?;
        Ok(ManagerSyncCommandResultViewDraft::from_result(result))
    }

    pub(crate) fn release(&mut self) {
        self.result = None;
    }
}

pub(crate) fn release_manager_sync_command_result_handle_draft(
    handle: Option<&mut ManagerSyncCommandResultHandleDraft>,
) {
    if let Some(handle) = handle {
        handle.release();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandErrorViewDraft<'a> {
    pub status_code: RadishLexStatusCode,
    pub message: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandErrorHandleDraft {
    error: Option<FfiError>,
}

impl ManagerSyncCommandErrorHandleDraft {
    pub(crate) fn new(error: FfiError) -> Result<Self, FfiError> {
        if contains_forbidden_material(&error.message) {
            return Err(FfiError::invalid_argument(ERROR_HANDLE_FORBIDDEN_MESSAGE));
        }
        Ok(Self { error: Some(error) })
    }

    pub(crate) fn view(&self) -> Result<ManagerSyncCommandErrorViewDraft<'_>, FfiError> {
        let error = self
            .error
            .as_ref()
            .ok_or_else(|| FfiError::invalid_state(ERROR_HANDLE_RELEASED_ERROR))?;
        Ok(ManagerSyncCommandErrorViewDraft {
            status_code: error.code,
            message: error.message.as_str(),
        })
    }

    pub(crate) fn release(&mut self) {
        self.error = None;
    }
}

pub(crate) fn release_manager_sync_command_error_handle_draft(
    handle: Option<&mut ManagerSyncCommandErrorHandleDraft>,
) {
    if let Some(handle) = handle {
        handle.release();
    }
}

#[derive(Debug)]
pub(crate) struct ManagerSyncCommandContextDraft {
    active_domains: Mutex<HashSet<&'static str>>,
    owner_thread: ThreadId,
}

impl ManagerSyncCommandContextDraft {
    pub(crate) fn new() -> Self {
        Self {
            active_domains: Mutex::new(HashSet::new()),
            owner_thread: thread::current().id(),
        }
    }

    fn ensure_owner_thread(&self) -> Result<(), FfiError> {
        if thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(FfiError::invalid_state(
                SYNC_COMMAND_CONTEXT_OWNER_MISMATCH_ERROR,
            ))
        }
    }

    pub(crate) fn enter_write_domain(
        &self,
    ) -> Result<ManagerSyncCommandDomainGuardDraft<'_>, FfiError> {
        self.ensure_owner_thread()?;
        let mut active_domains = self
            .active_domains
            .lock()
            .map_err(|_| FfiError::internal(SYNC_COMMAND_CONTEXT_LOCK_ERROR))?;

        if active_domains.contains(SYNC_COMMAND_DOMAIN_DRAFT) {
            return Err(FfiError::invalid_state(SYNC_COMMAND_DOMAIN_BUSY_ERROR));
        }

        active_domains.insert(SYNC_COMMAND_DOMAIN_DRAFT);
        Ok(ManagerSyncCommandDomainGuardDraft {
            context: self,
            domain: SYNC_COMMAND_DOMAIN_DRAFT,
        })
    }

    fn evaluate_current_phase(
        &self,
        raw: ManagerSyncCommandRequestRawDraft,
    ) -> Result<ManagerSyncCommandResultDraft, FfiError> {
        let request = ManagerSyncCommandRequestDraft::from_raw(raw)?;
        let _domain_guard = self.enter_write_domain()?;
        let result = ManagerSyncCommandResultDraft::current_phase_closed_with_section(
            request.action(),
            request.action_section(),
        );
        result.assert_safe_summary()?;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) struct ManagerSyncCommandDomainGuardDraft<'a> {
    context: &'a ManagerSyncCommandContextDraft,
    domain: &'static str,
}

impl Drop for ManagerSyncCommandDomainGuardDraft<'_> {
    fn drop(&mut self) {
        if let Ok(mut active_domains) = self.context.active_domains.lock() {
            active_domains.remove(self.domain);
        }
    }
}

pub(crate) fn execute_manager_sync_command_current_phase_with_context_draft(
    context: &ManagerSyncCommandContextDraft,
    raw: ManagerSyncCommandRequestRawDraft,
) -> Result<ManagerSyncCommandResultHandleDraft, FfiError> {
    capture_manager_sync_command_panic_boundary_draft(|| context.evaluate_current_phase(raw))
}

pub(crate) fn capture_manager_sync_command_panic_boundary_draft<F>(
    operation: F,
) -> Result<ManagerSyncCommandResultHandleDraft, FfiError>
where
    F: FnOnce() -> Result<ManagerSyncCommandResultDraft, FfiError>,
{
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(result)) => ManagerSyncCommandResultHandleDraft::new(result),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(FfiError::internal(PANIC_BOUNDARY_ERROR)),
    }
}

pub(crate) fn evaluate_manager_sync_command_current_phase_draft(
    raw: ManagerSyncCommandRequestRawDraft,
) -> Result<ManagerSyncCommandResultDraft, FfiError> {
    let request = ManagerSyncCommandRequestDraft::from_raw(raw)?;
    let result = ManagerSyncCommandResultDraft::current_phase_closed_with_section(
        request.action(),
        request.action_section(),
    );
    result.assert_safe_summary()?;
    Ok(result)
}

fn read_expected_action_section_code(
    view: RadishLexStringView,
    field: &'static str,
    expected: &'static str,
) -> Result<&'static str, FfiError> {
    let value = read_required_safe_utf8_view(view, field)?;
    if value != expected {
        return Err(FfiError::invalid_argument(ACTION_SECTION_VALUE_ERROR));
    }
    Ok(expected)
}

fn read_required_safe_utf8_view(
    view: RadishLexStringView,
    field: &'static str,
) -> Result<String, FfiError> {
    let value = read_utf8_view(view, field)?;
    if value.is_empty() {
        return Err(FfiError::invalid_argument(format!(
            "{field} cannot be empty"
        )));
    }
    ensure_safe_summary_value(value, field)?;
    Ok(value.to_owned())
}

fn read_utf8_view<'a>(view: RadishLexStringView, field: &'static str) -> Result<&'a str, FfiError> {
    if view.data.is_null() {
        if view.len == 0 {
            return Ok("");
        }
        return Err(FfiError::invalid_argument(format!(
            "{field} pointer is null with non-zero length"
        )));
    }

    let bytes = unsafe { slice::from_raw_parts(view.data, view.len) };
    str::from_utf8(bytes)
        .map_err(|_| FfiError::invalid_argument(format!("{field} must be valid UTF-8")))
}

fn read_ffi_bool(value: u8, field: &'static str) -> Result<bool, FfiError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(FfiError::invalid_argument(format!(
            "{field} must be 0 or 1"
        ))),
    }
}

fn ensure_safe_summary_value(value: &str, field: &'static str) -> Result<(), FfiError> {
    if contains_forbidden_material(value) {
        return Err(FfiError::invalid_argument(format!(
            "{field} contains forbidden material"
        )));
    }
    Ok(())
}

fn contains_forbidden_material(value: &str) -> bool {
    FORBIDDEN_SUMMARY_FRAGMENTS
        .iter()
        .any(|fragment| value.contains(fragment))
}

#[cfg(test)]
mod tests;
