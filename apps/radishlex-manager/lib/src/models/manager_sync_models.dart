import 'manager_settings_models.dart';

enum SyncUiState {
  localOnly,
  preflightReady,
  serverConfigured,
  backendUnavailable,
  deploymentUnverified,
  syncDisabledByPolicy,
  readyForUserSync,
}

extension SyncUiStateLabel on SyncUiState {
  String get code {
    switch (this) {
      case SyncUiState.localOnly:
        return 'local_only';
      case SyncUiState.preflightReady:
        return 'preflight_ready';
      case SyncUiState.serverConfigured:
        return 'server_configured';
      case SyncUiState.backendUnavailable:
        return 'backend_unavailable';
      case SyncUiState.deploymentUnverified:
        return 'deployment_unverified';
      case SyncUiState.syncDisabledByPolicy:
        return 'sync_disabled_by_policy';
      case SyncUiState.readyForUserSync:
        return 'ready_for_user_sync';
    }
  }

  bool get canEnableUserSync => this == SyncUiState.readyForUserSync;
}

enum SyncEntryState {
  localOnly,
  syncDisabledByPolicy,
  backendUnavailable,
  deploymentUnverified,
  localSmokeReady,
  preflightReady,
  blockedBeforeUserSync,
  readyForUserSync,
}

extension SyncEntryStateLabel on SyncEntryState {
  String get code {
    switch (this) {
      case SyncEntryState.localOnly:
        return 'local_only';
      case SyncEntryState.syncDisabledByPolicy:
        return 'sync_disabled_by_policy';
      case SyncEntryState.backendUnavailable:
        return 'backend_unavailable';
      case SyncEntryState.deploymentUnverified:
        return 'deployment_unverified';
      case SyncEntryState.localSmokeReady:
        return 'local_smoke_ready';
      case SyncEntryState.preflightReady:
        return 'preflight_ready';
      case SyncEntryState.blockedBeforeUserSync:
        return 'blocked_before_user_sync';
      case SyncEntryState.readyForUserSync:
        return 'ready_for_user_sync';
    }
  }
}

enum RecoveryEntryStatus {
  flowClosed,
  recoveryCodeRequired,
  saveConfirmationRequired,
  ready,
}

extension RecoveryEntryStatusLabel on RecoveryEntryStatus {
  String get code {
    switch (this) {
      case RecoveryEntryStatus.flowClosed:
        return 'recovery_code_flow_closed';
      case RecoveryEntryStatus.recoveryCodeRequired:
        return 'recovery_code_required';
      case RecoveryEntryStatus.saveConfirmationRequired:
        return 'recovery_code_save_confirmation_required';
      case RecoveryEntryStatus.ready:
        return 'recovery_ready';
    }
  }

  String get label {
    switch (this) {
      case RecoveryEntryStatus.flowClosed:
        return '恢复码流程关闭';
      case RecoveryEntryStatus.recoveryCodeRequired:
        return '等待恢复码';
      case RecoveryEntryStatus.saveConfirmationRequired:
        return '等待保存确认';
      case RecoveryEntryStatus.ready:
        return '恢复码准备完成';
    }
  }
}

enum DeviceAuthorizationEntryStatus {
  flowClosed,
  joinRequestUnavailable,
  authorizationUnavailable,
  ready,
}

extension DeviceAuthorizationEntryStatusLabel
    on DeviceAuthorizationEntryStatus {
  String get code {
    switch (this) {
      case DeviceAuthorizationEntryStatus.flowClosed:
        return 'device_authorization_flow_closed';
      case DeviceAuthorizationEntryStatus.joinRequestUnavailable:
        return 'join_request_unavailable';
      case DeviceAuthorizationEntryStatus.authorizationUnavailable:
        return 'authorization_unavailable';
      case DeviceAuthorizationEntryStatus.ready:
        return 'device_authorization_ready';
    }
  }

  String get label {
    switch (this) {
      case DeviceAuthorizationEntryStatus.flowClosed:
        return '设备授权流程关闭';
      case DeviceAuthorizationEntryStatus.joinRequestUnavailable:
        return '加入请求不可用';
      case DeviceAuthorizationEntryStatus.authorizationUnavailable:
        return '授权不可用';
      case DeviceAuthorizationEntryStatus.ready:
        return '设备授权准备完成';
    }
  }
}

enum JoinRequestStatus { unavailable, pending, expired, authorized }

extension JoinRequestStatusLabel on JoinRequestStatus {
  String get code {
    switch (this) {
      case JoinRequestStatus.unavailable:
        return 'join_request_unavailable';
      case JoinRequestStatus.pending:
        return 'join_request_pending';
      case JoinRequestStatus.expired:
        return 'join_request_expired';
      case JoinRequestStatus.authorized:
        return 'join_request_authorized';
    }
  }

  String get label {
    switch (this) {
      case JoinRequestStatus.unavailable:
        return '加入请求不可用';
      case JoinRequestStatus.pending:
        return '加入请求待处理';
      case JoinRequestStatus.expired:
        return '加入请求已过期';
      case JoinRequestStatus.authorized:
        return '加入请求已授权';
    }
  }
}

enum SyncConnectionStatus {
  notConfigured,
  syncDisabledByPolicy,
  endpointInvalid,
  accessTokenMissing,
  localHttpsReadyForProbe,
  localHttpReadyForProbe,
  externalProbeDeferred,
  unsupportedTransport,
  readOnlyProbeReachable,
  readOnlyProbeNetworkUnavailable,
  readOnlyProbeTlsError,
  readOnlyProbeUnexpectedStatus,
  readOnlyProbeSummaryInvalid,
}

