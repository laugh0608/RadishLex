import 'package:radishlex_manager/src/models/manager_models.dart';

import 'sync_ffi_command_boundary_fixtures.dart';

const syncFfiRustHostContractReviewFormat =
    'future_manager_sync_ffi_rust_host_c_abi_contract_review.v1_draft';

const syncFfiRustHostContractReviewStatus =
    'c_abi_contract_review_ready_no_native_symbol';

const syncFfiRustHostImplementationReviewFormat =
    'future_manager_sync_ffi_rust_host_implementation_review.v1_draft';

const syncFfiRustHostImplementationReviewStatus =
    'rust_host_implementation_review_ready_no_native_symbol';

const syncFfiRustHostContractReviewDecisionRecordPath =
    'docs/adr/0006-manager-sync-c-abi-contract-governance.md';

const syncFfiRustHostContractReviewImplementationStatus =
    'implementation_review_ready_no_native_symbol';

const syncFfiRustHostInternalDraftEvidenceFormat =
    'future_manager_sync_ffi_internal_draft_evidence.v1_draft';

const syncFfiRustHostResultAccessorFieldSetReviewStatus =
    'result_accessor_field_set_review_ready_no_native_symbol';

const syncFfiRustHostSummaryStorageReviewStatus =
    'summary_storage_review_ready_no_native_symbol';

const syncFfiRustHostCommandContextOwnerScopeReviewStatus =
    'command_context_owner_scope_review_ready_no_native_symbol';

const syncFfiRustHostCommandWorkerThreadPolicyReviewStatus =
    'command_worker_thread_policy_review_ready_no_native_symbol';

const syncFfiRustHostDebugRedactionReviewStatus =
    'debug_redaction_test_shape_review_ready_no_native_symbol';

const syncFfiRustHostGateMigrationReviewStatus =
    'host_contract_gate_migration_review_ready_no_native_symbol';

const syncFfiRustHostGateReadinessReviewStatus =
    'host_gate_readiness_review_ready_no_native_symbol';

const syncFfiRustHostGateReadinessState = 'host_gate_blocked_no_native_symbol';

const syncFfiRustHostGateMigrationReplayStatus =
    'dart_fake_native_gate_migration_replay_ready_no_native_symbol';

const syncFfiRustHostResultAccessorFieldSetItems = [
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'schema_version',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'u32',
    storagePolicy: 'numeric_copy',
    numericWidth: 'u32',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'action_id',
    accessorGroup: 'result_action_id_accessor_group',
    valueKind: 'u32',
    storagePolicy: 'numeric_copy',
    numericWidth: 'u32',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'command_status',
    accessorGroup: 'result_status_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'error_code',
    accessorGroup: 'result_error_code_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'retry_policy',
    accessorGroup: 'result_retry_policy_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'user_visible_summary_code',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'diagnostics_summary_code',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'next_required_evidence',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'object_type_summary',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'object_count_summary',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'u64_count',
    storagePolicy: 'numeric_copy',
    numericWidth: 'u64',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'object_version_summary',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
  SyncFfiRustHostResultAccessorFieldSetItem(
    fieldName: 'recorded_at_summary',
    accessorGroup: 'result_summary_accessor_group',
    valueKind: 'summary_code',
    storagePolicy: 'handle_owned_or_static_summary',
    numericWidth: 'not_numeric',
  ),
];

const syncFfiRustHostSummaryStorageReview = SyncFfiRustHostSummaryStorageReview(
  currentPhaseStorage: 'static_summary_code_table',
  futureDynamicStorage: 'rust_owned_result_handle_storage',
  borrowedViewLifetime: 'valid_until_result_release',
  storesCallerPointer: false,
  storesProviderMessage: false,
  storesTransportPayload: false,
);

const syncFfiRustHostCommandContextOwnerScopeReview =
    SyncFfiRustHostCommandContextOwnerScopeReview(
      ownerScope: 'manager_sync_command_context_owner_thread',
      domainGuardStorageScope: 'rust_owned_context_mutex_active_domain_set',
      commandWorkerScope: 'not_started_current_phase',
      crossThreadStatus: 'InvalidState',
      ownsFlutterWidgetState: false,
      ownsSettingsPayload: false,
      ownsDartPointer: false,
      ownsPlatformUiObject: false,
    );

