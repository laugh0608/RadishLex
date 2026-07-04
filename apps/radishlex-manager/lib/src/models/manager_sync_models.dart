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
  if (!draft.deploymentEvidenceRecorded) {
    return SyncUiState.deploymentUnverified;
  }
  return SyncUiState.preflightReady;
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