extension SyncConnectionStatusLabel on SyncConnectionStatus {
  String get code {
    switch (this) {
      case SyncConnectionStatus.notConfigured:
        return 'not_configured';
      case SyncConnectionStatus.syncDisabledByPolicy:
        return 'sync_disabled_by_policy';
      case SyncConnectionStatus.endpointInvalid:
        return 'endpoint_invalid';
      case SyncConnectionStatus.accessTokenMissing:
        return 'access_token_missing';
      case SyncConnectionStatus.localHttpsReadyForProbe:
        return 'local_https_ready_for_probe';
      case SyncConnectionStatus.localHttpReadyForProbe:
        return 'local_http_ready_for_probe';
      case SyncConnectionStatus.externalProbeDeferred:
        return 'external_probe_deferred';
      case SyncConnectionStatus.unsupportedTransport:
        return 'unsupported_transport';
      case SyncConnectionStatus.readOnlyProbeReachable:
        return 'reachable';
      case SyncConnectionStatus.readOnlyProbeNetworkUnavailable:
        return 'network_unreachable';
      case SyncConnectionStatus.readOnlyProbeTlsError:
        return 'tls_error';
      case SyncConnectionStatus.readOnlyProbeUnexpectedStatus:
        return 'reachable_with_unexpected_status';
      case SyncConnectionStatus.readOnlyProbeSummaryInvalid:
        return 'probe_summary_invalid';
    }
  }

  String get label {
    switch (this) {
      case SyncConnectionStatus.notConfigured:
        return '连接未配置';
      case SyncConnectionStatus.syncDisabledByPolicy:
        return '策略禁用连接检查';
      case SyncConnectionStatus.endpointInvalid:
        return 'endpoint 草案不可用';
      case SyncConnectionStatus.accessTokenMissing:
        return 'access token 未记录';
      case SyncConnectionStatus.localHttpsReadyForProbe:
        return '本地 HTTPS 可执行只读探测';
      case SyncConnectionStatus.localHttpReadyForProbe:
        return '本地 HTTP 可执行只读探测';
      case SyncConnectionStatus.externalProbeDeferred:
        return '外部目标探测后置';
      case SyncConnectionStatus.unsupportedTransport:
        return '连接传输模式不受支持';
      case SyncConnectionStatus.readOnlyProbeReachable:
        return '只读探测已到达服务';
      case SyncConnectionStatus.readOnlyProbeNetworkUnavailable:
        return '只读探测网络不可达';
      case SyncConnectionStatus.readOnlyProbeTlsError:
        return '只读探测 TLS 不可用';
      case SyncConnectionStatus.readOnlyProbeUnexpectedStatus:
        return '只读探测返回非预期状态';
      case SyncConnectionStatus.readOnlyProbeSummaryInvalid:
        return '只读探测摘要不可用';
    }
  }
}

const managerSyncConnectionHealthSummaryFormat = 'sync_connection_health.v1';
const managerSyncConnectionHealthSummaryRedactionPolicy =
    'summary_only_no_endpoint_tokens_or_response_body';

class SyncConnectionHealth {
  const SyncConnectionHealth({
    required this.status,
    required this.connectionBlocker,
    required this.endpointStatus,
    required this.accessTokenStatus,
    required this.transportMode,
    required this.serverStateStatus,
    required this.lastRemoteErrorCode,
    this.probeSource = 'not_recorded',
    this.probeRecordedAt = 'not_recorded',
    this.authStatus = 'not_checked',
    this.httpStatus = 0,
    this.httpStatusClass = 'not_checked',
    this.localInsecureTls = 'not_checked',
  });

  final SyncConnectionStatus status;
  final String connectionBlocker;
  final String endpointStatus;
  final String accessTokenStatus;
  final String transportMode;
  final String serverStateStatus;
  final String lastRemoteErrorCode;
  final String probeSource;
  final String probeRecordedAt;
  final String authStatus;
  final int httpStatus;
  final String httpStatusClass;
  final String localInsecureTls;

  bool get canRunReadOnlyProbe =>
      status == SyncConnectionStatus.localHttpsReadyForProbe ||
      status == SyncConnectionStatus.localHttpReadyForProbe;

  bool get hasReadOnlyProbeResult =>
      status == SyncConnectionStatus.readOnlyProbeReachable ||
      status == SyncConnectionStatus.readOnlyProbeNetworkUnavailable ||
      status == SyncConnectionStatus.readOnlyProbeTlsError ||
      status == SyncConnectionStatus.readOnlyProbeUnexpectedStatus;

  bool get isConnectionHealthy =>
      status == SyncConnectionStatus.readOnlyProbeReachable &&
      connectionBlocker == 'none';
}

class SyncConnectionProbeSummary {
  const SyncConnectionProbeSummary._({
    required this.format,
    required this.redactionPolicy,
    required this.endpointStatus,
    required this.transportMode,
    required this.accessTokenStatus,
    required this.connectionStatus,
    required this.authStatus,
    required this.serverStateStatus,
    required this.httpStatus,
    required this.httpStatusClass,
    required this.lastRemoteErrorCode,
    required this.localInsecureTls,
  });

  factory SyncConnectionProbeSummary.fromJson(Map<String, Object?> json) {
    return SyncConnectionProbeSummary._(
      format: _summaryString(json, 'format', const {
        managerSyncConnectionHealthSummaryFormat,
      }, 'unsupported_format'),
      redactionPolicy: _summaryString(json, 'redaction_policy', const {
        managerSyncConnectionHealthSummaryRedactionPolicy,
      }, 'unsupported_redaction_policy'),
      endpointStatus: _summaryString(
        json,
        'endpoint_status',
        _syncConnectionEndpointStatuses,
        'unknown_endpoint_status',
      ),
      transportMode: _summaryString(
        json,
        'transport_mode',
        _syncConnectionTransportModes,
        'unknown_transport_mode',
      ),
      accessTokenStatus: _summaryString(json, 'access_token_status', const {
        'configured',
        'not_configured',
      }, 'unknown_access_token_status'),
      connectionStatus: _summaryString(
        json,
        'connection_status',
        _syncConnectionProbeStatuses,
        'unknown_connection_status',
      ),
      authStatus: _summaryString(
        json,
        'auth_status',
        _syncConnectionAuthStatuses,
        'unknown_auth_status',
      ),
      serverStateStatus: _summaryString(
        json,
        'server_state_status',
        _syncConnectionServerStateStatuses,
        'unknown_server_state_status',
      ),
      httpStatus: _summaryHttpStatus(json['http_status']),
      httpStatusClass: _summaryString(
        json,
        'http_status_class',
        _syncConnectionHttpStatusClasses,
        'unknown_http_status_class',
      ),
      lastRemoteErrorCode: _summaryString(
        json,
        'last_remote_error_code',
        _syncConnectionRemoteErrorCodes,
        'unexpected_remote_error_code',
      ),
      localInsecureTls: _summaryString(json, 'local_insecure_tls', const {
        'allowed',
        'system_trust',
      }, 'unknown_tls_policy'),
    );
  }

