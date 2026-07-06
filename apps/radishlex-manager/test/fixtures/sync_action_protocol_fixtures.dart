import 'package:radishlex_manager/src/models/manager_models.dart';

const syncActionCommandDataPolicySummary =
    'no_recovery_code_or_wrapped_material, no_recovery_code_input_or_device_secret, no_short_code_signature_or_wrapped_material, no_signature_key_epoch_or_wrapped_material';

const syncActionCommandStopLineSummary =
    'no_recovery_code_generation_current_phase, no_recovery_code_input_current_phase, no_join_request_or_authorization_package_current_phase, no_device_revocation_current_phase';

const syncActionCommandRequestBoundarySummary =
    'request_summary_only_no_recovery_code_generation, request_summary_only_no_recovery_code_input, request_summary_only_no_join_request_or_short_code, request_summary_only_no_device_signature_or_key_epoch';

const syncActionCommandResultBoundarySummary =
    'result_summary_only_no_recovery_record_or_wrapped_material, result_summary_only_no_unwrapped_device_material, result_summary_only_no_authorization_package_or_signature, result_summary_only_no_revocation_record_or_key_epoch_material';

const syncActionCommandErrorCodeSummary =
    'configuration_missing, backend_unavailable, deployment_unverified, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent, authentication_required, recovery_code_invalid, network_unreachable, join_request_expired, authorization_rejected, device_revoked, key_epoch_rotation_required';

const syncActionRequestAllowedFieldSummary =
    'deployment_evidence_source_tag, device_backend_gate, explicit_user_start, save_confirmation_status, restore_attempt_status, recovery_record_lookup_status, device_registration_status, join_request_status, short_code_verification_status, active_device_requirement, authorization_package_preconditions, target_device_status_summary, lost_device_risk_acknowledgement, key_epoch_status';

const syncActionResultAllowedFieldSummary =
    'status_code, recovery_record_status, first_upload_gate, next_required_evidence, restore_result_status, device_registration_status, authorization_package_status, device_state_status, revocation_record_status, key_epoch_status';

const syncActionForbiddenMaterialSummary =
    'recovery_secret_material, join_verifier_material, bearer_credential_material, signature_material, wrapped_sync_material, opaque_transport_content';

class SyncActionProtocolExpectation {
  const SyncActionProtocolExpectation({
    required this.actionId,
    required this.dataPolicy,
    required this.stopLine,
    required this.requestBoundary,
    required this.resultBoundary,
    required this.requestAllowedFieldSummary,
    required this.resultAllowedFieldSummary,
    required this.errorCodeSummary,
  });

  final String actionId;
  final String dataPolicy;
  final String stopLine;
  final String requestBoundary;
  final String resultBoundary;
  final String requestAllowedFieldSummary;
  final String resultAllowedFieldSummary;
  final String errorCodeSummary;
}

