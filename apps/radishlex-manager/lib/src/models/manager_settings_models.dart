class ManagerSettings {
  const ManagerSettings({
    required this.privacyMode,
    required this.diagnosticsExport,
    required this.syncConfigured,
    required this.draft,
    required this.runtimeDiagnostics,
  });

  final bool privacyMode;
  final bool diagnosticsExport;
  final bool syncConfigured;
  final ManagerSettingsDraft draft;
  final ManagerRuntimeDiagnostics runtimeDiagnostics;

  ManagerSettings copyWith({
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? syncConfigured,
    ManagerSettingsDraft? draft,
    ManagerRuntimeDiagnostics? runtimeDiagnostics,
  }) {
    return ManagerSettings(
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      syncConfigured: syncConfigured ?? this.syncConfigured,
      draft: draft ?? this.draft,
      runtimeDiagnostics: runtimeDiagnostics ?? this.runtimeDiagnostics,
    );
  }
}

class ManagerSettingsDraft {
  const ManagerSettingsDraft({
    required this.serverEndpoint,
    required this.retainSyncConfig,
    required this.privacyMode,
    required this.diagnosticsExport,
    required this.deploymentEvidenceRecorded,
    this.accessTokenConfigured = false,
    this.deploymentEvidenceSource = '',
    this.syncConnectionProbeRecord =
        const ManagerSyncConnectionProbeRecord.empty(),
  });

  const ManagerSettingsDraft.empty()
    : serverEndpoint = '',
      retainSyncConfig = false,
      privacyMode = false,
      diagnosticsExport = false,
      deploymentEvidenceRecorded = false,
      accessTokenConfigured = false,
      deploymentEvidenceSource = '',
      syncConnectionProbeRecord =
          const ManagerSyncConnectionProbeRecord.empty();

  final String serverEndpoint;
  final bool retainSyncConfig;
  final bool privacyMode;
  final bool diagnosticsExport;
  final bool deploymentEvidenceRecorded;
  final bool accessTokenConfigured;
  final String deploymentEvidenceSource;
  final ManagerSyncConnectionProbeRecord syncConnectionProbeRecord;

  bool get hasServerEndpoint =>
      retainSyncConfig && serverEndpoint.trim().isNotEmpty;

  bool get hasAccessToken => retainSyncConfig && accessTokenConfigured;

  bool get hasSyncConnectionProbeRecord =>
      retainSyncConfig && syncConnectionProbeRecord.isRecorded;

  bool get hasDeploymentEvidence =>
      deploymentEvidenceRecorded &&
      isValidManagerDeploymentEvidenceSource(deploymentEvidenceSource);

  ManagerSettingsDraft copyWith({
    String? serverEndpoint,
    bool? retainSyncConfig,
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? deploymentEvidenceRecorded,
    bool? accessTokenConfigured,
    String? deploymentEvidenceSource,
    ManagerSyncConnectionProbeRecord? syncConnectionProbeRecord,
  }) {
    return ManagerSettingsDraft(
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      retainSyncConfig: retainSyncConfig ?? this.retainSyncConfig,
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      deploymentEvidenceRecorded:
          deploymentEvidenceRecorded ?? this.deploymentEvidenceRecorded,
      accessTokenConfigured:
          accessTokenConfigured ?? this.accessTokenConfigured,
      deploymentEvidenceSource:
          deploymentEvidenceSource ?? this.deploymentEvidenceSource,
      syncConnectionProbeRecord:
          syncConnectionProbeRecord ?? this.syncConnectionProbeRecord,
    );
  }

  ManagerSettingsDraft normalized() {
    return copyWith(
      serverEndpoint: serverEndpoint.trim(),
      deploymentEvidenceSource: deploymentEvidenceRecorded
          ? deploymentEvidenceSource.trim()
          : '',
      syncConnectionProbeRecord: retainSyncConfig
          ? syncConnectionProbeRecord.normalized()
          : const ManagerSyncConnectionProbeRecord.empty(),
    );
  }
}

const managerSyncConnectionProbeSourceLocalDockerHttps = 'local_docker_https';
const managerSyncConnectionProbeSourceLocalHttp = 'local_http';
const managerSyncConnectionProbeSourceExternalHttps = 'external_https_probe';
const managerSyncConnectionProbeSourceImportedSummary = 'imported_summary';
const managerSyncConnectionProbeSourceUnknown = 'unknown_source';

const managerSyncConnectionProbeSources = [
  managerSyncConnectionProbeSourceLocalDockerHttps,
  managerSyncConnectionProbeSourceLocalHttp,
  managerSyncConnectionProbeSourceExternalHttps,
  managerSyncConnectionProbeSourceImportedSummary,
  managerSyncConnectionProbeSourceUnknown,
];