  final String format;
  final String redactionPolicy;
  final String endpointStatus;
  final String transportMode;
  final String accessTokenStatus;
  final String connectionStatus;
  final String authStatus;
  final String serverStateStatus;
  final int httpStatus;
  final String httpStatusClass;
  final String lastRemoteErrorCode;
  final String localInsecureTls;

  bool get hasSupportedFormat =>
      format == managerSyncConnectionHealthSummaryFormat &&
      redactionPolicy == managerSyncConnectionHealthSummaryRedactionPolicy;
}

class RecoveryEntryGate {
  const RecoveryEntryGate({
    required this.status,
    required this.blocker,
    required this.canGenerateCode,
    required this.canRestoreDevice,
    required this.requiresSaveConfirmation,
  });

  final RecoveryEntryStatus status;
  final String blocker;
  final bool canGenerateCode;
  final bool canRestoreDevice;
  final bool requiresSaveConfirmation;

  String get generateStatus {
    return canGenerateCode ? 'available' : 'closed_current_phase';
  }

  String get restoreStatus {
    return canRestoreDevice ? 'available' : 'closed_current_phase';
  }

  String get confirmationStatus {
    return requiresSaveConfirmation ? 'required' : 'not_started';
  }
}

class DeviceAuthorizationEntryGate {
  const DeviceAuthorizationEntryGate({
    required this.status,
    required this.blocker,
    required this.joinRequestStatus,
    required this.canCreateJoinRequest,
    required this.canApproveJoinRequest,
    required this.canRevokeDevice,
  });

  final DeviceAuthorizationEntryStatus status;
  final String blocker;
  final JoinRequestStatus joinRequestStatus;
  final bool canCreateJoinRequest;
  final bool canApproveJoinRequest;
  final bool canRevokeDevice;

  String get createJoinRequestStatus {
    return canCreateJoinRequest ? 'available' : 'closed_current_phase';
  }

  String get approveJoinRequestStatus {
    return canApproveJoinRequest ? 'available' : 'closed_current_phase';
  }

  String get revokeDeviceStatus {
    return canRevokeDevice ? 'available' : 'closed_current_phase';
  }
}

const managerClosedRecoveryEntryGate = RecoveryEntryGate(
  status: RecoveryEntryStatus.flowClosed,
  blocker: 'recovery_code_flow_closed',
  canGenerateCode: false,
  canRestoreDevice: false,
  requiresSaveConfirmation: false,
);

const managerClosedDeviceAuthorizationEntryGate = DeviceAuthorizationEntryGate(
  status: DeviceAuthorizationEntryStatus.flowClosed,
  blocker: 'device_authorization_flow_closed',
  joinRequestStatus: JoinRequestStatus.unavailable,
  canCreateJoinRequest: false,
  canApproveJoinRequest: false,
  canRevokeDevice: false,
);

const managerUnconfiguredSyncConnectionHealth = SyncConnectionHealth(
  status: SyncConnectionStatus.notConfigured,
  connectionBlocker: 'server_endpoint_missing',
  endpointStatus: 'not_configured',
  accessTokenStatus: 'not_configured',
  transportMode: 'not_configured',
  serverStateStatus: 'not_checked_endpoint_missing',
  lastRemoteErrorCode: 'none',
);

class ManagerSyncEntryGate {
  const ManagerSyncEntryGate({
    required this.entryState,
    required this.entryBlocker,
    required this.localEvidenceSource,
    required this.productionBlockers,
    this.connectionHealth = managerUnconfiguredSyncConnectionHealth,
    required this.recovery,
    required this.deviceAuthorization,
    required this.userSyncEnabled,
  });

  final SyncEntryState entryState;
  final String entryBlocker;
  final String localEvidenceSource;
  final List<String> productionBlockers;
  final SyncConnectionHealth connectionHealth;
  final RecoveryEntryGate recovery;
  final DeviceAuthorizationEntryGate deviceAuthorization;
  final bool userSyncEnabled;

  SyncUiState get uiState {
    switch (entryState) {
      case SyncEntryState.localOnly:
        return SyncUiState.localOnly;
      case SyncEntryState.syncDisabledByPolicy:
        return SyncUiState.syncDisabledByPolicy;
      case SyncEntryState.backendUnavailable:
        return SyncUiState.backendUnavailable;
      case SyncEntryState.deploymentUnverified:
        return SyncUiState.deploymentUnverified;
      case SyncEntryState.localSmokeReady:
      case SyncEntryState.preflightReady:
      case SyncEntryState.blockedBeforeUserSync:
        return SyncUiState.preflightReady;
      case SyncEntryState.readyForUserSync:
        return SyncUiState.readyForUserSync;
    }
  }

  String get productionBlockerSummary {
    return productionBlockers.isEmpty ? 'none' : productionBlockers.join(', ');
  }
}

class ManagerSyncGateAudit {
  const ManagerSyncGateAudit({
    required this.state,
    required this.entryGate,
    required this.stateLabel,
    required this.stateSource,
    required this.actionStopLine,
    required this.deviceGateLabel,
    required this.deviceGateReady,
  });

  final SyncUiState state;
  final ManagerSyncEntryGate entryGate;
  final String stateLabel;
  final String stateSource;
  final String actionStopLine;
  final String deviceGateLabel;
  final bool deviceGateReady;
}

class SyncPreflightSummary {
  const SyncPreflightSummary({
    required this.state,
    required this.serverEndpoint,
    required this.reason,
    required this.syncableObjects,
    required this.localOnlyEvents,
    required this.lastUpload,
    required this.lastDownload,
    required this.categories,
    required this.device,
  });

