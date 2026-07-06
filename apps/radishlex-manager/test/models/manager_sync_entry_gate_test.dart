import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  const readyDevice = DeviceSecuritySummary(
    deviceId: 'device-ready-01',
    backendId: 'test-production-ready',
    capabilityStatus: 'ready_for_test',
    productionGate: 'ready',
  );

  const blockedDevice = DeviceSecuritySummary(
    deviceId: 'device-blocked-01',
    backendId: 'android-keystore-v1',
    capabilityStatus: 'unsupported_signature_algorithm',
    productionGate: 'blocked',
  );

  test('readiness summary import accepts safe v1 summaries', () {
    final imported = importManagerSyncReadinessBridgeSummaryFromJson(
      readySyncReadinessBridgeJson(),
    );

    expect(imported.accepted, isTrue);
    expect(imported.errorCode, isEmpty);
    expect(imported.snapshot.source, 'ffi_native_readiness');
    expect(
      imported.snapshot.recoverySetupReadiness.status,
      'recovery_setup_ready',
    );
    expect(
      imported.snapshot.deviceJoinReadiness.authorizationPackagePreconditions,
      'satisfied',
    );
  });

  test('readiness summary import rejects unsupported format and redaction', () {
    final unsupportedFormat = importManagerSyncReadinessBridgeSummaryFromJson({
      ...readySyncReadinessBridgeJson(),
      'format': 'manager_sync_readiness.v2',
    });
    final unsupportedRedaction =
        importManagerSyncReadinessBridgeSummaryFromJson({
          ...readySyncReadinessBridgeJson(),
          'redaction_policy': 'raw_native_payload_with_secret_fields',
        });

    expect(unsupportedFormat.accepted, isFalse);
    expect(unsupportedFormat.errorCode, 'readiness_summary_format_unsupported');
    expect(
      unsupportedFormat.snapshot.source,
      managerSyncReadinessBridgeSourceDefault,
    );
    expect(unsupportedRedaction.accepted, isFalse);
    expect(
      unsupportedRedaction.errorCode,
      'readiness_summary_redaction_policy_unsupported',
    );
    expect(
      unsupportedRedaction.snapshot.source,
      managerSyncReadinessBridgeSourceDefault,
    );
  });

  test('readiness summary import enforces envelope schema', () {
    final cases = [
      const _RejectedReadinessImportCase(
        json: {},
        errorCode: 'readiness_summary_format_unsupported',
      ),
      const _RejectedReadinessImportCase(
        json: {'format': managerSyncReadinessBridgeSummaryFormat},
        errorCode: 'readiness_summary_redaction_policy_unsupported',
      ),
      const _RejectedReadinessImportCase(
        json: {
          'format': 1,
          'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
        },
        errorCode: 'readiness_summary_format_unsupported',
      ),
      const _RejectedReadinessImportCase(
        json: {
          'format': managerSyncReadinessBridgeSummaryFormat,
          'redaction_policy': ['raw_native_payload'],
        },
        errorCode: 'readiness_summary_redaction_policy_unsupported',
      ),
    ];

    for (final importCase in cases) {
      final imported = importManagerSyncReadinessBridgeSummaryFromJson(
        importCase.json,
      );

      expect(imported.accepted, isFalse, reason: importCase.errorCode);
      expect(imported.errorCode, importCase.errorCode);
      expect(imported.snapshot.source, managerSyncReadinessBridgeSourceDefault);
    }
  });

  test('readiness summary import accepts and sanitizes unknown fields', () {
    final imported = importManagerSyncReadinessBridgeSummaryFromJson(
      unsafeSyncReadinessBridgeJson(),
    );

    expect(imported.accepted, isTrue);
    expect(imported.snapshot.source, 'unknown_bridge_readiness_source');
    expect(
      imported.snapshot.recoverySetupReadiness.status,
      managerClosedRecoverySetupReadiness.status,
    );
    expect(
      imported.snapshot.recoverySetupReadiness.prerequisiteSummary,
      'platform_private_key_backend_ready, unexpected_bridge_required_evidence',
    );
    expect(
      imported.snapshot.recoverySetupReadiness.errorCodeSummary,
      contains('unexpected_bridge_error_code'),
    );
    expect(
      imported.snapshot.deviceJoinReadiness.errorCodeSummary,
      contains('unexpected_bridge_error_code'),
    );
  });

  test('readiness summary import strips payload-shaped fields', () {
    final scenario = syncReadinessScenarioById(
      'unknown_native_status_sanitized',
    );
    final imported = importManagerSyncReadinessBridgeSummaryFromJson(
      scenario.summaryJson!(),
    );
    final snapshot = managerSnapshotForSyncReadinessScenario(scenario);
    final gate = deriveManagerSyncEntryGate(
      draft: snapshot.settings.draft,
      device: snapshot.sync.device,
      readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
    );
    final summary = [
      _readinessSnapshotSummary(imported.snapshot),
      _entryGateSummary(gate),
      createManagerDiagnosticsReport(snapshot).toRedactedText(),
    ].join('\n');

    expect(imported.accepted, isTrue);
    expect(summary, contains('unexpected_bridge_error_code'));
    expect(summary, contains('unexpected_bridge_required_evidence'));
    for (final fragment in syncReadinessSensitiveLeakFragments) {
      expect(summary, isNot(contains(fragment)), reason: fragment);
    }
  });

  test('readiness scenario catalog maps to stable gate summaries', () {
    for (final scenario in syncReadinessScenarioCatalog()) {
      var readinessBridgeSnapshot = managerDefaultSyncReadinessBridgeSnapshot;
      if (scenario.summaryJson != null) {
        final imported = importManagerSyncReadinessBridgeSummaryFromJson(
          scenario.summaryJson!(),
        );
        expect(
          imported.accepted,
          scenario.expectedImportAccepted,
          reason: scenario.id,
        );
        expect(
          imported.errorCode,
          scenario.expectedImportErrorCode,
          reason: scenario.id,
        );
        readinessBridgeSnapshot = imported.snapshot;
      }

      final gate = deriveManagerSyncEntryGate(
        draft: scenario.draft,
        device: scenario.device,
        readinessBridgeSnapshot: readinessBridgeSnapshot,
      );

      expect(gate.readinessBridgeSource, scenario.expectedBridgeSource);
      expect(gate.entryState, scenario.expectedEntryState);
      expect(gate.entryBlocker, scenario.expectedEntryBlocker);
      expect(gate.readinessBlockedFlowSummary, scenario.expectedBlockedFlows);
      if (scenario.expectedIssueCodes != null) {
        expect(gate.readinessIssueCodeSummary, scenario.expectedIssueCodes);
      }
      for (final code in scenario.expectedIssueCodeContains) {
        expect(gate.readinessIssueCodeSummary, contains(code));
      }
      for (final fragment in scenario.forbiddenIssueCodeFragments) {
        expect(gate.readinessIssueCodeSummary, isNot(contains(fragment)));
      }
      if (scenario.expectedNextEvidence != null) {
        expect(
          gate.readinessNextRequiredEvidenceSummary,
          scenario.expectedNextEvidence,
        );
      }
      for (final code in scenario.expectedNextEvidenceContains) {
        expect(gate.readinessNextRequiredEvidenceSummary, contains(code));
      }
      for (final fragment in scenario.forbiddenNextEvidenceFragments) {
        expect(
          gate.readinessNextRequiredEvidenceSummary,
          isNot(contains(fragment)),
        );
      }
      expect(
        gate.interactionEntryPlan.intentStatusSummary,
        scenario.expectedInteractionStatuses,
      );
      expect(
        gate.interactionEntryPlan.blockerSummary,
        scenario.expectedInteractionBlockers,
      );
      expect(gate.userSyncEnabled, scenario.expectedUserSyncEnabled);

      final summary = _entryGateSummary(gate);
      for (final fragment in syncReadinessSensitiveLeakFragments) {
        expect(summary, isNot(contains(fragment)), reason: scenario.id);
      }
    }
  });

  test('sync evidence bundle scenarios map to stable gate diagnostics', () {
    for (final scenario in syncEvidenceBundleScenarioCatalog()) {
      final readinessImport = importManagerSyncReadinessBridgeSummaryFromJson(
        scenario.readinessSummaryJson(),
      );
      expect(
        readinessImport.accepted,
        scenario.expectedReadinessImportAccepted,
        reason: scenario.id,
      );
      expect(
        readinessImport.errorCode,
        scenario.expectedReadinessImportErrorCode,
        reason: scenario.id,
      );

      final bundle = syncEvidenceBundleJsonForScenario(scenario);
      expect(bundle['format'], managerSyncEvidenceBundleFormat);
      expect(
        bundle['redaction_policy'],
        managerSyncEvidenceBundleRedactionPolicy,
      );

      final snapshot = managerSnapshotForSyncEvidenceBundleScenario(scenario);
      final gate = deriveManagerSyncEntryGate(
        draft: snapshot.settings.draft,
        device: snapshot.sync.device,
        readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
      );
      final health = gate.connectionHealth;

      expect(gate.readinessBridgeSource, scenario.expectedBridgeSource);
      expect(gate.entryState, scenario.expectedEntryState);
      expect(gate.entryBlocker, scenario.expectedEntryBlocker);
      expect(gate.readinessBlockedFlowSummary, scenario.expectedBlockedFlows);
      expect(
        gate.interactionEntryPlan.intentStatusSummary,
        scenario.expectedInteractionStatuses,
      );
      expect(
        gate.interactionEntryPlan.blockerSummary,
        scenario.expectedInteractionBlockers,
      );
      expect(gate.userSyncEnabled, scenario.expectedUserSyncEnabled);
      expect(
        snapshot.settings.draft.syncConnectionProbeRecord.isRecorded,
        isTrue,
      );

      expect(health.status.code, scenario.expectedConnectionStatusCode);
      expect(health.connectionBlocker, scenario.expectedConnectionBlocker);
      expect(health.probeSource, scenario.expectedConnectionProbeSource);
      expect(health.probeRecordedAt, syncEvidenceBundleRecordedAt);
      expect(health.endpointStatus, scenario.expectedEndpointStatus);
      expect(health.accessTokenStatus, scenario.expectedAccessTokenStatus);
      expect(health.transportMode, scenario.expectedTransportMode);
      expect(health.serverStateStatus, scenario.expectedServerStateStatus);
      expect(health.authStatus, scenario.expectedAuthStatus);
      expect(
        '${health.httpStatus} ${health.httpStatusClass}',
        scenario.expectedHttpStatusText,
      );
      expect(health.lastRemoteErrorCode, scenario.expectedLastRemoteErrorCode);
      expect(health.localInsecureTls, scenario.expectedLocalInsecureTls);
      if (scenario.expectedConnectionBlocker != 'none') {
        expect(
          gate.productionBlockers,
          isNot(contains(scenario.expectedConnectionBlocker)),
          reason: scenario.id,
        );
      }
      expect(
        gate.productionBlockers,
        contains('user_sync_entry_closed_current_phase'),
      );

      final diagnostics = createManagerDiagnosticsReport(
        snapshot,
      ).toRedactedText();
      expect(
        diagnostics,
        contains('sync.entry_blocker: ${scenario.expectedEntryBlocker}'),
      );
      expect(
        diagnostics,
        contains(
          'sync.readiness_bridge_source: ${scenario.expectedBridgeSource}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.connection_status: ${scenario.expectedConnectionStatusCode}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.connection_blocker: ${scenario.expectedConnectionBlocker}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.connection_probe_source: ${scenario.expectedConnectionProbeSource}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.server_state_status: ${scenario.expectedServerStateStatus}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_format: $managerSyncActionCommandPreviewFormat',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_intent_statuses: ${scenario.expectedInteractionStatuses}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_execution_statuses: ${syncActionExpectedExecutionSummary(scenario.expectedInteractionStatuses)}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_blockers: ${scenario.expectedInteractionBlockers}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_request_boundaries: ${scenario.expectedActionCommandRequestBoundaries}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_result_boundaries: ${scenario.expectedActionCommandResultBoundaries}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_command_error_codes: ${scenario.expectedActionCommandErrorCodes}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_request_statuses: ${syncActionExpectedRequestStatusSummary(scenario.expectedInteractionStatuses)}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_request_allowed_fields: ${scenario.expectedActionRequestAllowedFields}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_request_forbidden_material: ${scenario.expectedActionForbiddenMaterials}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_result_statuses: ${syncActionExpectedResultStatusSummary(scenario.expectedInteractionStatuses)}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_result_allowed_fields: ${scenario.expectedActionResultAllowedFields}',
        ),
      );
      expect(
        diagnostics,
        contains(
          'sync.action_result_forbidden_material: ${scenario.expectedActionForbiddenMaterials}',
        ),
      );

      final redactedSummary = [
        _entryGateSummary(gate),
        _connectionHealthSummary(health),
        diagnostics,
      ].join('\n');
      for (final fragment in syncEvidenceBundleSensitiveLeakFragments) {
        expect(redactedSummary, isNot(contains(fragment)), reason: scenario.id);
      }
    }
  });

  test('entry gate keeps privacy mode ahead of deployment evidence', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: true,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    );

    final gate = deriveManagerSyncEntryGate(draft: draft, device: readyDevice);

    expect(gate.entryState, SyncEntryState.syncDisabledByPolicy);
    expect(gate.entryBlocker, 'sync_disabled_by_policy');
    expect(gate.localEvidenceSource, managerDeploymentEvidenceExternalTls);
    expect(gate.userSyncEnabled, isFalse);
    expect(gate.uiState, SyncUiState.syncDisabledByPolicy);
    expect(gate.productionBlockers, contains('sync_disabled_by_policy'));
    expect(gate.recovery.status, RecoveryEntryStatus.flowClosed);
    expect(gate.recovery.blocker, 'recovery_code_flow_closed');
    expect(
      gate.recovery.saveConfirmationRequirement,
      'required_before_first_upload',
    );
    expect(gate.recovery.recoveryRecordStatus, 'recovery_record_not_created');
    expect(
      gate.recovery.recoveryRecordBlocker,
      'recovery_record_creation_closed',
    );
    expect(gate.recovery.firstUploadGate, 'blocked_until_recovery_code_saved');
    expect(
      gate.recovery.readinessBlockers,
      containsAll([
        'recovery_code_flow_closed',
        'recovery_record_not_created',
        'recovery_code_save_confirmation_required',
      ]),
    );
    expect(gate.recovery.setupReadiness.status, 'recovery_setup_flow_closed');
    expect(
      gate.recovery.setupReadiness.blocker,
      'recovery_code_generation_closed',
    );
    expect(
      gate.recovery.setupReadiness.prerequisiteSummary,
      contains('platform_private_key_backend_ready'),
    );
    expect(
      gate.recovery.setupReadiness.errorCodes,
      containsAll([
        'recovery_code_required',
        'recovery_record_missing',
        'recovery_record_revoked',
        'local_data_inconsistent',
      ]),
    );
    expect(
      gate.recovery.restoreReadiness.status,
      'recovery_restore_flow_closed',
    );
    expect(
      gate.recovery.restoreReadiness.blocker,
      'recovery_code_input_closed',
    );
    expect(
      gate.recovery.restoreReadiness.errorCodes,
      containsAll([
        'recovery_code_invalid',
        'authentication_required',
        'network_unreachable',
      ]),
    );
    expect(
      gate.deviceAuthorization.status,
      DeviceAuthorizationEntryStatus.flowClosed,
    );
    expect(
      gate.deviceAuthorization.joinRequestStatus,
      JoinRequestStatus.unavailable,
    );
    expect(
      gate.deviceAuthorization.authorizationPackageStatus,
      'authorization_package_not_created',
    );
    expect(
      gate.deviceAuthorization.authorizationPackageBlocker,
      'authorization_package_prerequisites_blocked',
    );
    expect(
      gate.deviceAuthorization.authorizationPackagePreconditions,
      contains('join_request_pending_required'),
    );
    expect(
      gate.deviceAuthorization.lostDeviceRiskNotice,
      'lost_device_prior_material_not_recallable',
    );
    expect(
      gate.deviceAuthorization.keyEpochStatus,
      'key_epoch_rotation_not_started',
    );
    expect(
      gate.deviceAuthorization.readinessBlockers,
      containsAll([
        'device_authorization_flow_closed',
        'join_request_unavailable',
        'authorization_package_prerequisites_blocked',
        'device_revocation_flow_closed',
        'lost_device_risk_notice_required',
        'key_epoch_rotation_not_started',
      ]),
    );
    expect(
      gate.deviceAuthorization.joinReadiness.status,
      'device_join_flow_closed',
    );
    expect(
      gate.deviceAuthorization.joinReadiness.blocker,
      'join_request_creation_closed',
    );
    expect(
      gate.deviceAuthorization.joinReadiness.shortCodeVerificationStatus,
      'short_code_verification_not_started',
    );
    expect(
      gate.deviceAuthorization.joinReadiness.errorCodes,
      containsAll([
        'join_request_expired',
        'authorization_rejected',
        'device_revoked',
        'backend_unavailable',
        'network_unreachable',
      ]),
    );
    expect(
      gate.deviceAuthorization.revocationReadiness.status,
      'device_revocation_flow_closed',
    );
    expect(
      gate.deviceAuthorization.revocationReadiness.activeDeviceRequirement,
      'active_existing_device_required',
    );
    expect(
      gate.deviceAuthorization.revocationReadiness.errorCodes,
      containsAll([
        'device_revoked',
        'key_epoch_rotation_required',
        'local_data_inconsistent',
        'network_unreachable',
      ]),
    );
    expect(
      gate.readinessBlockedFlowSummary,
      'recovery_setup, recovery_restore, device_join, device_revocation',
    );
    expect(
      gate.readinessIssueCodeSummary,
      'recovery_code_generation_closed, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent, recovery_code_input_closed, recovery_code_invalid, authentication_required, network_unreachable, join_request_creation_closed, join_request_expired, authorization_rejected, device_revoked, backend_unavailable, device_revocation_flow_closed, key_epoch_rotation_required',
    );
    expect(
      gate.readinessNextRequiredEvidenceSummary,
      'platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required, input_not_available_current_phase, not_checked_current_phase, blocked_until_recovery_success, join_request_unavailable, short_code_verification_not_started, active_existing_device_required, join_request_pending_required, short_code_match_required, lost_device_prior_material_not_recallable, key_epoch_rotation_not_started',
    );
    expect(
      gate.readinessSourceTagSummary,
      'recovery_setup_readiness, recovery_restore_readiness, device_join_readiness, device_revocation_readiness',
    );
    expect(gate.readinessUserSyncBlocked, isTrue);
    expect(
      gate.interactionEntryPlan.actionIdSummary,
      'recovery_setup, recovery_restore, join_request_authorization, device_revocation',
    );
    expect(
      gate.interactionEntryPlan.visibilitySummary,
      'recovery_setup=visible, recovery_restore=visible, join_request_authorization=visible, device_revocation=visible',
    );
    expect(
      gate.interactionEntryPlan.intentStatusSummary,
      'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
    );
    expect(
      gate.interactionEntryPlan.blockerSummary,
      'recovery_code_generation_closed, recovery_code_input_closed, join_request_creation_closed, device_revocation_flow_closed',
    );
    expect(
      gate.interactionEntryPlan.requiredEvidenceSummary,
      contains('blocked_until_recovery_success'),
    );
    expect(
      gate.interactionEntryPlan.sourceTagSummary,
      'recovery_setup_readiness, recovery_restore_readiness, device_join_readiness, device_revocation_readiness',
    );
  });

  test('entry gate reports blocked platform backend before evidence checks', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
    );

    final audit = managerSyncGateAuditForDraft(
      draft: draft,
      device: blockedDevice,
    );

    expect(audit.state, SyncUiState.backendUnavailable);
    expect(audit.entryGate.entryState, SyncEntryState.backendUnavailable);
    expect(audit.entryGate.entryBlocker, 'backend_unavailable');
    expect(audit.entryGate.recovery.status.code, 'recovery_code_flow_closed');
    expect(
      audit.entryGate.deviceAuthorization.status.code,
      'device_authorization_flow_closed',
    );
    expect(
      audit.entryGate.productionBlockers,
      contains('platform_private_key_backend_blocked'),
    );
    expect(
      audit.entryGate.connectionHealth.status,
      SyncConnectionStatus.accessTokenMissing,
    );
    expect(
      audit.entryGate.connectionHealth.connectionBlocker,
      'access_token_missing',
    );
    expect(audit.entryGate.userSyncEnabled, isFalse);
    expect(
      audit.entryGate.readinessBlockedFlowSummary,
      'recovery_setup, recovery_restore, device_join, device_revocation',
    );
    expect(
      audit.entryGate.interactionEntryPlan.intentStatusSummary,
      'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
    );
    expect(audit.entryGate.readinessUserSyncBlocked, isTrue);
  });

  test('local smoke supports development preflight without user sync', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      accessTokenConfigured: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
    );

    final gate = deriveManagerSyncEntryGate(draft: draft, device: readyDevice);

    expect(gate.entryState, SyncEntryState.localSmokeReady);
    expect(gate.entryBlocker, 'release_deployment_evidence_required');
    expect(gate.localEvidenceSource, managerDeploymentEvidenceLocalSmoke);
    expect(
      gate.connectionHealth.status,
      SyncConnectionStatus.externalProbeDeferred,
    );
    expect(gate.uiState, SyncUiState.preflightReady);
    expect(gate.userSyncEnabled, isFalse);
    expect(
      gate.recovery.setupReadiness.entryActionStatus,
      'read_only_current_phase',
    );
    expect(
      gate.recovery.restoreReadiness.codeInputStatus,
      'input_not_available_current_phase',
    );
    expect(
      gate.deviceAuthorization.joinReadiness.errorCodeSummary,
      contains('authorization_rejected'),
    );
    expect(
      gate.deviceAuthorization.revocationReadiness.errorCodeSummary,
      contains('key_epoch_rotation_required'),
    );
    expect(gate.readinessIssueCodeSummary, contains('authorization_rejected'));
    expect(
      gate.readinessNextRequiredEvidenceSummary,
      contains('release_deployment_evidence_summary_required'),
    );
    expect(
      gate.productionBlockers,
      containsAll([
        'release_deployment_evidence_required',
        'recovery_code_flow_closed',
        'recovery_record_not_created',
        'recovery_code_save_confirmation_required',
        'device_authorization_flow_closed',
        'join_request_unavailable',
        'authorization_package_prerequisites_blocked',
        'device_revocation_flow_closed',
        'lost_device_risk_notice_required',
        'key_epoch_rotation_not_started',
        'user_sync_entry_closed_current_phase',
      ]),
    );
  });

  test('local https endpoint with token is ready for read-only probe', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://localhost:7319',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      accessTokenConfigured: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
    );

    final gate = deriveManagerSyncEntryGate(draft: draft, device: readyDevice);

    expect(
      gate.connectionHealth.status,
      SyncConnectionStatus.localHttpsReadyForProbe,
    );
    expect(gate.connectionHealth.transportMode, 'local_https');
    expect(gate.connectionHealth.accessTokenStatus, 'configured');
    expect(gate.connectionHealth.serverStateStatus, 'read_only_probe_pending');
    expect(gate.connectionHealth.canRunReadOnlyProbe, isTrue);
  });

  test('connection health imports local docker probe summary', () {
    final summary = SyncConnectionProbeSummary.fromJson({
      'format': managerSyncConnectionHealthSummaryFormat,
      'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
      'endpoint_status': 'configured',
      'transport_mode': 'local_https',
      'access_token_status': 'not_configured',
      'connection_status': 'reachable',
      'auth_status': 'not_required_for_local_probe',
      'server_state_status': 'domain_missing_expected',
      'http_status': 404,
      'http_status_class': 'client_error',
      'last_remote_error_code': 'not_found',
      'local_insecure_tls': 'allowed',
    });

    final health = managerSyncConnectionHealthFromProbeSummary(summary);

    expect(health.status, SyncConnectionStatus.readOnlyProbeReachable);
    expect(health.status.code, 'reachable');
    expect(health.connectionBlocker, 'none');
    expect(health.endpointStatus, 'configured');
    expect(health.accessTokenStatus, 'not_configured');
    expect(health.transportMode, 'local_https');
    expect(health.serverStateStatus, 'domain_missing_expected');
    expect(health.lastRemoteErrorCode, 'not_found');
    expect(health.isConnectionHealthy, isTrue);
    expect(health.hasReadOnlyProbeResult, isTrue);
    expect(health.canRunReadOnlyProbe, isFalse);
  });

  test('connection health draft uses imported probe record', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://localhost:7319',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
      syncConnectionProbeRecord: ManagerSyncConnectionProbeRecord(
        source: managerSyncConnectionProbeSourceLocalDockerHttps,
        recordedAt: '2026-07-05T00:00:00Z',
        format: managerSyncConnectionHealthSummaryFormat,
        redactionPolicy: managerSyncConnectionHealthSummaryRedactionPolicy,
        endpointStatus: 'configured',
        transportMode: 'local_https',
        accessTokenStatus: 'not_configured',
        connectionStatus: 'reachable',
        authStatus: 'not_required_for_local_probe',
        serverStateStatus: 'domain_missing_expected',
        httpStatus: 404,
        httpStatusClass: 'client_error',
        lastRemoteErrorCode: 'not_found',
        localInsecureTls: 'allowed',
      ),
    );

    final health = deriveManagerSyncConnectionHealth(draft);

    expect(health.status, SyncConnectionStatus.readOnlyProbeReachable);
    expect(health.connectionBlocker, 'none');
    expect(
      health.probeSource,
      managerSyncConnectionProbeSourceLocalDockerHttps,
    );
    expect(health.probeRecordedAt, '2026-07-05T00:00:00.000Z');
    expect(health.authStatus, 'not_required_for_local_probe');
    expect(health.httpStatus, 404);
    expect(health.httpStatusClass, 'client_error');
    expect(health.localInsecureTls, 'allowed');
  });

  test('connection health imports network failure probe summary', () {
    final summary = SyncConnectionProbeSummary.fromJson({
      'format': managerSyncConnectionHealthSummaryFormat,
      'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
      'endpoint_status': 'configured',
      'transport_mode': 'local_https',
      'access_token_status': 'not_configured',
      'connection_status': 'network_unreachable',
      'auth_status': 'not_checked',
      'server_state_status': 'not_checked_network_unreachable',
      'http_status': 0,
      'http_status_class': 'network_error',
      'last_remote_error_code': 'network_unreachable',
      'local_insecure_tls': 'allowed',
    });

    final health = managerSyncConnectionHealthFromProbeSummary(summary);

    expect(health.status, SyncConnectionStatus.readOnlyProbeNetworkUnavailable);
    expect(health.connectionBlocker, 'network_unreachable');
    expect(health.serverStateStatus, 'not_checked_network_unreachable');
    expect(health.isConnectionHealthy, isFalse);
    expect(health.hasReadOnlyProbeResult, isTrue);
  });

  test('connection health rejects unsupported probe summary fields', () {
    final summary = SyncConnectionProbeSummary.fromJson({
      'format': managerSyncConnectionHealthSummaryFormat,
      'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
      'endpoint_status': 'configured',
      'transport_mode': 'local_https',
      'access_token_status': 'not_configured',
      'connection_status': 'reachable',
      'auth_status': 'accepted',
      'server_state_status': 'domain_missing_expected',
      'http_status': 404,
      'http_status_class': 'client_error',
      'last_remote_error_code': 'new_error_shape',
      'local_insecure_tls': 'allowed',
    });

    final health = managerSyncConnectionHealthFromProbeSummary(summary);

    expect(health.status, SyncConnectionStatus.readOnlyProbeReachable);
    expect(health.connectionBlocker, 'none');
    expect(health.lastRemoteErrorCode, 'unexpected_remote_error_code');

    final invalid = managerSyncConnectionHealthFromProbeSummary(
      SyncConnectionProbeSummary.fromJson({
        'format': 'unsupported_format',
        'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
        'endpoint_status': 'configured',
        'transport_mode': 'local_https',
        'access_token_status': 'not_configured',
        'connection_status': 'reachable',
        'auth_status': 'accepted',
        'server_state_status': 'domain_missing_expected',
        'http_status': 404,
        'http_status_class': 'client_error',
        'last_remote_error_code': 'not_found',
        'local_insecure_tls': 'allowed',
      }),
    );

    expect(invalid.status, SyncConnectionStatus.readOnlyProbeSummaryInvalid);
    expect(invalid.connectionBlocker, 'probe_summary_format_unsupported');
  });

  test('connection health rejects unsafe endpoint material', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://user:token@localhost:7319',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: false,
      accessTokenConfigured: true,
    );

    final health = deriveManagerSyncConnectionHealth(draft);

    expect(health.status, SyncConnectionStatus.endpointInvalid);
    expect(health.connectionBlocker, 'server_endpoint_userinfo_forbidden');
    expect(health.serverStateStatus, 'not_checked_endpoint_invalid');
    expect(health.lastRemoteErrorCode, 'configuration_invalid');
  });

  test('external evidence remains blocked before recovery and authorization', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    );

    final audit = managerSyncGateAuditForDraft(
      draft: draft,
      device: readyDevice,
    );

    expect(audit.state, SyncUiState.preflightReady);
    expect(audit.entryGate.entryState, SyncEntryState.blockedBeforeUserSync);
    expect(audit.entryGate.entryBlocker, 'recovery_code_flow_closed');
    expect(
      audit.entryGate.localEvidenceSource,
      managerDeploymentEvidenceExternalTls,
    );
    expect(
      audit.entryGate.deviceAuthorization.joinRequestStatus.code,
      'join_request_unavailable',
    );
    expect(
      audit.entryGate.productionBlockers,
      contains('release_deployment_evidence_summary_required'),
    );
    expect(
      audit.entryGate.readinessSourceTagSummary,
      'recovery_setup_readiness, recovery_restore_readiness, device_join_readiness, device_revocation_readiness',
    );
    expect(audit.entryGate.readinessUserSyncBlocked, isTrue);
    expect(audit.entryGate.userSyncEnabled, isFalse);
  });

  test('bridge readiness mapper feeds gate without opening user sync', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      accessTokenConfigured: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    );
    final readiness = managerSyncReadinessBridgeSnapshotFromJson(
      readySyncReadinessBridgeJson(),
    );

    final gate = deriveManagerSyncEntryGate(
      draft: draft,
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );

    expect(gate.entryState, SyncEntryState.blockedBeforeUserSync);
    expect(gate.entryBlocker, 'user_sync_entry_closed_current_phase');
    expect(gate.readinessBridgeSource, 'ffi_native_readiness');
    expect(gate.readinessBlockedFlowSummary, 'none');
    expect(gate.readinessIssueCodeSummary, 'none');
    expect(
      gate.readinessSourceTagSummary,
      'bridge_recovery_setup_readiness, bridge_recovery_restore_readiness, bridge_device_join_readiness, bridge_device_revocation_readiness',
    );
    expect(gate.readinessUserSyncBlocked, isFalse);
    expect(gate.userSyncEnabled, isFalse);
    expect(gate.recovery.status, RecoveryEntryStatus.ready);
    expect(gate.recovery.canGenerateCode, isFalse);
    expect(
      gate.deviceAuthorization.status,
      DeviceAuthorizationEntryStatus.ready,
    );
    expect(gate.deviceAuthorization.canCreateJoinRequest, isFalse);
    expect(
      gate.interactionEntryPlan.intentStatusSummary,
      'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
    );
    expect(
      gate.interactionEntryPlan.blockerSummary,
      'user_sync_entry_closed_current_phase',
    );
    expect(
      gate.interactionEntryPlan.requiredEvidenceSummary,
      'user_sync_entry_current_phase_open_required',
    );
    expect(
      gate.productionBlockers,
      contains('user_sync_entry_closed_current_phase'),
    );
    expect(
      gate.productionBlockers,
      isNot(contains('recovery_code_flow_closed')),
    );
  });

  test('bridge readiness mapper sanitizes unknown and unsafe fields', () {
    final readiness = managerSyncReadinessBridgeSnapshotFromJson(
      unsafeSyncReadinessBridgeJson(),
    );

    final gate = deriveManagerSyncEntryGate(
      draft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      ),
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );

    expect(readiness.source, 'unknown_bridge_readiness_source');
    expect(gate.recovery.setupReadiness.status, 'recovery_setup_flow_closed');
    expect(gate.recovery.setupReadiness.blocker, 'recovery_record_missing');
    expect(
      gate.recovery.setupReadiness.prerequisiteSummary,
      'platform_private_key_backend_ready, unexpected_bridge_required_evidence',
    );
    expect(
      gate.readinessIssueCodeSummary,
      contains('unexpected_bridge_error_code'),
    );
    expect(
      gate.deviceAuthorization.joinReadiness.authorizationPackagePreconditions,
      'active_existing_device_required, unexpected_bridge_required_evidence',
    );
    expect(gate.readinessIssueCodeSummary, isNot(contains('secret-token')));
    expect(gate.readinessIssueCodeSummary, isNot(contains('signature_bytes')));
    expect(
      gate.readinessNextRequiredEvidenceSummary,
      isNot(contains('wrapped_material_bytes')),
    );
    expect(
      gate.deviceAuthorization.joinReadiness.errorCodeSummary,
      isNot(contains('payload_bytes')),
    );

    final gateSummary = _entryGateSummary(gate);
    expect(gateSummary, isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')));
    expect(gateSummary, isNot(contains('secret-token')));
    expect(gateSummary, isNot(contains('/synthetic/private')));
    expect(gateSummary, isNot(contains('short_code=123456')));
    expect(gateSummary, isNot(contains('signature_bytes')));
    expect(gateSummary, isNot(contains('wrapped_material_bytes')));
    expect(gateSummary, isNot(contains('payload_bytes')));
    expect(gateSummary, isNot(contains('private_key=abcdef')));

    final invalid = managerSyncReadinessBridgeSnapshotFromJson({
      'format': 'unsupported',
      'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    });
    expect(invalid.source, 'bridge_readiness_summary_invalid');
    expect(
      invalid.recoveryEntryGate.readinessBlockerSummary,
      contains('recovery_code_generation_closed'),
    );
    expect(
      invalid.recoveryEntryGate.setupReadiness.errorCodeSummary,
      'bridge_readiness_summary_invalid',
    );
  });

  test('bridge readiness partial blockers preserve flow-specific evidence', () {
    final readiness = managerSyncReadinessBridgeSnapshotFromJson(
      partiallyBlockedSyncReadinessBridgeJson(),
    );

    final gate = deriveManagerSyncEntryGate(
      draft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      ),
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );

    expect(readiness.source, 'fixture_readiness');
    expect(gate.entryState, SyncEntryState.blockedBeforeUserSync);
    expect(gate.entryBlocker, 'recovery_record_missing');
    expect(gate.recovery.status, RecoveryEntryStatus.saveConfirmationRequired);
    expect(gate.recovery.recoveryRecordBlocker, 'recovery_record_missing');
    expect(
      gate.deviceAuthorization.status,
      DeviceAuthorizationEntryStatus.authorizationUnavailable,
    );
    expect(
      gate.deviceAuthorization.authorizationPackageBlocker,
      'authorization_package_prerequisites_blocked',
    );
    expect(gate.readinessBlockedFlowSummary, 'recovery_setup, device_join');
    expect(
      gate.readinessIssueCodeSummary,
      'recovery_record_missing, network_unreachable, join_request_expired, authorization_rejected',
    );
    expect(
      gate.readinessNextRequiredEvidenceSummary,
      'platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required, join_request_expired, short_code_match_required, join_request_pending_required',
    );
    expect(
      gate.interactionEntryPlan.intentStatusSummary,
      'recovery_setup=blocked, recovery_restore=closed_current_phase, join_request_authorization=blocked, device_revocation=closed_current_phase',
    );
    expect(
      gate.interactionEntryPlan.blockerSummary,
      'recovery_record_missing, user_sync_entry_closed_current_phase, join_request_expired',
    );
    expect(gate.userSyncEnabled, isFalse);
  });

  test('bridge readiness invalid redaction policy falls back closed', () {
    final readiness = managerSyncReadinessBridgeSnapshotFromJson({
      'format': managerSyncReadinessBridgeSummaryFormat,
      'redaction_policy': 'raw_native_payload_with_secret_fields',
      'source': 'ffi_native_readiness',
      'recovery_setup': {'status': 'recovery_setup_ready', 'blocker': 'none'},
    });

    final gate = deriveManagerSyncEntryGate(
      draft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      ),
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );

    expect(readiness.source, 'bridge_readiness_summary_invalid');
    expect(gate.readinessBridgeSource, 'bridge_readiness_summary_invalid');
    expect(
      gate.readinessBlockedFlowSummary,
      'recovery_setup, recovery_restore, device_join, device_revocation',
    );
    expect(
      gate.readinessIssueCodeSummary,
      contains('bridge_readiness_summary_invalid'),
    );
    expect(gate.userSyncEnabled, isFalse);
    expect(
      gate.interactionEntryPlan.intentStatusSummary,
      'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
    );
  });
}

