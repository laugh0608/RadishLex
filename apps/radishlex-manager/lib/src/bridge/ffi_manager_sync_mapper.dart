import '../models/manager_models.dart';
import 'ffi_manager_native_models.dart';

const ffiManagerDeviceSecuritySummary = DeviceSecuritySummary(
  deviceId: 'local-manager',
  backendId: 'unavailable',
  capabilityStatus: 'platform_private_key_backend_unavailable',
  productionGate: 'blocked',
);

const _invalidNativeSyncProductStatus = DeviceSecuritySummary(
  deviceId: 'local-manager',
  backendId: 'unavailable',
  capabilityStatus: 'native_sync_product_status_invalid',
  productionGate: 'blocked',
);

DeviceSecuritySummary managerDeviceSecuritySummaryFromNative(
  NativeSyncProductStatus status,
) {
  if (!_validNativeSyncProductStatus(status)) {
    return _invalidNativeSyncProductStatus;
  }

  final backendId = switch (status.signingBackend) {
    0 => 'unavailable',
    1 => 'apple-secure-enclave-p256-v1',
    _ => 'unavailable',
  };
  final capabilityStatus = switch (status.blocker) {
    0 => 'product_backend_ready_but_sync_entry_closed',
    1 => 'signing_backend_not_compiled',
    2 => 'signing_backend_unavailable',
    3 => 'signing_backend_product_qualification_required',
    4 => 'key_agreement_backend_not_compiled',
    5 => 'key_agreement_runtime_qualification_required',
    6 => 'key_agreement_backend_product_qualification_required',
    7 => 'user_sync_closed_current_phase',
    _ => 'native_sync_product_status_invalid',
  };

  return DeviceSecuritySummary(
    deviceId: 'local-manager',
    backendId: backendId,
    capabilityStatus: capabilityStatus,
    // Status schema v1 is explanatory only. It cannot open a product command.
    productionGate: 'blocked',
  );
}

bool _validNativeSyncProductStatus(NativeSyncProductStatus status) {
  if (status.version != 1 ||
      !_allFlags([
        status.signingCompiled,
        status.signingRuntimeAvailable,
        status.signingCanCreate,
        status.signingCanSign,
        status.signingExportable,
        status.signingHardwareBacked,
        status.signingUserPresenceRequired,
        status.signingBackupMigratable,
        status.signingProductQualified,
        status.keyAgreementCompiled,
        status.keyAgreementRuntimeQualified,
        status.keyAgreementProductQualified,
        status.productQualified,
        status.userSyncEnabled,
      ])) {
    return false;
  }
  if (!_validSigningIdentity(status) || !_validKeyAgreementIdentity(status)) {
    return false;
  }
  if (status.signingProductQualified == 1 &&
      (status.signingCompiled != 1 ||
          status.signingRuntimeAvailable != 1 ||
          status.signingCanCreate != 1 ||
          status.signingCanSign != 1 ||
          status.signingExportable != 0 ||
          status.signingHardwareBacked != 1 ||
          status.signingUserPresenceRequired != 0 ||
          status.signingBackupMigratable != 0)) {
    return false;
  }
  if (status.keyAgreementProductQualified == 1 &&
      (status.keyAgreementCompiled != 1 ||
          status.keyAgreementRuntimeQualified != 1)) {
    return false;
  }
  if (status.productQualified !=
      status.signingProductQualified * status.keyAgreementProductQualified) {
    return false;
  }
  // The current product stage has no executable sync entry. A native library
  // claiming otherwise is incompatible with this Manager contract.
  if (status.userSyncEnabled != 0) {
    return false;
  }
  return status.blocker == _expectedProductBlocker(status);
}

bool _validSigningIdentity(NativeSyncProductStatus status) {
  return switch (status.signingBackend) {
    0 =>
      status.signingAlgorithm == 0 &&
          status.signingCompiled == 0 &&
          status.signingRuntimeAvailable == 0 &&
          status.signingCanCreate == 0 &&
          status.signingCanSign == 0 &&
          status.signingExportable == 0 &&
          status.signingHardwareBacked == 0 &&
          status.signingUserPresenceRequired == 0 &&
          status.signingBackupMigratable == 0 &&
          status.signingProductQualified == 0,
    1 => status.signingAlgorithm == 1,
    _ => false,
  };
}

bool _validKeyAgreementIdentity(NativeSyncProductStatus status) {
  return switch (status.keyAgreementBackend) {
    0 =>
      status.keyAgreementCompiled == 0 &&
          status.keyAgreementRuntimeQualified == 0 &&
          status.keyAgreementProductQualified == 0,
    1 => true,
    _ => false,
  };
}

bool _allFlags(Iterable<int> values) {
  return values.every((value) => value == 0 || value == 1);
}

int _expectedProductBlocker(NativeSyncProductStatus status) {
  if (status.signingCompiled == 0) return 1;
  if (status.signingRuntimeAvailable == 0 ||
      status.signingCanCreate == 0 ||
      status.signingCanSign == 0) {
    return 2;
  }
  if (status.signingProductQualified == 0) return 3;
  if (status.keyAgreementCompiled == 0) return 4;
  if (status.keyAgreementRuntimeQualified == 0) return 5;
  if (status.keyAgreementProductQualified == 0) return 6;
  return 7;
}

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
