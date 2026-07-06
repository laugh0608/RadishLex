import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import 'sync_action_protocol_fixtures.dart';

export 'sync_action_protocol_fixtures.dart';

const syncReadinessScenarioReadyDevice = DeviceSecuritySummary(
  deviceId: 'device-ready-01',
  backendId: 'test-production-ready',
  capabilityStatus: 'ready_for_test',
  productionGate: 'ready',
);

const syncReadinessScenarioBlockedDevice = DeviceSecuritySummary(
  deviceId: 'device-blocked-01',
  backendId: 'android-keystore-v1',
  capabilityStatus: 'unsupported_signature_algorithm',
  productionGate: 'blocked',
);

const syncReadinessScenarioExternalTlsDraft = ManagerSettingsDraft(
  serverEndpoint: 'https://sync.example.invalid',
  retainSyncConfig: true,
  privacyMode: false,
  diagnosticsExport: false,
  deploymentEvidenceRecorded: true,
  accessTokenConfigured: true,
  deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
);

const syncReadinessScenarioLocalSmokeDraft = ManagerSettingsDraft(
  serverEndpoint: 'https://sync.example.invalid',
  retainSyncConfig: true,
  privacyMode: false,
  diagnosticsExport: false,
  deploymentEvidenceRecorded: true,
  accessTokenConfigured: true,
  deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
);

const syncReadinessScenarioMissingEvidenceDraft = ManagerSettingsDraft(
  serverEndpoint: 'https://sync.example.invalid',
  retainSyncConfig: true,
  privacyMode: false,
  diagnosticsExport: false,
  deploymentEvidenceRecorded: false,
  accessTokenConfigured: true,
);

const syncReadinessDefaultBlockedFlows =
    'recovery_setup, recovery_restore, device_join, device_revocation';

const syncReadinessDefaultIssueCodes =
    'recovery_code_generation_closed, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent, recovery_code_input_closed, recovery_code_invalid, authentication_required, network_unreachable, join_request_creation_closed, join_request_expired, authorization_rejected, device_revoked, backend_unavailable, device_revocation_flow_closed, key_epoch_rotation_required';

const syncReadinessDefaultNextEvidence =
    'platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required, input_not_available_current_phase, not_checked_current_phase, blocked_until_recovery_success, join_request_unavailable, short_code_verification_not_started, active_existing_device_required, join_request_pending_required, short_code_match_required, lost_device_prior_material_not_recallable, key_epoch_rotation_not_started';

const syncReadinessClosedInteractionStatuses =
    'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase';

const syncReadinessClosedInteractionBlockers =
    'recovery_code_generation_closed, recovery_code_input_closed, join_request_creation_closed, device_revocation_flow_closed';

const representativeSyncReadinessScenarioIds = [
  'all_ready_current_phase_closed',
  'recovery_record_missing',
  'join_request_expired',
  'unknown_native_status_sanitized',
  'unsafe_redaction_rejected',
];

const syncReadinessSensitiveLeakFragments = [
  'secret-token',
  'RADISHLEX-RECOVERY-CODE-SECRET',
  '/synthetic/private',
  'payload_bytes=abcdef',
  'wrapped_material_bytes=abcdef',
  'signature_bytes=abcdef',
  'private_key=abcdef',
  'short_code=',
];

class SyncReadinessScenario {
  const SyncReadinessScenario({
    required this.id,
    required this.title,
    required this.draft,
    required this.device,
    this.summaryJson,
    this.expectedImportAccepted = true,
    this.expectedImportErrorCode = '',
    required this.expectedBridgeSource,
    required this.expectedEntryState,
    required this.expectedEntryBlocker,
    required this.expectedBlockedFlows,
    this.expectedIssueCodes,
    this.expectedIssueCodeContains = const [],
    this.forbiddenIssueCodeFragments = const [],
    this.expectedNextEvidence,
    this.expectedNextEvidenceContains = const [],
    this.forbiddenNextEvidenceFragments = const [],
    required this.expectedInteractionStatuses,
    required this.expectedInteractionBlockers,
    this.expectedActionCommandRequestBoundaries =
        syncActionCommandRequestBoundarySummary,
    this.expectedActionCommandResultBoundaries =
        syncActionCommandResultBoundarySummary,
    this.expectedActionCommandErrorCodes = syncActionCommandErrorCodeSummary,
    this.expectedActionRequestAllowedFields =
        syncActionRequestAllowedFieldSummary,
    this.expectedActionResultAllowedFields =
        syncActionResultAllowedFieldSummary,
    this.expectedActionForbiddenMaterials = syncActionForbiddenMaterialSummary,
    required this.expectedUserSyncEnabled,
  });

