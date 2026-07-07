import 'package:radishlex_manager/src/models/manager_models.dart';

const syncTransientSecretInteractionFormat =
    'future_manager_sync_transient_secret_interaction.v1_draft';

const syncTransientSecretInteractionReviewStatus =
    'future_interaction_review_only_no_secret_input';

const syncTransientSecretInteractionCurrentPhaseStatus = 'closed_current_phase';

const syncTransientSecretLifecycleRules = [
  'user_explicit_operation_only',
  'call_scope_or_visible_modal_scope_only',
  'clear_on_submit_cancel_navigation_or_timeout',
  'no_autofill_or_suggestion_persistence',
  'no_clipboard_auto_copy',
  'not_in_settings',
  'not_in_diagnostics',
  'not_in_logs',
  'not_in_test_golden',
  'not_in_crash_report',
  'not_in_route_arguments',
  'not_in_async_snapshot_state',
];

const syncTransientSecretForbiddenPersistenceTargets = [
  'settings_json',
  'diagnostics_export',
  'widget_text_snapshot',
  'test_golden',
  'log_line',
  'crash_report',
  'route_arguments',
  'clipboard_history',
  'analytics_event',
];

const syncTransientSecretInteractionFixtures = [
  SyncTransientSecretInteractionFixture(
    id: 'recovery_setup_one_time_display',
    actionId: 'recovery_setup',
    direction: 'one_time_display',
    secretRole: 'one_time_recovery_code_display_placeholder',
    currentPhaseStatus: 'display_not_available_current_phase',
    allowedUiStateFields: [
      'display_placeholder_status',
      'save_confirmation_status',
      'recovery_record_status',
      'first_upload_gate',
    ],
    allowedDiagnosticsFields: [
      'recovery_setup_status',
      'save_confirmation_status',
      'recovery_record_status',
      'first_upload_gate',
    ],
    completionEvidenceCodes: [
      'save_confirmation_ack_code_only',
      'recovery_record_status_summary',
      'first_upload_gate_summary',
    ],
    prohibitedOperations: [
      'generate_recovery_code',
      'persist_recovery_code',
      'unlock_first_upload',
    ],
    contractTransientFields: [
      'one_time_recovery_code_display_placeholder',
      'recovery_code_save_confirmation_ack',
    ],
  ),
  SyncTransientSecretInteractionFixture(
    id: 'recovery_restore_code_input',
    actionId: 'recovery_restore',
    direction: 'transient_input',
    secretRole: 'recovery_code_input_transient_placeholder',
    currentPhaseStatus: 'input_not_available_current_phase',
    allowedUiStateFields: [
      'restore_attempt_status',
      'recovery_record_lookup_status',
      'attempt_limit_status',
      'device_registration_status',
    ],
    allowedDiagnosticsFields: [
      'restore_attempt_status',
      'recovery_record_lookup_status',
      'attempt_limit_status',
      'device_registration_status',
    ],
    completionEvidenceCodes: [
      'restore_attempt_status_code',
      'recovery_record_lookup_summary',
      'device_registration_status_summary',
    ],
    prohibitedOperations: [
      'input_recovery_code',
      'unwrap_device_material',
      'register_restored_device',
    ],
    contractTransientFields: ['recovery_code_input_transient_placeholder'],
  ),
  SyncTransientSecretInteractionFixture(
    id: 'join_short_code_verification_input',
    actionId: 'join_request_authorization',
    direction: 'transient_input',
    secretRole: 'short_code_verification_transient_placeholder',
    currentPhaseStatus: 'input_not_available_current_phase',
    allowedUiStateFields: [
      'join_request_status',
      'short_code_verification_status',
      'authorization_package_preconditions',
      'authorization_package_status',
    ],
    allowedDiagnosticsFields: [
      'join_request_status',
      'short_code_verification_status',
      'authorization_package_preconditions',
      'authorization_package_status',
    ],
    completionEvidenceCodes: [
      'short_code_match_status_code',
      'authorization_package_precondition_summary',
    ],
    prohibitedOperations: [
      'create_join_request',
      'sign_authorization_package',
      'write_wrapped_material',
    ],
    contractTransientFields: ['short_code_verification_transient_placeholder'],
  ),
  SyncTransientSecretInteractionFixture(
    id: 'join_authorization_explicit_confirmation',
    actionId: 'join_request_authorization',
    direction: 'explicit_confirmation',
    secretRole: 'explicit_authorization_confirmation',
    currentPhaseStatus: 'confirmation_not_available_current_phase',
    allowedUiStateFields: [
      'join_request_status',
      'active_device_requirement',
      'short_code_verification_status',
      'authorization_package_preconditions',
    ],
    allowedDiagnosticsFields: [
      'join_request_status',
      'active_device_requirement',
      'authorization_package_preconditions',
    ],
    completionEvidenceCodes: [
      'explicit_authorization_ack_code_only',
      'authorization_package_status_summary',
    ],
    prohibitedOperations: [
      'sign_authorization_package',
      'write_wrapped_material',
    ],
    contractTransientFields: ['explicit_authorization_confirmation'],
  ),
  SyncTransientSecretInteractionFixture(
    id: 'device_revocation_explicit_confirmation',
    actionId: 'device_revocation',
    direction: 'explicit_confirmation',
    secretRole: 'explicit_revocation_confirmation',
    currentPhaseStatus: 'confirmation_not_available_current_phase',
    allowedUiStateFields: [
      'target_device_status_summary',
      'active_device_requirement',
      'lost_device_risk_acknowledgement',
      'key_epoch_status',
    ],
    allowedDiagnosticsFields: [
      'target_device_status_summary',
      'active_device_requirement',
      'lost_device_risk_acknowledgement',
      'key_epoch_status',
    ],
    completionEvidenceCodes: [
      'explicit_revocation_ack_code_only',
      'revocation_record_status_summary',
      'key_epoch_status_summary',
    ],
    prohibitedOperations: [
      'revoke_device',
      'sign_revocation_record',
      'advance_key_epoch',
    ],
    contractTransientFields: ['explicit_revocation_confirmation'],
  ),
];

