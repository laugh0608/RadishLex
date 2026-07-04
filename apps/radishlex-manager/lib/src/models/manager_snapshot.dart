import 'manager_dictionary_models.dart';
import 'manager_learning_models.dart';
import 'manager_settings_models.dart';
import 'manager_sync_models.dart';

class ManagerSnapshot {
  const ManagerSnapshot({
    required this.generatedAt,
    required this.dictionaryTerms,
    required this.deletedTerms,
    required this.importBatches,
    required this.learningSummary,
    required this.explanations,
    required this.sync,
    required this.settings,
  });

  final String generatedAt;
  final List<UserTerm> dictionaryTerms;
  final List<DeletedTerm> deletedTerms;
  final List<DictionaryImportBatchSummary> importBatches;
  final LearningSummary learningSummary;
  final List<RankerExplanation> explanations;
  final SyncPreflightSummary sync;
  final ManagerSettings settings;

  ManagerSnapshot copyWith({
    String? generatedAt,
    List<UserTerm>? dictionaryTerms,
    List<DeletedTerm>? deletedTerms,
    List<DictionaryImportBatchSummary>? importBatches,
    LearningSummary? learningSummary,
    List<RankerExplanation>? explanations,
    SyncPreflightSummary? sync,
    ManagerSettings? settings,
  }) {
    return ManagerSnapshot(
      generatedAt: generatedAt ?? this.generatedAt,
      dictionaryTerms: dictionaryTerms ?? this.dictionaryTerms,
      deletedTerms: deletedTerms ?? this.deletedTerms,
      importBatches: importBatches ?? this.importBatches,
      learningSummary: learningSummary ?? this.learningSummary,
      explanations: explanations ?? this.explanations,
      sync: sync ?? this.sync,
      settings: settings ?? this.settings,
    );
  }
}
