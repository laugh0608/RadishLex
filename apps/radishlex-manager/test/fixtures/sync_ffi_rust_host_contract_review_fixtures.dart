import 'package:radishlex_manager/src/models/manager_models.dart';

import 'sync_ffi_command_boundary_fixtures.dart';

const syncFfiRustHostContractReviewFormat =
    'future_manager_sync_ffi_rust_host_c_abi_contract_review.v1_draft';

const syncFfiRustHostContractReviewStatus =
    'c_abi_contract_review_ready_no_native_symbol';

const syncFfiRustHostContractReviewImplementationStatus =
    'implementation_review_ready_no_native_symbol';

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

List<String> syncFfiRustHostContractReviewItemIds() {
  return List.unmodifiable(
    syncFfiRustHostContractReviewItems.map((fixture) => fixture.id).toList(),
  );
}

Map<String, Object?> syncFfiRustHostContractReviewItemShape(
  SyncFfiRustHostContractReviewItem fixture,
) {
  return {
    'format': syncFfiRustHostContractReviewFormat,
    'review_status': syncFfiRustHostContractReviewStatus,
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