  final String id;
  final String title;
  final ManagerSettingsDraft draft;
  final DeviceSecuritySummary device;
  final Map<String, Object?> Function()? summaryJson;
  final bool expectedImportAccepted;
  final String expectedImportErrorCode;
  final String expectedBridgeSource;
  final SyncEntryState expectedEntryState;
  final String expectedEntryBlocker;
  final String expectedBlockedFlows;
  final String? expectedIssueCodes;
  final List<String> expectedIssueCodeContains;
  final List<String> forbiddenIssueCodeFragments;
  final String? expectedNextEvidence;
  final List<String> expectedNextEvidenceContains;
  final List<String> forbiddenNextEvidenceFragments;
  final String expectedInteractionStatuses;
  final String expectedInteractionBlockers;
  final String expectedActionCommandRequestBoundaries;
  final String expectedActionCommandResultBoundaries;
  final String expectedActionCommandErrorCodes;
  final String expectedActionRequestAllowedFields;
  final String expectedActionResultAllowedFields;
  final String expectedActionForbiddenMaterials;
  final bool expectedUserSyncEnabled;
}

ManagerSyncReadinessBridgeSnapshot syncReadinessSnapshotForScenario(
  SyncReadinessScenario scenario,
) {
  if (scenario.summaryJson == null) {
    return managerDefaultSyncReadinessBridgeSnapshot;
  }
  return importManagerSyncReadinessBridgeSummaryFromJson(
    scenario.summaryJson!(),
  ).snapshot;
}

ManagerSnapshot managerSnapshotForSyncReadinessScenario(
  SyncReadinessScenario scenario,
) {
  final fixture = createManagerFixture();
  final readinessBridgeSnapshot = syncReadinessSnapshotForScenario(scenario);
  final state = deriveManagerSyncUiState(
    draft: scenario.draft,
    device: scenario.device,
    readinessBridgeSnapshot: readinessBridgeSnapshot,
  );
  return fixture.copyWith(
    sync: fixture.sync.copyWith(
      state: state,
      device: scenario.device,
      serverEndpoint: managerSyncEndpointLabel(scenario.draft),
      reason: managerSyncGateReason(
        state: state,
        draft: scenario.draft,
        device: scenario.device,
        readinessBridgeSnapshot: readinessBridgeSnapshot,
      ),
      readinessBridgeSnapshot: readinessBridgeSnapshot,
    ),
    settings: fixture.settings.copyWith(
      draft: scenario.draft,
      syncConfigured: scenario.draft.retainSyncConfig,
    ),
  );
}

