import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_native_sync.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_qualification_mapper.dart';
import 'package:radishlex_manager/src/models/manager_sync_qualification_models.dart';

void main() {
  test('maps a completed fixed qualification snapshot', () {
    final snapshot = managerSyncQualificationFromNative(
      _snapshot(state: 4, phase: 12, conflicts: 1, convergenceRounds: 2),
    );

    expect(snapshot.state, ManagerSyncQualificationState.completed);
    expect(snapshot.phase, ManagerSyncQualificationPhase.complete);
    expect(snapshot.temporaryFilesCleaned, isTrue);
    expect(snapshot.transientInputsCleared, isTrue);
  });

  test('fails closed on unknown enums and non-canonical flags', () {
    expect(
      () => managerSyncQualificationFromNative(_snapshot(state: 99)),
      throwsA(isA<Exception>()),
    );
    expect(
      () => managerSyncQualificationFromNative(
        _snapshot(
          state: 4,
          phase: 12,
          conflicts: 1,
          convergenceRounds: 2,
          transientInputsCleared: 2,
        ),
      ),
      throwsA(isA<Exception>()),
    );
  });

  test('rejects false success without conflict, convergence, or cleanup', () {
    expect(
      () => managerSyncQualificationFromNative(_snapshot(state: 4, phase: 12)),
      throwsA(isA<Exception>()),
    );
  });
}

NativeSyncQualificationSnapshot _snapshot({
  int state = 2,
  int phase = 3,
  int conflicts = 0,
  int convergenceRounds = 0,
  int temporaryFilesCleaned = 1,
  int workerStopped = 1,
  int transientInputsCleared = 1,
}) {
  final terminal = state == 4 || state == 5 || state == 6;
  return NativeSyncQualificationSnapshot(
    version: 1,
    state: state,
    phase: phase,
    discovered: 3,
    downloaded: 3,
    applied: 3,
    uploaded: 3,
    conflicts: conflicts,
    retries: 1,
    convergenceRounds: convergenceRounds,
    temporaryFilesCleaned: terminal ? temporaryFilesCleaned : 0,
    workerStopped: terminal ? workerStopped : 0,
    transientInputsCleared: terminal ? transientInputsCleared : 0,
    errorCode: state == 5 ? 255 : (state == 6 ? 12 : 0),
    errorPhase: state == 5 || state == 6 ? phase : 0,
    errorRetryable: 0,
  );
}
