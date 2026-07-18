import '../models/manager_models.dart';
import 'manager_bridge.dart';

class UnavailableManagerBridge implements ManagerBridge {
  const UnavailableManagerBridge(this.failure);

  final ManagerBridgeFailure failure;

  @override
  Future<ManagerSnapshot> loadSnapshot() => Future.error(failure);

  @override
  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term) =>
      Future.error(failure);

  @override
  Future<ManagerSnapshot> restoreUserTerm(UserTermKey term) =>
      Future.error(failure);

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(String filePath) =>
      Future.error(failure);

  @override
  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  }) => Future.error(failure);

  @override
  Future<DictionaryExportResult> exportDictionaryFile(String filePath) =>
      Future.error(failure);

  @override
  Future<ManagerDiagnosticsReport> loadDiagnosticsReport() =>
      Future.error(failure);

  @override
  Future<ManagerDiagnosticsExportResult> exportDiagnosticsReport(
    String filePath,
  ) => Future.error(failure);

  @override
  Future<ManagerSnapshot> saveSettingsDraft(ManagerSettingsDraft draft) =>
      Future.error(failure);
}