List<SyncReadinessScenario> syncReadinessScenarioCatalog() {
  return [
    SyncReadinessScenario(
      id: 'all_ready_current_phase_closed',
      title: '四条 readiness 均 ready，但当前阶段仍关闭用户同步入口',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: readySyncReadinessBridgeJson,
      expectedBridgeSource: 'ffi_native_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'user_sync_entry_closed_current_phase',
      expectedBlockedFlows: 'none',
      expectedIssueCodes: 'none',
      expectedNextEvidence: 'none',
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: 'user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'recovery_record_missing',
      title: '恢复记录缺失阻塞首台设备同步准备',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: recoveryRecordMissingSyncReadinessBridgeJson,
      expectedBridgeSource: 'fixture_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_record_missing',
      expectedBlockedFlows: 'recovery_setup',
      expectedIssueCodes: 'recovery_record_missing',
      expectedNextEvidence:
          'platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required',
      expectedInteractionStatuses:
          'recovery_setup=blocked, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      expectedInteractionBlockers:
          'recovery_record_missing, user_sync_entry_closed_current_phase',
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'join_request_expired',
      title: '设备加入请求过期阻塞设备授权准备',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: joinRequestExpiredSyncReadinessBridgeJson,
      expectedBridgeSource: 'fixture_readiness',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'join_request_expired',
      expectedBlockedFlows: 'device_join',
      expectedIssueCodes: 'join_request_expired, authorization_rejected',
      expectedNextEvidence:
          'join_request_expired, short_code_match_required, join_request_pending_required',
      expectedInteractionStatuses:
          'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=blocked, device_revocation=closed_current_phase',
      expectedInteractionBlockers:
          'user_sync_entry_closed_current_phase, join_request_expired',
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'platform_backend_blocked',
      title: '平台私钥 backend 未解除生产门禁',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioBlockedDevice,
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.backendUnavailable,
      expectedEntryBlocker: 'backend_unavailable',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodes: syncReadinessDefaultIssueCodes,
      expectedNextEvidence: syncReadinessDefaultNextEvidence,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'deployment_evidence_local_smoke_only',
      title: '只有本地 smoke 证据，正式发布级部署证据仍不足',
      draft: syncReadinessScenarioLocalSmokeDraft,
      device: syncReadinessScenarioReadyDevice,
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.localSmokeReady,
      expectedEntryBlocker: 'release_deployment_evidence_required',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodes: syncReadinessDefaultIssueCodes,
      expectedNextEvidence: syncReadinessDefaultNextEvidence,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'deployment_evidence_missing',
      title: '目标部署证据未记录',
      draft: syncReadinessScenarioMissingEvidenceDraft,
      device: syncReadinessScenarioReadyDevice,
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.deploymentUnverified,
      expectedEntryBlocker: 'deployment_unverified',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodes: syncReadinessDefaultIssueCodes,
      expectedNextEvidence: syncReadinessDefaultNextEvidence,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'unknown_native_status_sanitized',
      title: '未知 native 状态和敏感字段被降级为安全分类',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: unsafeSyncReadinessBridgeJson,
      expectedBridgeSource: 'unknown_bridge_readiness_source',
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_record_missing',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodeContains: const [
        'unexpected_bridge_error_code',
        'recovery_record_missing',
        'recovery_code_invalid',
        'join_request_expired',
        'key_epoch_rotation_required',
      ],
      forbiddenIssueCodeFragments: const [
        'secret-token',
        'signature_bytes',
        'payload_bytes',
        'private_key',
      ],
      expectedNextEvidenceContains: const [
        'unexpected_bridge_required_evidence',
        'platform_private_key_backend_ready',
        'blocked_until_recovery_success',
        'join_request_unavailable',
        'key_epoch_rotation_required',
      ],
      forbiddenNextEvidenceFragments: const [
        'wrapped_material_bytes',
        '/synthetic/private',
        'short_code=',
      ],
      expectedInteractionStatuses:
          'recovery_setup=closed_current_phase, recovery_restore=blocked, join_request_authorization=blocked, device_revocation=blocked',
      expectedInteractionBlockers:
          'recovery_record_missing, recovery_code_invalid, join_request_expired, key_epoch_rotation_required',
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'unsupported_format_rejected',
      title: '不支持的 readiness 摘要格式被拒绝',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: unsupportedFormatSyncReadinessBridgeJson,
      expectedImportAccepted: false,
      expectedImportErrorCode: 'readiness_summary_format_unsupported',
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_code_flow_closed',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodes: syncReadinessDefaultIssueCodes,
      expectedNextEvidence: syncReadinessDefaultNextEvidence,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
    ),
    SyncReadinessScenario(
      id: 'unsafe_redaction_rejected',
      title: '不安全 redaction policy 被拒绝',
      draft: syncReadinessScenarioExternalTlsDraft,
      device: syncReadinessScenarioReadyDevice,
      summaryJson: unsafeRedactionSyncReadinessBridgeJson,
      expectedImportAccepted: false,
      expectedImportErrorCode: 'readiness_summary_redaction_policy_unsupported',
      expectedBridgeSource: managerSyncReadinessBridgeSourceDefault,
      expectedEntryState: SyncEntryState.blockedBeforeUserSync,
      expectedEntryBlocker: 'recovery_code_flow_closed',
      expectedBlockedFlows: syncReadinessDefaultBlockedFlows,
      expectedIssueCodes: syncReadinessDefaultIssueCodes,
      expectedNextEvidence: syncReadinessDefaultNextEvidence,
      expectedInteractionStatuses: syncReadinessClosedInteractionStatuses,
      expectedInteractionBlockers: syncReadinessClosedInteractionBlockers,
      expectedUserSyncEnabled: false,
    ),
  ];
}

