import 'package:radishlex_manager/src/models/manager_models.dart';

import 'sync_bridge_command_contract_fixtures.dart';

const syncFfiCommandBoundaryReviewStatus =
    'future_ffi_boundary_review_only_no_native_symbol';

const syncFfiCommandBoundaryFormat =
    'future_manager_sync_ffi_command_boundary.v1_draft';

const syncFfiCommandBoundaryRustHostContractFormat =
    'future_manager_sync_ffi_rust_host_contract_catalog.v1_draft';

const syncFfiCommandBoundaryRustHostContractReviewStatus =
    'future_rust_host_contract_review_only_no_native_symbol';

const syncFfiCommandBoundaryRustHostContractTargetTestFile =
    'crates/ime-ffi/tests/manager_sync_command_boundary.rs';

const syncFfiCommandBoundaryRecommendedStrategy =
    'single_versioned_manager_sync_command_executor';

const syncFfiCommandBoundaryRejectedStrategies = [
  'json_tunnel_request_response',
  'dedicated_symbols_before_contract_review',
  'flutter_visible_transport_payload',
  'dart_owned_opaque_crypto_material',
];

const syncFfiCommandBoundaryCurrentNativeSymbols = <String>[];

const syncFfiCommandBoundaryCandidateSymbols = [
  'radishlex_manager_sync_command_execute_v1',
  'radishlex_manager_sync_command_result_*',
  'radishlex_manager_sync_command_result_free',
];

const syncFfiCommandBoundaryRequestCommonFields = [
  'schema_version',
  'action_id',
  'operation_id',
  'readiness_snapshot_id',
  'deployment_evidence_source_tag',
  'device_backend_gate',
  'explicit_user_start',
];

const syncFfiCommandBoundaryResultSummaryFields = [
  'schema_version',
  'action_id',
  'command_status',
  'error_code',
  'retry_policy',
  'user_visible_summary_code',
  'diagnostics_summary_code',
  'next_required_evidence',
  'object_type_summary',
  'object_count_summary',
  'object_version_summary',
  'recorded_at_summary',
];

const syncFfiCommandBoundaryOwnershipRules = [
  'request_allocated_by_caller_call_scope_only',
  'borrowed_utf8_views_copied_before_rust_async_boundary',
  'rust_owned_result_handle',
  'dart_copies_summary_then_frees_result',
  'result_views_invalid_after_free',
  'free_null_is_noop',
  'error_read_then_free',
  'no_pointer_in_settings_diagnostics_or_widget_state',
];

const syncFfiCommandBoundaryTransientSecretRules = [
  'user_explicit_operation_only',
  'borrowed_view_call_scope',
  'rust_short_lifecycle_internal_copy_only',
  'not_in_settings',
  'not_in_diagnostics',
  'not_in_logs',
  'not_in_test_golden',
  'one_time_display_material_separate_lifecycle',
];

const syncFfiCommandBoundaryAbiStatusCases = [
  SyncFfiCommandBoundaryStatusCase(
    input: 'null_pointer',
    statusCode: 'InvalidArgument',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'invalid_utf8',
    statusCode: 'InvalidArgument',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'invalid_bool',
    statusCode: 'InvalidArgument',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'unknown_schema_version',
    statusCode: 'InvalidArgument',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'unknown_action',
    statusCode: 'InvalidArgument',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'sync_command_not_enabled_current_phase',
    statusCode: 'InvalidState',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'same_domain_concurrent_command',
    statusCode: 'InvalidState',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'sync_command_internal_unenveloped_failure',
    statusCode: 'SyncError',
    boundary: 'abi_error',
  ),
  SyncFfiCommandBoundaryStatusCase(
    input: 'panic_boundary_caught',
    statusCode: 'InternalError',
    boundary: 'abi_error',
  ),
];

class SyncFfiCommandBoundaryStatusCase {
  const SyncFfiCommandBoundaryStatusCase({
    required this.input,
    required this.statusCode,
    required this.boundary,
  });