  final SyncUiState state;
  final String serverEndpoint;
  final String reason;
  final int syncableObjects;
  final int localOnlyEvents;
  final String lastUpload;
  final String lastDownload;
  final List<SyncCategorySummary> categories;
  final DeviceSecuritySummary device;

  SyncPreflightSummary copyWith({
    SyncUiState? state,
    String? serverEndpoint,
    String? reason,
    int? syncableObjects,
    int? localOnlyEvents,
    String? lastUpload,
    String? lastDownload,
    List<SyncCategorySummary>? categories,
    DeviceSecuritySummary? device,
  }) {
    return SyncPreflightSummary(
      state: state ?? this.state,
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      reason: reason ?? this.reason,
      syncableObjects: syncableObjects ?? this.syncableObjects,
      localOnlyEvents: localOnlyEvents ?? this.localOnlyEvents,
      lastUpload: lastUpload ?? this.lastUpload,
      lastDownload: lastDownload ?? this.lastDownload,
      categories: categories ?? this.categories,
      device: device ?? this.device,
    );
  }
}

class SyncCategorySummary {
  const SyncCategorySummary({required this.name, required this.count});

  final String name;
  final int count;
}

class DeviceSecuritySummary {
  const DeviceSecuritySummary({
    required this.deviceId,
    required this.backendId,
    required this.capabilityStatus,
    required this.productionGate,
  });

  final String deviceId;
  final String backendId;
  final String capabilityStatus;
  final String productionGate;
}

