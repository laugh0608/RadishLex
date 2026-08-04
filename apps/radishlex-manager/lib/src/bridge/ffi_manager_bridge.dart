import '../models/manager_models.dart';
import 'ffi_dynamic_native_binding.dart';
import 'ffi_manager_dictionary_mapper.dart';
import 'ffi_manager_learning_mapper.dart';
import 'ffi_manager_native_models.dart';
import 'ffi_manager_runtime_diagnostics.dart';
import 'ffi_manager_snapshot_mapper.dart';
import 'ffi_manager_sync_qualification_mapper.dart';
import 'manager_bridge.dart';
import 'manager_diagnostics_export.dart';
import 'manager_platform_control.dart';
import 'manager_settings_store.dart';

export 'ffi_manager_native_models.dart';

class FfiManagerBridge implements ManagerBridge {
  FfiManagerBridge({
    required String dbPath,
    String? libraryPath,
    String? serverEndpoint,
    String? settingsFilePath,
    RadishLexManagerNativeBinding? native,
    ManagerSettingsStore? settingsStore,
    this.platformControl,
  }) : dbPath = dbPath.trim(),
       libraryPath = libraryPath?.trim() ?? '',
       _settingsStore =
           settingsStore ??
           ManagerSettingsStore(
             filePath: settingsFilePath,
             fallbackDraft: fallbackFfiManagerSettingsDraft(serverEndpoint),
           ),
       _nativeInjected = native != null,
       _native =
           native ??
           DynamicRadishLexManagerNativeBinding.open(libraryPath: libraryPath) {
    if (this.dbPath.isEmpty) {
      throw ArgumentError.value(dbPath, 'dbPath', 'dbPath cannot be empty');
    }
  }

  final String dbPath;
  final String libraryPath;
  final bool _nativeInjected;
  final RadishLexManagerNativeBinding _native;
  final ManagerSettingsStore _settingsStore;
  final ManagerPlatformControl? platformControl;

  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    return _loadSnapshot();
  }

  @override
  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term) async {
    _native.deleteUserTerm(
      dbPath: dbPath,
      inputCode: term.inputCode,
      text: term.text,
      reading: term.reading.isEmpty ? null : term.reading,
    );
    return _loadSnapshot();
  }

  @override
  Future<ManagerSnapshot> restoreUserTerm(UserTermKey term) async {
    _native.restoreUserTerm(
      dbPath: dbPath,
      inputCode: term.inputCode,
      text: term.text,
      reading: term.reading.isEmpty ? null : term.reading,
    );
    return _loadSnapshot();
  }

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(
    String filePath,
  ) async {
    return managerDictionaryImportPreviewFromNative(
      filePath: filePath,
      summary: _native.inspectDictionaryImport(filePath),
    );
  }

  @override
  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  }) async {
    final summary = _native.importDictionaryFile(
      dbPath: dbPath,
      filePath: filePath,
      sourceName: sourceName.trim().isEmpty ? null : sourceName.trim(),
      dryRun: dryRun,
    );
    return managerDictionaryImportResultFromNative(
      filePath: filePath,
      sourceName: sourceName,
      summary: summary,
    );
  }

  @override
  Future<DictionaryExportResult> exportDictionaryFile(String filePath) async {
    return managerDictionaryExportResultFromNative(
      filePath: filePath,
      summary: _native.exportDictionaryFile(dbPath: dbPath, filePath: filePath),
    );
  }

  @override
  Future<ManagerDiagnosticsReport> loadDiagnosticsReport() async {
    return createManagerDiagnosticsReport(await _loadSnapshot());
  }

  @override
  Future<ManagerDiagnosticsExportResult> exportDiagnosticsReport(
    String filePath,
  ) async {
    return writeManagerDiagnosticsReport(
      filePath: filePath,
      report: await loadDiagnosticsReport(),
    );
  }

  @override
  Future<ManagerSnapshot> saveSettingsDraft(ManagerSettingsDraft draft) async {
    final platform = platformControl;
    if (platform == null) {
      _settingsStore.save(draft);
      return _loadSnapshot();
    }

    final previousPrivacyState = await platform.readPrivacyModeState();
    final privacyChanged = previousPrivacyState.enabled != draft.privacyMode;
    try {
      if (privacyChanged) {
        await platform.writePrivacyMode(draft.privacyMode);
      }
      _settingsStore.save(draft);
      await platform.secureLocalFiles();
    } on Object {
      if (privacyChanged) {
        try {
          await platform.restorePrivacyModeState(previousPrivacyState);
          _settingsStore.save(
            draft.copyWith(privacyMode: previousPrivacyState.enabled),
          );
        } on Object {
          throw const ManagerPlatformException(
            code: 'privacy_rollback_failed',
            message: 'privacy mode could not be restored after save failure',
          );
        }
      }
      rethrow;
    }
    return _loadSnapshot();
  }

  @override
  ManagerSyncQualificationRun startSyncQualification(
    ManagerSyncQualificationRequest request,
  ) {
    final nativeRun = _native.startSyncQualification(
      endpoint: request.endpoint,
      accessToken: request.accessToken,
      localCaDer: request.localCaDer,
      timeoutMs: request.timeoutMs,
    );
    return _FfiManagerSyncQualificationRun(nativeRun);
  }

  Future<ManagerSnapshot> _loadSnapshot() async {
    var settingsDraft = _settingsStore.load();
    final platform = platformControl;
    if (platform != null) {
      settingsDraft = settingsDraft.copyWith(
        privacyMode: (await platform.readPrivacyModeState()).enabled,
      );
    }
    return managerSnapshotFromNative(
      generatedAtMs: DateTime.now().millisecondsSinceEpoch,
      nativeTerms: _native.listUserTerms(dbPath),
      nativeDeletedTerms: _native.listDeletedTerms(dbPath),
      nativeImportBatches: _native.listImportBatches(dbPath),
      nativeLearning: _native.learningStatus(dbPath),
      nativeSync: _native.syncPreflight(dbPath),
      nativeSyncProductStatus: _native.syncProductStatus(),
      settingsDraft: settingsDraft,
      runtimeDiagnostics: managerRuntimeDiagnosticsFromFfi(
        nativeInjected: _nativeInjected,
        libraryPath: libraryPath,
        settingsStoreSourceLabel: _settingsStore.sourceLabel,
        draft: settingsDraft,
      ),
      explainTerm: _rankExplainTerm,
    );
  }

  NativeRankExplainSummary _rankExplainTerm(UserTerm term, String contextKind) {
    return _native.rankExplain(
      dbPath: dbPath,
      inputCode: term.inputCode,
      candidateText: term.text,
      reading: managerRankExplainReading(term),
      contextKind: contextKind,
    );
  }
}

