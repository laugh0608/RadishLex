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
    this.deploymentEvidenceSource = '',
  });

  const ManagerSettingsDraft.empty()
    : serverEndpoint = '',
      retainSyncConfig = false,
      privacyMode = false,
      diagnosticsExport = false,
      deploymentEvidenceRecorded = false,
      deploymentEvidenceSource = '';

  final String serverEndpoint;
  final bool retainSyncConfig;
  final bool privacyMode;
  final bool diagnosticsExport;
  final bool deploymentEvidenceRecorded;
  final String deploymentEvidenceSource;

  bool get hasServerEndpoint =>
      retainSyncConfig && serverEndpoint.trim().isNotEmpty;

  bool get hasDeploymentEvidence =>
      deploymentEvidenceRecorded &&
      isValidManagerDeploymentEvidenceSource(deploymentEvidenceSource);

  ManagerSettingsDraft copyWith({
    String? serverEndpoint,
    bool? retainSyncConfig,
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? deploymentEvidenceRecorded,
    String? deploymentEvidenceSource,
  }) {
    return ManagerSettingsDraft(
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      retainSyncConfig: retainSyncConfig ?? this.retainSyncConfig,
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      deploymentEvidenceRecorded:
          deploymentEvidenceRecorded ?? this.deploymentEvidenceRecorded,
      deploymentEvidenceSource:
          deploymentEvidenceSource ?? this.deploymentEvidenceSource,
    );
  }

  ManagerSettingsDraft normalized() {
    return copyWith(
      serverEndpoint: serverEndpoint.trim(),
      deploymentEvidenceSource: deploymentEvidenceRecorded
          ? deploymentEvidenceSource.trim()
          : '',
    );
  }
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
