use super::*;

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
