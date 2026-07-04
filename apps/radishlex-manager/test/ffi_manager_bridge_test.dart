import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_dictionary_mapper.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_learning_mapper.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_runtime_diagnostics.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_mapper.dart';
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

      final report = await bridge.loadDiagnosticsReport();
      final text = report.toRedactedText();
      expect(text, contains('runtime.bridge_mode: ffi_injected'));
      expect(text, contains('sync.state: backend_unavailable'));
      expect(text, contains('redaction.user_terms: omitted'));
      expect(text, isNot(contains('萝卜词核')));
      expect(text, isNot(contains('/tmp/radishlex-userdb.sqlite')));
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

  test(
    'ffi manager bridge persists settings draft and derives sync gate',
    () async {
      final tempDir = Directory.systemTemp.createTempSync(
        'radishlex-manager-settings-test-',
      );
      addTearDown(() {
        if (tempDir.existsSync()) {
          tempDir.deleteSync(recursive: true);
        }
      });
      final settingsFile = '${tempDir.path}/manager-settings.json';

      final bridge = FfiManagerBridge(
        dbPath: '/tmp/radishlex-userdb.sqlite',
        settingsFilePath: settingsFile,
        native: _FakeNativeBinding(),
      );
      final saved = await bridge.saveSettingsDraft(
        const ManagerSettingsDraft(
          serverEndpoint: 'https://draft.example.invalid',
          retainSyncConfig: true,
          privacyMode: true,
          diagnosticsExport: true,
          deploymentEvidenceRecorded: true,
        ),
      );

      expect(saved.settings.draft.privacyMode, isTrue);
      expect(saved.settings.draft.diagnosticsExport, isTrue);
      expect(
        saved.settings.runtimeDiagnostics.settingsStore,
        contains('configured'),
      );
      expect(saved.sync.state, SyncUiState.syncDisabledByPolicy);
      expect(saved.sync.serverEndpoint, 'https://draft.example.invalid');
      expect(
        File(settingsFile).readAsStringSync(),
        contains('server_endpoint'),
      );

      final reloaded = await FfiManagerBridge(
        dbPath: '/tmp/radishlex-userdb.sqlite',
        settingsFilePath: settingsFile,
        native: _FakeNativeBinding(),
      ).loadSnapshot();

      expect(reloaded.settings.draft.privacyMode, isTrue);
      expect(reloaded.sync.state, SyncUiState.syncDisabledByPolicy);
    },
  );

  test('ffi manager mappers keep native DTO conversion explicit', () {
    final term = managerUserTermFromNative(
      const NativeUserTermRecord(
        id: 1,
        inputCode: 'luobo',
        text: '萝卜词核',
        reading: '',
        source: nativeTermSourceManualImport,
        status: 1,
        weight: 1.25,
        createdAtMs: 0,
        updatedAtMs: 0,
        lastUsedAtMs: 0,
        lastUsedAtPresent: false,
      ),
    );
    final learning = managerLearningSummaryFromNative(
      const NativeLearningStatusSummary(
        schemaVersion: 1,
        plaintextPayload: false,
        p1RawDetails: false,
        contextStats: false,
        activeUserTerms: 3,
        suppressedUserTerms: 2,
        rankerWeights: 5,
        deletedTermTombstones: 7,
        selectionEvents: 11,
        negativeFeedback: 13,
        importBatches: 17,
        latestUserTermUpdatedAtMs: 0,
        latestUserTermUpdatedAtPresent: false,
        latestSelectionEventAtMs: 0,
        latestSelectionEventAtPresent: false,
        latestNegativeFeedbackAtMs: 0,
        latestNegativeFeedbackAtPresent: false,
        latestDeletedTermAtMs: 0,
        latestDeletedTermAtPresent: false,
        latestImportBatchAtMs: 0,
        latestImportBatchAtPresent: false,
        latestActivityAtMs: 0,
        latestActivityAtPresent: false,
      ),
    );
    final sync = managerSyncSummaryFromNative(
      summary: const NativeSyncPreflightSummary(
        schemaVersion: 1,
        plaintextPayload: false,
        syncableUserTerms: 2,
        syncableRankerWeights: 3,
        syncableDeletedTerms: 5,
        localSelectionEvents: 7,
        localNegativeFeedback: 11,
        localImportBatches: 13,
      ),
      settingsDraft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
      ),
    );
    final explanation = managerRankerExplanationFromNative(
      const NativeRankExplainSummary(
        inputCode: 'luobo',
        candidateText: '萝卜词核',
        reading: null,
        readingPresent: false,
        contextKind: 'general',
        originalIndex: 0,
        finalScore: 2.5,
        engineOrderFactor: 0.1,
        userTermBoost: 1.0,
        frequencyBoost: 0.2,
        recencyBoost: 0.3,
        contextBoost: 0.4,
        negativeFeedbackPenalty: -0.5,
        suppressedPenalty: 0.0,
        deletedPenalty: 0.0,
      ),
    );
    final diagnostics = managerRuntimeDiagnosticsFromFfi(
      nativeInjected: false,
      libraryPath: '/tmp/libradishlex_ime_ffi.dylib',
      settingsStoreSourceLabel: 'settings file configured',
      draft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: true,
        deploymentEvidenceRecorded: true,
      ),
    );

    expect(term.source, 'import');
    expect(term.lastUsed, '未使用');
    expect(learning.lastUpdated, '无记录');
    expect(sync.state, SyncUiState.backendUnavailable);
    expect(sync.syncableObjects, 10);
    expect(sync.localOnlyEvents, 31);
    expect(
      sync.categories.map((category) => '${category.name}:${category.count}'),
      containsAll([
        'dictionary.user_terms:2',
        'dictionary.deleted_terms:5',
        'ranker.weights:3',
        'learning.selection_events:7',
        'learning.negative_feedback:11',
        'dictionary.import_batches:13',
      ]),
    );
    expect(explanation.signals, contains('negative=-0.500'));
    expect(managerRankExplainReading(term), isNull);
    expect(
      diagnostics.nativeLibrary,
      'RADISHLEX_MANAGER_FFI_LIBRARY configured',
    );
    expect(diagnostics.syncEndpoint, 'sync endpoint draft configured');
    expect(
      fallbackFfiManagerSettingsDraft(
        ' https://draft.example.invalid ',
      ).serverEndpoint,
      'https://draft.example.invalid',
    );
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
