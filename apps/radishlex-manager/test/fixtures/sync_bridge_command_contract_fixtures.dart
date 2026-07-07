import 'package:radishlex_manager/src/models/manager_models.dart';

export 'sync_action_protocol_fixtures.dart';

const syncBridgeCommandContractReviewStatus =
    'future_contract_review_only_no_bridge_command';

const syncBridgeCommandContractEnvelopeRequiredFields = [
  'action_id',
  'command_status',
  'error_code',
  'retry_policy',
  'user_visible_summary_code',
  'diagnostics_summary_code',
  'next_required_evidence',
];

const syncBridgeCommandContractCommandStatuses = [
  'success',
  'already_done',
  'blocked_by_readiness',
  'requires_user_confirmation',
  'conflict',
  'retryable_failure',
  'fatal_failure',
  'unexpected_bridge_error',
];

const syncBridgeCommandContractRetryPolicies = [
  'not_retryable',
  'retry_after_user_action',
  'retry_after_network_recovery',
  'retry_after_conflict_resolution',
];

const syncBridgeCommandContractForbiddenFragments = [
  'secret-token',
  'Bearer ',
  'RADISHLEX-RECOVERY-CODE-SECRET',
  'short_code=',
  'signature_bytes',
  'wrapped_material_bytes',
  'payload_bytes=abcdef',
  'request_body',
  'response_body',
  '/synthetic/private',
];

const syncBridgeCommandContractDiagnosticsAllowedFields = [
  'action_id',
  'command_status',
  'error_code',
  'retry_policy',
  'next_required_evidence',
  'source_tag',
  'object_type_summary',
  'object_count_summary',
  'object_version_summary',
  'recorded_at_summary',
];

class SyncBridgeCommandContractActionFixture {
  const SyncBridgeCommandContractActionFixture({
    required this.actionId,
    required this.requestSafeFields,
    required this.resultSafeFields,
    required this.transientSecretFields,
    required this.opaqueMaterialFields,
    required this.transportPayloadFields,
    required this.errorCodes,
    required this.idempotencyStatuses,
    required this.currentPhaseProhibitedOperations,
  });

  final String actionId;
  final List<String> requestSafeFields;
  final List<String> resultSafeFields;
  final List<String> transientSecretFields;
  final List<String> opaqueMaterialFields;
  final List<String> transportPayloadFields;
  final List<String> errorCodes;
  final List<String> idempotencyStatuses;
  final List<String> currentPhaseProhibitedOperations;

  String get requestSafeFieldSummary {
    return managerSyncCodeSummary(requestSafeFields);
  }

  String get resultSafeFieldSummary {
    return managerSyncCodeSummary(resultSafeFields);
  }

  String get transientSecretSummary {
    return managerSyncCodeSummary(transientSecretFields);
  }

  String get opaqueMaterialSummary {
    return managerSyncCodeSummary(opaqueMaterialFields);
  }

  String get transportPayloadSummary {
    return managerSyncCodeSummary(transportPayloadFields);
  }

  String get errorCodeSummary {
    return managerSyncCodeSummary(errorCodes);
  }

  String get idempotencyStatusSummary {
    return managerSyncCodeSummary(idempotencyStatuses);
  }
}

