const nativeTermSourceEngineSelection = 1;
const nativeTermSourceManualImport = 2;
const nativeTermSourceManualAdd = 3;
const nativeTermSourcePhraseLearning = 4;
const nativeDictionaryFormatUserTermsV1 = 1;
const nativeSyncClassP2EncryptedSync = 2;

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

  NativeSyncPreflightSummary syncPreflight(String dbPath);
}

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

final class NativeLearningStatusSummary {
  const NativeLearningStatusSummary({
    required this.schemaVersion,
    required this.plaintextPayload,
    required this.p1RawDetails,
    required this.contextStats,
    required this.activeUserTerms,
    required this.suppressedUserTerms,
    required this.rankerWeights,
    required this.deletedTermTombstones,
    required this.selectionEvents,
    required this.negativeFeedback,
    required this.importBatches,
    required this.latestUserTermUpdatedAtMs,
    required this.latestUserTermUpdatedAtPresent,
    required this.latestSelectionEventAtMs,
    required this.latestSelectionEventAtPresent,
    required this.latestNegativeFeedbackAtMs,
    required this.latestNegativeFeedbackAtPresent,
    required this.latestDeletedTermAtMs,
    required this.latestDeletedTermAtPresent,
    required this.latestImportBatchAtMs,
    required this.latestImportBatchAtPresent,
    required this.latestActivityAtMs,
    required this.latestActivityAtPresent,
  });

  final int schemaVersion;
  final bool plaintextPayload;
  final bool p1RawDetails;
  final bool contextStats;
  final int activeUserTerms;
  final int suppressedUserTerms;
  final int rankerWeights;
  final int deletedTermTombstones;
  final int selectionEvents;
  final int negativeFeedback;
  final int importBatches;
  final int latestUserTermUpdatedAtMs;
  final bool latestUserTermUpdatedAtPresent;
  final int latestSelectionEventAtMs;
  final bool latestSelectionEventAtPresent;
  final int latestNegativeFeedbackAtMs;
  final bool latestNegativeFeedbackAtPresent;
  final int latestDeletedTermAtMs;
  final bool latestDeletedTermAtPresent;
  final int latestImportBatchAtMs;
  final bool latestImportBatchAtPresent;
  final int latestActivityAtMs;
  final bool latestActivityAtPresent;
}

final class NativeSyncPreflightSummary {
  const NativeSyncPreflightSummary({
    required this.schemaVersion,
    required this.plaintextPayload,
    required this.syncableUserTerms,
    required this.syncableRankerWeights,
    required this.syncableDeletedTerms,
    required this.localSelectionEvents,
    required this.localNegativeFeedback,
    required this.localImportBatches,
  });

  final int schemaVersion;
  final bool plaintextPayload;
  final int syncableUserTerms;
  final int syncableRankerWeights;
  final int syncableDeletedTerms;
  final int localSelectionEvents;
  final int localNegativeFeedback;
  final int localImportBatches;
}

class FfiManagerBridgeException implements Exception {
  const FfiManagerBridgeException({
    required this.statusCode,
    required this.code,
    required this.message,
  });

  final int statusCode;
  final String code;
  final String message;

  @override
  String toString() => 'FfiManagerBridgeException($code): $message';
}