const syncActionProtocolExpectations = [
  SyncActionProtocolExpectation(
    actionId: 'recovery_setup',
    dataPolicy: 'no_recovery_code_or_wrapped_material',
    stopLine: 'no_recovery_code_generation_current_phase',
    requestBoundary: 'request_summary_only_no_recovery_code_generation',
    resultBoundary:
        'result_summary_only_no_recovery_record_or_wrapped_material',
    requestAllowedFieldSummary:
        'deployment_evidence_source_tag, device_backend_gate, explicit_user_start, save_confirmation_status',
    resultAllowedFieldSummary:
        'status_code, recovery_record_status, first_upload_gate, next_required_evidence',
    errorCodeSummary:
        'configuration_missing, backend_unavailable, deployment_unverified, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
  ),
  SyncActionProtocolExpectation(
    actionId: 'recovery_restore',
    dataPolicy: 'no_recovery_code_input_or_device_secret',
    stopLine: 'no_recovery_code_input_current_phase',
    requestBoundary: 'request_summary_only_no_recovery_code_input',
    resultBoundary: 'result_summary_only_no_unwrapped_device_material',
    requestAllowedFieldSummary:
        'deployment_evidence_source_tag, restore_attempt_status, recovery_record_lookup_status, device_registration_status',
    resultAllowedFieldSummary:
        'status_code, restore_result_status, device_registration_status, next_required_evidence',
    errorCodeSummary:
        'configuration_missing, authentication_required, deployment_unverified, recovery_code_required, recovery_code_invalid, recovery_record_missing, recovery_record_revoked, network_unreachable, local_data_inconsistent',
  ),
  SyncActionProtocolExpectation(
    actionId: 'join_request_authorization',
    dataPolicy: 'no_short_code_signature_or_wrapped_material',
    stopLine: 'no_join_request_or_authorization_package_current_phase',
    requestBoundary: 'request_summary_only_no_join_request_or_short_code',
    resultBoundary: 'result_summary_only_no_authorization_package_or_signature',
    requestAllowedFieldSummary:
        'join_request_status, short_code_verification_status, active_device_requirement, authorization_package_preconditions',
    resultAllowedFieldSummary:
        'status_code, authorization_package_status, device_state_status, next_required_evidence',
    errorCodeSummary:
        'configuration_missing, authentication_required, backend_unavailable, deployment_unverified, join_request_expired, authorization_rejected, device_revoked, network_unreachable, local_data_inconsistent',
  ),
  SyncActionProtocolExpectation(
    actionId: 'device_revocation',
    dataPolicy: 'no_signature_key_epoch_or_wrapped_material',
    stopLine: 'no_device_revocation_current_phase',
    requestBoundary: 'request_summary_only_no_device_signature_or_key_epoch',
    resultBoundary:
        'result_summary_only_no_revocation_record_or_key_epoch_material',
    requestAllowedFieldSummary:
        'target_device_status_summary, active_device_requirement, lost_device_risk_acknowledgement, key_epoch_status',
    resultAllowedFieldSummary:
        'status_code, revocation_record_status, key_epoch_status, next_required_evidence',
    errorCodeSummary:
        'configuration_missing, authentication_required, backend_unavailable, deployment_unverified, device_revoked, key_epoch_rotation_required, network_unreachable, local_data_inconsistent',
  ),
];

class SyncActionProtocolStatusScenario {
  const SyncActionProtocolStatusScenario({
    required this.id,
    required this.title,
    required this.intentStatus,
    required this.blocker,
    required this.requiredEvidenceCodes,
    required this.expectedExecutionStatus,
    required this.expectedRequestStatus,
    required this.expectedResultStatus,
  });

  final String id;
  final String title;
  final String intentStatus;
  final String blocker;
  final List<String> requiredEvidenceCodes;
  final String expectedExecutionStatus;
  final String expectedRequestStatus;
  final String expectedResultStatus;

  String get requiredEvidenceSummary {
    return managerSyncCodeSummary(requiredEvidenceCodes);
  }
}

const syncActionProtocolStatusScenarios = [
  SyncActionProtocolStatusScenario(
    id: 'current_phase_closed',
    title: '当前阶段关闭',
    intentStatus: 'closed_current_phase',
    blocker: 'user_sync_entry_closed_current_phase',
    requiredEvidenceCodes: ['user_sync_entry_current_phase_open_required'],
    expectedExecutionStatus: 'not_executable_current_phase',
    expectedRequestStatus: 'request_not_built_current_phase',
    expectedResultStatus: 'result_not_available_current_phase',
  ),
  SyncActionProtocolStatusScenario(
    id: 'readiness_blocked',
    title: 'readiness 阻塞',
    intentStatus: 'blocked',
    blocker: 'backend_unavailable',
    requiredEvidenceCodes: ['platform_private_key_backend_ready'],
    expectedExecutionStatus: 'blocked_by_readiness',
    expectedRequestStatus: 'request_blocked_by_readiness',
    expectedResultStatus: 'result_blocked_by_readiness',
  ),
  SyncActionProtocolStatusScenario(
    id: 'user_confirmation_required',
    title: '需要用户确认',
    intentStatus: 'requires_confirmation',
    blocker: 'user_confirmation_required',
    requiredEvidenceCodes: [
      'blocked_until_recovery_code_saved',
      'explicit_user_confirmation_required',
    ],
    expectedExecutionStatus: 'blocked_until_user_confirmation',
    expectedRequestStatus: 'request_blocked_until_user_confirmation',
    expectedResultStatus: 'result_blocked_until_user_confirmation',
  ),
  SyncActionProtocolStatusScenario(
    id: 'future_ready_shape',
    title: 'future bridge shape ready',
    intentStatus: 'ready',
    blocker: 'none',
    requiredEvidenceCodes: [],
    expectedExecutionStatus: 'ready_for_future_bridge_command',
    expectedRequestStatus: 'request_shape_ready_for_future_bridge',
    expectedResultStatus: 'result_shape_ready_for_future_bridge',
  ),
  SyncActionProtocolStatusScenario(
    id: 'unknown_intent_status',
    title: '未知 intent status 降级',
    intentStatus: 'provider_future_status',
    blocker: 'unknown_bridge_intent_status',
    requiredEvidenceCodes: ['unexpected_bridge_required_evidence'],
    expectedExecutionStatus: 'blocked_by_unknown_intent_status',
    expectedRequestStatus: 'request_blocked_by_unknown_execution_status',
    expectedResultStatus: 'result_blocked_by_unknown_execution_status',
  ),
];

