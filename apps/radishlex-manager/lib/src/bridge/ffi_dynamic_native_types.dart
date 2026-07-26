part of 'ffi_dynamic_native_binding.dart';

final class _RadishLexError extends ffi.Opaque {}

final class _RadishLexUserTermList extends ffi.Opaque {}

final class _RadishLexDeletedTermList extends ffi.Opaque {}

final class _RadishLexImportBatchList extends ffi.Opaque {}

final class _RadishLexRankExplain extends ffi.Opaque {}

final class _RadishLexManagerSyncQualificationRun extends ffi.Opaque {}

final class _RadishLexFfiContract extends ffi.Struct {
  @ffi.Uint32()
  external int version;

  @ffi.Uint32()
  external int sessionThreadPolicy;

  @ffi.Uint32()
  external int panicBoundary;
}

final class _RadishLexStringView extends ffi.Struct {
  external ffi.Pointer<ffi.Uint8> data;

  @ffi.Size()
  external int len;
}

final class _RadishLexUserTermView extends ffi.Struct {
  @ffi.Int64()
  external int id;

  external _RadishLexStringView inputCode;
  external _RadishLexStringView text;
  external _RadishLexStringView reading;

  @ffi.Uint8()
  external int readingPresent;

  @ffi.Uint32()
  external int source;

  @ffi.Uint32()
  external int status;

  @ffi.Double()
  external double weight;

  @ffi.Int64()
  external int createdAtMs;

  @ffi.Int64()
  external int updatedAtMs;

  @ffi.Int64()
  external int lastUsedAtMs;

  @ffi.Uint8()
  external int lastUsedAtPresent;

  @ffi.Int64()
  external int importBatchId;

  @ffi.Uint8()
  external int importBatchIdPresent;
}

final class _RadishLexDeletedTermView extends ffi.Struct {
  external _RadishLexStringView inputCode;
  external _RadishLexStringView text;
  external _RadishLexStringView reading;

  @ffi.Uint8()
  external int readingPresent;

  @ffi.Int64()
  external int deletedAtMs;

  external _RadishLexStringView reason;
}

final class _RadishLexDictionaryInspectSummary extends ffi.Struct {
  @ffi.Uint32()
  external int formatVersion;

  @ffi.Size()
  external int recordCount;

  @ffi.Uint32()
  external int syncClass;
}

final class _RadishLexDictionaryExportSummary extends ffi.Struct {
  @ffi.Uint32()
  external int formatVersion;

  @ffi.Size()
  external int exportedTerms;

  @ffi.Uint32()
  external int syncClass;
}

final class _RadishLexDictionaryImportSummary extends ffi.Struct {
  @ffi.Int64()
  external int importBatchId;

  @ffi.Uint8()
  external int importBatchIdPresent;

  @ffi.Size()
  external int totalRecords;

  @ffi.Size()
  external int importedTerms;

  @ffi.Size()
  external int insertedTerms;

  @ffi.Size()
  external int updatedTerms;

  @ffi.Size()
  external int skippedDeletedTerms;

  @ffi.Size()
  external int skippedDuplicateTerms;

  @ffi.Uint8()
  external int dryRun;
}

final class _RadishLexImportBatchView extends ffi.Struct {
  @ffi.Int64()
  external int id;

  external _RadishLexStringView sourceName;

  @ffi.Size()
  external int totalRecords;

  @ffi.Size()
  external int importedTerms;

  @ffi.Size()
  external int insertedTerms;

  @ffi.Size()
  external int updatedTerms;

  @ffi.Size()
  external int skippedDeletedTerms;

  @ffi.Size()
  external int skippedDuplicateTerms;

  @ffi.Int64()
  external int createdAtMs;

  external _RadishLexStringView notes;

  @ffi.Uint8()
  external int notesPresent;
}

final class _RadishLexLearningStatusSummary extends ffi.Struct {
  @ffi.Int64()
  external int schemaVersion;

  @ffi.Uint8()
  external int plaintextPayload;

  @ffi.Uint8()
  external int p1RawDetails;

  @ffi.Uint8()
  external int contextStats;

  @ffi.Size()
  external int activeUserTerms;

  @ffi.Size()
  external int suppressedUserTerms;

  @ffi.Size()
  external int rankerWeights;

  @ffi.Size()
  external int deletedTermTombstones;

  @ffi.Size()
  external int selectionEvents;

  @ffi.Size()
  external int negativeFeedback;

  @ffi.Size()
  external int importBatches;

  @ffi.Int64()
  external int latestUserTermUpdatedAtMs;

  @ffi.Uint8()
  external int latestUserTermUpdatedAtPresent;

  @ffi.Int64()
  external int latestSelectionEventAtMs;

