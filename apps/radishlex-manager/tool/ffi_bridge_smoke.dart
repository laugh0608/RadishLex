import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

import 'package:radishlex_manager/src/bridge/ffi_manager_bridge.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

Future<void> main(List<String> args) async {
  final options = _SmokeOptions.parse(args);
  final workDir = Directory(options.workDir)..createSync(recursive: true);
  final dbPath = '${workDir.path}/radishlex-manager-ffi-smoke.sqlite';
  final importPath = '${workDir.path}/manager-import.tsv';
  final exportPath = '${workDir.path}/manager-export.tsv';
  final diagnosticsPath = '${workDir.path}/manager-diagnostics.txt';
  final settingsPath = '${workDir.path}/manager-settings.json';

  File(importPath).writeAsStringSync(_dictionaryFixture, encoding: utf8);
  _expectFutureSyncCommandSymbolsAbsent(options.libraryPath);

  final bridge = FfiManagerBridge(
    dbPath: dbPath,
    libraryPath: options.libraryPath,
    settingsFilePath: settingsPath,
  );

  final preview = await bridge.inspectDictionaryImport(importPath);
  _expect(preview.format == 'dictionary.user_terms.v1', 'format label');
  _expect(preview.recordCount == 2, 'inspect record count');
  _expect(preview.syncClass == 'P2 encrypted sync', 'sync class label');

  final dryRun = await bridge.importDictionaryFile(
    filePath: importPath,
    sourceName: 'manager-ffi-smoke',
    dryRun: true,
  );
  _expect(dryRun.dryRun, 'dry-run flag');
  _expect(dryRun.totalRecords == 2, 'dry-run total records');
  _expect(dryRun.importedTerms == 2, 'dry-run imported terms');

  final importResult = await bridge.importDictionaryFile(
    filePath: importPath,
    sourceName: 'manager-ffi-smoke',
    dryRun: false,
  );
  _expect(!importResult.dryRun, 'import flag');
  _expect(importResult.totalRecords == 2, 'import total records');
  _expect(importResult.importedTerms == 2, 'imported term count');

  final importedSnapshot = await bridge.loadSnapshot();
  _expect(importedSnapshot.dictionaryTerms.length == 2, 'snapshot term count');
  _expect(
    importedSnapshot.learningSummary.userTerms == 2,
    'learning user terms',
  );
  _expect(
    importedSnapshot.learningSummary.deletedTerms == 0,
    'initial deleted tombstones',
  );
  _expect(importedSnapshot.sync.state == SyncUiState.localOnly, 'sync state');
  _expect(importedSnapshot.sync.syncableObjects == 2, 'syncable objects');
  _expect(importedSnapshot.sync.localOnlyEvents == 1, 'local import batch');
  _expect(importedSnapshot.explanations.length == 2, 'explain summaries');
  _expect(importedSnapshot.importBatches.length == 1, 'import batch count');
  _expect(
    importedSnapshot.importBatches.single.sourceName == 'manager-ffi-smoke',
    'import batch source name',
  );
  _expect(
    importedSnapshot.importBatches.single.importedTerms == 2,
    'import batch imported terms',
  );
  _expect(
    importedSnapshot.dictionaryTerms.every(
      (term) => term.importBatchId == importedSnapshot.importBatches.single.id,
    ),
    'imported terms reference the recorded batch id',
  );
  _expect(
    importedSnapshot.explanations.first.signals.contains('user=2.500'),
    'rank explain user term boost',
  );
  _expect(
    (importedSnapshot.explanations.first.score - 2.5).abs() < 0.000001,
    'rank explain final score',
  );

  final settingsSnapshot = await bridge.saveSettingsDraft(
    const ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: true,
      diagnosticsExport: true,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    ),
  );
  _expect(
    settingsSnapshot.sync.state == SyncUiState.syncDisabledByPolicy,
    'settings privacy sync gate',
  );
  _expect(File(settingsPath).existsSync(), 'settings file persisted');
  _expect(
    File(
      settingsPath,
    ).readAsStringSync(encoding: utf8).contains('server_endpoint'),
    'settings file schema',
  );
  _expect(
    File(settingsPath)
        .readAsStringSync(encoding: utf8)
        .contains('"deployment_evidence_source": "external_tls"'),
    'settings evidence source persisted',
  );

  final deletedSnapshot = await bridge.deleteUserTerm(
    const UserTermKey(
      inputCode: 'luobo',
      text: '萝卜词核',
      reading: 'luo bo ci he',
    ),
  );
  _expect(deletedSnapshot.dictionaryTerms.length == 1, 'post-delete terms');
  _expect(
    deletedSnapshot.dictionaryTerms.every((term) => term.inputCode != 'luobo'),
    'deleted term absent from list',
  );
  _expect(
    deletedSnapshot.learningSummary.deletedTerms == 1,
    'deleted tombstone count',
  );
  _expect(deletedSnapshot.deletedTerms.length == 1, 'deleted term view count');
  _expect(
    deletedSnapshot.deletedTerms.single.inputCode == 'luobo' &&
        deletedSnapshot.deletedTerms.single.reason == 'manual_delete',
    'deleted term identity and reason',
  );
  _expect(
    deletedSnapshot.sync.syncableObjects == 2,
    'post-delete syncable user term plus tombstone',
  );

  final restartedAfterDelete = await FfiManagerBridge(
    dbPath: dbPath,
    libraryPath: options.libraryPath,
    settingsFilePath: settingsPath,
  ).loadSnapshot();
  _expect(
    restartedAfterDelete.deletedTerms.single.inputCode == 'luobo',
    'deleted tombstone survives manager restart',
  );

  final restoredSnapshot = await bridge.restoreUserTerm(
    const UserTermKey(
      inputCode: 'luobo',
      text: '萝卜词核',
      reading: 'luo bo ci he',
    ),
  );
  _expect(restoredSnapshot.dictionaryTerms.length == 2, 'restored term count');
  _expect(restoredSnapshot.deletedTerms.isEmpty, 'restore clears tombstone');
  _expect(
    restoredSnapshot.dictionaryTerms
            .singleWhere((term) => term.inputCode == 'luobo')
            .status ==
        'active',
    'restore returns active status',
  );
  _expect(
    restoredSnapshot.dictionaryTerms
            .singleWhere((term) => term.inputCode == 'luobo')
            .importBatchId ==
        importedSnapshot.importBatches.single.id,
    'restore preserves local import provenance',
  );

  final restartedAfterRestore = await FfiManagerBridge(
    dbPath: dbPath,
    libraryPath: options.libraryPath,
    settingsFilePath: settingsPath,
  ).loadSnapshot();
  _expect(
    restartedAfterRestore.dictionaryTerms.length == 2 &&
        restartedAfterRestore.deletedTerms.isEmpty &&
        restartedAfterRestore.dictionaryTerms.every(
          (term) =>
              term.importBatchId == importedSnapshot.importBatches.single.id,
        ),
    'explicit restore survives manager restart',
  );

  await bridge.deleteUserTerm(
    const UserTermKey(
      inputCode: 'luobo',
      text: '萝卜词核',
      reading: 'luo bo ci he',
    ),
  );

  final exportResult = await bridge.exportDictionaryFile(exportPath);
  _expect(exportResult.exportedTerms == 1, 'exported term count');
  _expect(exportResult.format == 'dictionary.user_terms.v1', 'export format');

  final exported = File(exportPath).readAsStringSync(encoding: utf8);
  _expect(exported.contains('input_code\ttext\treading'), 'export header');
  _expect(exported.contains('同步预检'), 'remaining synthetic term exported');
  _expect(!exported.contains('萝卜词核'), 'deleted term not exported');

  final diagnostics = await bridge.loadDiagnosticsReport();
  final diagnosticsText = diagnostics.toRedactedText();
  _expect(
    diagnosticsText.contains('format: manager.diagnostics.v1'),
    'diagnostics format',
  );
  _expect(
    diagnosticsText.contains('sync.state: sync_disabled_by_policy'),
    'diagnostics sync state',
  );
  _expect(
    diagnosticsText.contains('redaction.user_terms: omitted'),
    'diagnostics redaction policy',
  );
  _expect(!diagnosticsText.contains(dbPath), 'diagnostics omits db path');
  _expect(
    !diagnosticsText.contains(settingsPath),
    'diagnostics omits settings path',
  );
  _expect(
    !diagnosticsText.contains(importPath),
    'diagnostics omits import path',
  );
  _expect(!diagnosticsText.contains('萝卜词核'), 'diagnostics omits user terms');

  final diagnosticsExport = await bridge.exportDiagnosticsReport(
    diagnosticsPath,
  );
  _expect(diagnosticsExport.lineCount > 0, 'diagnostics export line count');
  final exportedDiagnostics = File(
    diagnosticsPath,
  ).readAsStringSync(encoding: utf8);
  _expect(
    exportedDiagnostics.contains('manager.diagnostics.v1'),
    'diagnostics export content',
  );
  _expect(
    !exportedDiagnostics.contains(dbPath),
    'diagnostics export omits db path',
  );

  stdout.writeln('RadishLex manager FFI smoke passed.');
}

