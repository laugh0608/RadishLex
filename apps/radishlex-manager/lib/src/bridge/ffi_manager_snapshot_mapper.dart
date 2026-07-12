import '../models/manager_models.dart';
import 'ffi_manager_dictionary_mapper.dart';
import 'ffi_manager_learning_mapper.dart';
import 'ffi_manager_native_models.dart';
import 'ffi_manager_runtime_diagnostics.dart';
import 'ffi_manager_sync_mapper.dart';

ManagerSnapshot managerSnapshotFromNative({
  required int generatedAtMs,
  required Iterable<NativeUserTermRecord> nativeTerms,
  required Iterable<NativeImportBatchRecord> nativeImportBatches,
  required NativeLearningStatusSummary nativeLearning,
  required NativeSyncPreflightSummary nativeSync,
  required ManagerSettingsDraft settingsDraft,
  required ManagerRuntimeDiagnostics runtimeDiagnostics,
  required NativeRankExplainSummary Function(UserTerm term) explainTerm,
}) {
  final terms = nativeTerms
      .map(managerUserTermFromNative)
      .toList(growable: false);

  return ManagerSnapshot(
    generatedAt: managerFormatTimestampMs(generatedAtMs),
    dictionaryTerms: terms,
    deletedTerms: const [],
    importBatches: nativeImportBatches
        .map(managerImportBatchFromNative)
        .toList(growable: false),
    learningSummary: managerLearningSummaryFromNative(nativeLearning),
    explanations: managerRankerExplanationSummaries(
      terms: terms,
      explainTerm: explainTerm,
    ),
    sync: managerSyncSummaryFromNative(
      summary: nativeSync,
      settingsDraft: settingsDraft,
    ),
    settings: managerSettingsFromFfiDraft(
      draft: settingsDraft,
      runtimeDiagnostics: runtimeDiagnostics,
    ),
  );
}
