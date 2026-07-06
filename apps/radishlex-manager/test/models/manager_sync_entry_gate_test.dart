import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

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

  test('entry plan reports blocked future action intent', () {
    final readiness = managerSyncReadinessBridgeSnapshotFromJson({
      'format': managerSyncReadinessBridgeSummaryFormat,
      'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
      'source': 'ffi_native_readiness',
      'recovery_setup': {
        'status': 'recovery_setup_blocked',
        'blocker': 'backend_unavailable',
        'entry_action_status': 'blocked_by_backend',
        'required_prerequisites': ['platform_private_key_backend_ready'],
        'error_codes': ['backend_unavailable'],
      },
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

    final intent = gate.interactionEntryPlan.intentFor('recovery_setup');
    expect(intent.visibilityStatus, 'visible');
    expect(intent.intentStatus, 'blocked');
    expect(intent.blocker, 'backend_unavailable');
    expect(
      intent.requiredEvidenceSummary,
      'platform_private_key_backend_ready',
    );
  });

  test('entry plan supports confirmation-only future action intent', () {
    final plan = managerSyncInteractionEntryPlanFromReadinessFlows(
      readinessFlows: const [
        SyncReadinessFlowSummary(
          flowId: 'recovery_setup',
          status: 'recovery_setup_blocked',
          blocker: 'none',
          actionStatus: 'available',
          requiredEvidenceCodes: ['blocked_until_recovery_code_saved'],
          errorCodes: [],
          blocksUserSync: true,
          sourceTag: 'unit_test_readiness',
        ),
      ],
      userSyncEnabled: true,
    );

    final intent = plan.intentFor('recovery_setup');
    expect(intent.visibilityStatus, 'visible');
    expect(intent.intentStatus, 'requires_confirmation');
    expect(intent.blocker, 'user_confirmation_required');
    expect(
      intent.requiredEvidenceSummary,
      'blocked_until_recovery_code_saved, explicit_user_confirmation_required',
    );
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
