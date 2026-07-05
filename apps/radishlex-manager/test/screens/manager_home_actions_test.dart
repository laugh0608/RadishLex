import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';
import 'package:radishlex_manager/src/screens/manager/manager_home_actions.dart';

void main() {
  group('dictionary action result messages', () {
    test('summarize dry-run import results with combined skipped count', () {
      const result = DictionaryImportResult(
        filePath: '/tmp/radishlex-import.tsv',
        sourceName: 'manager-import',
        totalRecords: 10,
        importedTerms: 7,
        insertedTerms: 3,
        updatedTerms: 4,
        skippedDeletedTerms: 1,
        skippedDuplicateTerms: 2,
        dryRun: true,
      );

      expect(
        dictionaryImportResultMessage(result),
        '导入检查完成：7 / 10 条，新增 3，更新 4，跳过 3',
      );
    });

    test('summarize committed import results', () {
      const result = DictionaryImportResult(
        filePath: '/tmp/radishlex-import.tsv',
        sourceName: 'ops-import',
        totalRecords: 5,
        importedTerms: 5,
        insertedTerms: 4,
        updatedTerms: 1,
        skippedDeletedTerms: 0,
        skippedDuplicateTerms: 0,
        dryRun: false,
      );

      expect(
        dictionaryImportResultMessage(result),
        '导入完成：5 / 5 条，新增 4，更新 1，跳过 0',
      );
    });

    test('summarize export results without exposing file paths', () {
      const result = DictionaryExportResult(
        filePath: '/tmp/private/radishlex-export.json',
        exportedTerms: 3,
        format: 'dictionary.user_terms.v1',
        syncClass: 'P2 encrypted sync',
      );

      expect(
        dictionaryExportResultMessage(result),
        '导出完成：3 条，dictionary.user_terms.v1 / P2 encrypted sync',
      );
    });
  });

  group('manager bridge failure messages', () {
    test('prefer dictionary import category during inspect failures', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'invalid_argument'),
        ManagerBridgeOperation.inspectDictionaryImport,
      );

      expect(message, '检查导入词库失败：词库导入或本地 userdb 错误（invalid_argument）');
    });

    test('surface native library category before operation category', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'ffi_library_load_failed'),
        ManagerBridgeOperation.previewDiagnostics,
      );

      expect(message, '预览诊断摘要失败：native library 不可用（ffi_library_load_failed）');
    });

    test('keep settings draft failures in settings category', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'invalid_argument'),
        ManagerBridgeOperation.saveSettingsDraft,
      );

      expect(message, '保存设置草案失败：设置草案错误（invalid_argument）');
    });

    test('classify non-bridge exceptions as manager bridge errors', () {
      final message = managerBridgeFailureMessage(
        StateError('details must stay out of the UI'),
        ManagerBridgeOperation.exportDiagnostics,
      );

      expect(message, '导出诊断摘要失败：管理端 bridge 错误（manager_bridge_error）');
    });
  });
}

class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure({required this.code});

  @override
  int get statusCode => 4;

  @override
  final String code;

  @override
  String get message => 'private detail omitted';
}
