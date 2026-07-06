import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import 'sync_readiness_bridge_fixtures.dart';

const managerSyncEvidenceBundleFormat = 'manager_sync_evidence_bundle.v1';
const managerSyncEvidenceBundleRedactionPolicy =
    'summary_only_no_endpoint_tokens_recovery_secret_or_payload_bytes';
const syncEvidenceBundleRecordedAt = '2026-07-06T00:00:00.000Z';

const syncEvidenceBundleScenarioLocalHttpsDraft = ManagerSettingsDraft(
  serverEndpoint: 'https://localhost:7319',
  retainSyncConfig: true,
  privacyMode: false,
  diagnosticsExport: false,
  deploymentEvidenceRecorded: true,
  accessTokenConfigured: true,
  deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
);

const representativeSyncEvidenceBundleScenarioIds = [
  'all_ready_external_probe_current_phase_closed',
  'local_https_reachable_recovery_record_missing',
  'network_unreachable_all_ready_current_phase_closed',
  'unsafe_connection_summary_rejected',
  'unsafe_readiness_summary_downgraded',
];

const syncEvidenceBundleSensitiveLeakFragments = [
  ...syncReadinessSensitiveLeakFragments,
  'Authorization',
  'Bearer ',
  'https://user:secret-token@sync.example.invalid',
  'raw_endpoint_tokens_and_response_body',
  'response_body',
  'endpoint=https://',
];

class SyncEvidenceBundleScenario {
  const SyncEvidenceBundleScenario({
    required this.id,
    required this.title,
    required this.draft,
    required this.device,
    required this.readinessSummaryJson,
    required this.connectionSummaryJson,
    this.expectedReadinessImportAccepted = true,
    this.expectedReadinessImportErrorCode = '',
    required this.expectedBridgeSource,
    required this.expectedEntryState,
    required this.expectedEntryBlocker,
    required this.expectedBlockedFlows,
    required this.expectedInteractionStatuses,
    required this.expectedInteractionBlockers,
    required this.expectedUserSyncEnabled,
    required this.expectedConnectionStatusCode,
    required this.expectedConnectionBlocker,
    required this.expectedConnectionProbeSource,
    required this.expectedEndpointStatus,
    required this.expectedAccessTokenStatus,
    required this.expectedTransportMode,
    required this.expectedServerStateStatus,
    required this.expectedAuthStatus,
    required this.expectedHttpStatusText,
    required this.expectedLastRemoteErrorCode,
    required this.expectedLocalInsecureTls,
  });

  final String id;
  final String title;
  final ManagerSettingsDraft draft;
  final DeviceSecuritySummary device;
  final Map<String, Object?> Function() readinessSummaryJson;
  final Map<String, Object?> Function() connectionSummaryJson;
  final bool expectedReadinessImportAccepted;
  final String expectedReadinessImportErrorCode;
  final String expectedBridgeSource;
  final SyncEntryState expectedEntryState;
  final String expectedEntryBlocker;
  final String expectedBlockedFlows;
  final String expectedInteractionStatuses;
  final String expectedInteractionBlockers;
  final bool expectedUserSyncEnabled;
  final String expectedConnectionStatusCode;
  final String expectedConnectionBlocker;
  final String expectedConnectionProbeSource;
  final String expectedEndpointStatus;
  final String expectedAccessTokenStatus;
  final String expectedTransportMode;
  final String expectedServerStateStatus;
  final String expectedAuthStatus;
  final String expectedHttpStatusText;
  final String expectedLastRemoteErrorCode;
  final String expectedLocalInsecureTls;
}

