import '../models/manager_models.dart';
import 'ffi_manager_native_models.dart';

const ffiManagerDeviceSecuritySummary = DeviceSecuritySummary(
  deviceId: 'local-manager',
  backendId: 'unavailable',
  capabilityStatus: 'platform_private_key_backend_unavailable',
  productionGate: 'blocked',
);

SyncPreflightSummary managerSyncSummaryFromNative({
  required NativeSyncPreflightSummary summary,
  required ManagerSettingsDraft settingsDraft,
  DeviceSecuritySummary device = ffiManagerDeviceSecuritySummary,
}) {
  final syncableObjects =
      summary.syncableUserTerms +
      summary.syncableRankerWeights +
      summary.syncableDeletedTerms;
  final localOnlyEvents =
      summary.localSelectionEvents +
      summary.localNegativeFeedback +
      summary.localImportBatches;
  final state = deriveManagerSyncUiState(draft: settingsDraft, device: device);

  return SyncPreflightSummary(
    state: state,
    serverEndpoint: managerSyncEndpointLabel(settingsDraft),
    reason: managerSyncGateReason(
      state: state,
      draft: settingsDraft,
      device: device,
    ),
    syncableObjects: syncableObjects,
    localOnlyEvents: localOnlyEvents,
    lastUpload: '未启用',
    lastDownload: '未启用',
    categories: [
      SyncCategorySummary(
        name: 'dictionary.user_terms',
        count: summary.syncableUserTerms,
      ),
      SyncCategorySummary(
        name: 'dictionary.deleted_terms',
        count: summary.syncableDeletedTerms,
      ),
      SyncCategorySummary(
        name: 'ranker.weights',
        count: summary.syncableRankerWeights,
      ),
      SyncCategorySummary(
        name: 'learning.selection_events',
        count: summary.localSelectionEvents,
      ),
      SyncCategorySummary(
        name: 'learning.negative_feedback',
        count: summary.localNegativeFeedback,
      ),
      SyncCategorySummary(
        name: 'dictionary.import_batches',
        count: summary.localImportBatches,
      ),
    ],
    device: device,
  );
}
