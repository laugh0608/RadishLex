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
