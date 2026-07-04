import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge_factory.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  test('factory keeps fixture bridge when no local userdb is configured', () {
    final bridge = createDefaultManagerBridge(environment: const {});

    expect(bridge, isA<FixtureManagerBridge>());
  });

  test(
    'ffi manager bridge maps local userdb summaries into manager snapshot',
    () async {
      final native = _FakeNativeBinding();
      final bridge = FfiManagerBridge(
        dbPath: '/tmp/radishlex-userdb.sqlite',
        serverEndpoint: 'https://sync.example.invalid',
        native: native,
      );

      final snapshot = await bridge.loadSnapshot();

      expect(native.listedDbPath, '/tmp/radishlex-userdb.sqlite');
      expect(snapshot.dictionaryTerms.single.text, '萝卜词核');
      expect(snapshot.dictionaryTerms.single.source, 'manual');
      expect(snapshot.learningSummary.userTerms, 1);
      expect(snapshot.learningSummary.deletedTerms, 2);
      expect(snapshot.learningSummary.selectionEvents, 4);
      expect(native.listedBatchesDbPath, '/tmp/radishlex-userdb.sqlite');
      expect(snapshot.importBatches.single.sourceName, 'manager-import');
      expect(snapshot.importBatches.single.importedTerms, 2);
      expect(snapshot.sync.state, SyncUiState.backendUnavailable);
      expect(snapshot.sync.syncableObjects, 4);
      expect(snapshot.sync.localOnlyEvents, 7);
      expect(snapshot.sync.device.productionGate, 'blocked');
      expect(snapshot.explanations.single.signals, contains('user=2.000'));
      expect(snapshot.explanations.single.signals, contains('freq=0.350'));
      expect(snapshot.explanations.single.score, 2.9);
      expect(native.explainInputCode, 'luobo');
      expect(native.explainCandidateText, '萝卜词核');
      expect(native.explainReading, isNull);
      expect(native.explainContextKind, 'general');
      expect(snapshot.settings.runtimeDiagnostics.bridgeMode, 'ffi_injected');
    },
  );

  test('ffi manager bridge delegates delete/import/export calls', () async {
    final native = _FakeNativeBinding();
    final bridge = FfiManagerBridge(
      dbPath: '/tmp/radishlex-userdb.sqlite',
      native: native,
    );

    await bridge.deleteUserTerm(
      const UserTermKey(inputCode: 'luobo', text: '萝卜词核', reading: ''),
    );
    final preview = await bridge.inspectDictionaryImport('/tmp/import.tsv');
    final importResult = await bridge.importDictionaryFile(
      filePath: '/tmp/import.tsv',
      sourceName: 'manager-import',
      dryRun: false,
    );
    final exportResult = await bridge.exportDictionaryFile('/tmp/export.tsv');

    expect(native.deletedInputCode, 'luobo');
    expect(native.deletedText, '萝卜词核');
    expect(native.deletedReading, isNull);
    expect(preview.format, 'dictionary.user_terms.v1');
    expect(preview.syncClass, 'P2 encrypted sync');
    expect(importResult.importedTerms, 2);
    expect(importResult.dryRun, isFalse);
    expect(native.importedSourceName, 'manager-import');
    expect(exportResult.exportedTerms, 3);
  });
}

final class _FakeNativeBinding implements RadishLexManagerNativeBinding {
  String? listedDbPath;
  String? deletedInputCode;
  String? deletedText;
  String? deletedReading;
  String? importedSourceName;
  String? explainInputCode;
  String? explainCandidateText;
  String? explainReading;
  String? explainContextKind;
  String? listedBatchesDbPath;

  @override
  List<NativeUserTermRecord> listUserTerms(String dbPath) {
    listedDbPath = dbPath;
    return const [
      NativeUserTermRecord(
        id: 1,
        inputCode: 'luobo',
        text: '萝卜词核',
        reading: null,
        source: 3,
        status: 1,
        weight: 0.88,
        createdAtMs: 1783123200000,
        updatedAtMs: 1783123260000,
        lastUsedAtMs: 1783123260000,
        lastUsedAtPresent: true,
      ),
    ];
  }

