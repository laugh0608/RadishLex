import '../models/manager_models.dart';
import 'ffi_manager_native_models.dart';

UserTerm managerUserTermFromNative(NativeUserTermRecord term) {
  return UserTerm(
    inputCode: term.inputCode,
    text: term.text,
    reading: term.reading ?? '',
    weight: term.weight,
    source: managerTermSourceLabel(term.source),
    lastUsed: term.lastUsedAtPresent
        ? managerFormatTimestampMs(term.lastUsedAtMs)
        : '未使用',
    status: managerTermStatusLabel(term.status),
    importBatchId: term.importBatchIdPresent ? term.importBatchId : null,
  );
}

DeletedTerm managerDeletedTermFromNative(NativeDeletedTermRecord term) {
  return DeletedTerm(
    inputCode: term.inputCode,
    text: term.text,
    reading: term.reading ?? '',
    deletedAt: managerFormatTimestampMs(term.deletedAtMs),
    reason: term.reason,
  );
}

DictionaryImportBatchSummary managerImportBatchFromNative(
  NativeImportBatchRecord batch,
) {
  return DictionaryImportBatchSummary(
    id: batch.id,
    sourceName: batch.sourceName,
    totalRecords: batch.totalRecords,
    importedTerms: batch.importedTerms,
    insertedTerms: batch.insertedTerms,
    updatedTerms: batch.updatedTerms,
    skippedDeletedTerms: batch.skippedDeletedTerms,
    skippedDuplicateTerms: batch.skippedDuplicateTerms,
    createdAt: managerFormatTimestampMs(batch.createdAtMs),
    notes: batch.notes ?? '',
  );
}

DictionaryImportPreview managerDictionaryImportPreviewFromNative({
  required String filePath,
  required NativeDictionaryInspectSummary summary,
}) {
  return DictionaryImportPreview(
    filePath: filePath,
    format: managerDictionaryFormatLabel(summary.formatVersion),
    recordCount: summary.recordCount,
    syncClass: managerSyncClassLabel(summary.syncClass),
  );
}

DictionaryImportResult managerDictionaryImportResultFromNative({
  required String filePath,
  required String sourceName,
  required NativeDictionaryImportSummary summary,
}) {
  return DictionaryImportResult(
    filePath: filePath,
    sourceName: sourceName,
    totalRecords: summary.totalRecords,
    importedTerms: summary.importedTerms,
    insertedTerms: summary.insertedTerms,
    updatedTerms: summary.updatedTerms,
    skippedDeletedTerms: summary.skippedDeletedTerms,
    skippedDuplicateTerms: summary.skippedDuplicateTerms,
    dryRun: summary.dryRun,
  );
}

DictionaryExportResult managerDictionaryExportResultFromNative({
  required String filePath,
  required NativeDictionaryExportSummary summary,
}) {
  return DictionaryExportResult(
    filePath: filePath,
    exportedTerms: summary.exportedTerms,
    format: managerDictionaryFormatLabel(summary.formatVersion),
    syncClass: managerSyncClassLabel(summary.syncClass),
  );
}

String managerTermSourceLabel(int source) {
  switch (source) {
    case nativeTermSourceEngineSelection:
      return 'selection';
    case nativeTermSourceManualImport:
      return 'import';
    case nativeTermSourceManualAdd:
      return 'manual';
    case nativeTermSourcePhraseLearning:
      return 'phrase_learning';
    default:
      return 'unknown($source)';
  }
}

String managerTermStatusLabel(int status) {
  switch (status) {
    case nativeTermStatusActive:
      return 'active';
    case nativeTermStatusSuppressed:
      return 'suppressed';
    case nativeTermStatusDeleted:
      return 'deleted';
    default:
      return 'unknown($status)';
  }
}

String managerDictionaryFormatLabel(int formatVersion) {
  return formatVersion == nativeDictionaryFormatUserTermsV1
      ? 'dictionary.user_terms.v1'
      : 'unknown($formatVersion)';
}

String managerSyncClassLabel(int syncClass) {
  return syncClass == nativeSyncClassP2EncryptedSync
      ? 'P2 encrypted sync'
      : 'unknown($syncClass)';
}

String managerFormatTimestampMs(int value) {
  final timestamp = DateTime.fromMillisecondsSinceEpoch(value);
  return '${timestamp.year.toString().padLeft(4, '0')}-'
      '${timestamp.month.toString().padLeft(2, '0')}-'
      '${timestamp.day.toString().padLeft(2, '0')} '
      '${timestamp.hour.toString().padLeft(2, '0')}:'
      '${timestamp.minute.toString().padLeft(2, '0')}';
}