const _dictionaryFixture = '''
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜词核\tluo bo ci he\tmanual_import\t2.5\tactive
tongbu\t同步预检\t\tmanual_import\t1.5\tactive
''';

const _futureSyncCommandSymbols = [
  'radishlex_manager_sync_command_execute_v1',
  'radishlex_manager_sync_command_result_action_id',
  'radishlex_manager_sync_command_result_status',
  'radishlex_manager_sync_command_result_error_code',
  'radishlex_manager_sync_command_result_retry_policy',
  'radishlex_manager_sync_command_result_summary',
  'radishlex_manager_sync_command_result_free',
];

void _expect(bool condition, String label) {
  if (!condition) {
    throw StateError('manager FFI smoke assertion failed: $label');
  }
}

void _expectFutureSyncCommandSymbolsAbsent(String libraryPath) {
  final library = ffi.DynamicLibrary.open(libraryPath);
  for (final symbol in _futureSyncCommandSymbols) {
    try {
      library.lookup<ffi.NativeFunction<ffi.Void Function()>>(symbol);
      throw StateError(
        'manager FFI smoke assertion failed: future sync command symbol '
        'unexpectedly exported: $symbol',
      );
    } on ArgumentError {
      // Missing symbols are the expected current-phase capability state.
    }
  }
}

