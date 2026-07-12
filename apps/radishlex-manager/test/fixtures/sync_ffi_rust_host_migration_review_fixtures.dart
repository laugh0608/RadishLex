import 'sync_ffi_command_boundary_fixtures.dart';
import 'sync_ffi_rust_host_contract_review_fixtures.dart';

const syncFfiRustHostExportApprovalReviewStatus =
    'native_symbol_export_approval_review_ready_no_native_symbol';

const syncFfiRustHostExportApprovalDecision =
    'native_symbol_export_blocked_until_adr_and_smoke_approval';

const syncFfiRustHostDartBindingMigrationReviewStatus =
    'dart_binding_migration_review_ready_no_native_symbol';

const syncFfiRustHostDartBindingMigrationDecision =
    'dart_binding_migration_blocked_until_native_export';

const syncFfiRustHostManagerBridgeMigrationReviewStatus =
    'manager_bridge_migration_review_ready_no_native_symbol';

const syncFfiRustHostManagerBridgeMigrationDecision =
    'manager_bridge_migration_blocked_until_binding_contract';

const syncFfiRustHostTestFileApprovalReviewStatus =
    'host_contract_test_file_approval_review_ready_no_native_symbol';

const syncFfiRustHostTestFileApprovalDecision =
    'host_contract_test_file_blocked_until_symbol_and_binding_approval';

const syncFfiRustHostRealSyncExecutionGateReviewStatus =
    'real_sync_execution_gate_review_ready_no_remote_sync';

const syncFfiRustHostRealSyncExecutionDecision =
    'real_sync_execution_blocked_until_platform_and_deployment_evidence';

const syncFfiRustHostRealSyncEvidenceBundleReviewStatus =
    'real_sync_execution_evidence_bundle_review_ready_no_remote_sync';

const syncFfiRustHostRealSyncEvidenceBundleDecision =
    'real_sync_execution_evidence_bundle_blocked_until_release_evidence';

const syncFfiRustHostExportApprovalRequiredEvidence = [
  'adr_0006_current',
  'c_abi_wrapper_shape_review',
  'result_accessor_field_set_review',
  'result_handle_copy_then_free_review',
  'panic_boundary_review',
  'ffi_bridge_smoke_candidate_symbols_absent',
];

const syncFfiRustHostExportSymbolReviewItems = [
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_execute_v1',
    symbolRole: 'command_executor',
    requiredContractFields: [
      'schema_version',
      'action_id',
      'command_status',
      'error_code',
      'retry_policy',
      'next_required_evidence',
    ],
    releaseResponsibility: 'returns_rust_owned_result_or_error_handle',
    errorBoundary: 'ffi_status_invalid_argument_invalid_state_sync_internal',
    panicBoundary: 'catch_unwind_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_action_id',
    symbolRole: 'result_accessor',
    requiredContractFields: ['action_id'],
    releaseResponsibility: 'caller_copies_before_result_free',
    errorBoundary: 'released_handle_returns_invalid_state',
    panicBoundary: 'accessor_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_status',
    symbolRole: 'result_accessor',
    requiredContractFields: ['command_status'],
    releaseResponsibility: 'caller_copies_before_result_free',
    errorBoundary: 'released_handle_returns_invalid_state',
    panicBoundary: 'accessor_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_error_code',
    symbolRole: 'result_accessor',
    requiredContractFields: ['error_code'],
    releaseResponsibility: 'caller_copies_before_result_free',
    errorBoundary: 'released_handle_returns_invalid_state',
    panicBoundary: 'accessor_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_retry_policy',
    symbolRole: 'result_accessor',
    requiredContractFields: ['retry_policy'],
    releaseResponsibility: 'caller_copies_before_result_free',
    errorBoundary: 'released_handle_returns_invalid_state',
    panicBoundary: 'accessor_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_summary',
    symbolRole: 'result_accessor',
    requiredContractFields: [
      'user_visible_summary_code',
      'diagnostics_summary_code',
      'next_required_evidence',
      'object_type_summary',
      'object_count_summary',
      'object_version_summary',
      'recorded_at_summary',
    ],
    releaseResponsibility: 'caller_copies_before_result_free',
    errorBoundary: 'released_handle_returns_invalid_state',
    panicBoundary: 'accessor_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
  SyncFfiRustHostExportSymbolReviewItem(
    symbolName: 'radishlex_manager_sync_command_result_free',
    symbolRole: 'result_release',
    requiredContractFields: ['result_handle_lifecycle'],
    releaseResponsibility:
        'free_null_noop_and_double_release_invalidates_views',
    errorBoundary: 'release_never_exposes_provider_message',
    panicBoundary: 'release_panic_maps_internal_error',
    smokeRequirement: 'symbol_absent_until_export_approval',
  ),
];

