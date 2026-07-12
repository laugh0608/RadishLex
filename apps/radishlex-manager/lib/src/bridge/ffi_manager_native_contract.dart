import 'ffi_manager_native_dictionary.dart';
import 'ffi_manager_native_learning.dart';
import 'ffi_manager_native_rank.dart';
import 'ffi_manager_native_sync.dart';
import 'manager_bridge.dart';

abstract interface class RadishLexManagerNativeBinding {
  List<NativeUserTermRecord> listUserTerms(String dbPath);

  void deleteUserTerm({
    required String dbPath,
    required String inputCode,
    required String text,
    required String? reading,
  });

  NativeDictionaryInspectSummary inspectDictionaryImport(String filePath);

  NativeDictionaryImportSummary importDictionaryFile({
    required String dbPath,
    required String filePath,
    required String? sourceName,
    required bool dryRun,
  });

  NativeDictionaryExportSummary exportDictionaryFile({
    required String dbPath,
    required String filePath,
  });

  NativeLearningStatusSummary learningStatus(String dbPath);

  List<NativeImportBatchRecord> listImportBatches(String dbPath);

  NativeSyncPreflightSummary syncPreflight(String dbPath);

  NativeRankExplainSummary rankExplain({
    required String dbPath,
    required String inputCode,
    required String candidateText,
    required String? reading,
    required String contextKind,
  });
}

class FfiManagerBridgeException implements ManagerBridgeFailure {
  const FfiManagerBridgeException({
    required this.statusCode,
    required this.code,
    required this.message,
  });

  @override
  final int statusCode;
  @override
  final String code;
  @override
  final String message;

  @override
  String toString() => 'FfiManagerBridgeException($code): $message';
}
