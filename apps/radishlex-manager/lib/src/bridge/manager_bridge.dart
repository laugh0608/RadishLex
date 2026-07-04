import '../models/manager_models.dart';

abstract interface class ManagerBridge {
  Future<ManagerSnapshot> loadSnapshot();

  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term);

  Future<DictionaryImportPreview> inspectDictionaryImport(String filePath);

  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  });

  Future<DictionaryExportResult> exportDictionaryFile(String filePath);
}
