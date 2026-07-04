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