  final String input;
  final String statusCode;
  final String boundary;
}

const syncFfiCommandBoundaryRustHostInputSamples = [
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'current_phase_capability_closed_recovery_setup',
    actionId: 'recovery_setup',
    operationId: 'op_test_non_secret_001',
    readinessSnapshotId: 'readiness_snapshot_test_001',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: true,
    abiInputCase: 'sync_command_not_enabled_current_phase',
    expectedStatusCode: 'InvalidState',
    forbiddenMaterialCategory: 'none',
    expectedEvidence: [
      'sync_command_capability_closed_current_phase',
      'invalid_state_without_native_symbol',
      'user_sync_entry_closed_current_phase',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'invalid_schema_version_rejected',
    actionId: 'recovery_restore',
    operationId: 'op_test_non_secret_002',
    readinessSnapshotId: 'readiness_snapshot_test_002',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: true,
    abiInputCase: 'unknown_schema_version',
    expectedStatusCode: 'InvalidArgument',
    forbiddenMaterialCategory: 'recovery_secret_material',
    expectedEvidence: [
      'unknown_schema_version_invalid_argument',
      'error_read_then_free',
      'no_recovery_secret_in_error',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'invalid_action_rejected',
    actionId: 'join_request_authorization',
    operationId: 'op_test_non_secret_003',
    readinessSnapshotId: 'readiness_snapshot_test_003',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: true,
    abiInputCase: 'unknown_action',
    expectedStatusCode: 'InvalidArgument',
    forbiddenMaterialCategory: 'join_verifier_material',
    expectedEvidence: [
      'unknown_action_invalid_argument',
      'error_read_then_free',
      'no_short_code_in_error',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'invalid_bool_rejected',
    actionId: 'device_revocation',
    operationId: 'op_test_non_secret_004',
    readinessSnapshotId: 'readiness_snapshot_test_004',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: false,
    abiInputCase: 'invalid_bool',
    expectedStatusCode: 'InvalidArgument',
    forbiddenMaterialCategory: 'signature_or_wrapped_sync_material',
    expectedEvidence: [
      'invalid_bool_invalid_argument',
      'error_read_then_free',
      'no_signature_or_wrapped_material_in_error',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'null_pointer_rejected',
    actionId: 'recovery_setup',
    operationId: 'op_test_non_secret_007',
    readinessSnapshotId: 'readiness_snapshot_test_007',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: false,
    abiInputCase: 'null_pointer',
    expectedStatusCode: 'InvalidArgument',
    forbiddenMaterialCategory: 'none',
    expectedEvidence: [
      'null_pointer_invalid_argument',
      'error_read_then_free',
      'free_null_is_noop',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'invalid_utf8_rejected',
    actionId: 'recovery_restore',
    operationId: 'op_test_non_secret_008',
    readinessSnapshotId: 'readiness_snapshot_test_008',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: true,
    abiInputCase: 'invalid_utf8',
    expectedStatusCode: 'InvalidArgument',
    forbiddenMaterialCategory: 'none',
    expectedEvidence: [
      'invalid_utf8_invalid_argument',
      'error_read_then_free',
      'borrowed_utf8_view_rejected_before_copy',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'sync_unenveloped_failure_maps_to_sync_error',
    actionId: 'device_revocation',
    operationId: 'op_test_non_secret_005',
    readinessSnapshotId: 'readiness_snapshot_test_005',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'ready_for_contract_test',
    explicitUserStart: true,
    abiInputCase: 'sync_command_internal_unenveloped_failure',
    expectedStatusCode: 'SyncError',
    forbiddenMaterialCategory: 'opaque_transport_content',
    expectedEvidence: [
      'sync_error_status',
      'no_payload_in_error_message',
      'no_provider_exception_in_debug',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'same_domain_concurrent_command_blocked_or_serialized',
    actionId: 'device_revocation',
    operationId: 'op_test_non_secret_009',
    readinessSnapshotId: 'readiness_snapshot_test_009',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'ready_for_contract_test',
    explicitUserStart: true,
    abiInputCase: 'same_domain_concurrent_command',
    expectedStatusCode: 'InvalidState',
    forbiddenMaterialCategory: 'none',
    expectedEvidence: [
      'same_domain_mutation_serialized',
      'concurrent_command_conflict_or_queue',
      'operation_id_non_sensitive',
    ],
  ),
  SyncFfiCommandBoundaryRustHostInputSample(
    id: 'panic_boundary_maps_to_internal_error',
    actionId: 'recovery_setup',
    operationId: 'op_test_non_secret_006',
    readinessSnapshotId: 'readiness_snapshot_test_006',
    sourceTag: 'local_smoke',
    deviceBackendGate: 'blocked',
    explicitUserStart: true,
    abiInputCase: 'panic_boundary_caught',
    expectedStatusCode: 'InternalError',
    forbiddenMaterialCategory: 'local_path_material',
    expectedEvidence: [
      'panic_caught',
      'internal_error_status',
      'no_panic_crosses_c_abi',
    ],
  ),
];

class SyncFfiCommandBoundaryRustHostInputSample {
  const SyncFfiCommandBoundaryRustHostInputSample({
    required this.id,
    required this.actionId,
    required this.operationId,
    required this.readinessSnapshotId,
    required this.sourceTag,
    required this.deviceBackendGate,
    required this.explicitUserStart,
    required this.abiInputCase,
    required this.expectedStatusCode,
    required this.forbiddenMaterialCategory,
    required this.expectedEvidence,
  });

  final String id;
  final String actionId;
  final String operationId;
  final String readinessSnapshotId;
  final String sourceTag;
  final String deviceBackendGate;
  final bool explicitUserStart;
  final String abiInputCase;
  final String expectedStatusCode;
  final String forbiddenMaterialCategory;
  final List<String> expectedEvidence;

  String get expectedEvidenceSummary {
    return managerSyncCodeSummary(expectedEvidence);
  }
}

const syncFfiCommandBoundaryRustHostSmokeCases = [
  SyncFfiCommandBoundarySmokeCase(
    id: 'contract_reports_command_capability_closed',
    layer: 'rust_host',
    expectedEvidence: [
      'abi_contract_version_checked',
      'sync_command_capability_closed_current_phase',
      'invalid_state_without_native_symbol',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'invalid_request_inputs_return_stable_status',
    layer: 'rust_host',
    expectedEvidence: [
      'unknown_schema_version_invalid_argument',
      'unknown_action_invalid_argument',
      'invalid_bool_invalid_argument',
      'invalid_utf8_invalid_argument',
      'null_pointer_invalid_argument',
      'error_read_then_free',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'result_handle_copy_then_free',
    layer: 'rust_host',
    expectedEvidence: [
      'rust_owned_result_handle',
      'borrowed_string_view_copied',
      'result_free_invalidates_views',
      'free_null_is_noop',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'envelope_allowlist_only',
    layer: 'rust_host',
    expectedEvidence: [
      'command_status_allowlist',
      'command_error_allowlist',
      'retry_policy_allowlist',
      'next_evidence_allowlist',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'forbidden_material_absent_from_native_outputs',
    layer: 'rust_host',
    expectedEvidence: [
      'no_secret_in_result',
      'no_payload_in_error_message',
      'no_provider_exception_in_debug',
      'no_real_path_in_summary',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'panic_boundary_returns_internal_error',
    layer: 'rust_host',
    expectedEvidence: [
      'panic_caught',
      'internal_error_status',
      'no_panic_crosses_c_abi',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'sync_domain_command_serialization',
    layer: 'rust_host',
    expectedEvidence: [
      'same_domain_mutation_serialized',
      'concurrent_command_conflict_or_queue',
      'operation_id_non_sensitive',
    ],
  ),
];

const syncFfiCommandBoundaryDartFfiSmokeCases = [
  SyncFfiCommandBoundarySmokeCase(
    id: 'native_command_capability_missing_keeps_ui_closed',
    layer: 'dart_ffi',
    expectedEvidence: [
      'ffi_capability_missing_detected',
      'manager_snapshot_stays_current_phase_closed',
      'no_action_button_enabled',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'dart_copies_result_then_releases_handle',
    layer: 'dart_ffi',
    expectedEvidence: [
      'summary_copied_to_dart_owned_model',
      'native_result_free_called',
      'native_handle_not_stored',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'forbidden_material_not_visible_to_manager',
    layer: 'dart_ffi',
    expectedEvidence: [
      'no_forbidden_material_in_diagnostics',
      'no_forbidden_material_in_settings_draft',
      'no_forbidden_material_in_widget_text',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'transient_secret_not_persisted',
    layer: 'dart_ffi',
    expectedEvidence: [
      'secret_input_not_written_to_settings_json',
      'secret_input_not_written_to_diagnostics_export',
      'secret_input_not_written_to_golden',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'unknown_native_status_downgrades_to_safe_blocker',
    layer: 'dart_ffi',
    expectedEvidence: [
      'unknown_status_rejected_or_sanitized',
      'unexpected_bridge_error_code',
      'user_sync_entry_closed_current_phase',
    ],
  ),
  SyncFfiCommandBoundarySmokeCase(
    id: 'ffi_and_command_errors_map_to_bridge_categories',
    layer: 'dart_ffi',
    expectedEvidence: [
      'ffi_library_load_failed_native_library',
      'invalid_argument_invalid_input',
      'sync_error_sync_preflight',
      'command_error_action_error_allowlist',
    ],
  ),
];

class SyncFfiCommandBoundarySmokeCase {
  const SyncFfiCommandBoundarySmokeCase({
    required this.id,
    required this.layer,
    required this.expectedEvidence,
  });

  final String id;
  final String layer;
  final List<String> expectedEvidence;

  String get evidenceSummary {
    return managerSyncCodeSummary(expectedEvidence);
  }
}

const syncFfiCommandBoundaryRustHostContractCases = [
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'contract_reports_command_capability_closed',
    targetTestName: 'contract_reports_command_capability_closed',
    sampleIds: ['current_phase_capability_closed_recovery_setup'],
    smokeCaseId: 'contract_reports_command_capability_closed',
    requiredAbiInputCases: ['sync_command_not_enabled_current_phase'],
    expectedStatusCodes: ['InvalidState'],
    requiredEvidence: [
      'abi_contract_version_checked',
      'sync_command_capability_closed_current_phase',
      'invalid_state_without_native_symbol',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'invalid_request_inputs_return_stable_status',
    targetTestName: 'invalid_request_inputs_return_stable_status',
    sampleIds: [
      'invalid_schema_version_rejected',
      'invalid_action_rejected',
      'invalid_bool_rejected',
      'null_pointer_rejected',
      'invalid_utf8_rejected',
    ],
    smokeCaseId: 'invalid_request_inputs_return_stable_status',
    requiredAbiInputCases: [
      'unknown_schema_version',
      'unknown_action',
      'invalid_bool',
      'null_pointer',
      'invalid_utf8',
    ],
    expectedStatusCodes: ['InvalidArgument'],
    requiredEvidence: [
      'invalid_inputs_rejected_before_command_execution',
      'error_read_then_free',
      'no_secret_or_payload_in_error',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'result_handle_copy_then_free',
    targetTestName: 'result_handle_copy_then_free',
    sampleIds: ['current_phase_capability_closed_recovery_setup'],
    smokeCaseId: 'result_handle_copy_then_free',
    requiredAbiInputCases: ['sync_command_not_enabled_current_phase'],
    expectedStatusCodes: ['InvalidState'],
    requiredEvidence: [
      'rust_owned_result_handle',
      'borrowed_string_view_copied',
      'result_free_invalidates_views',
      'free_null_is_noop',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'envelope_allowlist_only',
    targetTestName: 'envelope_allowlist_only',
    sampleIds: ['sync_unenveloped_failure_maps_to_sync_error'],
    smokeCaseId: 'envelope_allowlist_only',
    requiredAbiInputCases: ['sync_command_internal_unenveloped_failure'],
    expectedStatusCodes: ['SyncError'],
    requiredEvidence: [
      'command_status_allowlist',
      'command_error_allowlist',
      'retry_policy_allowlist',
      'next_evidence_allowlist',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'forbidden_material_absent_from_native_outputs',
    targetTestName: 'forbidden_material_absent_from_native_outputs',
    sampleIds: [
      'invalid_schema_version_rejected',
      'invalid_action_rejected',
      'invalid_bool_rejected',
      'sync_unenveloped_failure_maps_to_sync_error',
      'panic_boundary_maps_to_internal_error',
    ],
    smokeCaseId: 'forbidden_material_absent_from_native_outputs',
    requiredAbiInputCases: [
      'unknown_schema_version',
      'unknown_action',
      'invalid_bool',
      'sync_command_internal_unenveloped_failure',
      'panic_boundary_caught',
    ],
    expectedStatusCodes: ['InvalidArgument', 'SyncError', 'InternalError'],
    requiredEvidence: [
      'no_secret_in_result',
      'no_payload_in_error_message',
      'no_provider_exception_in_debug',
      'no_real_path_in_summary',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'panic_boundary_returns_internal_error',
    targetTestName: 'panic_boundary_returns_internal_error',
    sampleIds: ['panic_boundary_maps_to_internal_error'],
    smokeCaseId: 'panic_boundary_returns_internal_error',
    requiredAbiInputCases: ['panic_boundary_caught'],
    expectedStatusCodes: ['InternalError'],
    requiredEvidence: [
      'panic_caught',
      'internal_error_status',
      'no_panic_crosses_c_abi',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
  SyncFfiCommandBoundaryRustHostContractCase(
    id: 'sync_domain_command_serialization',
    targetTestName: 'sync_domain_command_serialization',
    sampleIds: ['same_domain_concurrent_command_blocked_or_serialized'],
    smokeCaseId: 'sync_domain_command_serialization',
    requiredAbiInputCases: ['same_domain_concurrent_command'],
    expectedStatusCodes: ['InvalidState'],
    requiredEvidence: [
      'same_domain_mutation_serialized',
      'concurrent_command_conflict_or_queue',
      'operation_id_non_sensitive',
    ],
    implementationStatus: 'planned_no_native_symbol',
  ),
];

class SyncFfiCommandBoundaryRustHostContractCase {
  const SyncFfiCommandBoundaryRustHostContractCase({
    required this.id,
    required this.targetTestName,
    required this.sampleIds,
    required this.smokeCaseId,
    required this.requiredAbiInputCases,
    required this.expectedStatusCodes,
    required this.requiredEvidence,
    required this.implementationStatus,
  });

  final String id;
  final String targetTestName;
  final List<String> sampleIds;
  final String smokeCaseId;
  final List<String> requiredAbiInputCases;
  final List<String> expectedStatusCodes;
  final List<String> requiredEvidence;
  final String implementationStatus;

  String get sampleIdSummary {
    return managerSyncCodeSummary(sampleIds);
  }

  String get requiredAbiInputSummary {
    return managerSyncCodeSummary(requiredAbiInputCases);
  }

  String get expectedStatusSummary {
    return managerSyncCodeSummary(expectedStatusCodes);
  }

  String get evidenceSummary {
    return managerSyncCodeSummary(requiredEvidence);
  }
}

Map<String, Object?> syncFfiCommandBoundaryStrategyShape() {
  return {
    'format': syncFfiCommandBoundaryFormat,
    'review_status': syncFfiCommandBoundaryReviewStatus,
    'recommended_strategy': syncFfiCommandBoundaryRecommendedStrategy,
    'current_native_symbols': syncFfiCommandBoundaryCurrentNativeSymbols,
    'candidate_symbols': syncFfiCommandBoundaryCandidateSymbols,
    'rejected_strategies': syncFfiCommandBoundaryRejectedStrategies,
  };
}

Map<String, Object?> syncFfiCommandBoundarySafeRequestShape(
  SyncBridgeCommandContractActionFixture fixture,
) {
  return {
    'format': 'future_manager_sync_ffi_command_request.v1_draft',
    'review_status': syncFfiCommandBoundaryReviewStatus,
    'action_id': fixture.actionId,
    'common_field_policies': {
      for (final field in syncFfiCommandBoundaryRequestCommonFields)
        field: _requestFieldPolicy(field),
    },
    'action_safe_fields': fixture.requestSafeFields,
    'transient_secret_policy': fixture.transientSecretSummary,
  };
}

Map<String, Object?> syncFfiCommandBoundarySafeResultShape(
  SyncBridgeCommandContractActionFixture fixture,
) {
  return {
    'format': 'future_manager_sync_ffi_command_result.v1_draft',
    'review_status': syncFfiCommandBoundaryReviewStatus,
    'action_id': fixture.actionId,
    'summary_field_policies': {
      for (final field in syncFfiCommandBoundaryResultSummaryFields)
        field: 'safe_summary',
    },
  };
}

List<String> syncFfiCommandBoundaryAllCommandErrorCodes() {
  return List.unmodifiable(
    {
      for (final fixture in syncBridgeCommandContractActionFixtures)
        ...fixture.errorCodes,
      'version_conflict',
      'unexpected_bridge_error',
    }.toList()..sort(),
  );
}

List<String> syncFfiCommandBoundaryAllSmokeCaseIds() {
  return List.unmodifiable([
    ...syncFfiCommandBoundaryRustHostSmokeCases.map((fixture) => fixture.id),
    ...syncFfiCommandBoundaryDartFfiSmokeCases.map((fixture) => fixture.id),
  ]);
}

List<String> syncFfiCommandBoundaryRustHostContractCaseIds() {
  return List.unmodifiable(
    syncFfiCommandBoundaryRustHostContractCases
        .map((fixture) => fixture.id)
        .toList(),
  );
}

Map<String, Object?> syncFfiCommandBoundaryRustHostInputSampleShape(
  SyncFfiCommandBoundaryRustHostInputSample sample,
) {
  return {
    'id': sample.id,
    'format': 'future_manager_sync_ffi_rust_host_input.v1_draft',
    'review_status': syncFfiCommandBoundaryReviewStatus,
    'action_id': sample.actionId,
    'operation_id': sample.operationId,
    'readiness_snapshot_id': sample.readinessSnapshotId,
    'source_tag': sample.sourceTag,
    'device_backend_gate': sample.deviceBackendGate,
    'explicit_user_start': sample.explicitUserStart,
    'abi_input_case': sample.abiInputCase,
    'expected_status_code': sample.expectedStatusCode,
    'forbidden_material_category': sample.forbiddenMaterialCategory,
    'expected_evidence': sample.expectedEvidence,
  };
}

Map<String, Object?> syncFfiCommandBoundaryRustHostContractCaseShape(
  SyncFfiCommandBoundaryRustHostContractCase fixture,
) {
  return {
    'format': syncFfiCommandBoundaryRustHostContractFormat,
    'review_status': syncFfiCommandBoundaryRustHostContractReviewStatus,
    'target_test_file': syncFfiCommandBoundaryRustHostContractTargetTestFile,
    'id': fixture.id,
    'target_test_name': fixture.targetTestName,
    'sample_ids': fixture.sampleIds,
    'smoke_case_id': fixture.smokeCaseId,
    'required_abi_input_cases': fixture.requiredAbiInputCases,
    'expected_status_codes': fixture.expectedStatusCodes,
    'required_evidence': fixture.requiredEvidence,
    'implementation_status': fixture.implementationStatus,
  };
}

String _requestFieldPolicy(String field) {
  switch (field) {
    case 'schema_version':
    case 'action_id':
      return 'integer_enum';
    case 'explicit_user_start':
      return 'u8_bool';
    default:
      return 'borrowed_utf8_view_call_scope';
  }
}
