import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

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
      gate.deviceAuthorization.status,
      DeviceAuthorizationEntryStatus.flowClosed,
    );
    expect(
      gate.deviceAuthorization.joinRequestStatus,
      JoinRequestStatus.unavailable,
    );
  });

  test(
    'entry gate reports blocked platform backend before evidence checks',
    () {
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
    },
  );

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
      gate.productionBlockers,
      containsAll([
        'release_deployment_evidence_required',
        'recovery_code_flow_closed',
        'device_authorization_flow_closed',
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

  test(
    'external evidence remains blocked before recovery and authorization',
    () {
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
      expect(audit.entryGate.userSyncEnabled, isFalse);
    },
  );
}
