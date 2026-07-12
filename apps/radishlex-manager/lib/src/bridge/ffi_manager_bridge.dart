import '../models/manager_models.dart';
import 'ffi_dynamic_native_binding.dart';
import 'ffi_manager_dictionary_mapper.dart';
import 'ffi_manager_learning_mapper.dart';
import 'ffi_manager_native_models.dart';
import 'ffi_manager_runtime_diagnostics.dart';
import 'ffi_manager_snapshot_mapper.dart';
import 'manager_bridge.dart';
import 'manager_diagnostics_export.dart';
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
    return createManagerDiagnosticsReport(_loadSnapshot());
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
    _settingsStore.save(draft);
    return _loadSnapshot();
  }

  ManagerSnapshot _loadSnapshot() {
    final settingsDraft = _settingsStore.load();
    return managerSnapshotFromNative(
      generatedAtMs: DateTime.now().millisecondsSinceEpoch,
      nativeTerms: _native.listUserTerms(dbPath),
      nativeImportBatches: _native.listImportBatches(dbPath),
      nativeLearning: _native.learningStatus(dbPath),
      nativeSync: _native.syncPreflight(dbPath),
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

  NativeRankExplainSummary _rankExplainTerm(UserTerm term) {
    return _native.rankExplain(
      dbPath: dbPath,
      inputCode: term.inputCode,
      candidateText: term.text,
      reading: managerRankExplainReading(term),
      contextKind: 'general',
    );
  }
}