const syncFfiRustHostExportApprovalReview = SyncFfiRustHostExportApprovalReview(
  exportDecision: syncFfiRustHostExportApprovalDecision,
  symbolReviews: syncFfiRustHostExportSymbolReviewItems,
  requiredEvidence: syncFfiRustHostExportApprovalRequiredEvidence,
);

const syncFfiRustHostDartBindingMigrationReviewedConditions = [
  'dart_copy_free_contract_reviewed',
  'unknown_native_status_mapping_reviewed',
  'ffi_command_error_mapping_reviewed',
  'forbidden_material_redaction_reviewed',
];

const syncFfiRustHostDartBindingMigrationBlockingConditions = [
  'native_symbol_export_approved_by_adr',
  'dart_native_binding_approved',
  'manager_bridge_command_approved',
];

const syncFfiRustHostDartBindingMigrationRequiredEvidence = [
  'fake_native_binding_replay',
  'copy_free_call_order_test',
  'unknown_status_safe_downgrade_test',
  'ffi_error_mapping_allowlist_test',
  'forbidden_material_not_visible_to_manager_test',
];

const syncFfiRustHostDartBindingMigrationReview =
    SyncFfiRustHostDartBindingMigrationReview(
      migrationDecision: syncFfiRustHostDartBindingMigrationDecision,
      reviewedConditions: syncFfiRustHostDartBindingMigrationReviewedConditions,
      blockingConditions: syncFfiRustHostDartBindingMigrationBlockingConditions,
      requiredEvidence: syncFfiRustHostDartBindingMigrationRequiredEvidence,
    );

const syncFfiRustHostManagerBridgeMigrationReviewedConditions = [
  'action_intent_mapping_reviewed',
  'settings_draft_write_absent_reviewed',
  'diagnostics_redaction_reviewed',
];

const syncFfiRustHostManagerBridgeMigrationBlockingConditions = [
  'manager_bridge_command_approved',
  'host_contract_test_file_approved',
  'real_sync_execution_approved_after_gate',
];

const syncFfiRustHostManagerBridgeMigrationRequiredEvidence = [
  'manager_sync_bridge_contract_current',
  'manager_sync_action_preview_current',
  'settings_diagnostics_redaction_current',
];

const syncFfiRustHostManagerBridgeMigrationReview =
    SyncFfiRustHostManagerBridgeMigrationReview(
      migrationDecision: syncFfiRustHostManagerBridgeMigrationDecision,
      reviewedConditions:
          syncFfiRustHostManagerBridgeMigrationReviewedConditions,
      blockingConditions:
          syncFfiRustHostManagerBridgeMigrationBlockingConditions,
      requiredEvidence: syncFfiRustHostManagerBridgeMigrationRequiredEvidence,
    );

const syncFfiRustHostTestFilePlannedCases = [
  'contract_reports_command_capability_closed',
  'invalid_request_inputs_return_stable_status',
  'result_handle_copy_then_free',
  'envelope_allowlist_only',
  'forbidden_material_absent_from_native_outputs',
  'panic_boundary_returns_internal_error',
  'sync_domain_command_serialization',
];

const syncFfiRustHostTestFileApprovalReviewedConditions = [
  'rust_host_contract_cases_mapped',
  'c_abi_symbol_lookup_strategy_reviewed',
  'capability_missing_behavior_reviewed',
  'result_handle_lifecycle_reviewed',
  'error_handle_lifecycle_reviewed',
  'panic_boundary_reviewed',
  'forbidden_material_assertions_reviewed',
  'dynamic_library_smoke_absence_reviewed',
];

const syncFfiRustHostTestFileApprovalBlockingConditions = [
  'native_symbol_export_approved_by_adr',
  'dart_native_binding_approved',
  'manager_bridge_command_approved',
  'host_contract_test_file_approved',
];

const syncFfiRustHostTestFileApprovalRequiredEvidence = [
  'host_contract_catalog_current',
  'host_test_design_package_current',
  'c_abi_contract_review_matrix_current',
  'manager_sync_command_internal_draft_tests',
  'ffi_bridge_smoke_candidate_symbols_absent',
];

