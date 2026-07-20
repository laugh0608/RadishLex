const nativeTermSourceEngineSelection = 1;
const nativeTermSourceManualImport = 2;
const nativeTermSourceManualAdd = 3;
const nativeTermSourcePhraseLearning = 4;
const nativeTermStatusActive = 1;
const nativeTermStatusSuppressed = 2;
const nativeTermStatusDeleted = 3;
const nativeDictionaryFormatUserTermsV1 = 1;
const nativeSyncClassP2EncryptedSync = 2;

final class NativeUserTermRecord {
  const NativeUserTermRecord({
    required this.id,
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.source,
    required this.status,
    required this.weight,
    required this.createdAtMs,
    required this.updatedAtMs,
    required this.lastUsedAtMs,
    required this.lastUsedAtPresent,
    required this.importBatchId,
    required this.importBatchIdPresent,
  });

  final int id;
  final String inputCode;
  final String text;
  final String? reading;
  final int source;
  final int status;
  final double weight;
  final int createdAtMs;
  final int updatedAtMs;
  final int lastUsedAtMs;
  final bool lastUsedAtPresent;
  final int importBatchId;
  final bool importBatchIdPresent;
}

final class NativeDeletedTermRecord {
  const NativeDeletedTermRecord({
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.deletedAtMs,
    required this.reason,
  });

  final String inputCode;
  final String text;
  final String? reading;
  final int deletedAtMs;
  final String reason;
}

final class NativeDictionaryInspectSummary {
  const NativeDictionaryInspectSummary({
    required this.formatVersion,
    required this.recordCount,
    required this.syncClass,
  });

  final int formatVersion;
  final int recordCount;
  final int syncClass;
}

final class NativeDictionaryImportSummary {
  const NativeDictionaryImportSummary({
    required this.importBatchId,
    required this.importBatchIdPresent,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.dryRun,
  });

  final int importBatchId;
  final bool importBatchIdPresent;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final bool dryRun;
}

final class NativeDictionaryExportSummary {
  const NativeDictionaryExportSummary({
    required this.formatVersion,
    required this.exportedTerms,
    required this.syncClass,
  });

  final int formatVersion;
  final int exportedTerms;
  final int syncClass;
}

final class NativeImportBatchRecord {
  const NativeImportBatchRecord({
    required this.id,
    required this.sourceName,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.createdAtMs,
    required this.notes,
    required this.notesPresent,
  });

  final int id;
  final String sourceName;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final int createdAtMs;
  final String? notes;
  final bool notesPresent;
}