  @ffi.Uint8()
  external int latestSelectionEventAtPresent;

  @ffi.Int64()
  external int latestNegativeFeedbackAtMs;

  @ffi.Uint8()
  external int latestNegativeFeedbackAtPresent;

  @ffi.Int64()
  external int latestDeletedTermAtMs;

  @ffi.Uint8()
  external int latestDeletedTermAtPresent;

  @ffi.Int64()
  external int latestImportBatchAtMs;

  @ffi.Uint8()
  external int latestImportBatchAtPresent;

  @ffi.Int64()
  external int latestActivityAtMs;

  @ffi.Uint8()
  external int latestActivityAtPresent;
}

final class _RadishLexSyncPreflightSummary extends ffi.Struct {
  @ffi.Int64()
  external int schemaVersion;

  @ffi.Uint8()
  external int plaintextPayload;

  @ffi.Size()
  external int syncableUserTerms;

  @ffi.Size()
  external int syncableRankerWeights;

  @ffi.Size()
  external int syncableDeletedTerms;

  @ffi.Size()
  external int localSelectionEvents;

  @ffi.Size()
  external int localNegativeFeedback;

  @ffi.Size()
  external int localImportBatches;
}

final class _RadishLexManagerSyncProductStatus extends ffi.Struct {
  @ffi.Uint32()
  external int version;

  @ffi.Uint32()
  external int signingBackend;

  @ffi.Uint32()
  external int signingAlgorithm;

  @ffi.Uint32()
  external int signingCompiled;

  @ffi.Uint32()
  external int signingRuntimeAvailable;

  @ffi.Uint32()
  external int signingCanCreate;

  @ffi.Uint32()
  external int signingCanSign;

  @ffi.Uint32()
  external int signingExportable;

  @ffi.Uint32()
  external int signingHardwareBacked;

  @ffi.Uint32()
  external int signingUserPresenceRequired;

  @ffi.Uint32()
  external int signingBackupMigratable;

  @ffi.Uint32()
  external int signingProductQualified;

  @ffi.Uint32()
  external int keyAgreementBackend;

  @ffi.Uint32()
  external int keyAgreementCompiled;

  @ffi.Uint32()
  external int keyAgreementRuntimeQualified;

  @ffi.Uint32()
  external int keyAgreementProductQualified;

  @ffi.Uint32()
  external int productQualified;

  @ffi.Uint32()
  external int userSyncEnabled;

  @ffi.Uint32()
  external int blocker;
}

final class _RadishLexManagerSyncQualificationRequest extends ffi.Struct {
  @ffi.Uint32()
  external int version;

  external ffi.Pointer<ffi.Char> endpoint;
  external ffi.Pointer<ffi.Uint8> accessTokenData;

  @ffi.Size()
  external int accessTokenLen;

  external ffi.Pointer<ffi.Uint8> localCaDerData;

  @ffi.Size()
  external int localCaDerLen;

  @ffi.Uint64()
  external int timeoutMs;
}

final class _RadishLexManagerSyncQualificationSnapshot extends ffi.Struct {
  @ffi.Uint32()
  external int version;

  @ffi.Uint32()
  external int state;

  @ffi.Uint32()
  external int phase;

  @ffi.Uint64()
  external int discovered;

  @ffi.Uint64()
  external int downloaded;

  @ffi.Uint64()
  external int applied;

  @ffi.Uint64()
  external int uploaded;

  @ffi.Uint64()
  external int conflicts;

  @ffi.Uint64()
  external int retries;

  @ffi.Uint64()
  external int convergenceRounds;

  @ffi.Uint32()
  external int temporaryFilesCleaned;

  @ffi.Uint32()
  external int workerStopped;

  @ffi.Uint32()
  external int transientInputsCleared;

  @ffi.Uint32()
  external int errorCode;

  @ffi.Uint32()
  external int errorPhase;

  @ffi.Uint32()
  external int errorRetryable;
}

final class _RadishLexRankExplainView extends ffi.Struct {
  external _RadishLexStringView inputCode;
  external _RadishLexStringView candidateText;
  external _RadishLexStringView reading;

  @ffi.Uint8()
  external int readingPresent;

  external _RadishLexStringView contextKind;

  @ffi.Size()
  external int originalIndex;

  @ffi.Double()
  external double finalScore;

  @ffi.Double()
  external double engineOrderFactor;

  @ffi.Double()
  external double userTermBoost;

  @ffi.Double()
  external double frequencyBoost;

  @ffi.Double()
  external double recencyBoost;

  @ffi.Double()
  external double contextBoost;

  @ffi.Double()
  external double negativeFeedbackPenalty;

  @ffi.Double()
  external double suppressedPenalty;

  @ffi.Double()
  external double deletedPenalty;
}
