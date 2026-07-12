part of 'manager_sync_entry_models.dart';

const managerSyncActionCommandPreviewFormat =
    'manager_sync_action_command_preview.v1';
const managerSyncActionCommandPreviewDataPolicy =
    'summary_only_no_secret_material_or_payload';

class SyncActionRequestPreview {
  const SyncActionRequestPreview({
    required this.actionId,
    required this.requestStatus,
    required this.boundary,
    required this.allowedFieldCodes,
    required this.forbiddenMaterialCodes,
  });

  final String actionId;
  final String requestStatus;
  final String boundary;
  final List<String> allowedFieldCodes;
  final List<String> forbiddenMaterialCodes;

  String get statusSummary {
    return '$actionId=$requestStatus';
  }

  String get allowedFieldSummary {
    return managerSyncCodeSummary(allowedFieldCodes);
  }

  String get forbiddenMaterialSummary {
    return managerSyncCodeSummary(forbiddenMaterialCodes);
  }
}

class SyncActionResultPreview {
  const SyncActionResultPreview({
    required this.actionId,
    required this.resultStatus,
    required this.boundary,
    required this.allowedFieldCodes,
    required this.forbiddenMaterialCodes,
  });

  final String actionId;
  final String resultStatus;
  final String boundary;
  final List<String> allowedFieldCodes;
  final List<String> forbiddenMaterialCodes;

  String get statusSummary {
    return '$actionId=$resultStatus';
  }

  String get allowedFieldSummary {
    return managerSyncCodeSummary(allowedFieldCodes);
  }

  String get forbiddenMaterialSummary {
    return managerSyncCodeSummary(forbiddenMaterialCodes);
  }
}

class SyncActionCommandPreview {
  const SyncActionCommandPreview({
    required this.actionId,
    required this.visibilityStatus,
    required this.intentStatus,
    required this.executionStatus,
    required this.blocker,
    required this.requiredEvidenceCodes,
    required this.sourceTag,
    required this.dataPolicy,
    required this.stopLine,
    required this.requestBoundary,
    required this.resultBoundary,
    required this.errorCodes,
    required this.requestPreview,
    required this.resultPreview,
  });

  final String actionId;
  final String visibilityStatus;
  final String intentStatus;
  final String executionStatus;
  final String blocker;
  final List<String> requiredEvidenceCodes;
  final String sourceTag;
  final String dataPolicy;
  final String stopLine;
  final String requestBoundary;
  final String resultBoundary;
  final List<String> errorCodes;
  final SyncActionRequestPreview requestPreview;
  final SyncActionResultPreview resultPreview;

  String get requiredEvidenceSummary {
    return managerSyncCodeSummary(requiredEvidenceCodes);
  }

  String get errorCodeSummary {
    return managerSyncCodeSummary(errorCodes);
  }

  String get visibilitySummary {
    return '$actionId=$visibilityStatus';
  }

  String get intentSummary {
    return '$actionId=$intentStatus';
  }

  String get executionSummary {
    return '$actionId=$executionStatus';
  }
}

class SyncActionCommandPreviewPlan {
  const SyncActionCommandPreviewPlan({required this.previews});

  final List<SyncActionCommandPreview> previews;

  SyncActionCommandPreview previewFor(String actionId) {
    return previews.firstWhere(
      (preview) => preview.actionId == actionId,
      orElse: () => SyncActionCommandPreview(
        actionId: actionId,
        visibilityStatus: 'hidden',
        intentStatus: 'blocked',
        executionStatus: 'blocked_by_missing_intent',
        blocker: 'interaction_intent_missing',
        requiredEvidenceCodes: const ['interaction_intent_missing'],
        sourceTag: 'manager_sync_action_command_preview',
        dataPolicy: managerSyncActionCommandPreviewDataPolicy,
        stopLine: 'no_bridge_command_without_interaction_intent',
        requestBoundary: 'no_request_without_interaction_intent',
        resultBoundary: 'no_result_without_interaction_intent',
        errorCodes: const ['interaction_intent_missing'],
        requestPreview: SyncActionRequestPreview(
          actionId: actionId,
          requestStatus: 'request_blocked_by_missing_intent',
          boundary: 'no_request_without_interaction_intent',
          allowedFieldCodes: const ['none'],
          forbiddenMaterialCodes: const ['no_secret_material_or_payload'],
        ),
        resultPreview: SyncActionResultPreview(
          actionId: actionId,
          resultStatus: 'result_blocked_by_missing_intent',
          boundary: 'no_result_without_interaction_intent',
          allowedFieldCodes: const ['none'],
          forbiddenMaterialCodes: const ['no_secret_material_or_payload'],
        ),
      ),
    );
  }

