final class NativeRankExplainSummary {
  const NativeRankExplainSummary({
    required this.inputCode,
    required this.candidateText,
    required this.reading,
    required this.readingPresent,
    required this.contextKind,
    required this.originalIndex,
    required this.finalScore,
    required this.engineOrderFactor,
    required this.userTermBoost,
    required this.frequencyBoost,
    required this.recencyBoost,
    required this.contextBoost,
    required this.negativeFeedbackPenalty,
    required this.suppressedPenalty,
    required this.deletedPenalty,
  });

  final String inputCode;
  final String candidateText;
  final String? reading;
  final bool readingPresent;
  final String contextKind;
  final int originalIndex;
  final double finalScore;
  final double engineOrderFactor;
  final double userTermBoost;
  final double frequencyBoost;
  final double recencyBoost;
  final double contextBoost;
  final double negativeFeedbackPenalty;
  final double suppressedPenalty;
  final double deletedPenalty;
}