List<SyncEvidenceBundleScenario> syncEvidenceBundleScenarioCatalog() {
  return [
    SyncEvidenceBundleScenario(
      id: 'all_ready_external_probe_current_phase_closed',
      title: 'readiness 和外部 HTTPS 探测均 ready，但当前阶段仍关闭用户同步入口',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: readySyncReadinessBridgeJson,
      connectionSummaryJson: reachableExternalHttpsConnectionSummaryJson,
      expectedBridgeSource: 'ffi_native_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'user_sync_entry_closed_current_phase',
      expectedBlockedFlows: 'none',
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: 'user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'reachable',
      expectedConnectionBlocker: 'none',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceExternalHttps,
      expectedEndpointStatus: 'configured',
      expectedAccessTokenStatus: 'configured',
      expectedTransportMode: 'external_https',
      expectedServerStateStatus: 'domain_state_returned',
      expectedAuthStatus: 'accepted',
      expectedHttpStatusText: '200 success',
      expectedLastRemoteErrorCode: 'none',
      expectedLocalInsecureTls: 'system_trust',
    ),
    SyncEvidenceBundleScenario(
      id: 'local_https_reachable_recovery_record_missing',
      title: '本地 HTTPS 只读探测可达，但恢复记录缺失仍阻塞 readiness',
      draft: syncEvidenceBundleScenarioLocalHttpsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: recoveryRecordMissingSyncReadinessBridgeJson,
      connectionSummaryJson: reachableLocalHttpsConnectionSummaryJson,
      expectedBridgeSource: 'fixture_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_record_missing',
      expectedBlockedFlows: 'recovery_setup',
      expectedInteractionStatuses:
          'recovery_setup=blocked, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      expectedInteractionBlockers:
          'recovery_record_missing, user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'reachable',
      expectedConnectionBlocker: 'none',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceLocalDockerHttps,
      expectedEndpointStatus: 'configured',
      expectedAccessTokenStatus: 'configured',
      expectedTransportMode: 'local_https',
      expectedServerStateStatus: 'domain_missing_expected',
      expectedAuthStatus: 'accepted',
      expectedHttpStatusText: '404 client_error',
      expectedLastRemoteErrorCode: 'not_found',
      expectedLocalInsecureTls: 'allowed',
    ),
    SyncEvidenceBundleScenario(
      id: 'network_unreachable_all_ready_current_phase_closed',
      title: 'readiness 均 ready 时，连接不可达只作为探测证据呈现',
      draft: syncEvidenceBundleScenarioLocalHttpsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: readySyncReadinessBridgeJson,
      connectionSummaryJson: networkUnreachableConnectionSummaryJson,
      expectedBridgeSource: 'ffi_native_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'user_sync_entry_closed_current_phase',
      expectedBlockedFlows: 'none',
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: 'user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'network_unreachable',
      expectedConnectionBlocker: 'network_unreachable',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceLocalDockerHttps,
      expectedEndpointStatus: 'configured',
      expectedAccessTokenStatus: 'configured',
      expectedTransportMode: 'local_https',
      expectedServerStateStatus: 'not_checked_network_unreachable',
      expectedAuthStatus: 'not_checked',
      expectedHttpStatusText: '0 network_error',
      expectedLastRemoteErrorCode: 'network_unreachable',
      expectedLocalInsecureTls: 'allowed',
    ),
    SyncEvidenceBundleScenario(
      id: 'unsafe_connection_summary_rejected',
      title: '不安全 connection redaction policy 被拒绝为只读探测摘要不可用',
      draft: syncEvidenceBundleScenarioLocalHttpsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: readySyncReadinessBridgeJson,
      connectionSummaryJson: unsafeConnectionSummaryJson,
      expectedBridgeSource: 'ffi_native_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'user_sync_entry_closed_current_phase',
      expectedBlockedFlows: 'none',
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: 'user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'probe_summary_invalid',
      expectedConnectionBlocker: 'probe_summary_format_unsupported',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceLocalDockerHttps,
      expectedEndpointStatus: 'not_checked_probe_summary_invalid',
      expectedAccessTokenStatus: 'not_checked_probe_summary_invalid',
      expectedTransportMode: 'not_checked_probe_summary_invalid',
      expectedServerStateStatus: 'not_checked_probe_summary_invalid',
      expectedAuthStatus: 'accepted',
      expectedHttpStatusText: '404 client_error',
      expectedLastRemoteErrorCode: 'probe_summary_invalid',
      expectedLocalInsecureTls: 'allowed',
    ),
    SyncEvidenceBundleScenario(
      id: 'unsafe_readiness_summary_rejected',
      title: '不安全 readiness redaction policy 被拒绝并回退到默认关闭 readiness',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: unsafeRedactionSyncReadinessBridgeJson,
      connectionSummaryJson: reachableExternalHttpsConnectionSummaryJson,
      expectedReadinessImportAccepted: false,
      expectedReadinessImportErrorCode:
          'readiness_summary_redaction_policy_unsupported',
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_code_flow_closed',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'reachable',
      expectedConnectionBlocker: 'none',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceExternalHttps,
      expectedEndpointStatus: 'configured',
      expectedAccessTokenStatus: 'configured',
      expectedTransportMode: 'external_https',
      expectedServerStateStatus: 'domain_state_returned',
      expectedAuthStatus: 'accepted',
      expectedHttpStatusText: '200 success',
      expectedLastRemoteErrorCode: 'none',
      expectedLocalInsecureTls: 'system_trust',
    ),
    SyncEvidenceBundleScenario(
      id: 'unsafe_readiness_summary_downgraded',
      title: '未知 readiness 字段被降级为安全分类且不泄漏原始 payload',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      readinessSummaryJson: unsafeSyncReadinessBridgeJson,
      connectionSummaryJson: reachableExternalHttpsConnectionSummaryJson,
      expectedBridgeSource: 'unknown_bridge_readiness_source',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_record_missing',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedInteractionStatuses:
          'recovery_setup=closed_current_phase, recovery_restore=blocked, join_request_authorization=blocked, device_revocation=blocked',
      expectedInteractionBlockers:
          'recovery_record_missing, recovery_code_invalid, join_request_expired, key_epoch_rotation_required',
      expectedUserSyncEnabled: false,
      expectedConnectionStatusCode: 'reachable',
      expectedConnectionBlocker: 'none',
      expectedConnectionProbeSource:
          managerSyncConnectionProbeSourceExternalHttps,
      expectedEndpointStatus: 'configured',
      expectedAccessTokenStatus: 'configured',
      expectedTransportMode: 'external_https',
      expectedServerStateStatus: 'domain_state_returned',
      expectedAuthStatus: 'accepted',
      expectedHttpStatusText: '200 success',
      expectedLastRemoteErrorCode: 'none',
      expectedLocalInsecureTls: 'system_trust',
    ),
  ];
}

