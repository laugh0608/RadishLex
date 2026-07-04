import 'dart:convert';
import 'dart:io';

import 'package:radishlex_manager/src/bridge/ffi_manager_bridge.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

Future<void> main(List<String> args) async {
  final options = _SmokeOptions.parse(args);
  final workDir = Directory(options.workDir)..createSync(recursive: true);
  final dbPath = '${workDir.path}/radishlex-manager-ffi-smoke.sqlite';
  final importPath = '${workDir.path}/manager-import.tsv';
  final exportPath = '${workDir.path}/manager-export.tsv';

  File(importPath).writeAsStringSync(_dictionaryFixture, encoding: utf8);

  final bridge = FfiManagerBridge(
    dbPath: dbPath,
    libraryPath: options.libraryPath,
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
  _expect(
    importedSnapshot.explanations.first.signals.contains('ffi_userdb_weight'),
    'explain bridge signal',
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
  _expect(
    deletedSnapshot.sync.syncableObjects == 2,
    'post-delete syncable user term plus tombstone',
  );

  final exportResult = await bridge.exportDictionaryFile(exportPath);
  _expect(exportResult.exportedTerms == 1, 'exported term count');
  _expect(exportResult.format == 'dictionary.user_terms.v1', 'export format');

  final exported = File(exportPath).readAsStringSync(encoding: utf8);
  _expect(exported.contains('input_code\ttext\treading'), 'export header');
  _expect(exported.contains('同步预检'), 'remaining synthetic term exported');
  _expect(!exported.contains('萝卜词核'), 'deleted term not exported');

  stdout.writeln('RadishLex manager FFI smoke passed.');
}

const _dictionaryFixture = '''
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜词核\tluo bo ci he\tmanual_import\t2.5\tactive
tongbu\t同步预检\t\tmanual_import\t1.5\tactive
''';

void _expect(bool condition, String label) {
  if (!condition) {
    throw StateError('manager FFI smoke assertion failed: $label');
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