const syncBridgeCommandContractActionFixtures = [
  SyncBridgeCommandContractActionFixture(
    actionId: 'recovery_setup',
    requestSafeFields: [
      'deployment_evidence_source_tag',
      'device_backend_gate',
      'explicit_user_start',
      'save_confirmation_status',
    ],
    resultSafeFields: [
      'status_code',
      'recovery_record_status',
      'first_upload_gate',
      'next_required_evidence',
    ],
    transientSecretFields: [
      'one_time_recovery_code_display_placeholder',
      'recovery_code_save_confirmation_ack',
    ],
    opaqueMaterialFields: [
      'sync_master_key',
      'recovery_record_ciphertext',
      'wrapped_recovery_material',
    ],
    transportPayloadFields: [
      'recovery_record_upload_request_body',
      'recovery_record_upload_response_body',
    ],
    errorCodes: [
      'configuration_missing',
      'backend_unavailable',
      'deployment_unverified',
      'recovery_code_required',
      'recovery_record_missing',
      'recovery_record_revoked',
      'local_data_inconsistent',
    ],
    idempotencyStatuses: [
      'active_recovery_record_requires_confirmation',
      'setup_already_prepared',
    ],
    currentPhaseProhibitedOperations: [
      'generate_recovery_code',
      'create_recovery_record',
      'unlock_first_upload',
    ],
  ),
  SyncBridgeCommandContractActionFixture(
    actionId: 'recovery_restore',
    requestSafeFields: [
      'deployment_evidence_source_tag',
      'restore_attempt_status',
      'recovery_record_lookup_status',
      'device_registration_status',
    ],
    resultSafeFields: [
      'status_code',
      'restore_result_status',
      'device_registration_status',
      'next_required_evidence',
    ],
    transientSecretFields: ['recovery_code_input_transient_placeholder'],
    opaqueMaterialFields: [
      'kdf_output',
      'unwrapped_device_material',
      'sync_master_key',
    ],
    transportPayloadFields: [
      'recovery_record_lookup_request_body',
      'recovery_record_lookup_response_body',
    ],
    errorCodes: [
      'configuration_missing',
      'authentication_required',
      'deployment_unverified',
      'recovery_code_required',
      'recovery_code_invalid',
      'recovery_record_missing',
      'recovery_record_revoked',
      'network_unreachable',
      'local_data_inconsistent',
    ],
    idempotencyStatuses: [
      'invalid_code_does_not_mutate_domain',
      'already_restored',
    ],
    currentPhaseProhibitedOperations: [
      'input_recovery_code',
      'unwrap_device_material',
      'register_restored_device',
    ],
  ),
  SyncBridgeCommandContractActionFixture(
    actionId: 'join_request_authorization',
    requestSafeFields: [
      'join_request_status',
      'short_code_verification_status',
      'active_device_requirement',
      'authorization_package_preconditions',
    ],
    resultSafeFields: [
      'status_code',
      'authorization_package_status',
      'device_state_status',
      'next_required_evidence',
    ],
    transientSecretFields: [
      'short_code_verification_transient_placeholder',
      'explicit_authorization_confirmation',
    ],
    opaqueMaterialFields: [
      'authorization_package_ciphertext',
      'device_signature',
      'wrapped_device_key',
    ],
    transportPayloadFields: [
      'join_request_fetch_response_body',
      'authorization_package_upload_request_body',
    ],
    errorCodes: [
      'configuration_missing',
      'authentication_required',
      'backend_unavailable',
      'deployment_unverified',
      'join_request_expired',
      'authorization_rejected',
      'device_revoked',
      'network_unreachable',
      'local_data_inconsistent',
    ],
    idempotencyStatuses: [
      'join_request_expired_blocks_authorization',
      'short_code_mismatch_blocks_authorization',
      'already_authorized',
    ],
    currentPhaseProhibitedOperations: [
      'create_join_request',
      'sign_authorization_package',
      'write_wrapped_material',
    ],
  ),
  SyncBridgeCommandContractActionFixture(
    actionId: 'device_revocation',
    requestSafeFields: [
      'target_device_status_summary',
      'active_device_requirement',
      'lost_device_risk_acknowledgement',
      'key_epoch_status',
    ],
    resultSafeFields: [
      'status_code',
      'revocation_record_status',
      'key_epoch_status',
      'next_required_evidence',
    ],
    transientSecretFields: ['explicit_revocation_confirmation'],
    opaqueMaterialFields: [
      'revocation_record_signature',
      'new_key_epoch_material',
      'wrapped_epoch_material',
    ],
    transportPayloadFields: [
      'revocation_upload_request_body',
      'key_epoch_delivery_response_body',
    ],
    errorCodes: [
      'configuration_missing',
      'authentication_required',
      'backend_unavailable',
      'deployment_unverified',
      'device_revoked',
      'key_epoch_rotation_required',
      'network_unreachable',
      'local_data_inconsistent',
    ],
    idempotencyStatuses: ['already_revoked', 'key_epoch_conflict_detected'],
    currentPhaseProhibitedOperations: [
      'revoke_device',
      'sign_revocation_record',
      'advance_key_epoch',
    ],
  ),
];

class SyncBridgeCommandContractRejectedSample {
  const SyncBridgeCommandContractRejectedSample({
    required this.id,
    required this.actionId,
    required this.payload,
    required this.expectedReason,
    required this.forbiddenFragment,
  });

  final String id;
  final String actionId;
  final Map<String, Object?> payload;
  final String expectedReason;
  final String forbiddenFragment;
}

