import '../models/manager_models.dart';
import 'ffi_manager_dictionary_mapper.dart';
import 'ffi_manager_learning_mapper.dart';
import 'ffi_manager_native_models.dart';
import 'ffi_manager_runtime_diagnostics.dart';
import 'ffi_manager_sync_mapper.dart';

ManagerSnapshot managerSnapshotFromNative({
  required int generatedAtMs,
  required Iterable<NativeUserTermRecord> nativeTerms,
  required Iterable<NativeDeletedTermRecord> nativeDeletedTerms,
  required Iterable<NativeImportBatchRecord> nativeImportBatches,
  required NativeLearningStatusSummary nativeLearning,
  required NativeSyncPreflightSummary nativeSync,
  required NativeSyncProductStatus nativeSyncProductStatus,
  required ManagerSettingsDraft settingsDraft,
  required ManagerRuntimeDiagnostics runtimeDiagnostics,
  required NativeRankExplainSummary Function(UserTerm term, String contextKind)
  explainTerm,
}) {
  final terms = nativeTerms
      .map(managerUserTermFromNative)
      .toList(growable: false);

  final device = managerDeviceSecuritySummaryFromNative(
    nativeSyncProductStatus,
  );

  return ManagerSnapshot(
    generatedAt: managerFormatTimestampMs(generatedAtMs),
    dictionaryTerms: terms,
    deletedTerms: nativeDeletedTerms
        .map(managerDeletedTermFromNative)
        .toList(growable: false),
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
      device: device,
    ),
    settings: managerSettingsFromFfiDraft(
      draft: settingsDraft,
      runtimeDiagnostics: runtimeDiagnostics,
    ),
  );
}