const syncFfiRustHostTestFileApprovalReview =
    SyncFfiRustHostTestFileApprovalReview(
      approvalDecision: syncFfiRustHostTestFileApprovalDecision,
      targetTestFile: syncFfiCommandBoundaryRustHostContractTargetTestFile,
      plannedTestCases: syncFfiRustHostTestFilePlannedCases,
      reviewedConditions: syncFfiRustHostTestFileApprovalReviewedConditions,
      blockingConditions: syncFfiRustHostTestFileApprovalBlockingConditions,
      requiredEvidence: syncFfiRustHostTestFileApprovalRequiredEvidence,
    );

const syncFfiRustHostRealSyncExecutionReviewedConditions = [
  'command_context_owner_scope_reviewed',
  'command_worker_thread_policy_reviewed',
  'sync_domain_serialization_reviewed',
  'operation_id_idempotency_reviewed',
  'readiness_snapshot_binding_reviewed',
  'deployment_evidence_summary_reviewed',
  'forbidden_material_redaction_reviewed',
];

const syncFfiRustHostRealSyncExecutionBlockingConditions = [
  'host_contract_test_file_approved',
  'native_symbol_export_approved_by_adr',
  'dart_native_binding_approved',
  'manager_bridge_command_approved',
  'platform_private_key_backend_production_ready',
  'recovery_authorization_interaction_tests_passed',
  'deployment_evidence_summary_approved',
  'real_sync_execution_approved_after_gate',
];

const syncFfiRustHostRealSyncExecutionRequiredEvidence = [
  'platform_private_key_backend_strategy_current',
  'manager_recovery_device_auth_flow_current',
  'sync_server_production_deployment_runbook_current',
  'manager_sync_entry_boundary_current',
  'local_docker_https_smoke_development_only',
];

const syncFfiRustHostRealSyncExecutionGateReview =
    SyncFfiRustHostRealSyncExecutionGateReview(
      executionDecision: syncFfiRustHostRealSyncExecutionDecision,
      reviewedConditions: syncFfiRustHostRealSyncExecutionReviewedConditions,
      blockingConditions: syncFfiRustHostRealSyncExecutionBlockingConditions,
      requiredEvidence: syncFfiRustHostRealSyncExecutionRequiredEvidence,
    );

const syncFfiRustHostRealSyncEvidenceBundleItems = [
  SyncFfiRustHostRealSyncEvidenceItem(
    evidenceId: 'platform_private_key_backend',
    evidenceDomain: 'platform_signing_backend',
    currentState: 'production_backend_not_ready_current_phase',
    blockingCondition: 'platform_private_key_backend_production_ready',
    requiredSource: 'platform_private_key_backend_strategy',
    safeSummary: 'backend_status_codes_only_no_key_material',
  ),
  SyncFfiRustHostRealSyncEvidenceItem(
    evidenceId: 'recovery_authorization_interaction',
    evidenceDomain: 'recovery_and_device_authorization',
    currentState: 'interaction_tests_not_passed_current_phase',
    blockingCondition: 'recovery_authorization_interaction_tests_passed',
    requiredSource: 'manager_recovery_device_auth_flow',
    safeSummary: 'confirmation_and_transient_lifecycle_codes_only',
  ),
  SyncFfiRustHostRealSyncEvidenceItem(
    evidenceId: 'deployment_evidence_summary',
    evidenceDomain: 'release_deployment_evidence',
    currentState: 'release_deployment_evidence_missing_current_phase',
    blockingCondition: 'deployment_evidence_summary_approved',
    requiredSource: 'sync_server_production_deployment_runbook',
    safeSummary: 'deployment_evidence_summary_v1_required_local_smoke_only',
  ),
];

const syncFfiRustHostRealSyncEvidenceBundleReviewedConditions = [
  'platform_private_key_backend_summary_mapped',
  'recovery_authorization_interaction_summary_mapped',
  'deployment_evidence_summary_source_mapped',
  'local_smoke_kept_development_only',
  'forbidden_material_redaction_reviewed',
  'current_phase_real_sync_unlock_blocked',
];

const syncFfiRustHostRealSyncEvidenceBundleBlockingConditions = [
  'platform_private_key_backend_production_ready',
  'recovery_authorization_interaction_tests_passed',
  'deployment_evidence_summary_approved',
  'release_evidence_bundle_approved_after_gate',
  'real_sync_execution_approved_after_gate',
];