const syncBridgeCommandContractRejectedSamples = [
  SyncBridgeCommandContractRejectedSample(
    id: 'bearer_token_rejected',
    actionId: 'recovery_setup',
    payload: {
      'authorization_header': 'Bearer secret-token',
      'access_token': 'secret-token',
    },
    expectedReason: 'bearer_credential_material',
    forbiddenFragment: 'secret-token',
  ),
  SyncBridgeCommandContractRejectedSample(
    id: 'recovery_code_rejected',
    actionId: 'recovery_restore',
    payload: {'recovery_code': 'RADISHLEX-RECOVERY-CODE-SECRET'},
    expectedReason: 'recovery_secret_material',
    forbiddenFragment: 'RADISHLEX-RECOVERY-CODE-SECRET',
  ),
  SyncBridgeCommandContractRejectedSample(
    id: 'short_code_rejected',
    actionId: 'join_request_authorization',
    payload: {'short_code': 'short_code=123456'},
    expectedReason: 'join_verifier_material',
    forbiddenFragment: 'short_code=',
  ),
  SyncBridgeCommandContractRejectedSample(
    id: 'signature_and_wrapped_material_rejected',
    actionId: 'device_revocation',
    payload: {
      'signature_bytes': 'signature_bytes=abcdef',
      'wrapped_material_bytes': 'wrapped_material_bytes=abcdef',
    },
    expectedReason: 'signature_or_wrapped_sync_material',
    forbiddenFragment: 'signature_bytes',
  ),
  SyncBridgeCommandContractRejectedSample(
    id: 'transport_payload_rejected',
    actionId: 'join_request_authorization',
    payload: {
      'request_body': 'request_body=payload_bytes=abcdef',
      'response_body': 'response_body={"wrapped_material_bytes":"abcdef"}',
    },
    expectedReason: 'opaque_transport_content',
    forbiddenFragment: 'payload_bytes=abcdef',
  ),
  SyncBridgeCommandContractRejectedSample(
    id: 'local_path_rejected',
    actionId: 'recovery_setup',
    payload: {'local_path': '/synthetic/private/userdb.sqlite'},
    expectedReason: 'local_path_material',
    forbiddenFragment: '/synthetic/private',
  ),
];

SyncBridgeCommandContractActionFixture syncBridgeCommandContractFixtureFor(
  String actionId,
) {
  return syncBridgeCommandContractActionFixtures.firstWhere(
    (fixture) => fixture.actionId == actionId,
  );
}

Map<String, Object?> syncBridgeCommandContractSafeRequestShape(
  SyncBridgeCommandContractActionFixture fixture,
) {
  return {
    'format': 'future_manager_bridge_command_request.v1_draft',
    'review_status': syncBridgeCommandContractReviewStatus,
    'action_id': fixture.actionId,
    'command_status': 'request_shape_ready_for_future_contract_review',
    'readiness_snapshot_binding': 'current_sanitized_readiness_snapshot',
    'operation_id_policy': 'non_sensitive_random_operation_id',
    for (final field in fixture.requestSafeFields) field: 'safe_summary',
  };
}

Map<String, Object?> syncBridgeCommandContractSafeResultShape(
  SyncBridgeCommandContractActionFixture fixture,
) {
  return {
    'format': 'future_manager_bridge_command_result.v1_draft',
    'review_status': syncBridgeCommandContractReviewStatus,
    'action_id': fixture.actionId,
    'command_status': 'result_shape_ready_for_future_contract_review',
    'retry_policy': 'not_retryable',
    for (final field in fixture.resultSafeFields) field: 'safe_summary',
  };
}

Map<String, Object?> syncBridgeCommandContractErrorEnvelope(
  SyncBridgeCommandContractActionFixture fixture,
  String errorCode,
) {
  return {
    'action_id': fixture.actionId,
    'command_status': 'blocked_by_readiness',
    'error_code': errorCode,
    'retry_policy': 'retry_after_user_action',
    'user_visible_summary_code': errorCode,
    'diagnostics_summary_code': errorCode,
    'next_required_evidence': 'safe_summary',
  };
}

List<String> syncBridgeCommandContractSplitSummary(String summary) {
  if (summary == 'none') {
    return const [];
  }
  return summary
      .split(',')
      .map((part) => part.trim())
      .where((part) => part.isNotEmpty && part != 'none')
      .toList(growable: false);
}
