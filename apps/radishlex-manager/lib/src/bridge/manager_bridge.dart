import '../models/manager_models.dart';

abstract interface class ManagerBridge {
  Future<ManagerSnapshot> loadSnapshot();

  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term);

  Future<ManagerSnapshot> restoreUserTerm(UserTermKey term);

  Future<DictionaryImportPreview> inspectDictionaryImport(String filePath);

  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  });

  Future<DictionaryExportResult> exportDictionaryFile(String filePath);

  Future<ManagerDiagnosticsReport> loadDiagnosticsReport();

  Future<ManagerDiagnosticsExportResult> exportDiagnosticsReport(
    String filePath,
  );

  Future<ManagerSnapshot> saveSettingsDraft(ManagerSettingsDraft draft);
}

abstract interface class ManagerBridgeFailure implements Exception {
  int get statusCode;

  String get code;

  String get message;
}

enum ManagerBridgeOperation {
  loadSnapshot,
  deleteUserTerm,
  restoreUserTerm,
  inspectDictionaryImport,
  importDictionaryFile,
  exportDictionaryFile,
  previewDiagnostics,
  exportDiagnostics,
  saveSettingsDraft,
}

class ManagerBridgeFailurePresentation {
  const ManagerBridgeFailurePresentation({
    required this.operation,
    required this.categoryCode,
    required this.categoryLabel,
    required this.failureCode,
  });

  final ManagerBridgeOperation operation;
  final String categoryCode;
  final String categoryLabel;
  final String failureCode;

  String get userMessage =>
      '${operation.userActionLabel}失败：$categoryLabel（$failureCode）';
}

ManagerBridgeFailurePresentation describeManagerBridgeFailure(
  Object? error,
  ManagerBridgeOperation operation,
) {
  final failureCode = error is ManagerBridgeFailure
      ? error.code
      : 'manager_bridge_error';
  final categoryCode = _failureCategoryCode(failureCode, operation);

  return ManagerBridgeFailurePresentation(
    operation: operation,
    categoryCode: categoryCode,
    categoryLabel: _failureCategoryLabel(categoryCode),
    failureCode: failureCode,
  );
}

extension ManagerBridgeOperationLabel on ManagerBridgeOperation {
  String get userActionLabel {
    switch (this) {
      case ManagerBridgeOperation.loadSnapshot:
        return '加载管理数据';
      case ManagerBridgeOperation.deleteUserTerm:
        return '删除词条';
      case ManagerBridgeOperation.restoreUserTerm:
        return '恢复词条';
      case ManagerBridgeOperation.inspectDictionaryImport:
        return '检查导入词库';
      case ManagerBridgeOperation.importDictionaryFile:
        return '导入词库';
      case ManagerBridgeOperation.exportDictionaryFile:
        return '导出词库';
      case ManagerBridgeOperation.previewDiagnostics:
        return '预览诊断摘要';
      case ManagerBridgeOperation.exportDiagnostics:
        return '导出诊断摘要';
      case ManagerBridgeOperation.saveSettingsDraft:
        return '保存设置草案';
    }
  }
}

String _failureCategoryCode(
  String failureCode,
  ManagerBridgeOperation operation,
) {
  if (failureCode == 'ffi_library_load_failed' ||
      failureCode == 'ffi_contract_mismatch' ||
      failureCode == 'platform_bridge_unavailable' ||
      failureCode == 'platform_paths_unavailable' ||
      failureCode == 'platform_paths_invalid' ||
      failureCode == 'unavailable') {
    return 'native_library';
  }

  if (failureCode == 'privacy_read_failed' ||
      failureCode == 'privacy_write_failed' ||
      failureCode == 'privacy_rollback_failed') {
    return 'privacy_control';
  }
  if (failureCode == 'local_file_permissions_failed') {
    return 'local_permissions';
  }

  if (operation == ManagerBridgeOperation.inspectDictionaryImport ||
      operation == ManagerBridgeOperation.importDictionaryFile) {
    return 'dictionary_import';
  }
  if (operation == ManagerBridgeOperation.exportDictionaryFile) {
    return 'dictionary_export';
  }
  if (operation == ManagerBridgeOperation.saveSettingsDraft) {
    return 'settings_draft';
  }

  switch (failureCode) {
    case 'invalid_argument':
      return 'invalid_input';
    case 'userdb_error':
      return 'local_userdb';
    case 'ranker_error':
      return 'ranker';
    case 'sync_error':
      return 'sync_preflight';
    case 'engine_error':
      return 'engine';
    case 'internal_error':
      return 'internal';
    default:
      return 'manager_bridge';
  }
}

String _failureCategoryLabel(String categoryCode) {
  switch (categoryCode) {
    case 'dictionary_import':
      return '词库导入或本地 userdb 错误';
    case 'dictionary_export':
      return '词库导出或本地 userdb 错误';
    case 'settings_draft':
      return '设置草案错误';
    case 'native_library':
      return 'native library 不可用';
    case 'privacy_control':
      return '系统隐私模式控制失败';
    case 'local_permissions':
      return '本地文件权限错误';
    case 'invalid_input':
      return '输入参数无效';
    case 'local_userdb':
      return '本地 userdb 错误';
    case 'ranker':
      return 'rank explain 错误';
    case 'sync_preflight':
      return '同步预检错误';
    case 'engine':
      return '输入引擎错误';
    case 'internal':
      return '内部错误';
    default:
      return '管理端 bridge 错误';
  }
}