const syncFfiRustHostCommandWorkerThreadPolicyReview =
    SyncFfiRustHostCommandWorkerThreadPolicyReview(
      currentPhasePolicy: 'caller_thread_owner_checked_current_phase',
      futureWorkerPolicy: 'single_serial_manager_sync_worker_before_real_sync',
      ownerMigrationPolicy: 'must_be_reviewed_before_worker_enabled',
      allowsBackgroundRemoteRetry: false,
      queuesSecretPayload: false,
      blocksFlutterUiIsolate: false,
    );

const syncFfiRustHostDebugRedactionReview = SyncFfiRustHostDebugRedactionReview(
  debugTargets: [
    'request_error',
    'command_result',
    'result_accessor_field',
    'owner_scope_review',
    'worker_policy_review',
    'gate_migration_review',
    'host_gate_readiness_review',
  ],
  forbiddenCategories: [
    'token_material',
    'recovery_secret_material',
    'join_verifier_material',
    'signature_or_wrapped_sync_material',
    'opaque_transport_content',
    'local_path_material',
  ],
);

const syncFfiRustHostGateMigrationConditions = [
  'result_accessor_field_set_reviewed',
  'summary_storage_policy_reviewed',
  'object_summary_numeric_width_reviewed',
  'command_context_owner_scope_reviewed',
  'command_worker_thread_policy_reviewed',
  'debug_redaction_test_shape_reviewed',
  'forbidden_material_redaction_reviewed',
  'native_symbol_export_approved_by_adr',
  'dart_native_binding_approved',
  'manager_bridge_command_approved',
  'host_contract_test_file_approved',
  'ffi_smoke_candidate_symbols_absent_until_approval',
  'real_sync_execution_approved_after_gate',
];

const syncFfiRustHostGateReadinessReviewedConditions = [
  'result_accessor_field_set_reviewed',
  'summary_storage_policy_reviewed',
  'object_summary_numeric_width_reviewed',
  'command_context_owner_scope_reviewed',
  'command_worker_thread_policy_reviewed',
  'debug_redaction_test_shape_reviewed',
  'forbidden_material_redaction_reviewed',
  'ffi_smoke_candidate_symbols_absent_until_approval',
];

const syncFfiRustHostGateReadinessBlockingConditions = [
  'native_symbol_export_approved_by_adr',
  'dart_native_binding_approved',
  'manager_bridge_command_approved',
  'host_contract_test_file_approved',
  'real_sync_execution_approved_after_gate',
];

const syncFfiRustHostGateReadinessReview = SyncFfiRustHostGateReadinessReview(
  readinessState: syncFfiRustHostGateReadinessState,
  readyForHostContractTest: false,
  reviewedConditions: syncFfiRustHostGateReadinessReviewedConditions,
  blockingConditions: syncFfiRustHostGateReadinessBlockingConditions,
  dartFakeReplayStatus: syncFfiRustHostGateMigrationReplayStatus,
);

const syncFfiRustHostGateMigrationReplayCases = [
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'current_phase_all_gate_blockers',
    simulatedMissingConditions: [
      'native_symbol_export_approved_by_adr',
      'dart_native_binding_approved',
      'manager_bridge_command_approved',
      'host_contract_test_file_approved',
      'real_sync_execution_approved_after_gate',
    ],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'host_gate_blocked_current_phase',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'host_gate_blocked_current_phase',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'native_symbol_export_not_approved',
    simulatedMissingConditions: ['native_symbol_export_approved_by_adr'],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'native_symbol_export_not_approved',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'native_symbol_export_approved_by_adr',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'dart_native_binding_not_approved',
    simulatedMissingConditions: ['dart_native_binding_approved'],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'dart_native_binding_not_approved',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'dart_native_binding_approved',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'manager_bridge_command_not_approved',
    simulatedMissingConditions: ['manager_bridge_command_approved'],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'manager_bridge_command_not_approved',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'manager_bridge_command_approved',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'host_contract_test_file_not_approved',
    simulatedMissingConditions: ['host_contract_test_file_approved'],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'host_contract_test_file_not_approved',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'host_contract_test_file_approved',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'dynamic_library_smoke_missing',
    simulatedMissingConditions: [
      'ffi_smoke_candidate_symbols_absent_until_approval',
    ],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'ffi_smoke_candidate_symbols_absent_missing',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence:
        'ffi_smoke_candidate_symbols_absent_until_approval',
  ),
  SyncFfiRustHostGateMigrationReplayCase(
    id: 'real_sync_execution_not_approved',
    simulatedMissingConditions: ['real_sync_execution_approved_after_gate'],
    expectedReadinessState: syncFfiRustHostGateReadinessState,
    expectedBlockerCode: 'real_sync_execution_not_approved',
    expectedCommandStatus: 'blocked_by_readiness',
    expectedErrorCode: 'unexpected_bridge_error',
    expectedRetryPolicy: 'not_retryable',
    expectedNextRequiredEvidence: 'real_sync_execution_approved_after_gate',
  ),
];