String _entryGateSummary(ManagerSyncEntryGate gate) {
  return [
    gate.entryBlocker,
    gate.productionBlockerSummary,
    gate.readinessBlockedFlowSummary,
    gate.readinessIssueCodeSummary,
    gate.readinessNextRequiredEvidenceSummary,
    gate.readinessSourceTagSummary,
    gate.interactionEntryPlan.blockerSummary,
    gate.interactionEntryPlan.requiredEvidenceSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.executionStatusSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.blockerSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.requiredEvidenceSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.dataPolicySummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.stopLineSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.requestBoundarySummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.resultBoundarySummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.errorCodeSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.requestStatusSummary,
    gate
        .interactionEntryPlan
        .actionCommandPreviewPlan
        .requestAllowedFieldSummary,
    gate
        .interactionEntryPlan
        .actionCommandPreviewPlan
        .requestForbiddenMaterialSummary,
    gate.interactionEntryPlan.actionCommandPreviewPlan.resultStatusSummary,
    gate
        .interactionEntryPlan
        .actionCommandPreviewPlan
        .resultAllowedFieldSummary,
    gate
        .interactionEntryPlan
        .actionCommandPreviewPlan
        .resultForbiddenMaterialSummary,
    gate.recovery.readinessBlockerSummary,
    gate.recovery.setupReadiness.prerequisiteSummary,
    gate.recovery.setupReadiness.errorCodeSummary,
    gate.recovery.restoreReadiness.errorCodeSummary,
    gate.deviceAuthorization.readinessBlockerSummary,
    gate.deviceAuthorization.joinReadiness.authorizationPackagePreconditions,
    gate.deviceAuthorization.joinReadiness.errorCodeSummary,
    gate.deviceAuthorization.revocationReadiness.errorCodeSummary,
  ].join('\n');
}