const syncFfiRustHostRealSyncEvidenceBundleRequiredEvidence = [
  'platform_private_key_backend_strategy_current',
  'manager_recovery_device_auth_flow_current',
  'sync_server_production_deployment_runbook_current',
  'deployment_evidence_summary_v1_release_target',
  'manager_sync_entry_gate_visible_layer_tests',
  'real_sync_execution_evidence_bundle_replay',
];

const syncFfiRustHostRealSyncEvidenceBundleReview =
    SyncFfiRustHostRealSyncEvidenceBundleReview(
      bundleDecision: syncFfiRustHostRealSyncEvidenceBundleDecision,
      evidenceItems: syncFfiRustHostRealSyncEvidenceBundleItems,
      reviewedConditions:
          syncFfiRustHostRealSyncEvidenceBundleReviewedConditions,
      blockingConditions:
          syncFfiRustHostRealSyncEvidenceBundleBlockingConditions,
      requiredEvidence: syncFfiRustHostRealSyncEvidenceBundleRequiredEvidence,
    );

class SyncFfiRustHostExportSymbolReviewItem {
  const SyncFfiRustHostExportSymbolReviewItem({
    required this.symbolName,
    required this.symbolRole,
    required this.requiredContractFields,
    required this.releaseResponsibility,
    required this.errorBoundary,
    required this.panicBoundary,
    required this.smokeRequirement,
  });

  final String symbolName;
  final String symbolRole;
  final List<String> requiredContractFields;
  final String releaseResponsibility;
  final String errorBoundary;
  final String panicBoundary;
  final String smokeRequirement;
}

class SyncFfiRustHostExportApprovalReview {
  const SyncFfiRustHostExportApprovalReview({
    required this.exportDecision,
    required this.symbolReviews,
    required this.requiredEvidence,
  });

  final String exportDecision;
  final List<SyncFfiRustHostExportSymbolReviewItem> symbolReviews;
  final List<String> requiredEvidence;
}

class SyncFfiRustHostDartBindingMigrationReview {
  const SyncFfiRustHostDartBindingMigrationReview({
    required this.migrationDecision,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.requiredEvidence,
  });

  final String migrationDecision;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final List<String> requiredEvidence;
}

class SyncFfiRustHostManagerBridgeMigrationReview {
  const SyncFfiRustHostManagerBridgeMigrationReview({
    required this.migrationDecision,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.requiredEvidence,
  });

  final String migrationDecision;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final List<String> requiredEvidence;
}

class SyncFfiRustHostTestFileApprovalReview {
  const SyncFfiRustHostTestFileApprovalReview({
    required this.approvalDecision,
    required this.targetTestFile,
    required this.plannedTestCases,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.requiredEvidence,
  });

  final String approvalDecision;
  final String targetTestFile;
  final List<String> plannedTestCases;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final List<String> requiredEvidence;
}

class SyncFfiRustHostRealSyncExecutionGateReview {
  const SyncFfiRustHostRealSyncExecutionGateReview({
    required this.executionDecision,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.requiredEvidence,
  });

  final String executionDecision;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final List<String> requiredEvidence;
}

class SyncFfiRustHostRealSyncEvidenceItem {
  const SyncFfiRustHostRealSyncEvidenceItem({
    required this.evidenceId,
    required this.evidenceDomain,
    required this.currentState,
    required this.blockingCondition,
    required this.requiredSource,
    required this.safeSummary,
  });

  final String evidenceId;
  final String evidenceDomain;
  final String currentState;
  final String blockingCondition;
  final String requiredSource;
  final String safeSummary;
}

class SyncFfiRustHostRealSyncEvidenceBundleReview {
  const SyncFfiRustHostRealSyncEvidenceBundleReview({
    required this.bundleDecision,
    required this.evidenceItems,
    required this.reviewedConditions,
    required this.blockingConditions,
    required this.requiredEvidence,
  });

  final String bundleDecision;
  final List<SyncFfiRustHostRealSyncEvidenceItem> evidenceItems;
  final List<String> reviewedConditions;
  final List<String> blockingConditions;
  final List<String> requiredEvidence;
}

Map<String, Object?> syncFfiRustHostExportApprovalReviewShape(
  SyncFfiRustHostExportApprovalReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostExportApprovalReviewStatus,
    'export_decision': fixture.exportDecision,
    'symbol_reviews': [
      for (final item in fixture.symbolReviews)
        syncFfiRustHostExportSymbolReviewItemShape(item),
    ],
    'required_evidence': fixture.requiredEvidence,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
    'can_export_native_symbol': false,
    'can_create_host_contract_test_file': false,
    'can_modify_dart_native_binding': false,
    'can_modify_manager_bridge': false,
  };
}