SyncUiState deriveManagerSyncUiState({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  return deriveManagerSyncEntryGate(draft: draft, device: device).uiState;
}

SyncConnectionHealth deriveManagerSyncConnectionHealth(
  ManagerSettingsDraft draft,
) {
  final normalized = draft.normalized();
  if (normalized.privacyMode) {
    return const SyncConnectionHealth(
      status: SyncConnectionStatus.syncDisabledByPolicy,
      connectionBlocker: 'sync_disabled_by_policy',
      endpointStatus: 'not_checked_policy_disabled',
      accessTokenStatus: 'not_checked_policy_disabled',
      transportMode: 'not_checked',
      serverStateStatus: 'not_checked_policy_disabled',
      lastRemoteErrorCode: 'none',
    );
  }
  if (!normalized.hasServerEndpoint) {
    return managerUnconfiguredSyncConnectionHealth;
  }

  final endpoint = _classifySyncEndpoint(normalized.serverEndpoint);
  final accessTokenStatus = managerSyncAccessTokenStatus(normalized);
  if (endpoint.blocker != 'none') {
    return SyncConnectionHealth(
      status: SyncConnectionStatus.endpointInvalid,
      connectionBlocker: endpoint.blocker,
      endpointStatus: endpoint.status,
      accessTokenStatus: accessTokenStatus,
      transportMode: endpoint.transportMode,
      serverStateStatus: 'not_checked_endpoint_invalid',
      lastRemoteErrorCode: 'configuration_invalid',
    );
  }
  if (normalized.hasSyncConnectionProbeRecord) {
    return managerSyncConnectionHealthFromProbeRecord(
      normalized.syncConnectionProbeRecord,
    );
  }
  if (!normalized.hasAccessToken) {
    return SyncConnectionHealth(
      status: SyncConnectionStatus.accessTokenMissing,
      connectionBlocker: 'access_token_missing',
      endpointStatus: endpoint.status,
      accessTokenStatus: accessTokenStatus,
      transportMode: endpoint.transportMode,
      serverStateStatus: 'not_checked_access_token_missing',
      lastRemoteErrorCode: 'none',
    );
  }

  switch (endpoint.transportMode) {
    case 'local_https':
      return SyncConnectionHealth(
        status: SyncConnectionStatus.localHttpsReadyForProbe,
        connectionBlocker: 'read_only_probe_not_run',
        endpointStatus: endpoint.status,
        accessTokenStatus: accessTokenStatus,
        transportMode: endpoint.transportMode,
        serverStateStatus: 'read_only_probe_pending',
        lastRemoteErrorCode: 'none',
      );
    case 'local_http':
      return SyncConnectionHealth(
        status: SyncConnectionStatus.localHttpReadyForProbe,
        connectionBlocker: 'read_only_probe_not_run',
        endpointStatus: endpoint.status,
        accessTokenStatus: accessTokenStatus,
        transportMode: endpoint.transportMode,
        serverStateStatus: 'read_only_probe_pending',
        lastRemoteErrorCode: 'none',
      );
    case 'external_https':
      return SyncConnectionHealth(
        status: SyncConnectionStatus.externalProbeDeferred,
        connectionBlocker: 'external_target_probe_deferred',
        endpointStatus: endpoint.status,
        accessTokenStatus: accessTokenStatus,
        transportMode: endpoint.transportMode,
        serverStateStatus: 'not_checked_release_probe_deferred',
        lastRemoteErrorCode: 'none',
      );
    default:
      return SyncConnectionHealth(
        status: SyncConnectionStatus.unsupportedTransport,
        connectionBlocker: 'remote_plain_http_forbidden',
        endpointStatus: endpoint.status,
        accessTokenStatus: accessTokenStatus,
        transportMode: endpoint.transportMode,
        serverStateStatus: 'not_checked_unsupported_transport',
        lastRemoteErrorCode: 'configuration_invalid',
      );
  }
}

SyncConnectionHealth managerSyncConnectionHealthFromProbeSummary(
  SyncConnectionProbeSummary summary,
) {
  if (!summary.hasSupportedFormat) {
    return SyncConnectionHealth(
      status: SyncConnectionStatus.readOnlyProbeSummaryInvalid,
      connectionBlocker: 'probe_summary_format_unsupported',
      endpointStatus: 'not_checked_probe_summary_invalid',
      accessTokenStatus: 'not_checked_probe_summary_invalid',
      transportMode: 'not_checked_probe_summary_invalid',
      serverStateStatus: 'not_checked_probe_summary_invalid',
      lastRemoteErrorCode: 'probe_summary_invalid',
      authStatus: summary.authStatus,
      httpStatus: summary.httpStatus,
      httpStatusClass: summary.httpStatusClass,
      localInsecureTls: summary.localInsecureTls,
    );
  }

  return SyncConnectionHealth(
    status: _syncConnectionStatusFromProbeSummary(summary),
    connectionBlocker: _syncConnectionBlockerFromProbeSummary(summary),
    endpointStatus: summary.endpointStatus,
    accessTokenStatus: summary.accessTokenStatus,
    transportMode: summary.transportMode,
    serverStateStatus: summary.serverStateStatus,
    lastRemoteErrorCode: summary.lastRemoteErrorCode,
    authStatus: summary.authStatus,
    httpStatus: summary.httpStatus,
    httpStatusClass: summary.httpStatusClass,
    localInsecureTls: summary.localInsecureTls,
  );
}

SyncConnectionHealth managerSyncConnectionHealthFromProbeRecord(
  ManagerSyncConnectionProbeRecord record,
) {
  final summary = _syncConnectionProbeSummaryFromRecord(record);
  final health = managerSyncConnectionHealthFromProbeSummary(summary);
  return SyncConnectionHealth(
    status: health.status,
    connectionBlocker: health.connectionBlocker,
    endpointStatus: health.endpointStatus,
    accessTokenStatus: health.accessTokenStatus,
    transportMode: health.transportMode,
    serverStateStatus: health.serverStateStatus,
    lastRemoteErrorCode: health.lastRemoteErrorCode,
    probeSource: _sanitizedSyncConnectionProbeSource(record.source),
    probeRecordedAt: _sanitizedSyncConnectionProbeRecordedAt(record.recordedAt),
    authStatus: health.authStatus,
    httpStatus: health.httpStatus,
    httpStatusClass: health.httpStatusClass,
    localInsecureTls: health.localInsecureTls,
  );
}

ManagerSyncConnectionProbeRecord managerSyncConnectionProbeRecordFromSummary(
  SyncConnectionProbeSummary summary, {
  required String source,
  required String recordedAt,
}) {
  return ManagerSyncConnectionProbeRecord(
    source: _sanitizedSyncConnectionProbeSource(source),
    recordedAt: _sanitizedSyncConnectionProbeRecordedAt(recordedAt),
    format: summary.format,
    redactionPolicy: summary.redactionPolicy,
    endpointStatus: summary.endpointStatus,
    transportMode: summary.transportMode,
    accessTokenStatus: summary.accessTokenStatus,
    connectionStatus: summary.connectionStatus,
    authStatus: summary.authStatus,
    serverStateStatus: summary.serverStateStatus,
    httpStatus: summary.httpStatus,
    httpStatusClass: summary.httpStatusClass,
    lastRemoteErrorCode: summary.lastRemoteErrorCode,
    localInsecureTls: summary.localInsecureTls,
  );
}

ManagerSyncConnectionProbeRecord managerSanitizeSyncConnectionProbeRecord(
  ManagerSyncConnectionProbeRecord record,
) {
  if (!record.isRecorded) {
    return const ManagerSyncConnectionProbeRecord.empty();
  }
  return managerSyncConnectionProbeRecordFromSummary(
    _syncConnectionProbeSummaryFromRecord(record),
    source: record.source,
    recordedAt: record.recordedAt,
  );
}

String managerSyncConnectionProbeSourceForSummary(
  SyncConnectionProbeSummary summary,
) {
  switch (summary.transportMode) {
    case 'local_https':
      return managerSyncConnectionProbeSourceLocalDockerHttps;
    case 'local_http':
      return managerSyncConnectionProbeSourceLocalHttp;
    case 'external_https':
      return managerSyncConnectionProbeSourceExternalHttps;
    default:
      return managerSyncConnectionProbeSourceImportedSummary;
  }
}

ManagerSyncEntryGate deriveManagerSyncEntryGate({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final normalized = draft.normalized();
  final localEvidenceSource = managerSyncLocalEvidenceSource(normalized);
  final connectionHealth = deriveManagerSyncConnectionHealth(normalized);
  final productionBlockers = managerSyncProductionBlockers(
    draft: normalized,
    device: device,
  );

  if (normalized.privacyMode) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.syncDisabledByPolicy,
      entryBlocker: 'sync_disabled_by_policy',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      connectionHealth: connectionHealth,
      recovery: managerClosedRecoveryEntryGate,
      deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
      userSyncEnabled: false,
    );
  }
  if (!normalized.hasServerEndpoint) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.localOnly,
      entryBlocker: 'server_endpoint_missing',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      connectionHealth: connectionHealth,
      recovery: managerClosedRecoveryEntryGate,
      deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
      userSyncEnabled: false,
    );
  }
  if (!managerDeviceGateReady(device)) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.backendUnavailable,
      entryBlocker: 'backend_unavailable',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      connectionHealth: connectionHealth,
      recovery: managerClosedRecoveryEntryGate,
      deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
      userSyncEnabled: false,
    );
  }
  if (!normalized.hasDeploymentEvidence) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.deploymentUnverified,
      entryBlocker: 'deployment_unverified',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      connectionHealth: connectionHealth,
      recovery: managerClosedRecoveryEntryGate,
      deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
      userSyncEnabled: false,
    );
  }
  if (normalized.deploymentEvidenceSource ==
      managerDeploymentEvidenceLocalSmoke) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.localSmokeReady,
      entryBlocker: 'release_deployment_evidence_required',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      connectionHealth: connectionHealth,
      recovery: managerClosedRecoveryEntryGate,
      deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
      userSyncEnabled: false,
    );
  }

  return ManagerSyncEntryGate(
    entryState: SyncEntryState.blockedBeforeUserSync,
    entryBlocker: managerClosedRecoveryEntryGate.blocker,
    localEvidenceSource: localEvidenceSource,
    productionBlockers: productionBlockers,
    connectionHealth: connectionHealth,
    recovery: managerClosedRecoveryEntryGate,
    deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
    userSyncEnabled: false,
  );
}

