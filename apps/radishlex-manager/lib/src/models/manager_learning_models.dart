class LearningSummary {
  const LearningSummary({
    required this.userTerms,
    required this.deletedTerms,
    required this.selectionEvents,
    required this.suppressedTerms,
    required this.lastUpdated,
  });

  final int userTerms;
  final int deletedTerms;
  final int selectionEvents;
  final int suppressedTerms;
  final String lastUpdated;

  LearningSummary copyWith({
    int? userTerms,
    int? deletedTerms,
    int? selectionEvents,
    int? suppressedTerms,
    String? lastUpdated,
  }) {
    return LearningSummary(
      userTerms: userTerms ?? this.userTerms,
      deletedTerms: deletedTerms ?? this.deletedTerms,
      selectionEvents: selectionEvents ?? this.selectionEvents,
      suppressedTerms: suppressedTerms ?? this.suppressedTerms,
      lastUpdated: lastUpdated ?? this.lastUpdated,
    );
  }
}

class RankerExplanation {
  const RankerExplanation({
    required this.inputCode,
    required this.candidate,
    required this.score,
    required this.signals,
  });

  final String inputCode;
  final String candidate;
  final double score;
  final List<String> signals;
}