class ManagerSyncConnectionProbeRecord {
  const ManagerSyncConnectionProbeRecord({
    required this.source,
    required this.recordedAt,
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

  const ManagerSyncConnectionProbeRecord.empty()
    : source = '',
      recordedAt = '',
      format = '',
      redactionPolicy = '',
      endpointStatus = '',
      transportMode = '',
      accessTokenStatus = '',
      connectionStatus = '',
      authStatus = '',
      serverStateStatus = '',
      httpStatus = 0,
      httpStatusClass = '',
      lastRemoteErrorCode = '',
      localInsecureTls = '';

  final String source;
  final String recordedAt;
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

  bool get isRecorded => source.trim().isNotEmpty && format.trim().isNotEmpty;

  ManagerSyncConnectionProbeRecord normalized() {
    if (!isRecorded) {
      return const ManagerSyncConnectionProbeRecord.empty();
    }
    return ManagerSyncConnectionProbeRecord(
      source: source.trim(),
      recordedAt: recordedAt.trim(),
      format: format.trim(),
      redactionPolicy: redactionPolicy.trim(),
      endpointStatus: endpointStatus.trim(),
      transportMode: transportMode.trim(),
      accessTokenStatus: accessTokenStatus.trim(),
      connectionStatus: connectionStatus.trim(),
      authStatus: authStatus.trim(),
      serverStateStatus: serverStateStatus.trim(),
      httpStatus: httpStatus < 0 || httpStatus > 599 ? 0 : httpStatus,
      httpStatusClass: httpStatusClass.trim(),
      lastRemoteErrorCode: lastRemoteErrorCode.trim(),
      localInsecureTls: localInsecureTls.trim(),
    );
  }

  @override
  bool operator ==(Object other) {
    return other is ManagerSyncConnectionProbeRecord &&
        source == other.source &&
        recordedAt == other.recordedAt &&
        format == other.format &&
        redactionPolicy == other.redactionPolicy &&
        endpointStatus == other.endpointStatus &&
        transportMode == other.transportMode &&
        accessTokenStatus == other.accessTokenStatus &&
        connectionStatus == other.connectionStatus &&
        authStatus == other.authStatus &&
        serverStateStatus == other.serverStateStatus &&
        httpStatus == other.httpStatus &&
        httpStatusClass == other.httpStatusClass &&
        lastRemoteErrorCode == other.lastRemoteErrorCode &&
        localInsecureTls == other.localInsecureTls;
  }

  @override
  int get hashCode => Object.hash(
    source,
    recordedAt,
    format,
    redactionPolicy,
    endpointStatus,
    transportMode,
    accessTokenStatus,
    connectionStatus,
    authStatus,
    serverStateStatus,
    httpStatus,
    httpStatusClass,
    lastRemoteErrorCode,
    localInsecureTls,
  );
}

const managerDeploymentEvidenceLocalSmoke = 'local_smoke';
const managerDeploymentEvidenceExternalTls = 'external_tls';
const managerDeploymentEvidenceBackupRestore = 'backup_restore';
const managerDeploymentEvidenceUpgradeRollback = 'upgrade_rollback';

const managerDeploymentEvidenceSources = [
  managerDeploymentEvidenceLocalSmoke,
  managerDeploymentEvidenceExternalTls,
  managerDeploymentEvidenceBackupRestore,
  managerDeploymentEvidenceUpgradeRollback,
];

bool isValidManagerDeploymentEvidenceSource(String source) {
  return managerDeploymentEvidenceSources.contains(source.trim());
}

String managerDeploymentEvidenceSourceLabel(String source) {
  switch (source.trim()) {
    case managerDeploymentEvidenceLocalSmoke:
      return 'local smoke';
    case managerDeploymentEvidenceExternalTls:
      return 'external TLS';
    case managerDeploymentEvidenceBackupRestore:
      return 'backup restore';
    case managerDeploymentEvidenceUpgradeRollback:
      return 'upgrade rollback';
  }
  return 'not recorded';
}

class ManagerRuntimeDiagnostics {
  const ManagerRuntimeDiagnostics({
    required this.bridgeMode,
    required this.userDb,
    required this.nativeLibrary,
    required this.settingsStore,
    required this.syncEndpoint,
    required this.lastErrorCode,
  });

  final String bridgeMode;
  final String userDb;
  final String nativeLibrary;
  final String settingsStore;
  final String syncEndpoint;
  final String lastErrorCode;
}
