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
  });

  const ManagerSettingsDraft.empty()
    : serverEndpoint = '',
      retainSyncConfig = false,
      privacyMode = false,
      diagnosticsExport = false,
      deploymentEvidenceRecorded = false;

  final String serverEndpoint;
  final bool retainSyncConfig;
  final bool privacyMode;
  final bool diagnosticsExport;
  final bool deploymentEvidenceRecorded;

  bool get hasServerEndpoint =>
      retainSyncConfig && serverEndpoint.trim().isNotEmpty;

  ManagerSettingsDraft copyWith({
    String? serverEndpoint,
    bool? retainSyncConfig,
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? deploymentEvidenceRecorded,
  }) {
    return ManagerSettingsDraft(
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      retainSyncConfig: retainSyncConfig ?? this.retainSyncConfig,
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      deploymentEvidenceRecorded:
          deploymentEvidenceRecorded ?? this.deploymentEvidenceRecorded,
    );
  }

  ManagerSettingsDraft normalized() {
    return copyWith(serverEndpoint: serverEndpoint.trim());
  }
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