SyncEvidenceBundleScenario syncEvidenceBundleScenarioById(String id) {
  return syncEvidenceBundleScenarioCatalog().firstWhere(
    (scenario) => scenario.id == id,
  );
}

Map<String, Object?> syncEvidenceBundleJsonForScenario(
  SyncEvidenceBundleScenario scenario,
) {
  return {
    'format': managerSyncEvidenceBundleFormat,
    'redaction_policy': managerSyncEvidenceBundleRedactionPolicy,
    'scenario_id': scenario.id,
    'deployment_evidence_source': scenario.draft.deploymentEvidenceSource,
    'device_production_gate': scenario.device.productionGate,
    'readiness_summary': scenario.readinessSummaryJson(),
    'connection_summary': scenario.connectionSummaryJson(),
  };
}

ManagerSnapshot managerSnapshotForSyncEvidenceBundleScenario(
  SyncEvidenceBundleScenario scenario,
) {
  final fixture = createManagerFixture();
  final readinessImport = importManagerSyncReadinessBridgeSummaryFromJson(
    scenario.readinessSummaryJson(),
  );
  final probeRecord = syncConnectionProbeRecordFromSummaryJson(
    scenario.connectionSummaryJson(),
  );
  final draft = scenario.draft.copyWith(syncConnectionProbeRecord: probeRecord);
  final state = deriveManagerSyncUiState(
    draft: draft,
    device: scenario.device,
    readinessBridgeSnapshot: readinessImport.snapshot,
  );

  return fixture.copyWith(
    sync: fixture.sync.copyWith(
      state: state,
      device: scenario.device,
      serverEndpoint: managerSyncEndpointLabel(draft),
      reason: managerSyncGateReason(
        state: state,
        draft: draft,
        device: scenario.device,
        readinessBridgeSnapshot: readinessImport.snapshot,
      ),
      readinessBridgeSnapshot: readinessImport.snapshot,
    ),
    settings: fixture.settings.copyWith(
      draft: draft,
      syncConfigured: draft.retainSyncConfig,
    ),
  );
}

ManagerSyncConnectionProbeRecord syncConnectionProbeRecordFromSummaryJson(
  Map<String, Object?> json,
) {
  final summary = SyncConnectionProbeSummary.fromJson(json);
  return managerSyncConnectionProbeRecordFromSummary(
    summary,
    source: managerSyncConnectionProbeSourceForSummary(summary),
    recordedAt: syncEvidenceBundleRecordedAt,
  );
}

Map<String, Object?> reachableExternalHttpsConnectionSummaryJson() {
  return {
    'format': managerSyncConnectionHealthSummaryFormat,
    'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
    'endpoint_status': 'configured',
    'transport_mode': 'external_https',
    'access_token_status': 'configured',
    'connection_status': 'reachable',
    'auth_status': 'accepted',
    'server_state_status': 'domain_state_returned',
    'http_status': 200,
    'http_status_class': 'success',
    'last_remote_error_code': 'none',
    'local_insecure_tls': 'system_trust',
  };
}

Map<String, Object?> reachableLocalHttpsConnectionSummaryJson() {
  return {
    'format': managerSyncConnectionHealthSummaryFormat,
    'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
    'endpoint_status': 'configured',
    'transport_mode': 'local_https',
    'access_token_status': 'configured',
    'connection_status': 'reachable',
    'auth_status': 'accepted',
    'server_state_status': 'domain_missing_expected',
    'http_status': 404,
    'http_status_class': 'client_error',
    'last_remote_error_code': 'not_found',
    'local_insecure_tls': 'allowed',
  };
}

Map<String, Object?> networkUnreachableConnectionSummaryJson() {
  return {
    'format': managerSyncConnectionHealthSummaryFormat,
    'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
    'endpoint_status': 'configured',
    'transport_mode': 'local_https',
    'access_token_status': 'configured',
    'connection_status': 'network_unreachable',
    'auth_status': 'not_checked',
    'server_state_status': 'not_checked_network_unreachable',
    'http_status': 0,
    'http_status_class': 'network_error',
    'last_remote_error_code': 'network_unreachable',
    'local_insecure_tls': 'allowed',
  };
}

Map<String, Object?> unsafeConnectionSummaryJson() {
  return {
    ...reachableLocalHttpsConnectionSummaryJson(),
    'redaction_policy': 'raw_endpoint_tokens_and_response_body',
    'endpoint': 'https://user:secret-token@sync.example.invalid',
    'authorization': 'Bearer secret-token',
    'response_body': '{"payload_bytes":"abcdef"}',
  };
}
