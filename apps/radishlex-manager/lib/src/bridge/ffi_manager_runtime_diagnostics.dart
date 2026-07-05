import '../models/manager_models.dart';

ManagerSettingsDraft fallbackFfiManagerSettingsDraft(String? serverEndpoint) {
  final endpoint = serverEndpoint?.trim() ?? '';
  return ManagerSettingsDraft(
    serverEndpoint: endpoint,
    retainSyncConfig: endpoint.isNotEmpty,
    privacyMode: false,
    diagnosticsExport: false,
    deploymentEvidenceRecorded: false,
    accessTokenConfigured: false,
  );
}

ManagerSettings managerSettingsFromFfiDraft({
  required ManagerSettingsDraft draft,
  required ManagerRuntimeDiagnostics runtimeDiagnostics,
}) {
  return ManagerSettings(
    privacyMode: draft.privacyMode,
    diagnosticsExport: draft.diagnosticsExport,
    syncConfigured: draft.retainSyncConfig,
    draft: draft,
    runtimeDiagnostics: runtimeDiagnostics,
  );
}

ManagerRuntimeDiagnostics managerRuntimeDiagnosticsFromFfi({
  required bool nativeInjected,
  required String libraryPath,
  required String settingsStoreSourceLabel,
  required ManagerSettingsDraft draft,
}) {
  return ManagerRuntimeDiagnostics(
    bridgeMode: nativeInjected ? 'ffi_injected' : 'dart_ffi',
    userDb: 'RADISHLEX_MANAGER_DB configured',
    nativeLibrary: nativeInjected
        ? 'injected native binding'
        : libraryPath.isEmpty
        ? 'default dynamic library lookup'
        : 'RADISHLEX_MANAGER_FFI_LIBRARY configured',
    settingsStore: settingsStoreSourceLabel,
    syncEndpoint: draft.hasServerEndpoint
        ? 'sync endpoint draft configured'
        : 'sync endpoint draft not configured',
    lastErrorCode: 'none',
  );
}