  String get actionIdSummary {
    return managerSyncCodeSummary(previews.map((preview) => preview.actionId));
  }

  String get visibilitySummary {
    if (previews.isEmpty) {
      return 'none';
    }
    return previews.map((preview) => preview.visibilitySummary).join(', ');
  }

  String get intentStatusSummary {
    if (previews.isEmpty) {
      return 'none';
    }
    return previews.map((preview) => preview.intentSummary).join(', ');
  }

  String get executionStatusSummary {
    if (previews.isEmpty) {
      return 'none';
    }
    return previews.map((preview) => preview.executionSummary).join(', ');
  }

  String get blockerSummary {
    return managerSyncCodeSummary(previews.map((preview) => preview.blocker));
  }

  String get requiredEvidenceSummary {
    return managerSyncCodeSummary(
      previews.expand((preview) => preview.requiredEvidenceCodes),
    );
  }

  String get sourceTagSummary {
    return managerSyncCodeSummary(previews.map((preview) => preview.sourceTag));
  }

  String get dataPolicySummary {
    return managerSyncCodeSummary(
      previews.map((preview) => preview.dataPolicy),
    );
  }

  String get stopLineSummary {
    return managerSyncCodeSummary(previews.map((preview) => preview.stopLine));
  }

  String get requestBoundarySummary {
    return managerSyncCodeSummary(
      previews.map((preview) => preview.requestBoundary),
    );
  }

  String get resultBoundarySummary {
    return managerSyncCodeSummary(
      previews.map((preview) => preview.resultBoundary),
    );
  }

  String get errorCodeSummary {
    return managerSyncCodeSummary(
      previews.expand((preview) => preview.errorCodes),
    );
  }

  String get requestStatusSummary {
    if (previews.isEmpty) {
      return 'none';
    }
    return previews
        .map((preview) => preview.requestPreview.statusSummary)
        .join(', ');
  }

  String get requestAllowedFieldSummary {
    return managerSyncCodeSummary(
      previews.expand((preview) => preview.requestPreview.allowedFieldCodes),
    );
  }

  String get requestForbiddenMaterialSummary {
    return managerSyncCodeSummary(
      previews.expand(
        (preview) => preview.requestPreview.forbiddenMaterialCodes,
      ),
    );
  }

  String get resultStatusSummary {
    if (previews.isEmpty) {
      return 'none';
    }
    return previews
        .map((preview) => preview.resultPreview.statusSummary)
        .join(', ');
  }

  String get resultAllowedFieldSummary {
    return managerSyncCodeSummary(
      previews.expand((preview) => preview.resultPreview.allowedFieldCodes),
    );
  }

  String get resultForbiddenMaterialSummary {
    return managerSyncCodeSummary(
      previews.expand(
        (preview) => preview.resultPreview.forbiddenMaterialCodes,
      ),
    );
  }
}

SyncActionCommandPreviewPlan
managerSyncActionCommandPreviewPlanFromInteractionPlan(
  SyncInteractionEntryPlan interactionPlan,
) {
  return SyncActionCommandPreviewPlan(
    previews: interactionPlan.intents
        .map(_syncActionCommandPreviewFromIntent)
        .toList(growable: false),
  );
}

