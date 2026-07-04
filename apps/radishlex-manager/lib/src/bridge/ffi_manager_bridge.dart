import '../models/manager_models.dart';
import 'ffi_dynamic_native_binding.dart';
import 'ffi_manager_native_models.dart';
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
             fallbackDraft: _fallbackSettingsDraft(serverEndpoint),
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
    final summary = _native.inspectDictionaryImport(filePath);
    return DictionaryImportPreview(
      filePath: filePath,
      format: _dictionaryFormatLabel(summary.formatVersion),
      recordCount: summary.recordCount,
      syncClass: _syncClassLabel(summary.syncClass),
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
    return DictionaryImportResult(
      filePath: filePath,
      sourceName: sourceName,
      totalRecords: summary.totalRecords,
      importedTerms: summary.importedTerms,
      insertedTerms: summary.insertedTerms,
      updatedTerms: summary.updatedTerms,
      skippedDeletedTerms: summary.skippedDeletedTerms,
      skippedDuplicateTerms: summary.skippedDuplicateTerms,
      dryRun: summary.dryRun,
    );
  }

  @override
  Future<DictionaryExportResult> exportDictionaryFile(String filePath) async {
    final summary = _native.exportDictionaryFile(
      dbPath: dbPath,
      filePath: filePath,
    );
    return DictionaryExportResult(
      filePath: filePath,
      exportedTerms: summary.exportedTerms,
      format: _dictionaryFormatLabel(summary.formatVersion),
      syncClass: _syncClassLabel(summary.syncClass),
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
    final nativeTerms = _native.listUserTerms(dbPath);
    final learning = _native.learningStatus(dbPath);
    final importBatches = _native.listImportBatches(dbPath);
    final sync = _native.syncPreflight(dbPath);
    final terms = nativeTerms.map(_userTermFromNative).toList(growable: false);

    return ManagerSnapshot(
      generatedAt: _formatTimestampMs(DateTime.now().millisecondsSinceEpoch),
      dictionaryTerms: terms,
      deletedTerms: const [],
      importBatches: importBatches
          .map(_importBatchFromNative)
          .toList(growable: false),
      learningSummary: _learningSummaryFromNative(learning),
      explanations: _rankerExplanationSummaries(terms),
      sync: _syncSummaryFromNative(sync, settingsDraft),
      settings: ManagerSettings(
        privacyMode: settingsDraft.privacyMode,
        diagnosticsExport: settingsDraft.diagnosticsExport,
        syncConfigured: settingsDraft.retainSyncConfig,
        draft: settingsDraft,
        runtimeDiagnostics: _runtimeDiagnostics(settingsDraft),
      ),
    );
  }

  UserTerm _userTermFromNative(NativeUserTermRecord term) {
    return UserTerm(
      inputCode: term.inputCode,
      text: term.text,
      reading: term.reading ?? '',
      weight: term.weight,
      source: _termSourceLabel(term.source),
      lastUsed: term.lastUsedAtPresent
          ? _formatTimestampMs(term.lastUsedAtMs)
          : '未使用',
    );
  }

  DictionaryImportBatchSummary _importBatchFromNative(
    NativeImportBatchRecord batch,
  ) {
    return DictionaryImportBatchSummary(
      id: batch.id,
      sourceName: batch.sourceName,
      totalRecords: batch.totalRecords,
      importedTerms: batch.importedTerms,
      insertedTerms: batch.insertedTerms,
      updatedTerms: batch.updatedTerms,
      skippedDeletedTerms: batch.skippedDeletedTerms,
      skippedDuplicateTerms: batch.skippedDuplicateTerms,
      createdAt: _formatTimestampMs(batch.createdAtMs),
      notes: batch.notes ?? '',
    );
  }

  LearningSummary _learningSummaryFromNative(
    NativeLearningStatusSummary summary,
  ) {
    return LearningSummary(
      userTerms: summary.activeUserTerms,
      deletedTerms: summary.deletedTermTombstones,
      selectionEvents: summary.selectionEvents,
      suppressedTerms: summary.suppressedUserTerms,
      lastUpdated: summary.latestActivityAtPresent
          ? _formatTimestampMs(summary.latestActivityAtMs)
          : '无记录',
    );
  }

  SyncPreflightSummary _syncSummaryFromNative(
    NativeSyncPreflightSummary summary,
    ManagerSettingsDraft settingsDraft,
  ) {
    final syncableObjects =
        summary.syncableUserTerms +
        summary.syncableRankerWeights +
        summary.syncableDeletedTerms;
    final localOnlyEvents =
        summary.localSelectionEvents +
        summary.localNegativeFeedback +
        summary.localImportBatches;
    const device = DeviceSecuritySummary(
      deviceId: 'local-manager',
      backendId: 'unavailable',
      capabilityStatus: 'platform_private_key_backend_unavailable',
      productionGate: 'blocked',
    );
    final state = deriveManagerSyncUiState(
      draft: settingsDraft,
      device: device,
    );

    return SyncPreflightSummary(
      state: state,
      serverEndpoint: managerSyncEndpointLabel(settingsDraft),
      reason: managerSyncGateReason(
        state: state,
        draft: settingsDraft,
        device: device,
      ),
      syncableObjects: syncableObjects,
      localOnlyEvents: localOnlyEvents,
      lastUpload: '未启用',
      lastDownload: '未启用',
      categories: [
        SyncCategorySummary(
          name: 'dictionary.user_terms',
          count: summary.syncableUserTerms,
        ),
        SyncCategorySummary(
          name: 'dictionary.deleted_terms',
          count: summary.syncableDeletedTerms,
        ),
        SyncCategorySummary(
          name: 'ranker.weights',
          count: summary.syncableRankerWeights,
        ),
        SyncCategorySummary(
          name: 'learning.selection_events',
          count: summary.localSelectionEvents,
        ),
        SyncCategorySummary(
          name: 'learning.negative_feedback',
          count: summary.localNegativeFeedback,
        ),
        SyncCategorySummary(
          name: 'dictionary.import_batches',
          count: summary.localImportBatches,
        ),
      ],
      device: device,
    );
  }

  List<RankerExplanation> _rankerExplanationSummaries(List<UserTerm> terms) {
    return terms
        .take(6)
        .map((term) {
          final explanation = _native.rankExplain(
            dbPath: dbPath,
            inputCode: term.inputCode,
            candidateText: term.text,
            reading: term.reading.trim().isEmpty ? null : term.reading,
            contextKind: 'general',
          );
          return RankerExplanation(
            inputCode: explanation.inputCode,
            candidate: explanation.candidateText,
            score: explanation.finalScore,
            signals: _rankExplainSignals(explanation),
          );
        })
        .toList(growable: false);
  }

  ManagerRuntimeDiagnostics _runtimeDiagnostics(ManagerSettingsDraft draft) {
    return ManagerRuntimeDiagnostics(
      bridgeMode: _nativeInjected ? 'ffi_injected' : 'dart_ffi',
      userDb: 'RADISHLEX_MANAGER_DB configured',
      nativeLibrary: _nativeInjected
          ? 'injected native binding'
          : libraryPath.isEmpty
          ? 'default dynamic library lookup'
          : 'RADISHLEX_MANAGER_FFI_LIBRARY configured',
      settingsStore: _settingsStore.sourceLabel,
      syncEndpoint: draft.hasServerEndpoint
          ? 'sync endpoint draft configured'
          : 'sync endpoint draft not configured',
      lastErrorCode: 'none',
    );
  }
}