String _connectionHealthSummary(SyncConnectionHealth health) {
  return [
    health.status.code,
    health.connectionBlocker,
    health.probeSource,
    health.probeRecordedAt,
    health.endpointStatus,
    health.accessTokenStatus,
    health.transportMode,
    health.serverStateStatus,
    health.authStatus,
    health.httpStatus.toString(),
    health.httpStatusClass,
    health.lastRemoteErrorCode,
    health.localInsecureTls,
  ].join('\n');
}

String _readinessSnapshotSummary(ManagerSyncReadinessBridgeSnapshot snapshot) {
  return [
    snapshot.source,
    snapshot.recoverySetupReadiness.status,
    snapshot.recoverySetupReadiness.blocker,
    snapshot.recoverySetupReadiness.entryActionStatus,
    snapshot.recoverySetupReadiness.generatedCodeStatus,
    snapshot.recoverySetupReadiness.saveConfirmationStatus,
    snapshot.recoverySetupReadiness.recoveryRecordStatus,
    snapshot.recoverySetupReadiness.firstUploadGate,
    snapshot.recoverySetupReadiness.prerequisiteSummary,
    snapshot.recoverySetupReadiness.errorCodeSummary,
    snapshot.recoveryRestoreReadiness.status,
    snapshot.recoveryRestoreReadiness.blocker,
    snapshot.recoveryRestoreReadiness.entryActionStatus,
    snapshot.recoveryRestoreReadiness.codeInputStatus,
    snapshot.recoveryRestoreReadiness.recoveryRecordLookupStatus,
    snapshot.recoveryRestoreReadiness.attemptLimitStatus,
    snapshot.recoveryRestoreReadiness.deviceRegistrationStatus,
    snapshot.recoveryRestoreReadiness.errorCodeSummary,
    snapshot.deviceJoinReadiness.status,
    snapshot.deviceJoinReadiness.blocker,
    snapshot.deviceJoinReadiness.entryActionStatus,
    snapshot.deviceJoinReadiness.joinRequestStatus.code,
    snapshot.deviceJoinReadiness.shortCodeVerificationStatus,
    snapshot.deviceJoinReadiness.authorizationPackageStatus,
    snapshot.deviceJoinReadiness.authorizationPackagePreconditions,
    snapshot.deviceJoinReadiness.errorCodeSummary,
    snapshot.deviceRevocationReadiness.status,
    snapshot.deviceRevocationReadiness.blocker,
    snapshot.deviceRevocationReadiness.entryActionStatus,
    snapshot.deviceRevocationReadiness.revokeDeviceStatus,
    snapshot.deviceRevocationReadiness.activeDeviceRequirement,
    snapshot.deviceRevocationReadiness.lostDeviceRiskNotice,
    snapshot.deviceRevocationReadiness.keyEpochStatus,
    snapshot.deviceRevocationReadiness.errorCodeSummary,
  ].join('\n');
}

class _RejectedReadinessImportCase {
  const _RejectedReadinessImportCase({
    required this.json,
    required this.errorCode,
  });

  final Map<String, Object?> json;
  final String errorCode;
}
