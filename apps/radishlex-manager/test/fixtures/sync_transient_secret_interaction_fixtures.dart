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

const syncRecoveryVisibleLayerFormat =
    'future_manager_sync_recovery_visible_layer.v1_draft';

const syncRecoveryVisibleLayerReviewStatus =
    'visible_copy_confirmation_placeholder_only';

const syncRecoveryFutureConfirmationDetailFormat =
    'future_manager_sync_recovery_confirmation_detail.v1_draft';

const syncRecoveryFutureConfirmationDetailReviewStatus =
    'future_confirmation_detail_review_only_no_secret_capture';

const syncRecoveryVisibleLayerFixtures = [
  SyncRecoveryVisibleLayerFixture(
    id: 'recovery_setup_visible_copy_confirmation_placeholder',
    actionId: 'recovery_setup',
    interactionFixtureId: 'recovery_setup_one_time_display',
    visibleTextBoundaryCode: 'setup_visible_text_status_codes_only',
    confirmationBoundaryCode: 'save_confirmation_ack_code_only',
    uiStateFields: [
      'display_placeholder_status',
      'save_confirmation_status',
      'recovery_record_status',
      'first_upload_gate',
    ],
    uiExpectedStatusCodes: [
      'display_not_available_current_phase',
      'required_before_first_upload',
      'recovery_record_not_created',
      'blocked_until_recovery_code_saved',
    ],
    diagnosticsKeys: [
      'sync.recovery_setup_display_status',
      'sync.recovery_setup_save_confirmation',
      'sync.recovery_record_status',
      'sync.recovery_first_upload_gate',
    ],
    diagnosticsExpectedStatusCodes: [
      'display_not_available_current_phase',
      'required_before_first_upload',
      'recovery_record_not_created',
      'blocked_until_recovery_code_saved',
    ],
    blockedOperationCodes: [
      'render_secret_value',
      'copy_secret_value_to_clipboard',
      'save_secret_value_to_settings',
      'unlock_first_upload',
    ],
  ),
  SyncRecoveryVisibleLayerFixture(
    id: 'recovery_restore_visible_input_rate_device_placeholder',
    actionId: 'recovery_restore',
    interactionFixtureId: 'recovery_restore_code_input',
    visibleTextBoundaryCode: 'restore_visible_text_status_codes_only',
    confirmationBoundaryCode:
        'restore_confirmation_not_available_current_phase',
    uiStateFields: [
      'code_input_status',
      'recovery_record_lookup_status',
      'attempt_limit_status',
      'device_registration_status',
    ],
    uiExpectedStatusCodes: [
      'input_not_available_current_phase',
      'not_checked_current_phase',
      'not_started',
      'blocked_until_recovery_success',
    ],
    diagnosticsKeys: [
      'sync.recovery_restore_code_input',
      'sync.recovery_restore_lookup_status',
      'sync.recovery_restore_attempt_limit',
      'sync.recovery_restore_device_registration',
    ],
    diagnosticsExpectedStatusCodes: [
      'input_not_available_current_phase',
      'not_checked_current_phase',
      'not_started',
      'blocked_until_recovery_success',
    ],
    blockedOperationCodes: [
      'accept_secret_input',
      'submit_secret_input',
      'unwrap_device_material',
      'register_restored_device',
    ],
  ),
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
      'attempt_limit_status_summary',
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

const syncRecoveryFutureConfirmationDetailFixtures = [
  SyncRecoveryFutureConfirmationDetailFixture(
    id: 'recovery_setup_save_confirmation_detail',
    actionId: 'recovery_setup',
    visibleLayerFixtureId:
        'recovery_setup_visible_copy_confirmation_placeholder',
    interactionFixtureId: 'recovery_setup_one_time_display',
    detailBoundaryCode: 'setup_save_confirmation_detail_status_only',
    entryStatusCode: 'read_only_current_phase',
    userDecisionStatusCode: 'confirmation_not_available_current_phase',
    preconditionStatusCodes: [
      'platform_private_key_backend_ready',
      'release_deployment_evidence_summary_required',
      'explicit_user_start_required',
    ],
    confirmationStateCodes: [
      'display_not_available_current_phase',
      'required_before_first_upload',
      'blocked_until_recovery_code_saved',
    ],
    diagnosticsKeys: [
      'sync.recovery_setup_display_status',
      'sync.recovery_setup_save_confirmation',
      'sync.recovery_first_upload_gate',
    ],
    diagnosticsExpectedStatusCodes: [
      'display_not_available_current_phase',
      'required_before_first_upload',
      'blocked_until_recovery_code_saved',
    ],
    allowedEvidenceCodes: [
      'save_confirmation_ack_code_only',
      'recovery_record_status_summary',
      'first_upload_gate_summary',
    ],
    prohibitedOperationCodes: [
      'render_secret_value',
      'copy_secret_value_to_clipboard',
      'save_secret_value_to_settings',
      'unlock_first_upload',
    ],
    persistencePolicyCodes: [
      'settings_action_absent',
      'diagnostics_status_codes_only',
      'widget_text_status_codes_only',
      'route_argument_absent',
      'clipboard_auto_copy_blocked',
    ],
  ),
  SyncRecoveryFutureConfirmationDetailFixture(
    id: 'recovery_restore_attempt_confirmation_detail',
    actionId: 'recovery_restore',
    visibleLayerFixtureId:
        'recovery_restore_visible_input_rate_device_placeholder',
    interactionFixtureId: 'recovery_restore_code_input',
    detailBoundaryCode: 'restore_attempt_confirmation_detail_status_only',
    entryStatusCode: 'read_only_current_phase',
    userDecisionStatusCode: 'confirmation_not_available_current_phase',
    preconditionStatusCodes: [
      'release_deployment_evidence_summary_required',
      'recovery_record_lookup_required',
      'explicit_user_start_required',
    ],
    confirmationStateCodes: [
      'input_not_available_current_phase',
      'not_checked_current_phase',
      'not_started',
      'blocked_until_recovery_success',
    ],
    diagnosticsKeys: [
      'sync.recovery_restore_code_input',
      'sync.recovery_restore_lookup_status',
      'sync.recovery_restore_attempt_limit',
      'sync.recovery_restore_device_registration',
    ],
    diagnosticsExpectedStatusCodes: [
      'input_not_available_current_phase',
      'not_checked_current_phase',
      'not_started',
      'blocked_until_recovery_success',
    ],
    allowedEvidenceCodes: [
      'restore_attempt_status_code',
      'recovery_record_lookup_summary',
      'attempt_limit_status_summary',
      'device_registration_status_summary',
    ],
    prohibitedOperationCodes: [
      'accept_secret_input',
      'submit_secret_input',
      'unwrap_device_material',
      'register_restored_device',
    ],
    persistencePolicyCodes: [
      'settings_action_absent',
      'diagnostics_status_codes_only',
      'widget_text_status_codes_only',
      'route_argument_absent',
      'clipboard_auto_copy_blocked',
    ],
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

class SyncRecoveryVisibleLayerFixture {
  const SyncRecoveryVisibleLayerFixture({
    required this.id,
    required this.actionId,
    required this.interactionFixtureId,
    required this.visibleTextBoundaryCode,
    required this.confirmationBoundaryCode,
    required this.uiStateFields,
    required this.uiExpectedStatusCodes,
    required this.diagnosticsKeys,
    required this.diagnosticsExpectedStatusCodes,
    required this.blockedOperationCodes,
  });

  final String id;
  final String actionId;
  final String interactionFixtureId;
  final String visibleTextBoundaryCode;
  final String confirmationBoundaryCode;
  final List<String> uiStateFields;
  final List<String> uiExpectedStatusCodes;
  final List<String> diagnosticsKeys;
  final List<String> diagnosticsExpectedStatusCodes;
  final List<String> blockedOperationCodes;

  String get uiStateSummary {
    return managerSyncCodeSummary(uiStateFields);
  }

  String get uiStatusSummary {
    return managerSyncCodeSummary(uiExpectedStatusCodes);
  }

  String get diagnosticsKeySummary {
    return managerSyncCodeSummary(diagnosticsKeys);
  }

  String get diagnosticsStatusSummary {
    return managerSyncCodeSummary(diagnosticsExpectedStatusCodes);
  }

  String get blockedOperationSummary {
    return managerSyncCodeSummary(blockedOperationCodes);
  }
}

class SyncRecoveryFutureConfirmationDetailFixture {
  const SyncRecoveryFutureConfirmationDetailFixture({
    required this.id,
    required this.actionId,
    required this.visibleLayerFixtureId,
    required this.interactionFixtureId,
    required this.detailBoundaryCode,
    required this.entryStatusCode,
    required this.userDecisionStatusCode,
    required this.preconditionStatusCodes,
    required this.confirmationStateCodes,
    required this.diagnosticsKeys,
    required this.diagnosticsExpectedStatusCodes,
    required this.allowedEvidenceCodes,
    required this.prohibitedOperationCodes,
    required this.persistencePolicyCodes,
  });

  final String id;
  final String actionId;
  final String visibleLayerFixtureId;
  final String interactionFixtureId;
  final String detailBoundaryCode;
  final String entryStatusCode;
  final String userDecisionStatusCode;
  final List<String> preconditionStatusCodes;
  final List<String> confirmationStateCodes;
  final List<String> diagnosticsKeys;
  final List<String> diagnosticsExpectedStatusCodes;
  final List<String> allowedEvidenceCodes;
  final List<String> prohibitedOperationCodes;
  final List<String> persistencePolicyCodes;

  String get preconditionSummary {
    return managerSyncCodeSummary(preconditionStatusCodes);
  }

  String get confirmationStateSummary {
    return managerSyncCodeSummary(confirmationStateCodes);
  }

  String get diagnosticsKeySummary {
    return managerSyncCodeSummary(diagnosticsKeys);
  }

  String get diagnosticsStatusSummary {
    return managerSyncCodeSummary(diagnosticsExpectedStatusCodes);
  }

  String get allowedEvidenceSummary {
    return managerSyncCodeSummary(allowedEvidenceCodes);
  }

  String get prohibitedOperationSummary {
    return managerSyncCodeSummary(prohibitedOperationCodes);
  }

  String get persistencePolicySummary {
    return managerSyncCodeSummary(persistencePolicyCodes);
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

Map<String, Object?> syncRecoveryVisibleLayerShape(
  SyncRecoveryVisibleLayerFixture fixture,
) {
  return {
    'format': syncRecoveryVisibleLayerFormat,
    'review_status': syncRecoveryVisibleLayerReviewStatus,
    'current_phase': syncTransientSecretInteractionCurrentPhaseStatus,
    'id': fixture.id,
    'action_id': fixture.actionId,
    'interaction_fixture_id': fixture.interactionFixtureId,
    'visible_text_boundary_code': fixture.visibleTextBoundaryCode,
    'confirmation_boundary_code': fixture.confirmationBoundaryCode,
    'ui_state_fields': fixture.uiStateFields,
    'ui_expected_status_codes': fixture.uiExpectedStatusCodes,
    'diagnostics_keys': fixture.diagnosticsKeys,
    'diagnostics_expected_status_codes': fixture.diagnosticsExpectedStatusCodes,
    'blocked_operation_codes': fixture.blockedOperationCodes,
    'lifecycle_rules': syncTransientSecretLifecycleRules,
    'forbidden_persistence_targets':
        syncTransientSecretForbiddenPersistenceTargets,
  };
}

Map<String, Object?> syncRecoveryFutureConfirmationDetailShape(
  SyncRecoveryFutureConfirmationDetailFixture fixture,
) {
  return {
    'format': syncRecoveryFutureConfirmationDetailFormat,
    'review_status': syncRecoveryFutureConfirmationDetailReviewStatus,
    'current_phase': syncTransientSecretInteractionCurrentPhaseStatus,
    'id': fixture.id,
    'action_id': fixture.actionId,
    'visible_layer_fixture_id': fixture.visibleLayerFixtureId,
    'interaction_fixture_id': fixture.interactionFixtureId,
    'detail_boundary_code': fixture.detailBoundaryCode,
    'entry_status_code': fixture.entryStatusCode,
    'user_decision_status_code': fixture.userDecisionStatusCode,
    'precondition_status_codes': fixture.preconditionStatusCodes,
    'confirmation_state_codes': fixture.confirmationStateCodes,
    'diagnostics_keys': fixture.diagnosticsKeys,
    'diagnostics_expected_status_codes': fixture.diagnosticsExpectedStatusCodes,
    'allowed_evidence_codes': fixture.allowedEvidenceCodes,
    'prohibited_operation_codes': fixture.prohibitedOperationCodes,
    'persistence_policy_codes': fixture.persistencePolicyCodes,
    'lifecycle_rules': syncTransientSecretLifecycleRules,
    'forbidden_persistence_targets':
        syncTransientSecretForbiddenPersistenceTargets,
  };
}

List<String> syncRecoveryVisibleLayerDiagnosticsKeys() {
  return List.unmodifiable(
    {
      for (final fixture in syncRecoveryVisibleLayerFixtures)
        ...fixture.diagnosticsKeys,
    }.toList()..sort(),
  );
}

List<String> syncRecoveryFutureConfirmationDetailIds() {
  return List.unmodifiable(
    syncRecoveryFutureConfirmationDetailFixtures
        .map((fixture) => fixture.id)
        .toList(),
  );
}

List<String> syncTransientSecretCoveredContractFields() {
  return List.unmodifiable(
    {
      for (final fixture in syncTransientSecretInteractionFixtures)
        ...fixture.contractTransientFields,
    }.toList()..sort(),
  );
}