const syncFfiRustHostContractReviewItems = [
  SyncFfiRustHostContractReviewItem(
    id: 'request_struct_layout_review',
    reviewScope: 'request_struct_layout',
    testDesignItemIds: [
      'contract_reports_command_capability_closed_test_design',
      'invalid_request_inputs_return_stable_status_test_design',
      'forbidden_material_absent_from_native_outputs_test_design',
    ],
    sourceChecklistItemIds: [
      'contract_reports_command_capability_closed_source_checklist',
      'invalid_request_inputs_return_stable_status_source_checklist',
      'forbidden_material_absent_from_native_outputs_source_checklist',
    ],
    requiredDecisions: [
      'schema_version_integer_repr',
      'action_id_integer_repr',
      'operation_id_borrowed_utf8_view',
      'readiness_snapshot_borrowed_utf8_view',
      'explicit_user_start_u8_bool',
      'action_sections_summary_only',
    ],
    requiredEvidence: [
      'unknown_schema_invalid_argument',
      'unknown_action_invalid_argument',
      'invalid_bool_invalid_argument',
      'borrowed_utf8_rejected_before_copy',
      'no_unchecked_input_enters_command',
    ],
    forbiddenOutputCategories: [
      'secret_material_echo',
      'transport_body_echo',
      'settings_action_material',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostContractReviewItem(
    id: 'result_struct_layout_review',
    reviewScope: 'result_struct_layout',
    testDesignItemIds: [
      'result_handle_copy_then_free_test_design',
      'envelope_allowlist_only_test_design',
      'forbidden_material_absent_from_native_outputs_test_design',
    ],
    sourceChecklistItemIds: [
      'result_handle_copy_then_free_source_checklist',
      'envelope_allowlist_only_source_checklist',
      'forbidden_material_absent_from_native_outputs_source_checklist',
    ],
    requiredDecisions: [
      'rust_owned_result_handle',
      'summary_accessor_views_only',
      'status_error_retry_evidence_allowlist',
      'object_summary_counts_only',
      'no_native_pointer_in_manager_state',
    ],
    requiredEvidence: [
      'rust_owned_result_handle',
      'borrowed_string_view_copied',
      'command_status_allowlist',
      'command_error_allowlist',
      'next_evidence_allowlist',
    ],
    forbiddenOutputCategories: [
      'stored_native_pointer',
      'raw_transport_content',
      'provider_debug_text',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostContractReviewItem(
    id: 'release_and_error_lifecycle_review',
    reviewScope: 'release_and_error_lifecycle',
    testDesignItemIds: [
      'invalid_request_inputs_return_stable_status_test_design',
      'result_handle_copy_then_free_test_design',
      'panic_boundary_returns_internal_error_test_design',
    ],
    sourceChecklistItemIds: [
      'invalid_request_inputs_return_stable_status_source_checklist',
      'result_handle_copy_then_free_source_checklist',
      'panic_boundary_returns_internal_error_source_checklist',
    ],
    requiredDecisions: [
      'result_free_null_noop',
      'error_read_then_free',
      'views_invalid_after_free',
      'release_path_safe_after_internal_error',
      'dart_copy_before_release_required',
    ],
    requiredEvidence: [
      'error_handle_read_then_free',
      'result_free_invalidates_views',
      'free_null_is_noop',
      'release_path_still_safe_after_error',
    ],
    forbiddenOutputCategories: [
      'result_handle_leak',
      'unreleased_error_handle',
      'dart_owned_rust_view',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostContractReviewItem(
    id: 'panic_and_status_boundary_review',
    reviewScope: 'panic_and_status_boundary',
    testDesignItemIds: [
      'invalid_request_inputs_return_stable_status_test_design',
      'envelope_allowlist_only_test_design',
      'panic_boundary_returns_internal_error_test_design',
    ],
    sourceChecklistItemIds: [
      'invalid_request_inputs_return_stable_status_source_checklist',
      'envelope_allowlist_only_source_checklist',
      'panic_boundary_returns_internal_error_source_checklist',
    ],
    requiredDecisions: [
      'catch_unwind_wraps_entry',
      'panic_maps_to_internal_error',
      'unknown_status_blocks_command',
      'sync_error_stays_structured',
    ],
    requiredEvidence: [
      'panic_caught',
      'internal_error_status',
      'no_panic_crosses_c_abi',
      'unexpected_code_blocks_command',
    ],
    forbiddenOutputCategories: [
      'panic_message_echo',
      'absolute_path_echo',
      'remote_retry_success_fallback',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostContractReviewItem(
    id: 'command_context_serialization_review',
    reviewScope: 'command_context_serialization',
    testDesignItemIds: [
      'contract_reports_command_capability_closed_test_design',
      'sync_domain_command_serialization_test_design',
    ],
    sourceChecklistItemIds: [
      'contract_reports_command_capability_closed_source_checklist',
      'sync_domain_command_serialization_source_checklist',
    ],
    requiredDecisions: [
      'current_phase_capability_gate_stays_first',
      'same_domain_write_guard',
      'operation_id_non_sensitive',
      'owner_context_invalid_state_pattern',
      'no_background_remote_retry',
    ],
    requiredEvidence: [
      'sync_command_capability_closed_current_phase',
      'same_domain_mutation_serialized',
      'concurrent_command_status_stable',
      'operation_id_non_sensitive',
    ],
    forbiddenOutputCategories: [
      'implicit_device_state_mutation',
      'ui_payload_queue',
      'background_upload_attempt',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostContractReviewItem(
    id: 'forbidden_material_contract_review',
    reviewScope: 'forbidden_material_contract',
    testDesignItemIds: [
      'invalid_request_inputs_return_stable_status_test_design',
      'envelope_allowlist_only_test_design',
      'forbidden_material_absent_from_native_outputs_test_design',
    ],
    sourceChecklistItemIds: [
      'invalid_request_inputs_return_stable_status_source_checklist',
      'forbidden_material_absent_from_native_outputs_source_checklist',
    ],
    requiredDecisions: [
      'token_material_absent',
      'recovery_secret_material_absent',
      'join_verifier_material_absent',
      'signature_material_absent',
      'wrapped_sync_material_absent',
      'real_path_absent',
    ],
    requiredEvidence: [
      'native_output_uses_safe_categories',
      'diagnostic_summary_redacted',
      'debug_summary_redacted',
      'no_secret_or_payload_in_error',
    ],
    forbiddenOutputCategories: [
      'token_material',
      'recovery_secret_material',
      'join_verifier_material',
      'signature_or_wrapped_sync_material',
      'opaque_transport_content',
      'local_path_material',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
];

const syncFfiRustHostImplementationReviewItems = [
  SyncFfiRustHostImplementationReviewItem(
    id: 'request_struct_layout_implementation_review',
    contractReviewItemId: 'request_struct_layout_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandRequestDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandActionDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncBorrowedViewDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/abi.rs::read_utf8',
      'crates/ime-ffi/src/abi.rs::read_ffi_bool',
      'crates/ime-ffi/src/abi.rs::ffi_status',
    ],
    requiredImplementationNotes: [
      'request_struct_repr_c_draft',
      'schema_version_rejected_before_context',
      'action_id_rejected_before_context',
      'borrowed_views_copied_before_async_boundary',
      'action_sections_summary_only',
    ],
    unresolvedImplementationQuestions: [
      'schema_version_numeric_values',
      'action_id_numeric_values',
      'action_specific_section_layout',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostImplementationReviewItem(
    id: 'result_struct_layout_implementation_review',
    contractReviewItemId: 'result_struct_layout_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandResultDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandResultViewDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandEnvelopeDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/abi.rs::ffi_ptr',
      'crates/ime-ffi/src/abi.rs::ffi_release',
      'crates/ime-ffi/src/sync_status.rs::RadishLexSyncPreflightSummary',
    ],
    requiredImplementationNotes: [
      'rust_owned_result_handle',
      'summary_view_accessors_only',
      'result_accessor_field_set_reviewed',
      'summary_storage_policy_reviewed',
      'command_status_error_retry_evidence_allowlist',
      'object_summary_counts_u64',
      'manager_state_never_stores_native_pointer',
    ],
    unresolvedImplementationQuestions: [
      'future_dynamic_summary_storage_owner_for_real_sync',
      'future_object_summary_count_source',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostImplementationReviewItem(
    id: 'release_and_error_lifecycle_implementation_review',
    contractReviewItemId: 'release_and_error_lifecycle_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandResultHandleDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ReleaseManagerSyncResultDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncErrorMappingDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/abi.rs::ffi_release',
      'crates/ime-ffi/src/abi.rs::radishlex_error_code',
      'crates/ime-ffi/src/abi.rs::radishlex_error_message',
      'crates/ime-ffi/src/abi.rs::radishlex_error_free',
    ],
    requiredImplementationNotes: [
      'result_free_null_noop',
      'error_handle_read_then_free',
      'views_invalid_after_free',
      'release_path_safe_after_internal_error',
      'dart_copy_before_release_required',
    ],
    unresolvedImplementationQuestions: [
      'result_handle_drop_order',
      'error_message_redaction_source',
      'double_release_test_shape',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostImplementationReviewItem(
    id: 'panic_and_status_boundary_implementation_review',
    contractReviewItemId: 'panic_and_status_boundary_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncEntryStatusDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncStatusMappingDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncPanicBoundaryDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/abi.rs::ffi_status',
      'crates/ime-ffi/src/abi.rs::ffi_ptr',
      'crates/ime-ffi/src/error.rs::RadishLexStatusCode::InternalError',
    ],
    requiredImplementationNotes: [
      'catch_unwind_wraps_entry',
      'panic_maps_to_internal_error',
      'unknown_status_blocks_command',
      'sync_error_stays_structured',
    ],
    unresolvedImplementationQuestions: [
      'sync_error_code_allowlist_source',
      'internal_error_summary_code',
      'unknown_status_dart_mapping_guard',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostImplementationReviewItem(
    id: 'command_context_serialization_implementation_review',
    contractReviewItemId: 'command_context_serialization_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCommandContextDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncDomainGuardDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncCapabilityGateDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/session.rs::RadishLexSession::ensure_owner_thread',
      'apps/radishlex-manager/tool/ffi_bridge_smoke.dart::_expectFutureSyncCommandSymbolsAbsent',
    ],
    requiredImplementationNotes: [
      'capability_gate_runs_before_operation',
      'same_domain_write_guard',
      'command_context_owner_scope_reviewed',
      'command_worker_thread_policy_reviewed',
      'operation_id_non_sensitive',
      'owner_context_invalid_state_pattern',
      'no_background_remote_retry',
    ],
    unresolvedImplementationQuestions: [
      'future_worker_queue_api_shape',
      'operation_id_uniqueness_scope',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
  SyncFfiRustHostImplementationReviewItem(
    id: 'forbidden_material_contract_implementation_review',
    contractReviewItemId: 'forbidden_material_contract_review',
    proposedRustArtifacts: [
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncForbiddenMaterialPolicyDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncSafeSummaryDraft',
      'crates/ime-ffi/src/manager_sync_command.rs::ManagerSyncDiagnosticsSummaryDraft',
    ],
    reusedSourceRefs: [
      'crates/ime-ffi/src/abi.rs::radishlex_error_message',
      'crates/ime-ffi/src/sync_status.rs::RadishLexSyncPreflightSummary',
      'apps/radishlex-manager/test/fixtures/sync_bridge_command_contract_fixtures.dart::syncBridgeCommandContractForbiddenFragments',
    ],
    requiredImplementationNotes: [
      'token_material_absent',
      'recovery_secret_material_absent',
      'join_verifier_material_absent',
      'signature_and_wrapped_material_absent',
      'transport_payload_and_real_path_absent',
      'debug_redaction_test_shape_reviewed',
    ],
    unresolvedImplementationQuestions: [
      'diagnostics_summary_code_allowlist_source',
      'forbidden_material_assertion_reuse_in_rust_test',
    ],
    implementationGuards: syncFfiCommandBoundaryRustHostReviewStopLines,
    implementationStatus: syncFfiRustHostContractReviewImplementationStatus,
  ),
];

class SyncFfiRustHostResultAccessorFieldSetItem {
  const SyncFfiRustHostResultAccessorFieldSetItem({
    required this.fieldName,
    required this.accessorGroup,
    required this.valueKind,
    required this.storagePolicy,
    required this.numericWidth,
  });

  final String fieldName;
  final String accessorGroup;
  final String valueKind;
  final String storagePolicy;
  final String numericWidth;
}

class SyncFfiRustHostSummaryStorageReview {
  const SyncFfiRustHostSummaryStorageReview({
    required this.currentPhaseStorage,
    required this.futureDynamicStorage,
    required this.borrowedViewLifetime,
    required this.storesCallerPointer,
    required this.storesProviderMessage,
    required this.storesTransportPayload,
  });

  final String currentPhaseStorage;
  final String futureDynamicStorage;
  final String borrowedViewLifetime;
  final bool storesCallerPointer;
  final bool storesProviderMessage;
  final bool storesTransportPayload;
}

class SyncFfiRustHostCommandContextOwnerScopeReview {
  const SyncFfiRustHostCommandContextOwnerScopeReview({
    required this.ownerScope,
    required this.domainGuardStorageScope,
    required this.commandWorkerScope,
    required this.crossThreadStatus,
    required this.ownsFlutterWidgetState,
    required this.ownsSettingsPayload,
    required this.ownsDartPointer,
    required this.ownsPlatformUiObject,
  });

  final String ownerScope;
  final String domainGuardStorageScope;
  final String commandWorkerScope;
  final String crossThreadStatus;
  final bool ownsFlutterWidgetState;
  final bool ownsSettingsPayload;
  final bool ownsDartPointer;
  final bool ownsPlatformUiObject;
}

class SyncFfiRustHostCommandWorkerThreadPolicyReview {
  const SyncFfiRustHostCommandWorkerThreadPolicyReview({
    required this.currentPhasePolicy,
    required this.futureWorkerPolicy,
    required this.ownerMigrationPolicy,
    required this.allowsBackgroundRemoteRetry,
    required this.queuesSecretPayload,
    required this.blocksFlutterUiIsolate,
  });

  final String currentPhasePolicy;
  final String futureWorkerPolicy;
  final String ownerMigrationPolicy;
  final bool allowsBackgroundRemoteRetry;
  final bool queuesSecretPayload;
  final bool blocksFlutterUiIsolate;
}

class SyncFfiRustHostDebugRedactionReview {
  const SyncFfiRustHostDebugRedactionReview({
    required this.debugTargets,
    required this.forbiddenCategories,
  });

  final List<String> debugTargets;
  final List<String> forbiddenCategories;
}

class SyncFfiRustHostGateReadinessReview {
  const SyncFfiRustHostGateReadinessReview({
    required this.readinessState,
    required this.readyForHostContractTest,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.dartFakeReplayStatus,
  });

  final String readinessState;
  final bool readyForHostContractTest;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final String dartFakeReplayStatus;
}

class SyncFfiRustHostGateMigrationReplayCase {
  const SyncFfiRustHostGateMigrationReplayCase({
    required this.id,
    required this.simulatedMissingConditions,
    required this.expectedReadinessState,
    required this.expectedBlockerCode,
    required this.expectedCommandStatus,
    required this.expectedErrorCode,
    required this.expectedRetryPolicy,
    required this.expectedNextRequiredEvidence,
  });

  final String id;
  final List<String> simulatedMissingConditions;
  final String expectedReadinessState;
  final String expectedBlockerCode;
  final String expectedCommandStatus;
  final String expectedErrorCode;
  final String expectedRetryPolicy;
  final String expectedNextRequiredEvidence;

  String get missingConditionSummary {
    return managerSyncCodeSummary(simulatedMissingConditions);
  }
}

class SyncFfiRustHostContractReviewItem {
  const SyncFfiRustHostContractReviewItem({
    required this.id,
    required this.reviewScope,
    required this.testDesignItemIds,
    required this.sourceChecklistItemIds,
    required this.requiredDecisions,
    required this.requiredEvidence,
    required this.forbiddenOutputCategories,
    required this.implementationGuards,
    required this.implementationStatus,
  });

  final String id;
  final String reviewScope;
  final List<String> testDesignItemIds;
  final List<String> sourceChecklistItemIds;
  final List<String> requiredDecisions;
  final List<String> requiredEvidence;
  final List<String> forbiddenOutputCategories;
  final List<String> implementationGuards;
  final String implementationStatus;

  String get testDesignSummary {
    return managerSyncCodeSummary(testDesignItemIds);
  }

  String get sourceChecklistSummary {
    return managerSyncCodeSummary(sourceChecklistItemIds);
  }

  String get decisionSummary {
    return managerSyncCodeSummary(requiredDecisions);
  }

  String get evidenceSummary {
    return managerSyncCodeSummary(requiredEvidence);
  }

  String get forbiddenOutputSummary {
    return managerSyncCodeSummary(forbiddenOutputCategories);
  }

  String get guardSummary {
    return managerSyncCodeSummary(implementationGuards);
  }
}

class SyncFfiRustHostImplementationReviewItem {
  const SyncFfiRustHostImplementationReviewItem({
    required this.id,
    required this.contractReviewItemId,
    required this.proposedRustArtifacts,
    required this.reusedSourceRefs,
    required this.requiredImplementationNotes,
    required this.unresolvedImplementationQuestions,
    required this.implementationGuards,
    required this.implementationStatus,
  });

  final String id;
  final String contractReviewItemId;
  final List<String> proposedRustArtifacts;
  final List<String> reusedSourceRefs;
  final List<String> requiredImplementationNotes;
  final List<String> unresolvedImplementationQuestions;
  final List<String> implementationGuards;
  final String implementationStatus;

  String get proposedArtifactSummary {
    return managerSyncCodeSummary(proposedRustArtifacts);
  }

  String get reusedSourceRefSummary {
    return managerSyncCodeSummary(reusedSourceRefs);
  }

  String get implementationNoteSummary {
    return managerSyncCodeSummary(requiredImplementationNotes);
  }

  String get unresolvedQuestionSummary {
    return managerSyncCodeSummary(unresolvedImplementationQuestions);
  }

  String get guardSummary {
    return managerSyncCodeSummary(implementationGuards);
  }
}

List<String> syncFfiRustHostContractReviewItemIds() {
  return List.unmodifiable(
    syncFfiRustHostContractReviewItems.map((fixture) => fixture.id).toList(),
  );
}

List<String> syncFfiRustHostImplementationReviewItemIds() {
  return List.unmodifiable(
    syncFfiRustHostImplementationReviewItems
        .map((fixture) => fixture.id)
        .toList(),
  );
}

Map<String, Object?> syncFfiRustHostContractReviewItemShape(
  SyncFfiRustHostContractReviewItem fixture,
) {
  return {
    'format': syncFfiRustHostContractReviewFormat,
    'review_status': syncFfiRustHostContractReviewStatus,
    'decision_record_path': syncFfiRustHostContractReviewDecisionRecordPath,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
    'id': fixture.id,
    'review_scope': fixture.reviewScope,
    'test_design_item_ids': fixture.testDesignItemIds,
    'source_checklist_item_ids': fixture.sourceChecklistItemIds,
    'required_decisions': fixture.requiredDecisions,
    'required_evidence': fixture.requiredEvidence,
    'forbidden_output_categories': fixture.forbiddenOutputCategories,
    'implementation_guards': fixture.implementationGuards,
    'implementation_status': fixture.implementationStatus,
  };
}

Map<String, Object?> syncFfiRustHostImplementationReviewItemShape(
  SyncFfiRustHostImplementationReviewItem fixture,
) {
  return {
    'format': syncFfiRustHostImplementationReviewFormat,
    'review_status': syncFfiRustHostImplementationReviewStatus,
    'decision_record_path': syncFfiRustHostContractReviewDecisionRecordPath,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
    'id': fixture.id,
    'contract_review_item_id': fixture.contractReviewItemId,
    'proposed_rust_artifacts': fixture.proposedRustArtifacts,
    'reused_source_refs': fixture.reusedSourceRefs,
    'required_implementation_notes': fixture.requiredImplementationNotes,
    'unresolved_implementation_questions':
        fixture.unresolvedImplementationQuestions,
    'implementation_guards': fixture.implementationGuards,
    'implementation_status': fixture.implementationStatus,
  };
}

Map<String, Object?> syncFfiRustHostResultAccessorFieldSetItemShape(
  SyncFfiRustHostResultAccessorFieldSetItem fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostResultAccessorFieldSetReviewStatus,
    'field_name': fixture.fieldName,
    'accessor_group': fixture.accessorGroup,
    'value_kind': fixture.valueKind,
    'storage_policy': fixture.storagePolicy,
    'numeric_width': fixture.numericWidth,
    'export_state': 'planned_not_exported_current_phase',
    'copy_required_before_release': true,
  };
}

Map<String, Object?> syncFfiRustHostSummaryStorageReviewShape(
  SyncFfiRustHostSummaryStorageReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostSummaryStorageReviewStatus,
    'current_phase_storage': fixture.currentPhaseStorage,
    'future_dynamic_storage': fixture.futureDynamicStorage,
    'borrowed_view_lifetime': fixture.borrowedViewLifetime,
    'stores_caller_pointer': fixture.storesCallerPointer,
    'stores_provider_message': fixture.storesProviderMessage,
    'stores_transport_payload': fixture.storesTransportPayload,
  };
}

Map<String, Object?> syncFfiRustHostCommandContextOwnerScopeReviewShape(
  SyncFfiRustHostCommandContextOwnerScopeReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostCommandContextOwnerScopeReviewStatus,
    'owner_scope': fixture.ownerScope,
    'domain_guard_storage_scope': fixture.domainGuardStorageScope,
    'command_worker_scope': fixture.commandWorkerScope,
    'cross_thread_status': fixture.crossThreadStatus,
    'owns_flutter_widget_state': fixture.ownsFlutterWidgetState,
    'owns_settings_payload': fixture.ownsSettingsPayload,
    'owns_dart_pointer': fixture.ownsDartPointer,
    'owns_platform_ui_object': fixture.ownsPlatformUiObject,
  };
}

Map<String, Object?> syncFfiRustHostCommandWorkerThreadPolicyReviewShape(
  SyncFfiRustHostCommandWorkerThreadPolicyReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostCommandWorkerThreadPolicyReviewStatus,
    'current_phase_policy': fixture.currentPhasePolicy,
    'future_worker_policy': fixture.futureWorkerPolicy,
    'owner_migration_policy': fixture.ownerMigrationPolicy,
    'allows_background_remote_retry': fixture.allowsBackgroundRemoteRetry,
    'queues_secret_payload': fixture.queuesSecretPayload,
    'blocks_flutter_ui_isolate': fixture.blocksFlutterUiIsolate,
  };
}

Map<String, Object?> syncFfiRustHostDebugRedactionReviewShape(
  SyncFfiRustHostDebugRedactionReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostDebugRedactionReviewStatus,
    'debug_targets': fixture.debugTargets,
    'forbidden_categories': fixture.forbiddenCategories,
  };
}

Map<String, Object?> syncFfiRustHostGateMigrationReviewShape() {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostGateMigrationReviewStatus,
    'ready_for_host_contract_test': false,
    'required_conditions': syncFfiRustHostGateMigrationConditions,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
  };
}

Map<String, Object?> syncFfiRustHostGateReadinessReviewShape(
  SyncFfiRustHostGateReadinessReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostGateReadinessReviewStatus,
    'readiness_state': fixture.readinessState,
    'ready_for_host_contract_test': fixture.readyForHostContractTest,
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'dart_fake_replay_status': fixture.dartFakeReplayStatus,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
  };
}

Map<String, Object?> syncFfiRustHostGateMigrationReplayCaseShape(
  SyncFfiRustHostGateMigrationReplayCase fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostGateMigrationReplayStatus,
    'id': fixture.id,
    'simulated_missing_conditions': fixture.simulatedMissingConditions,
    'expected_readiness_state': fixture.expectedReadinessState,
    'expected_blocker_code': fixture.expectedBlockerCode,
    'expected_command_status': fixture.expectedCommandStatus,
    'expected_error_code': fixture.expectedErrorCode,
    'expected_retry_policy': fixture.expectedRetryPolicy,
    'expected_next_required_evidence': fixture.expectedNextRequiredEvidence,
    'can_create_host_contract_test_file': false,
    'can_export_native_symbol': false,
    'can_modify_manager_bridge': false,
    'can_execute_real_sync': false,
  };
}
