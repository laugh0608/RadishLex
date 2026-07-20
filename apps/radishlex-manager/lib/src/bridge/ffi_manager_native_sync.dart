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

final class NativeSyncProductStatus {
  const NativeSyncProductStatus({
    required this.version,
    required this.signingBackend,
    required this.signingAlgorithm,
    required this.signingCompiled,
    required this.signingRuntimeAvailable,
    required this.signingCanCreate,
    required this.signingCanSign,
    required this.signingExportable,
    required this.signingHardwareBacked,
    required this.signingUserPresenceRequired,
    required this.signingBackupMigratable,
    required this.signingProductQualified,
    required this.keyAgreementBackend,
    required this.keyAgreementCompiled,
    required this.keyAgreementRuntimeQualified,
    required this.keyAgreementProductQualified,
    required this.productQualified,
    required this.userSyncEnabled,
    required this.blocker,
  });

  final int version;
  final int signingBackend;
  final int signingAlgorithm;
  final int signingCompiled;
  final int signingRuntimeAvailable;
  final int signingCanCreate;
  final int signingCanSign;
  final int signingExportable;
  final int signingHardwareBacked;
  final int signingUserPresenceRequired;
  final int signingBackupMigratable;
  final int signingProductQualified;
  final int keyAgreementBackend;
  final int keyAgreementCompiled;
  final int keyAgreementRuntimeQualified;
  final int keyAgreementProductQualified;
  final int productQualified;
  final int userSyncEnabled;
  final int blocker;
}

abstract interface class NativeSyncQualificationRun {
  NativeSyncQualificationSnapshot poll();

  bool cancel();

  void dispose();
}

final class NativeSyncQualificationSnapshot {
  const NativeSyncQualificationSnapshot({
    required this.version,
    required this.state,
    required this.phase,
    required this.discovered,
    required this.downloaded,
    required this.applied,
    required this.uploaded,
    required this.conflicts,
    required this.retries,
    required this.convergenceRounds,
    required this.temporaryFilesCleaned,
    required this.workerStopped,
    required this.transientInputsCleared,
    required this.errorCode,
    required this.errorPhase,
    required this.errorRetryable,
  });

  final int version;
  final int state;
  final int phase;
  final int discovered;
  final int downloaded;
  final int applied;
  final int uploaded;
  final int conflicts;
  final int retries;
  final int convergenceRounds;
  final int temporaryFilesCleaned;
  final int workerStopped;
  final int transientInputsCleared;
  final int errorCode;
  final int errorPhase;
  final int errorRetryable;
}