SyncReadinessScenario syncReadinessScenarioById(String id) {
  return syncReadinessScenarioCatalog().firstWhere(
    (scenario) => scenario.id == id,
  );
}

Map<String, Object?> readySyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'ffi_native_readiness',
    'recovery_setup': {
      'status': 'recovery_setup_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'generated_code_status': 'generated_once',
      'save_confirmation_status': 'confirmed',
      'recovery_record_status': 'recovery_record_active',
      'first_upload_gate': 'ready_for_encrypted_p2_upload',
      'required_prerequisites': <String>[],
      'error_codes': <String>[],
    },
    'recovery_restore': {
      'status': 'recovery_restore_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'code_input_status': 'validated',
      'recovery_record_lookup_status': 'recovery_record_active',
      'attempt_limit_status': 'available',
      'device_registration_status': 'ready_after_recovery_success',
      'error_codes': <String>[],
    },
    'device_join': {
      'status': 'device_join_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'join_request_status': 'join_request_authorized',
      'short_code_verification_status': 'verified',
      'authorization_package_status': 'authorization_package_ready',
      'authorization_package_preconditions': 'satisfied',
      'error_codes': <String>[],
    },
    'device_revocation': {
      'status': 'device_revocation_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_ready',
      'error_codes': <String>[],
    },
  };
}

Map<String, Object?> partiallyBlockedSyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'fixture_readiness',
    'recovery_setup': {
      'status': 'recovery_setup_blocked',
      'blocker': 'recovery_record_missing',
      'entry_action_status': 'blocked_by_recovery',
      'generated_code_status': 'not_generated',
      'save_confirmation_status': 'required_before_first_upload',
      'recovery_record_status': 'recovery_record_missing',
      'first_upload_gate': 'blocked_until_recovery_record_active',
      'required_prerequisites': [
        'platform_private_key_backend_ready',
        'release_deployment_evidence_summary_required',
        'explicit_user_start_required',
      ],
      'error_codes': ['recovery_record_missing', 'network_unreachable'],
    },
    'recovery_restore': {
      'status': 'recovery_restore_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'code_input_status': 'available',
      'recovery_record_lookup_status': 'recovery_record_active',
      'attempt_limit_status': 'available',
      'device_registration_status': 'ready_after_recovery_success',
      'error_codes': <String>[],
    },
    'device_join': {
      'status': 'device_join_blocked',
      'blocker': 'join_request_expired',
      'entry_action_status': 'blocked_by_recovery',
      'join_request_status': 'join_request_expired',
      'short_code_verification_status': 'short_code_match_required',
      'authorization_package_status': 'authorization_package_blocked',
      'authorization_package_preconditions': [
        'join_request_pending_required',
        'short_code_match_required',
      ],
      'error_codes': ['join_request_expired', 'authorization_rejected'],
    },
    'device_revocation': {
      'status': 'device_revocation_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_ready',
      'error_codes': <String>[],
    },
  };
}

