use super::admission::ManagerSyncCommandHostTestAdmissionReviewDraft;
use super::host_test_gate_review::{
    ManagerSyncCommandHostTestFileApprovalReviewDraft, ManagerSyncCommandRealSyncEvidenceItemDraft,
    ManagerSyncCommandRealSyncExecutionEvidenceBundleReviewDraft,
    ManagerSyncCommandRealSyncExecutionGateReviewDraft, HOST_TEST_FILE_APPROVAL_BLOCKED,
    HOST_TEST_FILE_APPROVAL_REVIEW_READY, REAL_SYNC_EVIDENCE_BUNDLE_BLOCKED,
    REAL_SYNC_EVIDENCE_BUNDLE_REVIEW_READY, REAL_SYNC_EXECUTION_BLOCKED,
    REAL_SYNC_EXECUTION_GATE_REVIEW_READY,
};
use super::migration_review::{
    ManagerSyncCommandDartBindingMigrationReviewDraft,
    ManagerSyncCommandHostExportApprovalReviewDraft,
    ManagerSyncCommandManagerBridgeMigrationReviewDraft, DART_BINDING_MIGRATION_BLOCKED,
    DART_BINDING_MIGRATION_REVIEW_READY, HOST_EXPORT_APPROVAL_BLOCKED,
    HOST_EXPORT_APPROVAL_REVIEW_READY, MANAGER_BRIDGE_MIGRATION_BLOCKED,
    MANAGER_BRIDGE_MIGRATION_REVIEW_READY,
};
use super::*;
use std::sync::Arc;

fn view(value: &str) -> RadishLexStringView {
    RadishLexStringView {
        data: value.as_ptr(),
        len: value.len(),
    }
}

