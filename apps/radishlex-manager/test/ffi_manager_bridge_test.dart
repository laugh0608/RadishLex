import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_dictionary_mapper.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_learning_mapper.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_runtime_diagnostics.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_mapper.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge_factory.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_platform_control.dart';
import 'package:radishlex_manager/src/bridge/manager_settings_store.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  test('factory enables fixture only through explicit demo mode', () async {
    final bootstrap = await createDefaultManagerBootstrap(mode: 'demo');

    expect(bootstrap.mode, ManagerRuntimeMode.demo);
    expect(bootstrap.bridge, isA<FixtureManagerBridge>());
  });

  test(
    'product bootstrap surfaces platform failure without fixture fallback',
    () async {
      final bootstrap = await createDefaultManagerBootstrap(
        platformControl: const _FailingPlatformControl(),
      );

      expect(bootstrap.mode, ManagerRuntimeMode.product);
      expect(bootstrap.bridge, isNot(isA<FixtureManagerBridge>()));
      await expectLater(
        bootstrap.bridge.loadSnapshot(),
        throwsA(
          isA<ManagerBridgeFailure>().having(
            (failure) => failure.code,
            'code',
            'platform_paths_unavailable',
          ),
        ),
      );
    },
  );

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
      expect(snapshot.dictionaryTerms.single.status, 'active');
      expect(snapshot.deletedTerms.single.text, '合成删除词');
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
      expect(text, contains('sync.state_label: 平台签名 backend 不可用'));
      expect(text, contains('sync.state_source: 设备 production gate 为 blocked'));
      expect(text, contains('sync.entry_state: backend_unavailable'));
      expect(text, contains('sync.entry_blocker: backend_unavailable'));
      expect(text, contains('sync.local_evidence_source: not_recorded'));
      expect(text, contains('sync.recovery_status: recovery_code_flow_closed'));
      expect(
        text,
        contains('sync.recovery_blocker: recovery_code_flow_closed'),
      );
      expect(
        text,
        contains(
          'sync.device_authorization_status: device_authorization_flow_closed',
        ),
      );
      expect(
        text,
        contains(
          'sync.device_authorization_blocker: device_authorization_flow_closed',
        ),
      );
      expect(
        text,
        contains('sync.join_request_status: join_request_unavailable'),
      );
      expect(
        text,
        contains('sync.deployment_evidence: deployment evidence missing'),
      );
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
    await bridge.restoreUserTerm(
      const UserTermKey(inputCode: 'huifu', text: '合成删除词', reading: ''),
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
    expect(native.restoredInputCode, 'huifu');
    expect(native.restoredText, '合成删除词');
    expect(native.restoredReading, isNull);
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
          accessTokenConfigured: true,
          deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
        ),
      );

      expect(saved.settings.draft.privacyMode, isTrue);
      expect(saved.settings.draft.diagnosticsExport, isTrue);
      expect(saved.settings.draft.accessTokenConfigured, isTrue);
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
      expect(reloaded.settings.draft.accessTokenConfigured, isTrue);
      expect(reloaded.sync.state, SyncUiState.syncDisabledByPolicy);
    },
  );

  test(
    'product platform privacy is authoritative and settings are secured',
    () async {
      final tempDir = Directory.systemTemp.createTempSync(
        'radishlex-manager-platform-settings-test-',
      );
      addTearDown(() {
        if (tempDir.existsSync()) {
          tempDir.deleteSync(recursive: true);
        }
      });
      final settingsFile = '${tempDir.path}/manager-settings.json';
      final platform = _RecordingPlatformControl();
      final bridge = FfiManagerBridge(
        dbPath: '/tmp/radishlex-userdb.sqlite',
        settingsFilePath: settingsFile,
        native: _FakeNativeBinding(),
        platformControl: platform,
      );

      final saved = await bridge.saveSettingsDraft(
        const ManagerSettingsDraft(
          serverEndpoint: '',
          retainSyncConfig: false,
          privacyMode: true,
          diagnosticsExport: false,
          deploymentEvidenceRecorded: false,
        ),
      );

      expect(platform.privacyMode, isTrue);
      expect(platform.privacyWrites, [true]);
      expect(platform.secureCalls, 1);
      expect(saved.settings.draft.privacyMode, isTrue);

      platform.privacyMode = false;
      final reloaded = await bridge.loadSnapshot();
      expect(reloaded.settings.draft.privacyMode, isFalse);
    },
  );

  test('product settings failure rolls privacy mode back', () async {
    final tempDir = Directory.systemTemp.createTempSync(
      'radishlex-manager-platform-rollback-test-',
    );
    addTearDown(() {
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });
    final settingsFile = '${tempDir.path}/manager-settings.json';
    final platform = _RecordingPlatformControl(failSecure: true);
    final bridge = FfiManagerBridge(
      dbPath: '/tmp/radishlex-userdb.sqlite',
      settingsFilePath: settingsFile,
      native: _FakeNativeBinding(),
      platformControl: platform,
    );

    await expectLater(
      bridge.saveSettingsDraft(
        const ManagerSettingsDraft(
          serverEndpoint: '',
          retainSyncConfig: false,
          privacyMode: true,
          diagnosticsExport: false,
          deploymentEvidenceRecorded: false,
        ),
      ),
      throwsA(
        isA<ManagerBridgeFailure>().having(
          (failure) => failure.code,
          'code',
          'local_file_permissions_failed',
        ),
      ),
    );

    expect(platform.privacyMode, isFalse);
    expect(platform.privacyPresent, isFalse);
    expect(platform.privacyWrites, [true, false]);
    expect(
      File(settingsFile).readAsStringSync(),
      contains('"privacy_mode": false'),
    );
  });

  test('ffi manager bridge keeps sync disabled and secrets redacted', () async {
    final tempDir = Directory.systemTemp.createTempSync(
      'radishlex-manager-sync-command-contract-test-',
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

    final snapshot = await bridge.saveSettingsDraft(
      const ManagerSettingsDraft(
        serverEndpoint: 'https://localhost:7319',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: true,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
      ),
    );
    final report = await bridge.loadDiagnosticsReport();
    final text = report.toRedactedText();
    final settingsJson = File(settingsFile).readAsStringSync();

    expect(snapshot.sync.state.canEnableUserSync, isFalse);
    expect(
      _diagnosticsValue(report, 'sync.interaction_statuses'),
      contains('closed_current_phase'),
    );
    expect(settingsJson, isNot(contains('action_command')));
    expect(settingsJson, isNot(contains('join_request_authorization')));
    for (final fragment in [
      'Bearer ',
      'access_token_value',
      'recovery_code=',
      'short_code=',
      'private_key=',
      'signature_bytes=',
      'wrapped_material=',
      'request_body',
      'response_body',
    ]) {
      expect(text, isNot(contains(fragment)), reason: fragment);
      expect(settingsJson, isNot(contains(fragment)), reason: fragment);
    }
  });

  test('settings store persists versioned deployment evidence draft', () {
    final tempDir = Directory.systemTemp.createTempSync(
      'radishlex-manager-settings-format-test-',
    );
    addTearDown(() {
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });
    final settingsFile = '${tempDir.path}/manager-settings.json';

    final store = ManagerSettingsStore(filePath: settingsFile);
    final saved = store.save(
      const ManagerSettingsDraft(
        serverEndpoint: ' https://localhost:7319 ',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: true,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
        syncConnectionProbeRecord: ManagerSyncConnectionProbeRecord(
          source: managerSyncConnectionProbeSourceLocalDockerHttps,
          recordedAt: '2026-07-05T00:00:00Z',
          format: managerSyncConnectionHealthSummaryFormat,
          redactionPolicy: managerSyncConnectionHealthSummaryRedactionPolicy,
          endpointStatus: 'configured',
          transportMode: 'local_https',
          accessTokenStatus: 'not_configured',
          connectionStatus: 'reachable',
          authStatus: 'not_required_for_local_probe',
          serverStateStatus: 'domain_missing_expected',
          httpStatus: 404,
          httpStatusClass: 'client_error',
          lastRemoteErrorCode: 'not_found',
          localInsecureTls: 'allowed',
        ),
      ),
    );

    expect(saved.serverEndpoint, 'https://localhost:7319');
    expect(saved.hasDeploymentEvidence, isTrue);
    expect(saved.hasAccessToken, isTrue);
    expect(saved.hasSyncConnectionProbeRecord, isTrue);
    final encoded = File(settingsFile).readAsStringSync();
    expect(encoded, contains('"format_version": 1'));
    expect(encoded, contains('"access_token_configured": true'));
    expect(encoded, contains('"deployment_evidence_source": "local_smoke"'));
    expect(encoded, contains('"sync_connection_health_summary"'));
    expect(encoded, contains('"connection_status": "reachable"'));
    expect(encoded, contains('"last_remote_error_code": "not_found"'));

    final reloaded = ManagerSettingsStore(filePath: settingsFile).load();
    expect(reloaded.hasDeploymentEvidence, isTrue);
    expect(reloaded.hasAccessToken, isTrue);
    expect(reloaded.hasSyncConnectionProbeRecord, isTrue);
    expect(
      deriveManagerSyncConnectionHealth(reloaded).status,
      SyncConnectionStatus.readOnlyProbeReachable,
    );
    expect(
      managerDeploymentEvidenceLabel(reloaded),
      'deployment evidence local smoke',
    );

    final legacySettingsFile = '${tempDir.path}/legacy-settings.json';
    File(legacySettingsFile).writeAsStringSync('''
{
  "format_version": 1,
  "server_endpoint": "https://legacy.example.invalid",
  "retain_sync_config": true,
  "privacy_mode": false,
  "diagnostics_export": false,
  "deployment_evidence_recorded": true
}
''');
    final legacy = ManagerSettingsStore(filePath: legacySettingsFile).load();
    expect(legacy.deploymentEvidenceRecorded, isFalse);
    expect(legacy.deploymentEvidenceSource, isEmpty);
    expect(legacy.hasDeploymentEvidence, isFalse);
  });

  test('settings store rejects unsupported or unsafe draft input', () {
    final tempDir = Directory.systemTemp.createTempSync(
      'radishlex-manager-settings-invalid-test-',
    );
    addTearDown(() {
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });
    final settingsFile = '${tempDir.path}/manager-settings.json';
    final store = ManagerSettingsStore(filePath: settingsFile);

    File(settingsFile).writeAsStringSync('''
{
  "format_version": 99,
  "server_endpoint": "https://sync.example.invalid"
}
''');
    expect(
      store.load,
      throwsA(
        isA<ManagerSettingsStoreException>().having(
          (error) => error.code,
          'code',
          'settings_store_error',
        ),
      ),
    );

    expect(
      () => store.save(
        const ManagerSettingsDraft(
          serverEndpoint: 'https://sync.example.invalid',
          retainSyncConfig: true,
          privacyMode: false,
          diagnosticsExport: false,
          deploymentEvidenceRecorded: true,
          deploymentEvidenceSource: 'raw-log-path',
        ),
      ),
      throwsA(
        isA<ManagerSettingsStoreException>().having(
          (error) => error.code,
          'code',
          'invalid_argument',
        ),
      ),
    );

    expect(
      () => store.save(
        const ManagerSettingsDraft(
          serverEndpoint: 'https://user:token@sync.example.invalid',
          retainSyncConfig: true,
          privacyMode: false,
          diagnosticsExport: false,
          deploymentEvidenceRecorded: false,
        ),
      ),
      throwsA(
        isA<ManagerSettingsStoreException>().having(
          (error) => error.code,
          'code',
          'invalid_argument',
        ),
      ),
    );

    expect(
      () => store.save(
        const ManagerSettingsDraft(
          serverEndpoint: 'https://sync.example.invalid?token=secret',
          retainSyncConfig: true,
          privacyMode: false,
          diagnosticsExport: false,
          deploymentEvidenceRecorded: false,
        ),
      ),
      throwsA(
        isA<ManagerSettingsStoreException>().having(
          (error) => error.code,
          'code',
          'invalid_argument',
        ),
      ),
    );
  });

  test('ffi manager mappers keep native DTO conversion explicit', () {
    final term = managerUserTermFromNative(
      const NativeUserTermRecord(
        id: 1,
        inputCode: 'luobo',
        text: '萝卜词核',
        reading: '',
        source: nativeTermSourceManualImport,
        status: nativeTermStatusSuppressed,
        weight: 1.25,
        createdAtMs: 0,
        updatedAtMs: 0,
        lastUsedAtMs: 0,
        lastUsedAtPresent: false,
        importBatchId: 9,
        importBatchIdPresent: true,
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
        deploymentEvidenceSource: managerDeploymentEvidenceBackupRestore,
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
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      ),
    );

    expect(term.source, 'import');
    expect(term.status, 'suppressed');
    expect(term.lastUsed, '未使用');
    expect(term.importBatchId, 9);
    expect(learning.lastUpdated, '无记录');
    expect(sync.state, SyncUiState.backendUnavailable);
    expect(managerSyncStateLabel(sync.state), '平台签名 backend 不可用');
    expect(
      managerSyncStateSourceDescription(state: sync.state, device: sync.device),
      '设备 production gate 为 blocked',
    );
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
    expect(diagnostics.nativeLibrary, 'app bundle Frameworks native library');
    expect(diagnostics.syncEndpoint, 'sync endpoint draft configured');
    expect(
      fallbackFfiManagerSettingsDraft(
        ' https://draft.example.invalid ',
      ).serverEndpoint,
      'https://draft.example.invalid',
    );
  });
}

