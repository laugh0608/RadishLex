import '../models/manager_sync_qualification_models.dart';
import 'ffi_manager_native_contract.dart';
import 'ffi_manager_native_sync.dart';

ManagerSyncQualificationSnapshot managerSyncQualificationFromNative(
  NativeSyncQualificationSnapshot native,
) {
  if (native.version != 1 ||
      !_allNonNegative(native) ||
      !_canonicalFlag(native.temporaryFilesCleaned) ||
      !_canonicalFlag(native.workerStopped) ||
      !_canonicalFlag(native.transientInputsCleared) ||
      !_canonicalFlag(native.errorRetryable)) {
    throw _invalidSnapshot();
  }
  final state = _state(native.state);
  final phase = _phase(native.phase);
  final errorCode = _errorCode(native.errorCode);
  final errorPhase = native.errorCode == 0 ? null : _phase(native.errorPhase);
  final terminal = state.isTerminal;
  if (terminal != (native.workerStopped == 1) ||
      terminal != (native.transientInputsCleared == 1) ||
      (state == ManagerSyncQualificationState.completed &&
          (phase != ManagerSyncQualificationPhase.complete ||
              errorCode != ManagerSyncQualificationErrorCode.none ||
              native.temporaryFilesCleaned != 1 ||
              native.conflicts == 0 ||
              native.convergenceRounds != 2)) ||
      (state == ManagerSyncQualificationState.failed &&
          errorCode == ManagerSyncQualificationErrorCode.none) ||
      (state == ManagerSyncQualificationState.cancelled &&
          errorCode != ManagerSyncQualificationErrorCode.cancelled) ||
      (!terminal && errorCode != ManagerSyncQualificationErrorCode.none) ||
      (errorCode == ManagerSyncQualificationErrorCode.none &&
          (native.errorPhase != 0 || native.errorRetryable != 0))) {
    throw _invalidSnapshot();
  }
  return ManagerSyncQualificationSnapshot(
    state: state,
    phase: phase,
    discovered: native.discovered,
    downloaded: native.downloaded,
    applied: native.applied,
    uploaded: native.uploaded,
    conflicts: native.conflicts,
    retries: native.retries,
    convergenceRounds: native.convergenceRounds,
    temporaryFilesCleaned: native.temporaryFilesCleaned == 1,
    workerStopped: native.workerStopped == 1,
    transientInputsCleared: native.transientInputsCleared == 1,
    errorCode: errorCode,
    errorPhase: errorPhase,
    errorRetryable: native.errorRetryable == 1,
  );
}

ManagerSyncQualificationState _state(int value) => switch (value) {
  1 => ManagerSyncQualificationState.created,
  2 => ManagerSyncQualificationState.running,
  3 => ManagerSyncQualificationState.cancelling,
  4 => ManagerSyncQualificationState.completed,
  5 => ManagerSyncQualificationState.failed,
  6 => ManagerSyncQualificationState.cancelled,
  _ => throw _invalidSnapshot(),
};

ManagerSyncQualificationPhase _phase(int value) => switch (value) {
  1 => ManagerSyncQualificationPhase.created,
  2 => ManagerSyncQualificationPhase.validateRequest,
  3 => ManagerSyncQualificationPhase.prepareWorkspace,
  4 => ManagerSyncQualificationPhase.createDomain,
  5 => ManagerSyncQualificationPhase.authorizeSecondDevice,
  6 => ManagerSyncQualificationPhase.clientAFirstSync,
  7 => ManagerSyncQualificationPhase.prepareConflict,
  8 => ManagerSyncQualificationPhase.clientBMerge,
  9 => ManagerSyncQualificationPhase.clientAConflictRecovery,
  10 => ManagerSyncQualificationPhase.verifyConvergence,
  11 => ManagerSyncQualificationPhase.cleanup,
  12 => ManagerSyncQualificationPhase.complete,
  _ => throw _invalidSnapshot(),
};

ManagerSyncQualificationErrorCode _errorCode(int value) => switch (value) {
  0 => ManagerSyncQualificationErrorCode.none,
  1 => ManagerSyncQualificationErrorCode.invalidRequest,
  2 => ManagerSyncQualificationErrorCode.alreadyRunning,
  3 => ManagerSyncQualificationErrorCode.unauthenticated,
  4 => ManagerSyncQualificationErrorCode.tlsRejected,
  5 => ManagerSyncQualificationErrorCode.transportTimeout,
  6 => ManagerSyncQualificationErrorCode.serverUnavailable,
  7 => ManagerSyncQualificationErrorCode.protocolRejected,
  8 => ManagerSyncQualificationErrorCode.cryptoRejected,
  9 => ManagerSyncQualificationErrorCode.localStorageFailed,
  10 => ManagerSyncQualificationErrorCode.conflictNotObserved,
  11 => ManagerSyncQualificationErrorCode.convergenceFailed,
  12 => ManagerSyncQualificationErrorCode.cancelled,
  255 => ManagerSyncQualificationErrorCode.internal,
  _ => throw _invalidSnapshot(),
};

bool _canonicalFlag(int value) => value == 0 || value == 1;

bool _allNonNegative(NativeSyncQualificationSnapshot value) =>
    value.discovered >= 0 &&
    value.downloaded >= 0 &&
    value.applied >= 0 &&
    value.uploaded >= 0 &&
    value.conflicts >= 0 &&
    value.retries >= 0 &&
    value.convergenceRounds >= 0;

FfiManagerBridgeException _invalidSnapshot() => const FfiManagerBridgeException(
  statusCode: 2,
  code: 'sync_qualification_snapshot_invalid',
  message: 'native sync qualification snapshot is invalid',
);