class SyncTransientSecretInteractionFixture {
  const SyncTransientSecretInteractionFixture({
    required this.id,
    required this.actionId,
    required this.direction,
    required this.secretRole,
    required this.currentPhaseStatus,
    required this.allowedUiStateFields,
    required this.allowedDiagnosticsFields,
    required this.completionEvidenceCodes,
    required this.prohibitedOperations,
    required this.contractTransientFields,
  });

  final String id;
  final String actionId;
  final String direction;
  final String secretRole;
  final String currentPhaseStatus;
  final List<String> allowedUiStateFields;
  final List<String> allowedDiagnosticsFields;
  final List<String> completionEvidenceCodes;
  final List<String> prohibitedOperations;
  final List<String> contractTransientFields;

  String get allowedUiStateSummary {
    return managerSyncCodeSummary(allowedUiStateFields);
  }

  String get allowedDiagnosticsSummary {
    return managerSyncCodeSummary(allowedDiagnosticsFields);
  }

  String get completionEvidenceSummary {
    return managerSyncCodeSummary(completionEvidenceCodes);
  }
}

Map<String, Object?> syncTransientSecretInteractionShape(
  SyncTransientSecretInteractionFixture fixture,
) {
  return {
    'format': syncTransientSecretInteractionFormat,
    'review_status': syncTransientSecretInteractionReviewStatus,
    'current_phase': syncTransientSecretInteractionCurrentPhaseStatus,
    'id': fixture.id,
    'action_id': fixture.actionId,
    'direction': fixture.direction,
    'secret_role': fixture.secretRole,
    'current_phase_status': fixture.currentPhaseStatus,
    'allowed_ui_state_fields': fixture.allowedUiStateFields,
    'allowed_diagnostics_fields': fixture.allowedDiagnosticsFields,
    'completion_evidence_codes': fixture.completionEvidenceCodes,
    'lifecycle_rules': syncTransientSecretLifecycleRules,
    'forbidden_persistence_targets':
        syncTransientSecretForbiddenPersistenceTargets,
    'prohibited_operations': fixture.prohibitedOperations,
    'contract_transient_fields': fixture.contractTransientFields,
  };
}

List<String> syncTransientSecretCoveredContractFields() {
  return List.unmodifiable(
    {
      for (final fixture in syncTransientSecretInteractionFixtures)
        ...fixture.contractTransientFields,
    }.toList()..sort(),
  );
}