SyncActionCommandPreview _syncActionCommandPreviewFromIntent(
  SyncInteractionActionIntent intent,
) {
  final executionStatus = _syncActionCommandExecutionStatus(
    intent.intentStatus,
  );
  final requestBoundary = _syncActionCommandRequestBoundary(intent.actionId);
  final resultBoundary = _syncActionCommandResultBoundary(intent.actionId);
  return SyncActionCommandPreview(
    actionId: intent.actionId,
    visibilityStatus: intent.visibilityStatus,
    intentStatus: intent.intentStatus,
    executionStatus: executionStatus,
    blocker: intent.blocker,
    requiredEvidenceCodes: intent.requiredEvidenceCodes,
    sourceTag: intent.sourceTag,
    dataPolicy: _syncActionCommandDataPolicy(intent.actionId),
    stopLine: _syncActionCommandStopLine(intent.actionId),
    requestBoundary: requestBoundary,
    resultBoundary: resultBoundary,
    errorCodes: _syncActionCommandErrorCodes(intent.actionId),
    requestPreview: _syncActionRequestPreview(
      actionId: intent.actionId,
      executionStatus: executionStatus,
      boundary: requestBoundary,
    ),
    resultPreview: _syncActionResultPreview(
      actionId: intent.actionId,
      executionStatus: executionStatus,
      boundary: resultBoundary,
    ),
  );
}

String _syncActionCommandExecutionStatus(String intentStatus) {
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

String _syncActionCommandDataPolicy(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return 'no_recovery_code_or_wrapped_material';
    case 'recovery_restore':
      return 'no_recovery_code_input_or_device_secret';
    case 'join_request_authorization':
      return 'no_short_code_signature_or_wrapped_material';
    case 'device_revocation':
      return 'no_signature_key_epoch_or_wrapped_material';
    default:
      return managerSyncActionCommandPreviewDataPolicy;
  }
}

String _syncActionCommandStopLine(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return 'no_recovery_code_generation_current_phase';
    case 'recovery_restore':
      return 'no_recovery_code_input_current_phase';
    case 'join_request_authorization':
      return 'no_join_request_or_authorization_package_current_phase';
    case 'device_revocation':
      return 'no_device_revocation_current_phase';
    default:
      return 'no_unknown_bridge_command_current_phase';
  }
}

String _syncActionCommandRequestBoundary(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return 'request_summary_only_no_recovery_code_generation';
    case 'recovery_restore':
      return 'request_summary_only_no_recovery_code_input';
    case 'join_request_authorization':
      return 'request_summary_only_no_join_request_or_short_code';
    case 'device_revocation':
      return 'request_summary_only_no_device_signature_or_key_epoch';
    default:
      return 'request_summary_only_no_unknown_bridge_command';
  }
}

String _syncActionCommandResultBoundary(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return 'result_summary_only_no_recovery_record_or_wrapped_material';
    case 'recovery_restore':
      return 'result_summary_only_no_unwrapped_device_material';
    case 'join_request_authorization':
      return 'result_summary_only_no_authorization_package_or_signature';
    case 'device_revocation':
      return 'result_summary_only_no_revocation_record_or_key_epoch_material';
    default:
      return 'result_summary_only_no_unknown_bridge_command';
  }
}

List<String> _syncActionCommandErrorCodes(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return const [
        'configuration_missing',
        'backend_unavailable',
        'deployment_unverified',
        'recovery_code_required',
        'recovery_record_missing',
        'recovery_record_revoked',
        'local_data_inconsistent',
      ];
    case 'recovery_restore':
      return const [
        'configuration_missing',
        'authentication_required',
        'deployment_unverified',
        'recovery_code_required',
        'recovery_code_invalid',
        'recovery_record_missing',
        'recovery_record_revoked',
        'network_unreachable',
        'local_data_inconsistent',
      ];
    case 'join_request_authorization':
      return const [
        'configuration_missing',
        'authentication_required',
        'backend_unavailable',
        'deployment_unverified',
        'join_request_expired',
        'authorization_rejected',
        'device_revoked',
        'network_unreachable',
        'local_data_inconsistent',
      ];
    case 'device_revocation':
      return const [
        'configuration_missing',
        'authentication_required',
        'backend_unavailable',
        'deployment_unverified',
        'device_revoked',
        'key_epoch_rotation_required',
        'network_unreachable',
        'local_data_inconsistent',
      ];
    default:
      return const ['unknown_action_id'];
  }
}