fn bytes_view(bytes: &[u8]) -> RadishLexStringView {
    RadishLexStringView {
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn valid_action_section(action_id: u32) -> ManagerSyncActionSectionRawDraft {
    match action_id {
        MANAGER_SYNC_ACTION_RECOVERY_SETUP => ManagerSyncActionSectionRawDraft {
            section_id: MANAGER_SYNC_SECTION_RECOVERY_SETUP,
            confirmation_status_code: view(SETUP_CONFIRMATION_UNAVAILABLE),
            prerequisite_evidence_code: view(SETUP_REQUIRED_BEFORE_UPLOAD),
            transient_material_status_code: view(SETUP_DISPLAY_UNAVAILABLE),
            user_confirmation_present: 0,
        },
        MANAGER_SYNC_ACTION_RECOVERY_RESTORE => ManagerSyncActionSectionRawDraft {
            section_id: MANAGER_SYNC_SECTION_RECOVERY_RESTORE,
            confirmation_status_code: view(RESTORE_CONFIRMATION_UNAVAILABLE),
            prerequisite_evidence_code: view(RESTORE_RECORD_NOT_CHECKED),
            transient_material_status_code: view(RESTORE_INPUT_UNAVAILABLE),
            user_confirmation_present: 0,
        },
        MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION => ManagerSyncActionSectionRawDraft {
            section_id: MANAGER_SYNC_SECTION_JOIN_REQUEST_AUTHORIZATION,
            confirmation_status_code: view(JOIN_CONFIRMATION_UNAVAILABLE),
            prerequisite_evidence_code: view(JOIN_REQUEST_NOT_CREATED),
            transient_material_status_code: view(JOIN_SHORT_CODE_UNAVAILABLE),
            user_confirmation_present: 0,
        },
        MANAGER_SYNC_ACTION_DEVICE_REVOCATION => ManagerSyncActionSectionRawDraft {
            section_id: MANAGER_SYNC_SECTION_DEVICE_REVOCATION,
            confirmation_status_code: view(REVOCATION_CONFIRMATION_UNAVAILABLE),
            prerequisite_evidence_code: view(REVOCATION_ACTIVE_DEVICE_REQUIRED),
            transient_material_status_code: view(REVOCATION_KEY_EPOCH_NOT_ROTATED),
            user_confirmation_present: 0,
        },
        _ => valid_action_section(MANAGER_SYNC_ACTION_RECOVERY_SETUP),
    }
}

fn valid_raw(action_id: u32) -> ManagerSyncCommandRequestRawDraft {
    ManagerSyncCommandRequestRawDraft {
        schema_version: MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1,
        action_id,
        operation_id: view("op_test_non_secret_001"),
        readiness_snapshot_id: view("readiness_snapshot_test_001"),
        deployment_evidence_source_tag: view("local_smoke"),
        device_backend_gate: view("blocked"),
        explicit_user_start: 1,
        action_section: valid_action_section(action_id),
    }
}

fn current_manager_bridge_migration_review() -> ManagerSyncCommandManagerBridgeMigrationReviewDraft
{
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    let readiness =
        ManagerSyncCommandHostGateReadinessReviewDraft::current_phase(&migration).unwrap();
    let admission =
        ManagerSyncCommandHostTestAdmissionReviewDraft::current_phase(&readiness).unwrap();
    let export_review = ManagerSyncCommandHostExportApprovalReviewDraft::current_phase(
        &admission,
        &wrapper_review,
        &result_fields,
    )
    .unwrap();
    let binding_review =
        ManagerSyncCommandDartBindingMigrationReviewDraft::current_phase(&export_review).unwrap();
    ManagerSyncCommandManagerBridgeMigrationReviewDraft::current_phase(&binding_review).unwrap()
}

#[test]
fn rejects_unknown_schema_before_command_context() {
    let raw = ManagerSyncCommandRequestRawDraft {
        schema_version: 999,
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };

    let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

    assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(error.message, "unknown manager sync command schema version");
}

#[test]
fn rejects_unknown_action_before_command_context() {
    let raw = ManagerSyncCommandRequestRawDraft {
        action_id: 999,
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };

    let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

    assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(error.message, "unknown manager sync action id");
}

#[test]
fn rejects_invalid_bool_and_utf8_without_echoing_input() {
    let invalid_bool = ManagerSyncCommandRequestRawDraft {
        explicit_user_start: 7,
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let invalid_bool_error = ManagerSyncCommandRequestDraft::from_raw(invalid_bool).unwrap_err();
    assert_eq!(
        invalid_bool_error.code,
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        invalid_bool_error.message,
        "explicit_user_start must be 0 or 1"
    );

    let invalid_utf8_bytes = [0xff, 0xfe, 0xfd];
    let invalid_utf8 = ManagerSyncCommandRequestRawDraft {
        operation_id: bytes_view(&invalid_utf8_bytes),
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let invalid_utf8_error = ManagerSyncCommandRequestDraft::from_raw(invalid_utf8).unwrap_err();
    assert_eq!(
        invalid_utf8_error.code,
        RadishLexStatusCode::InvalidArgument
    );
    assert_eq!(
        invalid_utf8_error.message,
        "operation_id must be valid UTF-8"
    );
}

#[test]
fn current_phase_gate_returns_stable_invalid_state_summary() {
    let result = evaluate_manager_sync_command_current_phase_draft(valid_raw(
        MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION,
    ))
    .unwrap();

    assert_eq!(result.abi_status, RadishLexStatusCode::InvalidState);
    assert_eq!(
        result.envelope.action_summary_code,
        "join_request_authorization"
    );
    assert_eq!(result.envelope.command_status, "blocked_by_readiness");
    assert_eq!(result.envelope.error_code, CURRENT_PHASE_COMMAND_ERROR);
    assert_eq!(result.envelope.retry_policy, "not_retryable");
    assert_eq!(
        result.envelope.diagnostics_summary_code,
        CURRENT_PHASE_DIAGNOSTICS_SUMMARY
    );
    assert_eq!(
        result.envelope.next_required_evidence,
        JOIN_REQUEST_NOT_CREATED
    );
    assert_eq!(result.envelope.object_type_summary, "none");
    assert_eq!(result.envelope.object_count_summary, 0);
    result.assert_safe_summary().unwrap();
}

#[test]
fn action_sections_bind_each_action_to_safe_current_phase_codes() {
    let cases = [
        (
            MANAGER_SYNC_ACTION_RECOVERY_SETUP,
            "recovery_setup",
            SETUP_REQUIRED_BEFORE_UPLOAD,
        ),
        (
            MANAGER_SYNC_ACTION_RECOVERY_RESTORE,
            "recovery_restore",
            RESTORE_RECORD_NOT_CHECKED,
        ),
        (
            MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION,
            "join_request_authorization",
            JOIN_REQUEST_NOT_CREATED,
        ),
        (
            MANAGER_SYNC_ACTION_DEVICE_REVOCATION,
            "device_revocation",
            REVOCATION_ACTIVE_DEVICE_REQUIRED,
        ),
    ];

    for (action_id, action_summary, next_evidence) in cases {
        let result =
            evaluate_manager_sync_command_current_phase_draft(valid_raw(action_id)).unwrap();
        assert_eq!(result.envelope.action_summary_code, action_summary);
        assert_eq!(result.envelope.next_required_evidence, next_evidence);
        result.assert_safe_summary().unwrap();
    }
}

#[test]
fn rejects_action_section_mismatch_confirmation_and_forbidden_values() {
    let mismatch = ManagerSyncCommandRequestRawDraft {
        action_section: valid_action_section(MANAGER_SYNC_ACTION_DEVICE_REVOCATION),
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let mismatch_error = ManagerSyncCommandRequestDraft::from_raw(mismatch).unwrap_err();
    assert_eq!(mismatch_error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(mismatch_error.message, ACTION_SECTION_MISMATCH_ERROR);

    let confirmation_present = ManagerSyncCommandRequestRawDraft {
        action_section: ManagerSyncActionSectionRawDraft {
            user_confirmation_present: 1,
            ..valid_action_section(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        },
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let confirmation_error =
        ManagerSyncCommandRequestDraft::from_raw(confirmation_present).unwrap_err();
    assert_eq!(confirmation_error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(
        confirmation_error.message,
        ACTION_CONFIRMATION_UNAVAILABLE_ERROR
    );

    let wrong_code = ManagerSyncCommandRequestRawDraft {
        action_section: ManagerSyncActionSectionRawDraft {
            prerequisite_evidence_code: view("unexpected_safe_code"),
            ..valid_action_section(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        },
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let wrong_code_error = ManagerSyncCommandRequestDraft::from_raw(wrong_code).unwrap_err();
    assert_eq!(wrong_code_error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(wrong_code_error.message, ACTION_SECTION_VALUE_ERROR);

    let forbidden_code = ManagerSyncCommandRequestRawDraft {
        action_section: ManagerSyncActionSectionRawDraft {
            transient_material_status_code: view("short_code=123456"),
            ..valid_action_section(MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION)
        },
        ..valid_raw(MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION)
    };
    let forbidden_error = ManagerSyncCommandRequestDraft::from_raw(forbidden_code).unwrap_err();
    assert_eq!(forbidden_error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(
        forbidden_error.message,
        "transient_material_status_code contains forbidden material"
    );
    assert!(!forbidden_error.message.contains("123456"));
}

#[test]
fn result_handle_views_are_copyable_until_release() {
    let context = ManagerSyncCommandContextDraft::new();
    let mut handle = execute_manager_sync_command_current_phase_with_context_draft(
        &context,
        valid_raw(MANAGER_SYNC_ACTION_RECOVERY_RESTORE),
    )
    .unwrap();

    let view = handle.view().unwrap();
    assert_eq!(view.schema_version, MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1);
    assert_eq!(view.action_id, MANAGER_SYNC_ACTION_RECOVERY_RESTORE);
    assert_eq!(view.abi_status, RadishLexStatusCode::InvalidState);
    assert_eq!(view.action_summary_code, "recovery_restore");
    assert_eq!(view.command_status, "blocked_by_readiness");
    assert_eq!(view.error_code, CURRENT_PHASE_COMMAND_ERROR);
    assert_eq!(view.retry_policy, "not_retryable");
    assert_eq!(view.object_count_summary, 0);
    let copied_summary = view.user_visible_summary_code.to_owned();

    release_manager_sync_command_result_handle_draft(Some(&mut handle));
    let released_error = handle.view().unwrap_err();
    assert_eq!(released_error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(released_error.message, RESULT_HANDLE_RELEASED_ERROR);
    assert_eq!(copied_summary, CURRENT_PHASE_USER_SUMMARY);

    release_manager_sync_command_result_handle_draft(None);
}

#[test]
fn result_accessor_field_set_matches_safe_view_fields() {
    let review = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    review.assert_safe_field_set().unwrap();

    let context = ManagerSyncCommandContextDraft::new();
    let handle = execute_manager_sync_command_current_phase_with_context_draft(
        &context,
        valid_raw(MANAGER_SYNC_ACTION_DEVICE_REVOCATION),
    )
    .unwrap();
    let view = handle.view().unwrap();
    let field_names = review
        .fields
        .iter()
        .map(|field| field.field_name)
        .collect::<Vec<_>>();

    assert_eq!(
        field_names,
        vec![
            "schema_version",
            "action_id",
            "command_status",
            "error_code",
            "retry_policy",
            "user_visible_summary_code",
            "diagnostics_summary_code",
            "next_required_evidence",
            "object_type_summary",
            "object_count_summary",
            "object_version_summary",
            "recorded_at_summary",
        ]
    );
    assert!(review.fields.iter().all(|field| {
        field.current_export_state == C_ABI_SYMBOL_NOT_EXPORTED
            && field.copy_required_before_release
    }));
    assert!(review
        .fields
        .iter()
        .any(|field| field.accessor_symbol == "radishlex_manager_sync_command_result_action_id"));
    assert!(review
        .fields
        .iter()
        .any(|field| field.accessor_symbol == "radishlex_manager_sync_command_result_summary"));
    assert_eq!(
        review
            .fields
            .iter()
            .find(|field| field.field_name == "object_count_summary")
            .unwrap()
            .value_kind,
        "u64_count"
    );

    for field in review.fields {
        let value = view.accessor_value(field.field_name).unwrap();
        value.assert_safe_summary().unwrap();
        assert_eq!(field.current_export_state, C_ABI_SYMBOL_NOT_EXPORTED);
        for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
            let debug = format!("{field:?} {value:?}");
            assert!(!debug.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn summary_storage_review_keeps_dynamic_text_handle_owned() {
    let storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    storage.assert_safe_storage_policy().unwrap();

    assert_eq!(storage.review_state, SUMMARY_STORAGE_REVIEW_READY);
    assert_eq!(storage.current_phase_storage, "static_summary_code_table");
    assert_eq!(
        storage.future_dynamic_storage,
        "rust_owned_result_handle_storage"
    );
    assert_eq!(storage.borrowed_view_lifetime, "valid_until_result_release");
    assert!(!storage.stores_caller_pointer);
    assert!(!storage.stores_provider_message);
    assert!(!storage.stores_transport_payload);

    let debug = format!("{storage:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn error_handle_views_are_copyable_until_release() {
    let raw = ManagerSyncCommandRequestRawDraft {
        explicit_user_start: 7,
        ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
    };
    let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();
    let mut handle = ManagerSyncCommandErrorHandleDraft::new(error).unwrap();

    let view = handle.view().unwrap();
    assert_eq!(view.status_code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(view.message, "explicit_user_start must be 0 or 1");
    let copied_message = view.message.to_owned();

    release_manager_sync_command_error_handle_draft(Some(&mut handle));
    let released_error = handle.view().unwrap_err();
    assert_eq!(released_error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(released_error.message, ERROR_HANDLE_RELEASED_ERROR);
    assert_eq!(copied_message, "explicit_user_start must be 0 or 1");

    release_manager_sync_command_error_handle_draft(None);
}

#[test]
fn error_handle_rejects_forbidden_material_without_echoing_value() {
    let error = ManagerSyncCommandErrorHandleDraft::new(FfiError::invalid_argument(
        "provider returned secret-token",
    ))
    .unwrap_err();

    assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
    assert_eq!(error.message, ERROR_HANDLE_FORBIDDEN_MESSAGE);
    assert!(!error.message.contains("secret-token"));
}

#[test]
fn panic_boundary_maps_unwind_to_internal_error() {
    let error = capture_manager_sync_command_panic_boundary_draft(|| {
        panic!("synthetic manager sync panic with secret-token")
    })
    .unwrap_err();

    assert_eq!(error.code, RadishLexStatusCode::InternalError);
    assert_eq!(error.message, PANIC_BOUNDARY_ERROR);
    assert!(!error.message.contains("secret-token"));
}

#[test]
fn command_context_rejects_same_domain_overlap_and_releases_guard() {
    let context = ManagerSyncCommandContextDraft::new();
    let first_guard = context.enter_write_domain().unwrap();

    let busy_error = context.enter_write_domain().unwrap_err();
    assert_eq!(busy_error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(busy_error.message, SYNC_COMMAND_DOMAIN_BUSY_ERROR);

    drop(first_guard);
    let released_guard = context.enter_write_domain().unwrap();
    drop(released_guard);
}

#[test]
fn command_context_owner_scope_rejects_cross_thread_and_ui_ownership() {
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    owner_scope.assert_safe_owner_scope().unwrap();

    assert_eq!(
        owner_scope.review_state,
        COMMAND_CONTEXT_OWNER_SCOPE_REVIEW_READY
    );
    assert_eq!(
        owner_scope.domain_guard_storage_scope,
        "rust_owned_context_mutex_active_domain_set"
    );
    assert!(!owner_scope.owns_flutter_widget_state);
    assert!(!owner_scope.owns_settings_payload);
    assert!(!owner_scope.owns_dart_pointer);
    assert!(!owner_scope.owns_platform_ui_object);

    let context = Arc::new(ManagerSyncCommandContextDraft::new());
    let same_thread_guard = context.enter_write_domain().unwrap();
    drop(same_thread_guard);

    let cross_thread_context = Arc::clone(&context);
    let error = std::thread::spawn(move || cross_thread_context.enter_write_domain().unwrap_err())
        .join()
        .unwrap();
    assert_eq!(error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(error.message, SYNC_COMMAND_CONTEXT_OWNER_MISMATCH_ERROR);

    let debug = format!("{owner_scope:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn command_worker_policy_requires_serial_worker_before_real_sync() {
    let worker = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    worker.assert_safe_worker_policy().unwrap();

    assert_eq!(
        worker.review_state,
        COMMAND_WORKER_THREAD_POLICY_REVIEW_READY
    );
    assert_eq!(
        worker.current_phase_policy,
        "caller_thread_owner_checked_current_phase"
    );
    assert_eq!(
        worker.future_worker_policy,
        "single_serial_manager_sync_worker_before_real_sync"
    );
    assert_eq!(
        worker.owner_migration_policy,
        "must_be_reviewed_before_worker_enabled"
    );
    assert!(!worker.allows_background_remote_retry);
    assert!(!worker.queues_secret_payload);
    assert!(!worker.blocks_flutter_ui_isolate);

    let debug = format!("{worker:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn debug_redaction_review_covers_internal_draft_targets() {
    let redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    redaction.assert_safe_debug_redaction_shape().unwrap();

    assert_eq!(
        redaction.review_state,
        DEBUG_REDACTION_TEST_SHAPE_REVIEW_READY
    );
    assert!(redaction.debug_targets.contains(&"request_error"));
    assert!(redaction.debug_targets.contains(&"command_result"));
    assert!(redaction.debug_targets.contains(&"result_accessor_field"));
    assert!(redaction.debug_targets.contains(&"owner_scope_review"));
    assert!(redaction.debug_targets.contains(&"worker_policy_review"));
    assert!(redaction.debug_targets.contains(&"gate_migration_review"));
    assert!(redaction
        .debug_targets
        .contains(&"host_gate_readiness_review"));
    assert!(redaction
        .debug_targets
        .contains(&"real_sync_execution_evidence_bundle_review"));
    assert!(redaction.forbidden_categories.contains(&"token_material"));
    assert!(redaction
        .forbidden_categories
        .contains(&"recovery_secret_material"));
    assert!(redaction
        .forbidden_categories
        .contains(&"signature_or_wrapped_sync_material"));

    let debug = format!("{redaction:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn forbidden_material_is_rejected_without_echoing_value() {
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        let raw = ManagerSyncCommandRequestRawDraft {
            operation_id: view(forbidden),
            ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        };
        let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

        assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
        assert_eq!(error.message, "operation_id contains forbidden material");
        assert!(!error.message.contains(forbidden));
    }
}

#[test]
fn result_debug_does_not_include_request_fields_or_forbidden_material() {
    let result = evaluate_manager_sync_command_current_phase_draft(valid_raw(
        MANAGER_SYNC_ACTION_DEVICE_REVOCATION,
    ))
    .unwrap();
    let debug = format!("{result:?}");

    assert!(!debug.contains("op_test_non_secret_001"));
    assert!(!debug.contains("readiness_snapshot_test_001"));
    assert!(!debug.contains("local_smoke"));
    assert!(!debug.contains("blocked\""));
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn c_abi_wrapper_shape_review_lists_candidate_symbols_without_export_approval() {
    let review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    review.assert_current_phase_not_exportable().unwrap();

    assert_eq!(review.review_state, C_ABI_WRAPPER_REVIEW_READY);
    assert_eq!(
        review.executor_strategy,
        "single_versioned_manager_sync_command_executor"
    );
    assert!(!review.native_symbol_export_approved);
    assert!(!review.dart_native_binding_approved);
    assert!(!review.manager_bridge_command_approved);

    let symbol_names = review
        .candidate_symbols
        .iter()
        .map(|symbol| symbol.name)
        .collect::<Vec<_>>();
    assert_eq!(
        symbol_names,
        vec![
            "radishlex_manager_sync_command_execute_v1",
            "radishlex_manager_sync_command_result_action_id",
            "radishlex_manager_sync_command_result_status",
            "radishlex_manager_sync_command_result_error_code",
            "radishlex_manager_sync_command_result_retry_policy",
            "radishlex_manager_sync_command_result_summary",
            "radishlex_manager_sync_command_result_free",
        ]
    );
    assert!(review.candidate_symbols.iter().all(|symbol| {
        symbol.current_export_state == C_ABI_SYMBOL_NOT_EXPORTED && !symbol.export_approved
    }));
    assert!(review
        .existing_error_lifecycle_symbols
        .contains(&"radishlex_error_code"));
    assert!(review
        .existing_error_lifecycle_symbols
        .contains(&"radishlex_error_message"));
    assert!(review
        .existing_error_lifecycle_symbols
        .contains(&"radishlex_error_free"));
    assert!(review.candidate_symbols.iter().all(|symbol| !symbol
        .name
        .starts_with("radishlex_manager_sync_command_error")));

    let debug = format!("{review:?}");
    assert!(debug.contains("radishlex_manager_sync_command_execute_v1"));
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_contract_test_gate_stays_closed_until_native_symbol_is_approved() {
    let review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&review).unwrap();
    gate.assert_safe_gate_summary().unwrap();

    assert_eq!(gate.gate_state, HOST_CONTRACT_TEST_GATE_CLOSED);
    assert!(!gate.ready_for_host_contract_test);
    assert!(gate.blockers.contains(&"native_symbol_export_not_approved"));
    assert!(gate.blockers.contains(&"dart_native_binding_not_approved"));
    assert!(gate
        .blockers
        .contains(&"manager_bridge_command_not_approved"));
    assert!(gate
        .blockers
        .contains(&"host_contract_test_file_not_approved"));
    assert!(gate.blockers.contains(&"real_sync_execution_not_approved"));
    assert!(gate
        .required_evidence
        .contains(&"manager_sync_command_internal_draft_tests"));
    assert!(gate
        .required_evidence
        .contains(&"ffi_bridge_smoke_candidate_symbols_absent"));

    let debug = format!("{gate:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_contract_gate_migration_conditions_remain_review_only() {
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    migration.assert_safe_migration_conditions().unwrap();

    assert_eq!(
        migration.review_state,
        HOST_CONTRACT_GATE_MIGRATION_REVIEW_READY
    );
    assert!(!migration.ready_for_host_contract_test);
    assert!(migration
        .required_conditions
        .contains(&"result_accessor_field_set_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"summary_storage_policy_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"object_summary_numeric_width_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"command_context_owner_scope_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"command_worker_thread_policy_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"debug_redaction_test_shape_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"forbidden_material_redaction_reviewed"));
    assert!(migration
        .required_conditions
        .contains(&"native_symbol_export_approved_by_adr"));
    assert!(migration
        .required_conditions
        .contains(&"dart_native_binding_approved"));
    assert!(migration
        .required_conditions
        .contains(&"manager_bridge_command_approved"));
    assert!(migration
        .required_conditions
        .contains(&"host_contract_test_file_approved"));
    assert!(migration
        .required_conditions
        .contains(&"ffi_smoke_candidate_symbols_absent_until_approval"));
    assert!(migration
        .required_conditions
        .contains(&"real_sync_execution_approved_after_gate"));

    let debug = format!("{migration:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_gate_readiness_review_keeps_reviewed_evidence_separate_from_blockers() {
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    let readiness =
        ManagerSyncCommandHostGateReadinessReviewDraft::current_phase(&migration).unwrap();
    readiness.assert_safe_readiness().unwrap();

    assert_eq!(readiness.review_state, HOST_GATE_READINESS_REVIEW_READY);
    assert_eq!(
        readiness.readiness_state,
        HOST_GATE_BLOCKED_NO_NATIVE_SYMBOL
    );
    assert!(!readiness.ready_for_host_contract_test);
    assert_eq!(
        readiness.dart_fake_replay_status,
        HOST_GATE_READINESS_REPLAY_READY
    );
    assert!(readiness
        .reviewed_conditions
        .contains(&"result_accessor_field_set_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"summary_storage_policy_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"object_summary_numeric_width_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"command_context_owner_scope_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"command_worker_thread_policy_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"debug_redaction_test_shape_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"forbidden_material_redaction_reviewed"));
    assert!(readiness
        .reviewed_conditions
        .contains(&"ffi_smoke_candidate_symbols_absent_until_approval"));
    assert!(readiness
        .blocking_conditions
        .contains(&"native_symbol_export_approved_by_adr"));
    assert!(readiness
        .blocking_conditions
        .contains(&"dart_native_binding_approved"));
    assert!(readiness
        .blocking_conditions
        .contains(&"manager_bridge_command_approved"));
    assert!(readiness
        .blocking_conditions
        .contains(&"host_contract_test_file_approved"));
    assert!(readiness
        .blocking_conditions
        .contains(&"real_sync_execution_approved_after_gate"));
    for condition in readiness.reviewed_conditions {
        assert!(!readiness.blocking_conditions.contains(condition));
        assert!(migration.required_conditions.contains(condition));
    }
    for condition in readiness.blocking_conditions {
        assert!(migration.required_conditions.contains(condition));
    }

    let debug = format!("{readiness:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_test_admission_review_blocks_real_host_test_until_external_approvals() {
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    let readiness =
        ManagerSyncCommandHostGateReadinessReviewDraft::current_phase(&migration).unwrap();
    let admission =
        ManagerSyncCommandHostTestAdmissionReviewDraft::current_phase(&readiness).unwrap();
    admission.assert_safe_admission().unwrap();

    assert_eq!(admission.review_state, HOST_TEST_ADMISSION_REVIEW_READY);
    assert_eq!(admission.admission_decision, HOST_TEST_ADMISSION_BLOCKED);
    assert_eq!(
        admission.next_decision_required,
        HOST_TEST_ADMISSION_NEXT_DECISION
    );
    assert!(!admission.ready_for_host_contract_test);
    assert!(!admission.can_create_host_contract_test_file);
    assert!(!admission.can_export_native_symbol);
    assert!(!admission.can_modify_manager_bridge);
    assert!(!admission.can_execute_real_sync);
    assert_eq!(
        admission.satisfied_conditions,
        readiness.reviewed_conditions
    );
    assert_eq!(admission.blocking_conditions, readiness.blocking_conditions);
    assert!(admission
        .evidence_sources
        .contains(&"dart_fake_native_gate_migration_replay"));
    assert!(admission
        .evidence_sources
        .contains(&"ffi_bridge_smoke_candidate_symbols_absent"));
    for condition in admission.satisfied_conditions {
        assert!(!admission.blocking_conditions.contains(condition));
        assert!(migration.required_conditions.contains(condition));
    }
    for condition in admission.blocking_conditions {
        assert!(migration.required_conditions.contains(condition));
    }
    let mut accidental_host_test = admission;
    accidental_host_test.can_create_host_contract_test_file = true;
    assert!(accidental_host_test.assert_safe_admission().is_err());

    let debug = format!("{admission:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_export_approval_review_maps_candidate_symbols_without_exporting() {
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    let readiness =
        ManagerSyncCommandHostGateReadinessReviewDraft::current_phase(&migration).unwrap();
    let admission =
        ManagerSyncCommandHostTestAdmissionReviewDraft::current_phase(&readiness).unwrap();
    let export_review = ManagerSyncCommandHostExportApprovalReviewDraft::current_phase(
        &admission,
        &wrapper_review,
        &result_fields,
    )
    .unwrap();
    export_review.assert_safe_export_review().unwrap();

    assert_eq!(
        export_review.review_state,
        HOST_EXPORT_APPROVAL_REVIEW_READY
    );
    assert_eq!(export_review.export_decision, HOST_EXPORT_APPROVAL_BLOCKED);
    assert!(!export_review.can_export_native_symbol);
    assert!(!export_review.can_create_host_contract_test_file);
    assert!(!export_review.can_modify_dart_native_binding);
    assert!(!export_review.can_modify_manager_bridge);
    assert_eq!(
        export_review.symbol_reviews.len(),
        wrapper_review.candidate_symbols.len()
    );
    assert!(export_review
        .required_evidence
        .contains(&"ffi_bridge_smoke_candidate_symbols_absent"));
    assert!(export_review.symbol_reviews.iter().any(|symbol| {
        symbol.symbol_name == "radishlex_manager_sync_command_execute_v1"
            && symbol.symbol_role == "command_executor"
            && symbol.required_contract_fields.contains(&"command_status")
            && symbol.panic_boundary == "catch_unwind_maps_internal_error"
    }));
    assert!(export_review.symbol_reviews.iter().any(|symbol| {
        symbol.symbol_name == "radishlex_manager_sync_command_result_free"
            && symbol.symbol_role == "result_release"
            && symbol.release_responsibility
                == "free_null_noop_and_double_release_invalidates_views"
    }));
    for symbol in export_review.symbol_reviews {
        assert_eq!(symbol.current_export_state, C_ABI_SYMBOL_NOT_EXPORTED);
        assert!(!symbol.export_approved);
        assert_eq!(
            symbol.smoke_requirement,
            "symbol_absent_until_export_approval"
        );
        if symbol.symbol_role == "result_accessor" {
            for field in symbol.required_contract_fields {
                assert!(result_fields
                    .fields
                    .iter()
                    .any(|result_field| result_field.field_name == *field));
            }
        }
    }

    let mut accidental_export = export_review;
    accidental_export.can_export_native_symbol = true;
    assert!(accidental_export.assert_safe_export_review().is_err());

    let debug = format!("{export_review:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn dart_binding_and_manager_bridge_migration_reviews_keep_blockers_closed() {
    let wrapper_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase();
    let gate = ManagerSyncCommandHostContractTestGateDraft::current_phase(&wrapper_review).unwrap();
    let result_fields = ManagerSyncCommandResultAccessorFieldSetReviewDraft::current_phase();
    let owner_scope = ManagerSyncCommandContextOwnerScopeReviewDraft::current_phase();
    let summary_storage = ManagerSyncCommandSummaryStorageReviewDraft::current_phase();
    let worker_policy = ManagerSyncCommandWorkerThreadPolicyReviewDraft::current_phase();
    let debug_redaction = ManagerSyncCommandDebugRedactionReviewDraft::current_phase();
    let migration = ManagerSyncCommandHostGateMigrationReviewDraft::current_phase(
        &gate,
        &result_fields,
        &owner_scope,
        &summary_storage,
        &worker_policy,
        &debug_redaction,
    )
    .unwrap();
    let readiness =
        ManagerSyncCommandHostGateReadinessReviewDraft::current_phase(&migration).unwrap();
    let admission =
        ManagerSyncCommandHostTestAdmissionReviewDraft::current_phase(&readiness).unwrap();
    let export_review = ManagerSyncCommandHostExportApprovalReviewDraft::current_phase(
        &admission,
        &wrapper_review,
        &result_fields,
    )
    .unwrap();
    let binding_review =
        ManagerSyncCommandDartBindingMigrationReviewDraft::current_phase(&export_review).unwrap();
    binding_review.assert_safe_binding_migration().unwrap();
    let bridge_review =
        ManagerSyncCommandManagerBridgeMigrationReviewDraft::current_phase(&binding_review)
            .unwrap();
    bridge_review.assert_safe_bridge_migration().unwrap();

    assert_eq!(
        binding_review.review_state,
        DART_BINDING_MIGRATION_REVIEW_READY
    );
    assert_eq!(
        binding_review.migration_decision,
        DART_BINDING_MIGRATION_BLOCKED
    );
    assert!(binding_review
        .reviewed_conditions
        .contains(&"dart_copy_free_contract_reviewed"));
    assert!(binding_review
        .reviewed_conditions
        .contains(&"unknown_native_status_mapping_reviewed"));
    assert!(binding_review
        .reviewed_conditions
        .contains(&"ffi_command_error_mapping_reviewed"));
    assert!(binding_review
        .reviewed_conditions
        .contains(&"forbidden_material_redaction_reviewed"));
    assert!(binding_review
        .blocking_conditions
        .contains(&"native_symbol_export_approved_by_adr"));
    assert!(binding_review
        .blocking_conditions
        .contains(&"dart_native_binding_approved"));
    assert!(binding_review
        .blocking_conditions
        .contains(&"manager_bridge_command_approved"));
    assert!(!binding_review.can_modify_dart_native_binding);
    assert!(!binding_review.can_call_native_command);
    assert!(!binding_review.can_hold_native_pointer_after_copy);
    assert!(!binding_review.can_write_settings_action);
    assert!(!binding_review.can_modify_manager_bridge);

    assert_eq!(
        bridge_review.review_state,
        MANAGER_BRIDGE_MIGRATION_REVIEW_READY
    );
    assert_eq!(
        bridge_review.migration_decision,
        MANAGER_BRIDGE_MIGRATION_BLOCKED
    );
    assert!(bridge_review
        .reviewed_conditions
        .contains(&"action_intent_mapping_reviewed"));
    assert!(bridge_review
        .reviewed_conditions
        .contains(&"settings_draft_write_absent_reviewed"));
    assert!(bridge_review
        .blocking_conditions
        .contains(&"manager_bridge_command_approved"));
    assert!(bridge_review
        .blocking_conditions
        .contains(&"host_contract_test_file_approved"));
    assert!(bridge_review
        .blocking_conditions
        .contains(&"real_sync_execution_approved_after_gate"));
    assert!(!bridge_review.can_modify_manager_bridge);
    assert!(!bridge_review.can_create_bridge_command_request);
    assert!(!bridge_review.can_write_settings_action);
    assert!(!bridge_review.can_execute_real_sync);

    let mut accidental_binding = binding_review;
    accidental_binding.can_call_native_command = true;
    assert!(accidental_binding.assert_safe_binding_migration().is_err());
    let mut accidental_bridge = bridge_review;
    accidental_bridge.can_modify_manager_bridge = true;
    assert!(accidental_bridge.assert_safe_bridge_migration().is_err());

    let debug = format!("{binding_review:?}{bridge_review:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn host_test_file_approval_review_keeps_real_test_file_uncreated() {
    let bridge_review = current_manager_bridge_migration_review();
    let host_file_review =
        ManagerSyncCommandHostTestFileApprovalReviewDraft::current_phase(&bridge_review).unwrap();
    host_file_review.assert_safe_file_approval().unwrap();

    assert_eq!(
        host_file_review.review_state,
        HOST_TEST_FILE_APPROVAL_REVIEW_READY
    );
    assert_eq!(
        host_file_review.approval_decision,
        HOST_TEST_FILE_APPROVAL_BLOCKED
    );
    assert_eq!(
        host_file_review.target_test_file,
        "crates/ime-ffi/tests/manager_sync_command_boundary.rs"
    );
    assert!(host_file_review
        .planned_test_cases
        .contains(&"contract_reports_command_capability_closed"));
    assert!(host_file_review
        .planned_test_cases
        .contains(&"result_handle_copy_then_free"));
    assert!(host_file_review
        .planned_test_cases
        .contains(&"forbidden_material_absent_from_native_outputs"));
    assert!(host_file_review
        .reviewed_conditions
        .contains(&"c_abi_symbol_lookup_strategy_reviewed"));
    assert!(host_file_review
        .reviewed_conditions
        .contains(&"dynamic_library_smoke_absence_reviewed"));
    assert!(host_file_review
        .blocking_conditions
        .contains(&"host_contract_test_file_approved"));
    assert!(host_file_review
        .required_evidence
        .contains(&"host_test_design_package_current"));
    assert!(host_file_review
        .required_evidence
        .contains(&"ffi_bridge_smoke_candidate_symbols_absent"));
    assert!(!host_file_review.can_create_host_contract_test_file);
    assert!(!host_file_review.can_export_native_symbol);
    assert!(!host_file_review.can_call_dynamic_library_symbol);
    assert!(!host_file_review.can_modify_dart_native_binding);

    let mut accidental_file = host_file_review;
    accidental_file.can_create_host_contract_test_file = true;
    assert!(accidental_file.assert_safe_file_approval().is_err());

    let debug = format!("{host_file_review:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn real_sync_execution_gate_review_blocks_remote_side_effects() {
    let bridge_review = current_manager_bridge_migration_review();
    let host_file_review =
        ManagerSyncCommandHostTestFileApprovalReviewDraft::current_phase(&bridge_review).unwrap();
    let execution_gate =
        ManagerSyncCommandRealSyncExecutionGateReviewDraft::current_phase(&host_file_review)
            .unwrap();
    execution_gate.assert_safe_execution_gate().unwrap();

    assert_eq!(
        execution_gate.review_state,
        REAL_SYNC_EXECUTION_GATE_REVIEW_READY
    );
    assert_eq!(
        execution_gate.execution_decision,
        REAL_SYNC_EXECUTION_BLOCKED
    );
    assert!(execution_gate
        .reviewed_conditions
        .contains(&"command_context_owner_scope_reviewed"));
    assert!(execution_gate
        .reviewed_conditions
        .contains(&"readiness_snapshot_binding_reviewed"));
    assert!(execution_gate
        .blocking_conditions
        .contains(&"platform_private_key_backend_production_ready"));
    assert!(execution_gate
        .blocking_conditions
        .contains(&"recovery_authorization_interaction_tests_passed"));
    assert!(execution_gate
        .blocking_conditions
        .contains(&"deployment_evidence_summary_approved"));
    assert!(execution_gate
        .blocking_conditions
        .contains(&"real_sync_execution_approved_after_gate"));
    assert!(execution_gate
        .required_evidence
        .contains(&"platform_private_key_backend_strategy_current"));
    assert!(execution_gate
        .required_evidence
        .contains(&"sync_server_production_deployment_runbook_current"));
    assert!(!execution_gate.can_execute_real_sync);
    assert!(!execution_gate.can_connect_go_server);
    assert!(!execution_gate.can_touch_platform_key_backend);
    assert!(!execution_gate.can_generate_recovery_code);
    assert!(!execution_gate.can_create_join_request);
    assert!(!execution_gate.can_revoke_device);

    let mut accidental_execution = execution_gate;
    accidental_execution.can_execute_real_sync = true;
    assert!(accidental_execution.assert_safe_execution_gate().is_err());

    let debug = format!("{execution_gate:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn real_sync_execution_evidence_bundle_review_keeps_release_evidence_blocking() {
    let bridge_review = current_manager_bridge_migration_review();
    let host_file_review =
        ManagerSyncCommandHostTestFileApprovalReviewDraft::current_phase(&bridge_review).unwrap();
    let execution_gate =
        ManagerSyncCommandRealSyncExecutionGateReviewDraft::current_phase(&host_file_review)
            .unwrap();
    let evidence_bundle =
        ManagerSyncCommandRealSyncExecutionEvidenceBundleReviewDraft::current_phase(
            &execution_gate,
        )
        .unwrap();
    evidence_bundle.assert_safe_evidence_bundle().unwrap();

    assert_eq!(
        evidence_bundle.review_state,
        REAL_SYNC_EVIDENCE_BUNDLE_REVIEW_READY
    );
    assert_eq!(
        evidence_bundle.bundle_decision,
        REAL_SYNC_EVIDENCE_BUNDLE_BLOCKED
    );
    assert_eq!(evidence_bundle.evidence_items.len(), 3);
    assert!(evidence_bundle.evidence_items.iter().any(|item| {
        item.evidence_id == "platform_private_key_backend"
            && item.current_state == "production_backend_not_ready_current_phase"
            && item.blocking_condition == "platform_private_key_backend_production_ready"
    }));
    assert!(evidence_bundle.evidence_items.iter().any(|item| {
        item.evidence_id == "recovery_authorization_interaction"
            && item.current_state == "interaction_tests_not_passed_current_phase"
            && item.blocking_condition == "recovery_authorization_interaction_tests_passed"
    }));
    assert!(evidence_bundle.evidence_items.iter().any(|item| {
        item.evidence_id == "deployment_evidence_summary"
            && item.current_state == "release_deployment_evidence_missing_current_phase"
            && item.safe_summary == "deployment_evidence_summary_v1_required_local_smoke_only"
    }));
    assert!(evidence_bundle
        .reviewed_conditions
        .contains(&"local_smoke_kept_development_only"));
    assert!(evidence_bundle
        .blocking_conditions
        .contains(&"release_evidence_bundle_approved_after_gate"));
    assert!(evidence_bundle
        .required_evidence
        .contains(&"deployment_evidence_summary_v1_release_target"));
    assert!(evidence_bundle
        .required_evidence
        .contains(&"real_sync_execution_evidence_bundle_replay"));
    assert!(!evidence_bundle.can_mark_platform_backend_ready);
    assert!(!evidence_bundle.can_mark_recovery_authorization_ready);
    assert!(!evidence_bundle.can_mark_deployment_summary_ready);
    assert!(!evidence_bundle.can_unlock_real_sync);
    assert!(!evidence_bundle.can_execute_remote_call);
    assert!(!evidence_bundle.can_persist_secret_material);

    let mut accidental_bundle = evidence_bundle;
    accidental_bundle.can_unlock_real_sync = true;
    assert!(accidental_bundle.assert_safe_evidence_bundle().is_err());

    let mut items = evidence_bundle.evidence_items.to_vec();
    items[0] = ManagerSyncCommandRealSyncEvidenceItemDraft {
        can_unlock_real_sync: true,
        ..items[0]
    };
    let leaked_items = Box::leak(items.into_boxed_slice());
    let accidental_item_bundle = ManagerSyncCommandRealSyncExecutionEvidenceBundleReviewDraft {
        evidence_items: leaked_items,
        ..evidence_bundle
    };
    assert!(accidental_item_bundle
        .assert_safe_evidence_bundle()
        .is_err());

    let debug = format!("{evidence_bundle:?}");
    for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
        assert!(!debug.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn c_abi_wrapper_shape_review_rejects_accidental_export_approval() {
    let mut candidate_symbols = MANAGER_SYNC_COMMAND_C_ABI_SYMBOLS_DRAFT
        .iter()
        .copied()
        .collect::<Vec<_>>();
    candidate_symbols[0].export_approved = true;
    let leaked_review = ManagerSyncCommandCAbiWrapperShapeReviewDraft {
        candidate_symbols: candidate_symbols.leak(),
        ..ManagerSyncCommandCAbiWrapperShapeReviewDraft::current_phase()
    };

    let error = leaked_review
        .assert_current_phase_not_exportable()
        .unwrap_err();
    assert_eq!(error.code, RadishLexStatusCode::InvalidState);
    assert_eq!(error.message, C_ABI_SYMBOL_EXPORT_UNAPPROVED_ERROR);
}