ManagerSyncGateAudit managerSyncGateAudit({
  required SyncUiState state,
  required DeviceSecuritySummary device,
  ManagerSettingsDraft? draft,
}) {
  final entryGate = draft == null
      ? _managerSyncEntryGateFromState(state: state, device: device)
      : deriveManagerSyncEntryGate(draft: draft, device: device);
  return ManagerSyncGateAudit(
    state: state,
    entryGate: entryGate,
    stateLabel: managerSyncStateLabel(state),
    stateSource: draft == null
        ? managerSyncStateSourceDescription(state: state, device: device)
        : managerSyncStateSourceDescriptionForDraft(
            draft: draft,
            device: device,
          ),
    actionStopLine: managerSyncActionStopLine(state, entryGate: entryGate),
    deviceGateLabel: managerDeviceGateLabel(device),
    deviceGateReady: managerDeviceGateReady(device),
  );
}

ManagerSyncGateAudit managerSyncGateAuditForDraft({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final entryGate = deriveManagerSyncEntryGate(draft: draft, device: device);
  return managerSyncGateAudit(
    state: entryGate.uiState,
    device: device,
    draft: draft,
  );
}

String managerSyncStateLabel(SyncUiState state) {
  switch (state) {
    case SyncUiState.localOnly:
      return '仅本地管理';
    case SyncUiState.preflightReady:
      return '本地预检通过';
    case SyncUiState.serverConfigured:
      return '服务端草案已保留';
    case SyncUiState.backendUnavailable:
      return '平台签名 backend 不可用';
    case SyncUiState.deploymentUnverified:
      return '部署证据未记录';
    case SyncUiState.syncDisabledByPolicy:
      return '策略禁用同步';
    case SyncUiState.readyForUserSync:
      return '等待后续开放';
  }
}

String managerSyncStateSourceDescription({
  required SyncUiState state,
  required DeviceSecuritySummary device,
}) {
  switch (state) {
    case SyncUiState.localOnly:
      return '未保留自部署服务端草案';
    case SyncUiState.syncDisabledByPolicy:
      return '设置草案启用隐私模式';
    case SyncUiState.backendUnavailable:
      return '设备 production gate 为 ${device.productionGate}';
    case SyncUiState.deploymentUnverified:
      return '设置草案缺少目标部署验证记录';
    case SyncUiState.preflightReady:
      return '本地对象与设置草案预检通过';
    case SyncUiState.serverConfigured:
      return '服务端草案已配置但仍未开放真实同步';
    case SyncUiState.readyForUserSync:
      return '当前 Phase 4 仍关闭用户可用同步入口';
  }
}

String managerSyncStateSourceDescriptionForDraft({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final entryGate = deriveManagerSyncEntryGate(draft: draft, device: device);
  switch (entryGate.entryState) {
    case SyncEntryState.localOnly:
      return '未保留自部署服务端草案';
    case SyncEntryState.syncDisabledByPolicy:
      return '设置草案启用隐私模式';
    case SyncEntryState.backendUnavailable:
      return '设备 production gate 为 ${device.productionGate}';
    case SyncEntryState.deploymentUnverified:
      return '设置草案缺少目标部署验证记录';
    case SyncEntryState.localSmokeReady:
      return '本地 Docker / 本地 HTTPS smoke 已记录，真实用户同步仍等待发布级证据';
    case SyncEntryState.preflightReady:
      return '本地对象、设置草案和部署来源预检通过';
    case SyncEntryState.blockedBeforeUserSync:
      return '本地预检通过，真实同步入口仍等待恢复码和设备授权';
    case SyncEntryState.readyForUserSync:
      return '用户可用同步入口已满足前置门禁';
  }
}

String managerSyncActionStopLine(
  SyncUiState state, {
  ManagerSyncEntryGate? entryGate,
}) {
  if (entryGate?.entryState == SyncEntryState.localSmokeReady) {
    return '本地 Docker / 本地 HTTPS 证据只支撑开发联调；真实远端同步、恢复码和设备授权仍处于关闭状态。';
  }
  if (entryGate != null && !entryGate.userSyncEnabled) {
    return '真实远端同步、恢复码和设备授权仍处于关闭状态；本页只展示本地预检和不可用原因。';
  }
  if (state.canEnableUserSync) {
    return '真实远端同步入口仍等待设备授权、恢复码和生产门禁完成后开放。';
  }
  return '真实远端同步、恢复码和设备授权仍处于关闭状态；本页只展示本地预检和不可用原因。';
}

bool managerDeviceGateReady(DeviceSecuritySummary device) {
  return device.productionGate == 'ready';
}

String managerDeviceGateLabel(DeviceSecuritySummary device) {
  return managerDeviceGateReady(device)
      ? 'production gate ready'
      : 'production gate blocked';
}

String managerDeploymentEvidenceLabel(ManagerSettingsDraft draft) {
  return draft.hasDeploymentEvidence
      ? 'deployment evidence ${managerDeploymentEvidenceSourceLabel(draft.deploymentEvidenceSource)}'
      : 'deployment evidence missing';
}

String managerSyncLocalEvidenceSource(ManagerSettingsDraft draft) {
  return draft.hasDeploymentEvidence
      ? draft.deploymentEvidenceSource
      : 'not_recorded';
}

List<String> managerSyncProductionBlockers({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final normalized = draft.normalized();
  const recovery = managerClosedRecoveryEntryGate;
  const deviceAuthorization = managerClosedDeviceAuthorizationEntryGate;
  final blockers = <String>[];
  if (normalized.privacyMode) {
    blockers.add('sync_disabled_by_policy');
  }
  if (!normalized.hasServerEndpoint) {
    blockers.add('server_endpoint_missing');
  }
  if (normalized.hasServerEndpoint && !normalized.hasAccessToken) {
    blockers.add('access_token_missing');
  }
  if (!managerDeviceGateReady(device)) {
    blockers.add('platform_private_key_backend_${device.productionGate}');
  }
  if (!normalized.hasDeploymentEvidence) {
    blockers.add('deployment_evidence_missing');
  } else if (normalized.deploymentEvidenceSource ==
      managerDeploymentEvidenceLocalSmoke) {
    blockers.add('release_deployment_evidence_required');
  } else {
    blockers.add('release_deployment_evidence_summary_required');
  }
  blockers
    ..add(recovery.blocker)
    ..add(deviceAuthorization.blocker)
    ..add('user_sync_entry_closed_current_phase');
  return List.unmodifiable(blockers);
}

String managerSyncGateReason({
  required SyncUiState state,
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final entryGate = deriveManagerSyncEntryGate(draft: draft, device: device);
  switch (entryGate.entryState) {
    case SyncEntryState.localOnly:
      return '未保留自部署服务端草案；真实远端同步保持关闭';
    case SyncEntryState.syncDisabledByPolicy:
      return '隐私模式已启用；真实远端同步保持关闭';
    case SyncEntryState.backendUnavailable:
      return '平台私钥 backend 未解除生产门禁：${device.productionGate}';
    case SyncEntryState.deploymentUnverified:
      return '目标部署运行证据未记录；真实远端同步保持关闭';
    case SyncEntryState.localSmokeReady:
      return '本地 smoke 已记录；真实用户同步仍等待发布级部署证据、恢复码和设备授权';
    case SyncEntryState.preflightReady:
      return '本地预检通过；用户可用同步入口仍等待后续阶段开放';
    case SyncEntryState.blockedBeforeUserSync:
      return '本地预检通过；真实同步入口仍等待恢复码和设备授权';
    case SyncEntryState.readyForUserSync:
      return '用户可用同步入口尚未在当前阶段开放';
  }
}

String managerSyncEndpointLabel(ManagerSettingsDraft draft) {
  return draft.hasServerEndpoint ? draft.serverEndpoint.trim() : '未配置';
}

String managerSyncAccessTokenStatus(ManagerSettingsDraft draft) {
  return draft.hasAccessToken ? 'configured' : 'not_configured';
}

SyncConnectionProbeSummary _syncConnectionProbeSummaryFromRecord(
  ManagerSyncConnectionProbeRecord record,
) {
  return SyncConnectionProbeSummary.fromJson({
    'format': record.format,
    'redaction_policy': record.redactionPolicy,
    'endpoint_status': record.endpointStatus,
    'transport_mode': record.transportMode,
    'access_token_status': record.accessTokenStatus,
    'connection_status': record.connectionStatus,
    'auth_status': record.authStatus,
    'server_state_status': record.serverStateStatus,
    'http_status': record.httpStatus,
    'http_status_class': record.httpStatusClass,
    'last_remote_error_code': record.lastRemoteErrorCode,
    'local_insecure_tls': record.localInsecureTls,
  });
}

String _sanitizedSyncConnectionProbeSource(String source) {
  final candidate = source.trim();
  return managerSyncConnectionProbeSources.contains(candidate)
      ? candidate
      : managerSyncConnectionProbeSourceUnknown;
}

String _sanitizedSyncConnectionProbeRecordedAt(String recordedAt) {
  final candidate = recordedAt.trim();
  if (candidate.isEmpty) {
    return 'unknown_time';
  }
  final parsed = DateTime.tryParse(candidate);
  if (parsed == null) {
    return 'unknown_time';
  }
  return parsed.toUtc().toIso8601String();
}

const _syncConnectionEndpointStatuses = {
  'not_configured',
  'invalid',
  'invalid_userinfo',
  'invalid_query',
  'invalid_fragment',
  'invalid_scheme',
  'configured',
};

const _syncConnectionTransportModes = {
  'not_configured',
  'invalid_endpoint',
  'local_https',
  'local_http',
  'external_https',
  'remote_http',
};

const _syncConnectionProbeStatuses = {
  'not_configured',
  'configuration_invalid',
  'network_unreachable',
  'tls_error',
  'reachable',
  'reachable_with_unexpected_status',
};

const _syncConnectionAuthStatuses = {
  'not_checked',
  'required',
  'failed',
  'accepted',
  'not_required_for_local_probe',
  'unknown',
};

const _syncConnectionServerStateStatuses = {
  'not_checked_configuration_blocked',
  'not_checked_unsupported_transport',
  'not_checked_network_unreachable',
  'auth_gate_reachable',
  'domain_missing_expected',
  'domain_state_returned',
  'unexpected_response_status',
};

const _syncConnectionHttpStatusClasses = {
  'not_checked',
  'network_error',
  'success',
  'redirect',
  'client_error',
  'server_error',
  'unexpected_status',
};

const _syncConnectionRemoteErrorCodes = {
  'none',
  'network_unreachable',
  'tls_error',
  'not_found',
  'unauthenticated',
  'server_endpoint_missing',
  'server_endpoint_invalid',
  'server_endpoint_userinfo_forbidden',
  'server_endpoint_query_forbidden',
  'server_endpoint_fragment_forbidden',
  'server_endpoint_scheme_unsupported',
  'remote_plain_http_forbidden',
  'configuration_invalid',
};

String _summaryString(
  Map<String, Object?> json,
  String key,
  Set<String> allowedValues,
  String fallback,
) {
  final value = json[key];
  return value is String && allowedValues.contains(value) ? value : fallback;
}

int _summaryHttpStatus(Object? value) {
  if (value is int && value >= 0 && value <= 599) {
    return value;
  }
  return 0;
}

SyncConnectionStatus _syncConnectionStatusFromProbeSummary(
  SyncConnectionProbeSummary summary,
) {
  switch (summary.connectionStatus) {
    case 'not_configured':
      return SyncConnectionStatus.notConfigured;
    case 'configuration_invalid':
      return SyncConnectionStatus.endpointInvalid;
    case 'network_unreachable':
      return SyncConnectionStatus.readOnlyProbeNetworkUnavailable;
    case 'tls_error':
      return SyncConnectionStatus.readOnlyProbeTlsError;
    case 'reachable':
      return SyncConnectionStatus.readOnlyProbeReachable;
    case 'reachable_with_unexpected_status':
      return SyncConnectionStatus.readOnlyProbeUnexpectedStatus;
    default:
      return SyncConnectionStatus.readOnlyProbeSummaryInvalid;
  }
}

String _syncConnectionBlockerFromProbeSummary(
  SyncConnectionProbeSummary summary,
) {
  switch (summary.connectionStatus) {
    case 'not_configured':
      return 'server_endpoint_missing';
    case 'configuration_invalid':
      return summary.lastRemoteErrorCode;
    case 'network_unreachable':
      return 'network_unreachable';
    case 'tls_error':
      return 'tls_error';
    case 'reachable_with_unexpected_status':
      return 'unexpected_response_status';
    case 'reachable':
      return _syncConnectionReachableBlocker(summary.authStatus);
    default:
      return 'probe_summary_invalid';
  }
}

String _syncConnectionReachableBlocker(String authStatus) {
  switch (authStatus) {
    case 'accepted':
    case 'not_required_for_local_probe':
      return 'none';
    case 'required':
      return 'access_token_required';
    case 'failed':
      return 'access_token_rejected';
    default:
      return 'auth_status_unknown';
  }
}

_SyncEndpointClassification _classifySyncEndpoint(String endpoint) {
  final uri = Uri.tryParse(endpoint.trim());
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return const _SyncEndpointClassification(
      status: 'invalid',
      transportMode: 'invalid_endpoint',
      blocker: 'server_endpoint_invalid',
    );
  }
  if (uri.userInfo.isNotEmpty) {
    return const _SyncEndpointClassification(
      status: 'invalid_userinfo',
      transportMode: 'invalid_endpoint',
      blocker: 'server_endpoint_userinfo_forbidden',
    );
  }
  if (uri.query.isNotEmpty) {
    return const _SyncEndpointClassification(
      status: 'invalid_query',
      transportMode: 'invalid_endpoint',
      blocker: 'server_endpoint_query_forbidden',
    );
  }
  if (uri.fragment.isNotEmpty) {
    return const _SyncEndpointClassification(
      status: 'invalid_fragment',
      transportMode: 'invalid_endpoint',
      blocker: 'server_endpoint_fragment_forbidden',
    );
  }
  if (uri.scheme != 'https' && uri.scheme != 'http') {
    return const _SyncEndpointClassification(
      status: 'invalid_scheme',
      transportMode: 'invalid_endpoint',
      blocker: 'server_endpoint_scheme_unsupported',
    );
  }

  final host = uri.host.toLowerCase();
  final isLocal = _isLocalSyncHost(host);
  if (isLocal && uri.scheme == 'https') {
    return const _SyncEndpointClassification(
      status: 'configured',
      transportMode: 'local_https',
      blocker: 'none',
    );
  }
  if (isLocal && uri.scheme == 'http') {
    return const _SyncEndpointClassification(
      status: 'configured',
      transportMode: 'local_http',
      blocker: 'none',
    );
  }
  if (uri.scheme == 'https') {
    return const _SyncEndpointClassification(
      status: 'configured',
      transportMode: 'external_https',
      blocker: 'none',
    );
  }
  return const _SyncEndpointClassification(
    status: 'configured',
    transportMode: 'remote_http',
    blocker: 'none',
  );
}

bool _isLocalSyncHost(String host) {
  return host == 'localhost' ||
      host == '127.0.0.1' ||
      host == '::1' ||
      host == '[::1]';
}

class _SyncEndpointClassification {
  const _SyncEndpointClassification({
    required this.status,
    required this.transportMode,
    required this.blocker,
  });

  final String status;
  final String transportMode;
  final String blocker;
}

ManagerSyncEntryGate _managerSyncEntryGateFromState({
  required SyncUiState state,
  required DeviceSecuritySummary device,
}) {
  final productionBlockers = <String>[
    if (!managerDeviceGateReady(device))
      'platform_private_key_backend_${device.productionGate}',
    managerClosedRecoveryEntryGate.blocker,
    managerClosedDeviceAuthorizationEntryGate.blocker,
    'user_sync_entry_closed_current_phase',
  ];

  switch (state) {
    case SyncUiState.localOnly:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.localOnly,
        entryBlocker: 'server_endpoint_missing',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        recovery: managerClosedRecoveryEntryGate,
        deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
        userSyncEnabled: false,
      );
    case SyncUiState.syncDisabledByPolicy:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.syncDisabledByPolicy,
        entryBlocker: 'sync_disabled_by_policy',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        recovery: managerClosedRecoveryEntryGate,
        deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
        userSyncEnabled: false,
      );
    case SyncUiState.backendUnavailable:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.backendUnavailable,
        entryBlocker: 'backend_unavailable',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        recovery: managerClosedRecoveryEntryGate,
        deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
        userSyncEnabled: false,
      );
    case SyncUiState.deploymentUnverified:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.deploymentUnverified,
        entryBlocker: 'deployment_unverified',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        recovery: managerClosedRecoveryEntryGate,
        deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
        userSyncEnabled: false,
      );
    case SyncUiState.preflightReady:
    case SyncUiState.serverConfigured:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.preflightReady,
        entryBlocker: managerClosedRecoveryEntryGate.blocker,
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        recovery: managerClosedRecoveryEntryGate,
        deviceAuthorization: managerClosedDeviceAuthorizationEntryGate,
        userSyncEnabled: false,
      );
    case SyncUiState.readyForUserSync:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.readyForUserSync,
        entryBlocker: 'none',
        localEvidenceSource: 'unknown',
        productionBlockers: const [],
        recovery: const RecoveryEntryGate(
          status: RecoveryEntryStatus.ready,
          blocker: 'none',
          canGenerateCode: true,
          canRestoreDevice: true,
          requiresSaveConfirmation: false,
        ),
        deviceAuthorization: const DeviceAuthorizationEntryGate(
          status: DeviceAuthorizationEntryStatus.ready,
          blocker: 'none',
          joinRequestStatus: JoinRequestStatus.authorized,
          canCreateJoinRequest: true,
          canApproveJoinRequest: true,
          canRevokeDevice: true,
        ),
        userSyncEnabled: true,
      );
  }
}