final class _FfiManagerSyncQualificationRun
    implements ManagerSyncQualificationRun {
  _FfiManagerSyncQualificationRun(this._native);

  final NativeSyncQualificationRun _native;
  ManagerSyncQualificationState? _lastState;
  bool _disposed = false;

  @override
  ManagerSyncQualificationSnapshot poll() {
    _ensureOpen();
    final snapshot = managerSyncQualificationFromNative(_native.poll());
    if (!_validTransition(_lastState, snapshot.state)) {
      throw const FfiManagerBridgeException(
        statusCode: 2,
        code: 'sync_qualification_transition_invalid',
        message: 'native sync qualification state transition is invalid',
      );
    }
    _lastState = snapshot.state;
    return snapshot;
  }

  @override
  bool cancel() {
    _ensureOpen();
    return _native.cancel();
  }

  @override
  void dispose() {
    if (_disposed) {
      return;
    }
    _native.dispose();
    _disposed = true;
  }

  void _ensureOpen() {
    if (_disposed) {
      throw const FfiManagerBridgeException(
        statusCode: 2,
        code: 'invalid_state',
        message: 'sync qualification run is already disposed',
      );
    }
  }
}

bool _validTransition(
  ManagerSyncQualificationState? previous,
  ManagerSyncQualificationState next,
) {
  if (previous == null || previous == next) {
    return true;
  }
  return switch (previous) {
    ManagerSyncQualificationState.created =>
      next == ManagerSyncQualificationState.running ||
          next == ManagerSyncQualificationState.cancelling ||
          next.isTerminal,
    ManagerSyncQualificationState.running =>
      next == ManagerSyncQualificationState.cancelling || next.isTerminal,
    ManagerSyncQualificationState.cancelling => next.isTerminal,
    ManagerSyncQualificationState.completed ||
    ManagerSyncQualificationState.failed ||
    ManagerSyncQualificationState.cancelled => false,
  };
}
