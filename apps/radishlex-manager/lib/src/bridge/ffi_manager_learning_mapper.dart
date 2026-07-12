import '../models/manager_models.dart';
import 'ffi_manager_dictionary_mapper.dart';
import 'ffi_manager_native_models.dart';

LearningSummary managerLearningSummaryFromNative(
  NativeLearningStatusSummary summary,
) {
  return LearningSummary(
    userTerms: summary.activeUserTerms,
    deletedTerms: summary.deletedTermTombstones,
    selectionEvents: summary.selectionEvents,
    suppressedTerms: summary.suppressedUserTerms,
    lastUpdated: summary.latestActivityAtPresent
        ? managerFormatTimestampMs(summary.latestActivityAtMs)
        : '无记录',
  );
}

List<RankerExplanation> managerRankerExplanationSummaries({
  required Iterable<UserTerm> terms,
  required NativeRankExplainSummary Function(UserTerm term) explainTerm,
}) {
  return terms
      .take(6)
      .map((term) => managerRankerExplanationFromNative(explainTerm(term)))
      .toList(growable: false);
}

RankerExplanation managerRankerExplanationFromNative(
  NativeRankExplainSummary explanation,
) {
  return RankerExplanation(
    inputCode: explanation.inputCode,
    candidate: explanation.candidateText,
    score: explanation.finalScore,
    signals: managerRankExplainSignals(explanation),
  );
}

String? managerRankExplainReading(UserTerm term) {
  return term.reading.trim().isEmpty ? null : term.reading;
}

List<String> managerRankExplainSignals(NativeRankExplainSummary explanation) {
  return [
    _rankSignal('engine', explanation.engineOrderFactor),
    _rankSignal('user', explanation.userTermBoost),
    _rankSignal('freq', explanation.frequencyBoost),
    _rankSignal('recent', explanation.recencyBoost),
    _rankSignal('context', explanation.contextBoost),
    _rankSignal('negative', explanation.negativeFeedbackPenalty),
    _rankSignal('suppressed', explanation.suppressedPenalty),
    _rankSignal('deleted', explanation.deletedPenalty),
  ];
}

String _rankSignal(String name, double value) {
  return '$name=${value.toStringAsFixed(3)}';
}
