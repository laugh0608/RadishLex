import '../data/manager_fixture.dart';
import '../models/manager_models.dart';
import 'manager_bridge.dart';
import 'manager_diagnostics_export.dart';

class FixtureManagerBridge implements ManagerBridge {
  FixtureManagerBridge({ManagerSnapshot? initialSnapshot})
    : _snapshot = initialSnapshot ?? createManagerFixture();

  FixtureManagerBridge.withDiagnostics({
    required ManagerRuntimeDiagnostics diagnostics,
    ManagerSnapshot? initialSnapshot,
  }) : _snapshot = _snapshotWithDiagnostics(initialSnapshot, diagnostics);

  ManagerSnapshot _snapshot;

  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    return _snapshot;
  }

  @override
  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term) async {
    final deletedAt = _snapshot.generatedAt;
    UserTerm? deletedTerm;
    final remainingTerms = <UserTerm>[];
    for (final candidate in _snapshot.dictionaryTerms) {
      if (candidate.key == term && deletedTerm == null) {
        deletedTerm = candidate;
      } else {
        remainingTerms.add(candidate);
      }
    }
    if (deletedTerm == null) {
      return _snapshot;
    }

    final deletedTerms = [
      ..._snapshot.deletedTerms,
      DeletedTerm(
        inputCode: deletedTerm.inputCode,
        text: deletedTerm.text,
        reading: deletedTerm.reading,
        deletedAt: deletedAt,
      ),
    ];

    _snapshot = _snapshot.copyWith(
      dictionaryTerms: remainingTerms,
      deletedTerms: deletedTerms,
      learningSummary: _snapshot.learningSummary.copyWith(
        userTerms: remainingTerms.length,
        deletedTerms: deletedTerms.length,
        lastUpdated: deletedAt,
      ),
      sync: _snapshot.sync.copyWith(
        syncableObjects: _snapshot.sync.syncableObjects + 1,
        categories: _replaceCategoryCount(
          _snapshot.sync.categories,
          'dictionary.deleted_terms',
          deletedTerms.length,
        ),
      ),
    );

    return _snapshot;
  }

  @override
  Future<ManagerSnapshot> restoreUserTerm(UserTermKey term) async {
    DeletedTerm? restoredTombstone;
    final remainingTombstones = <DeletedTerm>[];
    for (final tombstone in _snapshot.deletedTerms) {
      if (tombstone.key == term && restoredTombstone == null) {
        restoredTombstone = tombstone;
      } else {
        remainingTombstones.add(tombstone);
      }
    }

    final restoredTerms = _snapshot.dictionaryTerms.map((candidate) {
      if (candidate.key != term || candidate.status != 'suppressed') {
        return candidate;
      }
      return UserTerm(
        inputCode: candidate.inputCode,
        text: candidate.text,
        reading: candidate.reading,
        weight: 1,
        source: 'manual',
        lastUsed: candidate.lastUsed,
      );
    }).toList();

    if (restoredTombstone != null) {
      restoredTerms.add(
        UserTerm(
          inputCode: restoredTombstone.inputCode,
          text: restoredTombstone.text,
          reading: restoredTombstone.reading,
          weight: 1,
          source: 'manual',
          lastUsed: '未使用',
        ),
      );
    }

    _snapshot = _snapshot.copyWith(
      dictionaryTerms: restoredTerms,
      deletedTerms: remainingTombstones,
      learningSummary: _snapshot.learningSummary.copyWith(
        userTerms: restoredTerms.length,
        deletedTerms: remainingTombstones.length,
        lastUpdated: _snapshot.generatedAt,
      ),
      sync: _snapshot.sync.copyWith(
        syncableObjects:
            _snapshot.sync.syncableObjects -
            (restoredTombstone == null ? 0 : 1),
        categories: _replaceCategoryCount(
          _snapshot.sync.categories,
          'dictionary.deleted_terms',
          remainingTombstones.length,
        ),
      ),
    );
    return _snapshot;
  }

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(
    String filePath,
  ) async {
    return DictionaryImportPreview(
      filePath: filePath,
      format: 'dictionary.user_terms.v1',
      recordCount: 0,
      syncClass: 'P2 encrypted sync',
    );
  }

  @override
  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  }) async {
    return DictionaryImportResult(
      filePath: filePath,
      sourceName: sourceName,
      totalRecords: 0,
      importedTerms: 0,
      insertedTerms: 0,
      updatedTerms: 0,
      skippedDeletedTerms: 0,
      skippedDuplicateTerms: 0,
      dryRun: dryRun,
    );
  }

  @override
  Future<DictionaryExportResult> exportDictionaryFile(String filePath) async {
    return DictionaryExportResult(
      filePath: filePath,
      exportedTerms: _snapshot.dictionaryTerms.length,
      format: 'dictionary.user_terms.v1',
      syncClass: 'P2 encrypted sync',
    );
  }

  @override
  Future<ManagerDiagnosticsReport> loadDiagnosticsReport() async {
    return createManagerDiagnosticsReport(_snapshot);
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
    _snapshot = _snapshotWithSettingsDraft(_snapshot, draft.normalized());
    return _snapshot;
  }
}

ManagerSnapshot _snapshotWithDiagnostics(
  ManagerSnapshot? initialSnapshot,
  ManagerRuntimeDiagnostics diagnostics,
) {
  final snapshot = initialSnapshot ?? createManagerFixture();
  return snapshot.copyWith(
    settings: snapshot.settings.copyWith(runtimeDiagnostics: diagnostics),
  );
}

ManagerSnapshot _snapshotWithSettingsDraft(
  ManagerSnapshot snapshot,
  ManagerSettingsDraft draft,
) {
  final state = deriveManagerSyncUiState(
    draft: draft,
    device: snapshot.sync.device,
  );
  final sync = snapshot.sync.copyWith(
    state: state,
    serverEndpoint: managerSyncEndpointLabel(draft),
    reason: managerSyncGateReason(
      state: state,
      draft: draft,
      device: snapshot.sync.device,
    ),
  );
  final diagnostics = snapshot.settings.runtimeDiagnostics;
  final settings = snapshot.settings.copyWith(
    privacyMode: draft.privacyMode,
    diagnosticsExport: draft.diagnosticsExport,
    syncConfigured: draft.retainSyncConfig,
    draft: draft,
    runtimeDiagnostics: ManagerRuntimeDiagnostics(
      bridgeMode: diagnostics.bridgeMode,
      userDb: diagnostics.userDb,
      nativeLibrary: diagnostics.nativeLibrary,
      settingsStore: diagnostics.settingsStore,
      syncEndpoint: draft.hasServerEndpoint
          ? 'sync endpoint draft configured'
          : 'sync endpoint draft not configured',
      lastErrorCode: diagnostics.lastErrorCode,
    ),
  );

  return snapshot.copyWith(sync: sync, settings: settings);
}

List<SyncCategorySummary> _replaceCategoryCount(
  List<SyncCategorySummary> categories,
  String name,
  int count,
) {
  var replaced = false;
  final updated = categories.map((category) {
    if (category.name != name) {
      return category;
    }
    replaced = true;
    return SyncCategorySummary(name: name, count: count);
  }).toList();

  if (!replaced) {
    updated.add(SyncCategorySummary(name: name, count: count));
  }

  return List.unmodifiable(updated);
}