ManagerSettingsDraft _fallbackSettingsDraft(String? serverEndpoint) {
  final endpoint = serverEndpoint?.trim() ?? '';
  return ManagerSettingsDraft(
    serverEndpoint: endpoint,
    retainSyncConfig: endpoint.isNotEmpty,
    privacyMode: false,
    diagnosticsExport: false,
    deploymentEvidenceRecorded: false,
  );
}

List<String> _rankExplainSignals(NativeRankExplainSummary explanation) {
  return [
    _rankSignal('engine', explanation.engineOrderFactor),
    _rankSignal('user', explanation.userTermBoost),
    _rankSignal('freq', explanation.frequencyBoost),
    _rankSignal('recent', explanation.recencyBoost),
    _rankSignal('context', explanation.contextBoost),
    _rankSignal('negative', explanation.negativeFeedbackPenalty),
    _rankSignal('suppressed', explanation.suppressedPenalty),
    _rankSignal('deleted', explanation.deletedPenalty),
  ];
}

String _rankSignal(String name, double value) {
  return '$name=${value.toStringAsFixed(3)}';
}

String _termSourceLabel(int source) {
  switch (source) {
    case nativeTermSourceEngineSelection:
      return 'selection';
    case nativeTermSourceManualImport:
      return 'import';
    case nativeTermSourceManualAdd:
      return 'manual';
    case nativeTermSourcePhraseLearning:
      return 'phrase_learning';
    default:
      return 'unknown($source)';
  }
}

String _dictionaryFormatLabel(int formatVersion) {
  return formatVersion == nativeDictionaryFormatUserTermsV1
      ? 'dictionary.user_terms.v1'
      : 'unknown($formatVersion)';
}

String _syncClassLabel(int syncClass) {
  return syncClass == nativeSyncClassP2EncryptedSync
      ? 'P2 encrypted sync'
      : 'unknown($syncClass)';
}

String _formatTimestampMs(int value) {
  final timestamp = DateTime.fromMillisecondsSinceEpoch(value);
  return '${timestamp.year.toString().padLeft(4, '0')}-'
      '${timestamp.month.toString().padLeft(2, '0')}-'
      '${timestamp.day.toString().padLeft(2, '0')} '
      '${timestamp.hour.toString().padLeft(2, '0')}:'
      '${timestamp.minute.toString().padLeft(2, '0')}';
}