String _diagnosticsValue(ManagerDiagnosticsReport report, String key) {
  return report.sections
      .expand((section) => section.items)
      .singleWhere((item) => item.key == key)
      .value;
}

final class _FakeNativeBinding implements RadishLexManagerNativeBinding {
  String? listedDbPath;
  String? deletedInputCode;
  String? deletedText;
  String? deletedReading;
  String? restoredInputCode;
  String? restoredText;
  String? restoredReading;
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
        importBatchId: 7,
        importBatchIdPresent: true,
      ),
    ];
  }

  @override
  List<NativeDeletedTermRecord> listDeletedTerms(String dbPath) {
    return const [
      NativeDeletedTermRecord(
        inputCode: 'huifu',
        text: '合成删除词',
        reading: null,
        deletedAtMs: 1783123260000,
        reason: 'manual_delete',
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
  void restoreUserTerm({
    required String dbPath,
    required String inputCode,
    required String text,
    required String? reading,
  }) {
    restoredInputCode = inputCode;
    restoredText = text;
    restoredReading = reading;
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

final class _FailingPlatformControl implements ManagerPlatformControl {
  const _FailingPlatformControl();

  @override
  Future<ManagerProductPaths> resolveProductPaths() {
    throw const ManagerPlatformException(
      code: 'platform_paths_unavailable',
      message: 'synthetic platform path failure',
    );
  }

  @override
  Future<ManagerPrivacyModeState> readPrivacyModeState() async =>
      const ManagerPrivacyModeState(present: false, enabled: false);

  @override
  Future<void> restorePrivacyModeState(ManagerPrivacyModeState state) async {}

  @override
  Future<void> secureLocalFiles() async {}

  @override
  Future<void> writePrivacyMode(bool enabled) async {}
}

final class _RecordingPlatformControl implements ManagerPlatformControl {
  _RecordingPlatformControl({this.failSecure = false});

  bool privacyMode = false;
  bool privacyPresent = false;
  final bool failSecure;
  final List<bool> privacyWrites = [];
  int secureCalls = 0;

  @override
  Future<ManagerProductPaths> resolveProductPaths() async {
    return const ManagerProductPaths(
      userDbPath: '/tmp/userdb.sqlite3',
      settingsFilePath: '/tmp/manager-settings.json',
      nativeLibraryPath: '/tmp/libradishlex_ime_ffi.dylib',
    );
  }

  @override
  Future<ManagerPrivacyModeState> readPrivacyModeState() async =>
      ManagerPrivacyModeState(present: privacyPresent, enabled: privacyMode);

  @override
  Future<void> restorePrivacyModeState(ManagerPrivacyModeState state) async {
    privacyWrites.add(state.enabled);
    privacyPresent = state.present;
    privacyMode = state.enabled;
  }

  @override
  Future<void> secureLocalFiles() async {
    secureCalls += 1;
    if (failSecure) {
      throw const ManagerPlatformException(
        code: 'local_file_permissions_failed',
        message: 'synthetic permission failure',
      );
    }
  }

  @override
  Future<void> writePrivacyMode(bool enabled) async {
    privacyWrites.add(enabled);
    privacyPresent = true;
    privacyMode = enabled;
  }
}
