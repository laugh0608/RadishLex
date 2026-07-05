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

class ManagerSyncEntryGate {
  const ManagerSyncEntryGate({
    required this.entryState,
    required this.entryBlocker,
    required this.localEvidenceSource,
    required this.productionBlockers,
    required this.userSyncEnabled,
  });

  final SyncEntryState entryState;
  final String entryBlocker;
  final String localEvidenceSource;
  final List<String> productionBlockers;
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

ManagerSyncEntryGate deriveManagerSyncEntryGate({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  final normalized = draft.normalized();
  final localEvidenceSource = managerSyncLocalEvidenceSource(normalized);
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
      userSyncEnabled: false,
    );
  }
  if (!normalized.hasServerEndpoint) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.localOnly,
      entryBlocker: 'server_endpoint_missing',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      userSyncEnabled: false,
    );
  }
  if (!managerDeviceGateReady(device)) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.backendUnavailable,
      entryBlocker: 'backend_unavailable',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
      userSyncEnabled: false,
    );
  }
  if (!normalized.hasDeploymentEvidence) {
    return ManagerSyncEntryGate(
      entryState: SyncEntryState.deploymentUnverified,
      entryBlocker: 'deployment_unverified',
      localEvidenceSource: localEvidenceSource,
      productionBlockers: productionBlockers,
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
      userSyncEnabled: false,
    );
  }

  return ManagerSyncEntryGate(
    entryState: SyncEntryState.blockedBeforeUserSync,
    entryBlocker: 'blocked_before_user_sync',
    localEvidenceSource: localEvidenceSource,
    productionBlockers: productionBlockers,
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
  final blockers = <String>[];
  if (normalized.privacyMode) {
    blockers.add('sync_disabled_by_policy');
  }
  if (!normalized.hasServerEndpoint) {
    blockers.add('server_endpoint_missing');
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
    ..add('recovery_code_flow_closed')
    ..add('device_authorization_flow_closed')
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

ManagerSyncEntryGate _managerSyncEntryGateFromState({
  required SyncUiState state,
  required DeviceSecuritySummary device,
}) {
  final productionBlockers = <String>[
    if (!managerDeviceGateReady(device))
      'platform_private_key_backend_${device.productionGate}',
    'recovery_code_flow_closed',
    'device_authorization_flow_closed',
    'user_sync_entry_closed_current_phase',
  ];

  switch (state) {
    case SyncUiState.localOnly:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.localOnly,
        entryBlocker: 'server_endpoint_missing',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        userSyncEnabled: false,
      );
    case SyncUiState.syncDisabledByPolicy:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.syncDisabledByPolicy,
        entryBlocker: 'sync_disabled_by_policy',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        userSyncEnabled: false,
      );
    case SyncUiState.backendUnavailable:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.backendUnavailable,
        entryBlocker: 'backend_unavailable',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        userSyncEnabled: false,
      );
    case SyncUiState.deploymentUnverified:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.deploymentUnverified,
        entryBlocker: 'deployment_unverified',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        userSyncEnabled: false,
      );
    case SyncUiState.preflightReady:
    case SyncUiState.serverConfigured:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.preflightReady,
        entryBlocker: 'blocked_before_user_sync',
        localEvidenceSource: 'unknown',
        productionBlockers: productionBlockers,
        userSyncEnabled: false,
      );
    case SyncUiState.readyForUserSync:
      return ManagerSyncEntryGate(
        entryState: SyncEntryState.readyForUserSync,
        entryBlocker: 'none',
        localEvidenceSource: 'unknown',
        productionBlockers: const [],
        userSyncEnabled: true,
      );
  }
}