String syncActionExpectedExecutionSummary(String intentStatusSummary) {
  return _syncActionExpectedStatusSummary(
    intentStatusSummary,
    syncActionExpectedExecutionStatus,
  );
}

String syncActionExpectedRequestStatusSummary(String intentStatusSummary) {
  return _syncActionExpectedStatusSummary(
    intentStatusSummary,
    syncActionExpectedRequestStatus,
  );
}

String syncActionExpectedResultStatusSummary(String intentStatusSummary) {
  return _syncActionExpectedStatusSummary(
    intentStatusSummary,
    syncActionExpectedResultStatus,
  );
}

List<String> syncActionExpectedExecutionStatuses(String intentStatusSummary) {
  return _syncActionExpectedStatuses(
    intentStatusSummary,
    syncActionExpectedExecutionStatus,
  );
}

List<String> syncActionExpectedRequestStatuses(String intentStatusSummary) {
  return _syncActionExpectedStatuses(
    intentStatusSummary,
    syncActionExpectedRequestStatus,
  );
}

List<String> syncActionExpectedResultStatuses(String intentStatusSummary) {
  return _syncActionExpectedStatuses(
    intentStatusSummary,
    syncActionExpectedResultStatus,
  );
}

String syncActionExpectedExecutionStatus(String intentStatus) {
  switch (intentStatus) {
    case 'ready':
      return 'ready_for_future_bridge_command';
    case 'requires_confirmation':
      return 'blocked_until_user_confirmation';
    case 'blocked':
      return 'blocked_by_readiness';
    case 'closed_current_phase':
      return 'not_executable_current_phase';
    default:
      return 'blocked_by_unknown_intent_status';
  }
}

String syncActionExpectedRequestStatus(String intentStatus) {
  switch (syncActionExpectedExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'request_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'request_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'request_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'request_not_built_current_phase';
    default:
      return 'request_blocked_by_unknown_execution_status';
  }
}

String syncActionExpectedResultStatus(String intentStatus) {
  switch (syncActionExpectedExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'result_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'result_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'result_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'result_not_available_current_phase';
    default:
      return 'result_blocked_by_unknown_execution_status';
  }
}

String _syncActionExpectedStatusSummary(
  String intentStatusSummary,
  String Function(String intentStatus) mapStatus,
) {
  if (intentStatusSummary == 'none') {
    return 'none';
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) {
        final separator = entry.indexOf('=');
        final actionId = entry.substring(0, separator);
        final intentStatus = entry.substring(separator + 1);
        return '$actionId=${mapStatus(intentStatus)}';
      })
      .join(', ');
}

List<String> _syncActionExpectedStatuses(
  String intentStatusSummary,
  String Function(String intentStatus) mapStatus,
) {
  if (intentStatusSummary == 'none') {
    return const ['none'];
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) => entry.substring(entry.indexOf('=') + 1))
      .map(mapStatus)
      .toSet()
      .toList(growable: false);
}