Map<String, Object?> recoveryRecordMissingSyncReadinessBridgeJson() {
  final json = readySyncReadinessBridgeJson();
  return {
    ...json,
    'source': 'fixture_readiness',
    'recovery_setup': {
      ..._bridgeSection(json, 'recovery_setup'),
      'status': 'recovery_setup_blocked',
      'blocker': 'recovery_record_missing',
      'entry_action_status': 'blocked_by_recovery',
      'generated_code_status': 'not_generated',
      'save_confirmation_status': 'required_before_first_upload',
      'recovery_record_status': 'recovery_record_missing',
      'first_upload_gate': 'blocked_until_recovery_record_active',
      'required_prerequisites': [
        'platform_private_key_backend_ready',
        'release_deployment_evidence_summary_required',
        'explicit_user_start_required',
      ],
      'error_codes': ['recovery_record_missing'],
    },
  };
}

Map<String, Object?> joinRequestExpiredSyncReadinessBridgeJson() {
  final json = readySyncReadinessBridgeJson();
  return {
    ...json,
    'source': 'fixture_readiness',
    'device_join': {
      ..._bridgeSection(json, 'device_join'),
      'status': 'device_join_blocked',
      'blocker': 'join_request_expired',
      'entry_action_status': 'blocked_by_recovery',
      'join_request_status': 'join_request_expired',
      'short_code_verification_status': 'short_code_match_required',
      'authorization_package_status': 'authorization_package_blocked',
      'authorization_package_preconditions': ['join_request_pending_required'],
      'error_codes': ['join_request_expired', 'authorization_rejected'],
    },
  };
}

Map<String, Object?> unsupportedFormatSyncReadinessBridgeJson() {
  return {
    ...readySyncReadinessBridgeJson(),
    'format': 'manager_sync_readiness.v2',
  };
}

Map<String, Object?> unsafeRedactionSyncReadinessBridgeJson() {
  return {
    ...readySyncReadinessBridgeJson(),
    'redaction_policy': 'raw_native_payload_with_secret_fields',
  };
}

Map<String, Object?> unsafeSyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'ffi_native_readiness_with_token=secret-token',
    'token': 'secret-token',
    'server_endpoint': 'https://user:secret-token@sync.example.invalid',
    'request_body': '{"recovery_code":"RADISHLEX-RECOVERY-CODE-SECRET"}',
    'recovery_setup': {
      'status': 'native_status:secret-token',
      'blocker': 'recovery_record_missing',
      'entry_action_status': 'available',
      'generated_code_status': 'RADISHLEX-RECOVERY-CODE-SECRET',
      'save_confirmation_status': '/synthetic/private/recovery.txt',
      'recovery_record_status': 'recovery_record_missing',
      'first_upload_gate': 'wrapped_material_bytes=abcdef',
      'required_prerequisites': [
        'platform_private_key_backend_ready',
        'wrapped_material_bytes=abcdef',
        42,
      ],
      'error_codes': [
        'recovery_record_missing',
        'signature_bytes=abcdef',
        false,
      ],
    },
    'recovery_restore': {
      'status': 'recovery_restore_blocked',
      'blocker': 'recovery_code_invalid',
      'entry_action_status': 'available',
      'code_input_status': 'RADISHLEX-RECOVERY-CODE-SECRET',
      'recovery_record_lookup_status': 'recovery_record_missing',
      'attempt_limit_status': 'rate_limited',
      'device_registration_status': 'blocked_until_recovery_success',
      'error_codes': 'recovery_code_invalid, token=secret-token',
    },
    'device_join': {
      'status': 'device_join_blocked',
      'blocker': 'join_request_expired',
      'entry_action_status': 'available',
      'join_request_status': 'join_request_pending short_code=123456',
      'short_code_verification_status': 'short_code=123456',
      'authorization_package_status': 'authorization_package_blocked',
      'authorization_package_preconditions':
          'active_existing_device_required, /synthetic/private/join.txt',
      'error_codes': ['network_unreachable', 'payload_bytes=abcdef'],
    },
    'device_revocation': {
      'status': 'device_revocation_blocked',
      'blocker': 'key_epoch_rotation_required',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_rotation_required',
      'error_codes': ['key_epoch_rotation_required', 'private_key=abcdef'],
    },
  };
}

Map<String, Object?> _bridgeSection(Map<String, Object?> json, String section) {
  return Map<String, Object?>.from(json[section]! as Map);
}
