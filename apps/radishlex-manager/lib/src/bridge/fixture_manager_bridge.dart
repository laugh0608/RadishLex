import '../data/manager_fixture.dart';
import '../models/manager_models.dart';
import 'manager_bridge.dart';

class FixtureManagerBridge implements ManagerBridge {
  FixtureManagerBridge({ManagerSnapshot? initialSnapshot})
    : _snapshot = initialSnapshot ?? createManagerFixture();

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