Map<String, Object?> syncFfiRustHostExportSymbolReviewItemShape(
  SyncFfiRustHostExportSymbolReviewItem fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostExportApprovalReviewStatus,
    'symbol_name': fixture.symbolName,
    'symbol_role': fixture.symbolRole,
    'required_contract_fields': fixture.requiredContractFields,
    'release_responsibility': fixture.releaseResponsibility,
    'error_boundary': fixture.errorBoundary,
    'panic_boundary': fixture.panicBoundary,
    'smoke_requirement': fixture.smokeRequirement,
    'export_state': 'planned_not_exported_current_phase',
    'export_approved': false,
  };
}

Map<String, Object?> syncFfiRustHostDartBindingMigrationReviewShape(
  SyncFfiRustHostDartBindingMigrationReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostDartBindingMigrationReviewStatus,
    'migration_decision': fixture.migrationDecision,
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'required_evidence': fixture.requiredEvidence,
    'can_modify_dart_native_binding': false,
    'can_call_native_command': false,
    'can_hold_native_pointer_after_copy': false,
    'can_write_settings_action': false,
    'can_modify_manager_bridge': false,
  };
}

Map<String, Object?> syncFfiRustHostManagerBridgeMigrationReviewShape(
  SyncFfiRustHostManagerBridgeMigrationReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostManagerBridgeMigrationReviewStatus,
    'migration_decision': fixture.migrationDecision,
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'required_evidence': fixture.requiredEvidence,
    'can_modify_manager_bridge': false,
    'can_create_bridge_command_request': false,
    'can_write_settings_action': false,
    'can_execute_real_sync': false,
  };
}

Map<String, Object?> syncFfiRustHostTestFileApprovalReviewShape(
  SyncFfiRustHostTestFileApprovalReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostTestFileApprovalReviewStatus,
    'approval_decision': fixture.approvalDecision,
    'target_test_file': fixture.targetTestFile,
    'planned_test_cases': fixture.plannedTestCases,
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'required_evidence': fixture.requiredEvidence,
    'can_create_host_contract_test_file': false,
    'can_export_native_symbol': false,
    'can_call_dynamic_library_symbol': false,
    'can_modify_dart_native_binding': false,
  };
}

Map<String, Object?> syncFfiRustHostRealSyncExecutionGateReviewShape(
  SyncFfiRustHostRealSyncExecutionGateReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostRealSyncExecutionGateReviewStatus,
    'execution_decision': fixture.executionDecision,
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'required_evidence': fixture.requiredEvidence,
    'can_execute_real_sync': false,
    'can_connect_go_server': false,
    'can_touch_platform_key_backend': false,
    'can_generate_recovery_code': false,
    'can_create_join_request': false,
    'can_revoke_device': false,
  };
}

Map<String, Object?> syncFfiRustHostRealSyncEvidenceBundleReviewShape(
  SyncFfiRustHostRealSyncEvidenceBundleReview fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'review_status': syncFfiRustHostRealSyncEvidenceBundleReviewStatus,
    'bundle_decision': fixture.bundleDecision,
    'evidence_items': [
      for (final item in fixture.evidenceItems)
        syncFfiRustHostRealSyncEvidenceItemShape(item),
    ],
    'reviewed_conditions': fixture.reviewedConditions,
    'blocking_conditions': fixture.blockingConditions,
    'required_evidence': fixture.requiredEvidence,
    'can_mark_platform_backend_ready': false,
    'can_mark_recovery_authorization_ready': false,
    'can_mark_deployment_summary_ready': false,
    'can_unlock_real_sync': false,
    'can_execute_remote_call': false,
    'can_persist_secret_material': false,
  };
}

Map<String, Object?> syncFfiRustHostRealSyncEvidenceItemShape(
  SyncFfiRustHostRealSyncEvidenceItem fixture,
) {
  return {
    'format': syncFfiRustHostInternalDraftEvidenceFormat,
    'evidence_id': fixture.evidenceId,
    'evidence_domain': fixture.evidenceDomain,
    'current_state': fixture.currentState,
    'blocking_condition': fixture.blockingCondition,
    'required_source': fixture.requiredSource,
    'safe_summary': fixture.safeSummary,
    'can_unlock_real_sync': false,
  };
}