  @override
  void deleteUserTerm({
    required String dbPath,
    required String inputCode,
    required String text,
    required String? reading,
  }) {
    deletedInputCode = inputCode;
    deletedText = text;
    deletedReading = reading;
  }

  @override
  NativeDictionaryInspectSummary inspectDictionaryImport(String filePath) {
    return const NativeDictionaryInspectSummary(
      formatVersion: 1,
      recordCount: 2,
      syncClass: 2,
    );
  }

  @override
  NativeDictionaryImportSummary importDictionaryFile({
    required String dbPath,
    required String filePath,
    required String? sourceName,
    required bool dryRun,
  }) {
    importedSourceName = sourceName;
    return NativeDictionaryImportSummary(
      importBatchId: dryRun ? 0 : 7,
      importBatchIdPresent: !dryRun,
      totalRecords: 2,
      importedTerms: 2,
      insertedTerms: 1,
      updatedTerms: 1,
      skippedDeletedTerms: 0,
      skippedDuplicateTerms: 0,
      dryRun: dryRun,
    );
  }

  @override
  NativeDictionaryExportSummary exportDictionaryFile({
    required String dbPath,
    required String filePath,
  }) {
    return const NativeDictionaryExportSummary(
      formatVersion: 1,
      exportedTerms: 3,
      syncClass: 2,
    );
  }

  @override
  NativeLearningStatusSummary learningStatus(String dbPath) {
    return const NativeLearningStatusSummary(
      schemaVersion: 1,
      plaintextPayload: false,
      p1RawDetails: false,
      contextStats: false,
      activeUserTerms: 1,
      suppressedUserTerms: 0,
      rankerWeights: 1,
      deletedTermTombstones: 2,
      selectionEvents: 4,
      negativeFeedback: 1,
      importBatches: 2,
      latestUserTermUpdatedAtMs: 1783123260000,
      latestUserTermUpdatedAtPresent: true,
      latestSelectionEventAtMs: 1783123260000,
      latestSelectionEventAtPresent: true,
      latestNegativeFeedbackAtMs: 1783123260000,
      latestNegativeFeedbackAtPresent: true,
      latestDeletedTermAtMs: 1783123260000,
      latestDeletedTermAtPresent: true,
      latestImportBatchAtMs: 1783123260000,
      latestImportBatchAtPresent: true,
      latestActivityAtMs: 1783123260000,
      latestActivityAtPresent: true,
    );
  }

  @override
  List<NativeImportBatchRecord> listImportBatches(String dbPath) {
    listedBatchesDbPath = dbPath;
    return const [
      NativeImportBatchRecord(
        id: 7,
        sourceName: 'manager-import',
        totalRecords: 2,
        importedTerms: 2,
        insertedTerms: 1,
        updatedTerms: 1,
        skippedDeletedTerms: 0,
        skippedDuplicateTerms: 0,
        createdAtMs: 1783123260000,
        notes: null,
        notesPresent: false,
      ),
    ];
  }

  @override
  NativeSyncPreflightSummary syncPreflight(String dbPath) {
    return const NativeSyncPreflightSummary(
      schemaVersion: 1,
      plaintextPayload: false,
      syncableUserTerms: 1,
      syncableRankerWeights: 1,
      syncableDeletedTerms: 2,
      localSelectionEvents: 4,
      localNegativeFeedback: 1,
      localImportBatches: 2,
    );
  }

  @override
  NativeRankExplainSummary rankExplain({
    required String dbPath,
    required String inputCode,
    required String candidateText,
    required String? reading,
    required String contextKind,
  }) {
    explainInputCode = inputCode;
    explainCandidateText = candidateText;
    explainReading = reading;
    explainContextKind = contextKind;
    return const NativeRankExplainSummary(
      inputCode: 'luobo',
      candidateText: '萝卜词核',
      reading: null,
      readingPresent: false,
      contextKind: 'general',
      originalIndex: 0,
      finalScore: 2.9,
      engineOrderFactor: 0.0,
      userTermBoost: 2.0,
      frequencyBoost: 0.35,
      recencyBoost: 0.25,
      contextBoost: 0.3,
      negativeFeedbackPenalty: 0.0,
      suppressedPenalty: 0.0,
      deletedPenalty: 0.0,
    );
  }
}
