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

class ManagerSyncGateAudit {
  const ManagerSyncGateAudit({
    required this.state,
    required this.stateLabel,
    required this.stateSource,
    required this.actionStopLine,
    required this.deviceGateLabel,
    required this.deviceGateReady,
  });

  final SyncUiState state;
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
  if (draft.privacyMode) {
    return SyncUiState.syncDisabledByPolicy;
  }
  if (!draft.hasServerEndpoint) {
    return SyncUiState.localOnly;
  }
  if (device.productionGate != 'ready') {
    return SyncUiState.backendUnavailable;
  }
  if (!draft.hasDeploymentEvidence) {
    return SyncUiState.deploymentUnverified;
  }
  return SyncUiState.preflightReady;
}

ManagerSyncGateAudit managerSyncGateAudit({
  required SyncUiState state,
  required DeviceSecuritySummary device,
}) {
  return ManagerSyncGateAudit(
    state: state,
    stateLabel: managerSyncStateLabel(state),
    stateSource: managerSyncStateSourceDescription(
      state: state,
      device: device,
    ),
    actionStopLine: managerSyncActionStopLine(state),
    deviceGateLabel: managerDeviceGateLabel(device),
    deviceGateReady: managerDeviceGateReady(device),
  );
}

ManagerSyncGateAudit managerSyncGateAuditForDraft({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  return managerSyncGateAudit(
    state: deriveManagerSyncUiState(draft: draft, device: device),
    device: device,
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

String managerSyncActionStopLine(SyncUiState state) {
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

String managerSyncGateReason({
  required SyncUiState state,
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  switch (state) {
    case SyncUiState.localOnly:
      return '未保留自部署服务端草案；真实远端同步保持关闭';
    case SyncUiState.syncDisabledByPolicy:
      return '隐私模式已启用；真实远端同步保持关闭';
    case SyncUiState.backendUnavailable:
      return '平台私钥 backend 未解除生产门禁：${device.productionGate}';
    case SyncUiState.deploymentUnverified:
      return '目标部署运行证据未记录；真实远端同步保持关闭';
    case SyncUiState.preflightReady:
      return '本地预检通过；用户可用同步入口仍等待后续阶段开放';
    case SyncUiState.serverConfigured:
      return '已保留服务端草案；仍未进入生产可用同步';
    case SyncUiState.readyForUserSync:
      return '用户可用同步入口尚未在当前阶段开放';
  }
}

String managerSyncEndpointLabel(ManagerSettingsDraft draft) {
  return draft.hasServerEndpoint ? draft.serverEndpoint.trim() : '未配置';
}