final class _SmokeOptions {
  const _SmokeOptions({required this.libraryPath, required this.workDir});

  final String libraryPath;
  final String workDir;

  static _SmokeOptions parse(List<String> args) {
    String? libraryPath;
    String? workDir;

    for (var index = 0; index < args.length; index += 1) {
      final arg = args[index];
      switch (arg) {
        case '--library':
          index += 1;
          libraryPath = _requiredValue(args, index, arg);
        case '--work-dir':
          index += 1;
          workDir = _requiredValue(args, index, arg);
        case '--help':
        case '-h':
          _printUsage();
          exit(0);
        default:
          stderr.writeln('unknown argument: $arg');
          _printUsage();
          exit(64);
      }
    }

    if (libraryPath == null || workDir == null) {
      _printUsage();
      exit(64);
    }

    return _SmokeOptions(libraryPath: libraryPath, workDir: workDir);
  }

  static String _requiredValue(List<String> args, int index, String option) {
    if (index >= args.length || args[index].startsWith('--')) {
      stderr.writeln('missing value for $option');
      exit(64);
    }
    return args[index];
  }

  static void _printUsage() {
    stdout.writeln(
      'Usage: dart run tool/ffi_bridge_smoke.dart '
      '--library <ime-ffi dynamic library> --work-dir <temp dir>',
    );
  }
}
