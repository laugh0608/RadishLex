import '../models/manager_models.dart';
import 'ffi_dynamic_native_binding.dart';
import 'ffi_manager_native_models.dart';
import 'manager_bridge.dart';

export 'ffi_manager_native_models.dart';

class FfiManagerBridge implements ManagerBridge {
  FfiManagerBridge({
    required String dbPath,
    String? libraryPath,
    String? serverEndpoint,
    RadishLexManagerNativeBinding? native,
  }) : dbPath = dbPath.trim(),
       serverEndpoint = serverEndpoint?.trim() ?? '',
       _native =
           native ??
           DynamicRadishLexManagerNativeBinding.open(libraryPath: libraryPath) {
    if (this.dbPath.isEmpty) {
      throw ArgumentError.value(dbPath, 'dbPath', 'dbPath cannot be empty');
    }
  }

  final String dbPath;
  final String serverEndpoint;
  final RadishLexManagerNativeBinding _native;

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

  ManagerSnapshot _loadSnapshot() {
    final nativeTerms = _native.listUserTerms(dbPath);
    final learning = _native.learningStatus(dbPath);
    final sync = _native.syncPreflight(dbPath);
    final terms = nativeTerms.map(_userTermFromNative).toList(growable: false);

    return ManagerSnapshot(
      generatedAt: _formatTimestampMs(DateTime.now().millisecondsSinceEpoch),
      dictionaryTerms: terms,
      deletedTerms: const [],
      learningSummary: _learningSummaryFromNative(learning),
      explanations: _rankerExplanationSummaries(terms, learning),
      sync: _syncSummaryFromNative(sync),
      settings: ManagerSettings(
        privacyMode: false,
        diagnosticsExport: false,
        syncConfigured: serverEndpoint.isNotEmpty,
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
  ) {
    final syncableObjects =
        summary.syncableUserTerms +
        summary.syncableRankerWeights +
        summary.syncableDeletedTerms;
    final localOnlyEvents =
        summary.localSelectionEvents +
        summary.localNegativeFeedback +
        summary.localImportBatches;
    final state = serverEndpoint.isEmpty
        ? SyncUiState.localOnly
        : SyncUiState.backendUnavailable;

    return SyncPreflightSummary(
      state: state,
      serverEndpoint: serverEndpoint.isEmpty ? '未配置' : serverEndpoint,
      reason: state == SyncUiState.localOnly
          ? '未配置自部署服务端；真实远端同步保持关闭'
          : '平台私钥 backend 与目标部署证据未解除生产门禁',
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
      device: const DeviceSecuritySummary(
        deviceId: 'local-manager',
        backendId: 'unavailable',
        capabilityStatus: 'platform_private_key_backend_unavailable',
        productionGate: 'blocked',
      ),
    );
  }

  List<RankerExplanation> _rankerExplanationSummaries(
    List<UserTerm> terms,
    NativeLearningStatusSummary summary,
  ) {
    return terms
        .take(6)
        .map((term) {
          final signals = <String>[
            'ffi_userdb_weight',
            'source:${term.source}',
            if (summary.rankerWeights > 0) 'ranker_weight_summary',
            if (summary.selectionEvents > 0) 'selection_summary',
            if (summary.negativeFeedback > 0) 'negative_feedback_summary',
            if (summary.deletedTermTombstones > 0) 'deleted_tombstone_gate',
          ];
          return RankerExplanation(
            inputCode: term.inputCode,
            candidate: term.text,
            score: term.weight,
            signals: signals,
          );
        })
        .toList(growable: false);
  }
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