SyncActionRequestPreview _syncActionRequestPreview({
  required String actionId,
  required String executionStatus,
  required String boundary,
}) {
  return SyncActionRequestPreview(
    actionId: actionId,
    requestStatus: _syncActionRequestStatus(executionStatus),
    boundary: boundary,
    allowedFieldCodes: _syncActionRequestAllowedFields(actionId),
    forbiddenMaterialCodes: _syncActionForbiddenMaterialCodes,
  );
}

SyncActionResultPreview _syncActionResultPreview({
  required String actionId,
  required String executionStatus,
  required String boundary,
}) {
  return SyncActionResultPreview(
    actionId: actionId,
    resultStatus: _syncActionResultStatus(executionStatus),
    boundary: boundary,
    allowedFieldCodes: _syncActionResultAllowedFields(actionId),
    forbiddenMaterialCodes: _syncActionForbiddenMaterialCodes,
  );
}

String _syncActionRequestStatus(String executionStatus) {
  switch (executionStatus) {
    case 'ready_for_future_bridge_command':
      return 'request_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'request_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'request_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'request_not_built_current_phase';
    case 'blocked_by_missing_intent':
      return 'request_blocked_by_missing_intent';
    default:
      return 'request_blocked_by_unknown_execution_status';
  }
}

String _syncActionResultStatus(String executionStatus) {
  switch (executionStatus) {
    case 'ready_for_future_bridge_command':
      return 'result_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'result_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'result_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'result_not_available_current_phase';
    case 'blocked_by_missing_intent':
      return 'result_blocked_by_missing_intent';
    default:
      return 'result_blocked_by_unknown_execution_status';
  }
}

List<String> _syncActionRequestAllowedFields(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return const [
        'deployment_evidence_source_tag',
        'device_backend_gate',
        'explicit_user_start',
        'save_confirmation_status',
      ];
    case 'recovery_restore':
      return const [
        'deployment_evidence_source_tag',
        'restore_attempt_status',
        'recovery_record_lookup_status',
        'device_registration_status',
      ];
    case 'join_request_authorization':
      return const [
        'join_request_status',
        'short_code_verification_status',
        'active_device_requirement',
        'authorization_package_preconditions',
      ];
    case 'device_revocation':
      return const [
        'target_device_status_summary',
        'active_device_requirement',
        'lost_device_risk_acknowledgement',
        'key_epoch_status',
      ];
    default:
      return const ['unknown_action_summary_only'];
  }
}

List<String> _syncActionResultAllowedFields(String actionId) {
  switch (actionId) {
    case 'recovery_setup':
      return const [
        'status_code',
        'recovery_record_status',
        'first_upload_gate',
        'next_required_evidence',
      ];
    case 'recovery_restore':
      return const [
        'status_code',
        'restore_result_status',
        'device_registration_status',
        'next_required_evidence',
      ];
    case 'join_request_authorization':
      return const [
        'status_code',
        'authorization_package_status',
        'device_state_status',
        'next_required_evidence',
      ];
    case 'device_revocation':
      return const [
        'status_code',
        'revocation_record_status',
        'key_epoch_status',
        'next_required_evidence',
      ];
    default:
      return const ['unknown_result_summary_only'];
  }
}

const _syncActionForbiddenMaterialCodes = [
  'recovery_secret_material',
  'join_verifier_material',
  'bearer_credential_material',
  'signature_material',
  'wrapped_sync_material',
  'opaque_transport_content',
];
