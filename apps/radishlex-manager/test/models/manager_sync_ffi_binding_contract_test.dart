import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_native_contract.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_ffi_command_boundary_fixtures.dart';

void main() {
  test('fake sync command binding copies summary before release', () {
    final fixture = syncBridgeCommandContractFixtureFor('recovery_setup');
    final native = _FakeSyncCommandNativeBinding(
      result: _FakeNativeSyncCommandResult(
        actionId: fixture.actionId,
        commandStatus: 'blocked_by_readiness',
        errorCode: fixture.errorCodes.first,
        retryPolicy: 'retry_after_user_action',
        userVisibleSummaryCode: 'configuration_missing',
        diagnosticsSummaryCode: 'configuration_missing',
        nextRequiredEvidence: 'deployment_evidence_summary.v1',
        objectTypeSummary: 'recovery_record',
        objectCountSummary: '0',
        objectVersionSummary: 'none',
        recordedAtSummary: 'not_recorded',
        nativeDebugText: syncBridgeCommandContractForbiddenFragments.join(' '),
      ),
    );

    final summary = _executeAndCopySyncCommandSummary(
      native,
      const _FakeSyncCommandRequest(
        actionId: 'recovery_setup',
        operationId: 'op_test_non_secret_001',
        readinessSnapshotId: 'readiness_snapshot_test_001',
        sourceTag: 'local_smoke',
        deviceBackendGate: 'blocked',
        explicitUserStart: true,
      ),
    );

    expect(native.events, ['execute:recovery_setup', 'copy:1', 'free:1']);
    expect(native.openHandleCount, 0);
    expect(summary.actionId, fixture.actionId);
    expect(summary.commandStatus, 'blocked_by_readiness');
    expect(summary.errorCode, fixture.errorCodes.first);
    expect(summary.diagnosticsText, contains('deployment_evidence_summary.v1'));
    expect(summary.diagnosticsText, isNot(contains('handle')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(summary.diagnosticsText, isNot(contains(fragment)));
    }
  });

  test('unknown native status downgrades to current phase safe blocker', () {
    final native = _FakeSyncCommandNativeBinding(
      result: const _FakeNativeSyncCommandResult(
        actionId: 'device_revocation',
        commandStatus: 'native_provider_exception_raw:secret-token',
        errorCode: 'native_raw_error_code',
        retryPolicy: 'retry_from_native_debug',
        userVisibleSummaryCode: 'raw_user_message',
        diagnosticsSummaryCode: 'provider_exception_raw',
        nextRequiredEvidence: 'raw_next_step',
        objectTypeSummary: 'raw_object_type',
        objectCountSummary: '99',
        objectVersionSummary: 'raw_version',
        recordedAtSummary: 'raw_timestamp',
        nativeDebugText: 'Bearer secret-token response_body',
      ),
    );

    final summary = _executeAndCopySyncCommandSummary(
      native,
      const _FakeSyncCommandRequest(
        actionId: 'device_revocation',
        operationId: 'op_test_non_secret_002',
        readinessSnapshotId: 'readiness_snapshot_test_002',
        sourceTag: 'local_smoke',
        deviceBackendGate: 'blocked',
        explicitUserStart: true,
      ),
    );

    expect(summary.commandStatus, 'unexpected_bridge_error');
    expect(summary.errorCode, 'unexpected_bridge_error');
    expect(summary.retryPolicy, 'not_retryable');
    expect(
      summary.nextRequiredEvidence,
      'user_sync_entry_closed_current_phase',
    );
    expect(summary.commandErrorCategory, 'future_action_error_allowlist');
    expect(summary.diagnosticsText, isNot(contains('native_provider')));
    expect(summary.diagnosticsText, isNot(contains('provider_exception_raw')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(summary.diagnosticsText, isNot(contains(fragment)));
    }
  });

  test('ffi and command errors map to stable bridge categories', () {
    final cases = [
      _BridgeFailureCategoryCase(
        error: const _TestBridgeFailure(code: 'ffi_library_load_failed'),
        operation: ManagerBridgeOperation.previewDiagnostics,
        expectedCategoryCode: 'native_library',
      ),
      _BridgeFailureCategoryCase(
        error: const FfiManagerBridgeException(
          statusCode: 1,
          code: 'invalid_argument',
          message: 'invalid request input omitted',
        ),
        operation: ManagerBridgeOperation.loadSnapshot,
        expectedCategoryCode: 'invalid_input',
      ),
      _BridgeFailureCategoryCase(
        error: const FfiManagerBridgeException(
          statusCode: 6,
          code: 'sync_error',
          message: 'sync command failed without payload details',
        ),
        operation: ManagerBridgeOperation.loadSnapshot,
        expectedCategoryCode: 'sync_preflight',
      ),
      _BridgeFailureCategoryCase(
        error: StateError('native raw detail must not become a failure code'),
        operation: ManagerBridgeOperation.loadSnapshot,
        expectedCategoryCode: 'manager_bridge',
      ),
    ];

    for (final failureCase in cases) {
      final presentation = describeManagerBridgeFailure(
        failureCase.error,
        failureCase.operation,
      );

      expect(
        presentation.categoryCode,
        failureCase.expectedCategoryCode,
        reason: failureCase.expectedCategoryCode,
      );
      expect(presentation.userMessage, isNot(contains('secret-token')));
      expect(presentation.userMessage, isNot(contains('response_body')));
    }

    final fixture = syncBridgeCommandContractFixtureFor('device_revocation');
    final commandError = _DartOwnedSyncCommandSummary.fromNative(
      _FakeNativeSyncCommandResult(
        actionId: fixture.actionId,
        commandStatus: 'blocked_by_readiness',
        errorCode: fixture.errorCodes.first,
        retryPolicy: 'retry_after_user_action',
        userVisibleSummaryCode: fixture.errorCodes.first,
        diagnosticsSummaryCode: fixture.errorCodes.first,
        nextRequiredEvidence: 'device_backend_gate',
        objectTypeSummary: 'device_record',
        objectCountSummary: '1',
        objectVersionSummary: 'none',
        recordedAtSummary: 'not_recorded',
        nativeDebugText: 'signature_bytes response_body',
      ),
    );

    expect(commandError.commandErrorCategory, 'future_action_error_allowlist');
    expect(
      syncFfiCommandBoundaryAllCommandErrorCodes(),
      contains(commandError.errorCode),
    );
    expect(commandError.diagnosticsText, isNot(contains('signature_bytes')));
    expect(commandError.diagnosticsText, isNot(contains('response_body')));
  });

  test('fake native binding replays rust host catalog status evidence', () {
    final abiStatusByInput = {
      for (final fixture in syncFfiCommandBoundaryAbiStatusCases)
        fixture.input: fixture.statusCode,
    };
    final exercisedSampleIds = <String>{};
    final exercisedContractCaseIds = <String>{};

    for (final sample in syncFfiCommandBoundaryRustHostInputSamples) {
      final contractCases = syncFfiCommandBoundaryRustHostContractCases
          .where((fixture) => fixture.sampleIds.contains(sample.id))
          .toList(growable: false);

      expect(contractCases, isNotEmpty, reason: sample.id);
      expect(
        abiStatusByInput[sample.abiInputCase],
        sample.expectedStatusCode,
        reason: sample.id,
      );

      for (final contractCase in contractCases) {
        final native = _FakeSyncCommandNativeBinding(
          result: _fakeNativeResultForHostCatalog(sample, contractCase),
        );
        final summary = _executeAndCopySyncCommandSummary(
          native,
          _FakeSyncCommandRequest.fromHostSample(sample),
        );

        exercisedSampleIds.add(sample.id);
        exercisedContractCaseIds.add(contractCase.id);

        expect(native.events, [
          'execute:${sample.actionId}',
          'copy:1',
          'free:1',
        ]);
        expect(native.openHandleCount, 0);
        expect(summary.actionId, sample.actionId);
        expect(summary.abiStatusCode, sample.expectedStatusCode);
        expect(summary.abiInputCase, sample.abiInputCase);
        expect(summary.hostSampleId, sample.id);
        expect(summary.hostContractCaseId, contractCase.id);
        expect(
          summary.commandStatus,
          _commandStatusForAbiStatus(sample.expectedStatusCode),
        );
        expect(
          contractCase.requiredAbiInputCases,
          contains(sample.abiInputCase),
          reason: contractCase.id,
        );
        expect(
          contractCase.expectedStatusCodes,
          contains(sample.expectedStatusCode),
          reason: contractCase.id,
        );
        expect(
          syncFfiCommandBoundaryAllCommandErrorCodes(),
          contains(summary.errorCode),
        );
        expect(summary.commandErrorCategory, 'future_action_error_allowlist');

        final diagnostics = summary.diagnosticsText;
        expect(diagnostics, contains(sample.abiInputCase));
        expect(diagnostics, contains(sample.expectedStatusCode));
        expect(diagnostics, contains(contractCase.id));
        for (final evidence in sample.expectedEvidence) {
          expect(diagnostics, contains(evidence), reason: sample.id);
        }
        for (final evidence in contractCase.requiredEvidence) {
          expect(diagnostics, contains(evidence), reason: contractCase.id);
        }
        for (final fragment in syncBridgeCommandContractForbiddenFragments) {
          expect(diagnostics, isNot(contains(fragment)), reason: sample.id);
        }
      }
    }

    expect(
      exercisedSampleIds,
      syncFfiCommandBoundaryRustHostInputSamples
          .map((sample) => sample.id)
          .toSet(),
    );
    expect(
      exercisedContractCaseIds,
      syncFfiCommandBoundaryRustHostContractCaseIds().toSet(),
    );
  });
}

_DartOwnedSyncCommandSummary _executeAndCopySyncCommandSummary(
  _FakeSyncCommandNativeBinding native,
  _FakeSyncCommandRequest request,
) {
  final handle = native.execute(request);
  try {
    final nativeResult = native.copySummary(handle);
    return _DartOwnedSyncCommandSummary.fromNative(nativeResult);
  } finally {
    native.freeResult(handle);
  }
}

final class _FakeSyncCommandRequest {
  const _FakeSyncCommandRequest({
    required this.actionId,
    required this.operationId,
    required this.readinessSnapshotId,
    required this.sourceTag,
    required this.deviceBackendGate,
    required this.explicitUserStart,
    this.abiInputCase = 'none',
    this.hostSampleId = 'none',
  });

  factory _FakeSyncCommandRequest.fromHostSample(
    SyncFfiCommandBoundaryRustHostInputSample sample,
  ) {
    return _FakeSyncCommandRequest(
      actionId: sample.actionId,
      operationId: sample.operationId,
      readinessSnapshotId: sample.readinessSnapshotId,
      sourceTag: sample.sourceTag,
      deviceBackendGate: sample.deviceBackendGate,
      explicitUserStart: sample.explicitUserStart,
      abiInputCase: sample.abiInputCase,
      hostSampleId: sample.id,
    );
  }

  final String actionId;
  final String operationId;
  final String readinessSnapshotId;
  final String sourceTag;
  final String deviceBackendGate;
  final bool explicitUserStart;
  final String abiInputCase;
  final String hostSampleId;
}

final class _FakeNativeSyncCommandResult {
  const _FakeNativeSyncCommandResult({
    required this.actionId,
    required this.commandStatus,
    required this.errorCode,
    required this.retryPolicy,
    required this.userVisibleSummaryCode,
    required this.diagnosticsSummaryCode,
    required this.nextRequiredEvidence,
    required this.objectTypeSummary,
    required this.objectCountSummary,
    required this.objectVersionSummary,
    required this.recordedAtSummary,
    required this.nativeDebugText,
    this.abiStatusCode = 'not_recorded',
    this.abiInputCase = 'none',
    this.hostSampleId = 'none',
    this.hostContractCaseId = 'none',
  });

  final String actionId;
  final String commandStatus;
  final String errorCode;
  final String retryPolicy;
  final String userVisibleSummaryCode;
  final String diagnosticsSummaryCode;
  final String nextRequiredEvidence;
  final String objectTypeSummary;
  final String objectCountSummary;
  final String objectVersionSummary;
  final String recordedAtSummary;
  final String nativeDebugText;
  final String abiStatusCode;
  final String abiInputCase;
  final String hostSampleId;
  final String hostContractCaseId;
}

final class _FakeSyncCommandNativeBinding {
  _FakeSyncCommandNativeBinding({required this.result});

  final _FakeNativeSyncCommandResult result;
  final events = <String>[];
  final _openHandles = <int, _FakeNativeSyncCommandResult>{};
  int _nextHandle = 1;

  int execute(_FakeSyncCommandRequest request) {
    events.add('execute:${request.actionId}');
    final handle = _nextHandle;
    _nextHandle += 1;
    _openHandles[handle] = result;
    return handle;
  }

  _FakeNativeSyncCommandResult copySummary(int handle) {
    events.add('copy:$handle');
    final result = _openHandles[handle];
    if (result == null) {
      throw StateError('native result handle is not open');
    }
    return result;
  }

  void freeResult(int handle) {
    events.add('free:$handle');
    _openHandles.remove(handle);
  }

  int get openHandleCount => _openHandles.length;
}

final class _DartOwnedSyncCommandSummary {
  _DartOwnedSyncCommandSummary({
    required this.actionId,
    required this.commandStatus,
    required this.errorCode,
    required this.retryPolicy,
    required this.userVisibleSummaryCode,
    required this.diagnosticsSummaryCode,
    required this.nextRequiredEvidence,
    required this.objectTypeSummary,
    required this.objectCountSummary,
    required this.objectVersionSummary,
    required this.recordedAtSummary,
    required this.abiStatusCode,
    required this.abiInputCase,
    required this.hostSampleId,
    required this.hostContractCaseId,
  });

  factory _DartOwnedSyncCommandSummary.fromNative(
    _FakeNativeSyncCommandResult result,
  ) {
    final commandStatus = _safeCommandStatus(result.commandStatus);
    if (commandStatus == 'unexpected_bridge_error') {
      return _DartOwnedSyncCommandSummary(
        actionId: _safeActionId(result.actionId),
        commandStatus: commandStatus,
        errorCode: 'unexpected_bridge_error',
        retryPolicy: 'not_retryable',
        userVisibleSummaryCode: 'unexpected_bridge_error',
        diagnosticsSummaryCode: 'unexpected_bridge_error',
        nextRequiredEvidence: 'user_sync_entry_closed_current_phase',
        objectTypeSummary: 'none',
        objectCountSummary: 'none',
        objectVersionSummary: 'none',
        recordedAtSummary: 'none',
        abiStatusCode: _safeAbiStatusCode(result.abiStatusCode),
        abiInputCase: _safeAbiInputCase(result.abiInputCase),
        hostSampleId: _safeHostSampleId(result.hostSampleId),
        hostContractCaseId: _safeHostContractCaseId(result.hostContractCaseId),
      );
    }

    return _DartOwnedSyncCommandSummary(
      actionId: _safeActionId(result.actionId),
      commandStatus: commandStatus,
      errorCode: _safeCommandErrorCode(result.errorCode),
      retryPolicy: _safeRetryPolicy(result.retryPolicy),
      userVisibleSummaryCode: _safeCommandErrorCode(
        result.userVisibleSummaryCode,
      ),
      diagnosticsSummaryCode: _safeCommandErrorCode(
        result.diagnosticsSummaryCode,
      ),
      nextRequiredEvidence: _safeSummaryCode(result.nextRequiredEvidence),
      objectTypeSummary: _safeSummaryCode(result.objectTypeSummary),
      objectCountSummary: _safeSummaryCode(result.objectCountSummary),
      objectVersionSummary: _safeSummaryCode(result.objectVersionSummary),
      recordedAtSummary: _safeSummaryCode(result.recordedAtSummary),
      abiStatusCode: _safeAbiStatusCode(result.abiStatusCode),
      abiInputCase: _safeAbiInputCase(result.abiInputCase),
      hostSampleId: _safeHostSampleId(result.hostSampleId),
      hostContractCaseId: _safeHostContractCaseId(result.hostContractCaseId),
    );
  }

  final String actionId;
  final String commandStatus;
  final String errorCode;
  final String retryPolicy;
  final String userVisibleSummaryCode;
  final String diagnosticsSummaryCode;
  final String nextRequiredEvidence;
  final String objectTypeSummary;
  final String objectCountSummary;
  final String objectVersionSummary;
  final String recordedAtSummary;
  final String abiStatusCode;
  final String abiInputCase;
  final String hostSampleId;
  final String hostContractCaseId;

  String get commandErrorCategory {
    return syncFfiCommandBoundaryAllCommandErrorCodes().contains(errorCode)
        ? 'future_action_error_allowlist'
        : 'unexpected_bridge_error';
  }

  String get diagnosticsText {
    return jsonEncode({
      'action_id': actionId,
      'command_status': commandStatus,
      'error_code': errorCode,
      'retry_policy': retryPolicy,
      'user_visible_summary_code': userVisibleSummaryCode,
      'diagnostics_summary_code': diagnosticsSummaryCode,
      'next_required_evidence': nextRequiredEvidence,
      'object_type_summary': objectTypeSummary,
      'object_count_summary': objectCountSummary,
      'object_version_summary': objectVersionSummary,
      'recorded_at_summary': recordedAtSummary,
      'abi_status_code': abiStatusCode,
      'abi_input_case': abiInputCase,
      'host_sample_id': hostSampleId,
      'host_contract_case_id': hostContractCaseId,
    });
  }
}

_FakeNativeSyncCommandResult _fakeNativeResultForHostCatalog(
  SyncFfiCommandBoundaryRustHostInputSample sample,
  SyncFfiCommandBoundaryRustHostContractCase contractCase,
) {
  final actionFixture = syncBridgeCommandContractFixtureFor(sample.actionId);
  return _FakeNativeSyncCommandResult(
    actionId: sample.actionId,
    commandStatus: _commandStatusForAbiStatus(sample.expectedStatusCode),
    errorCode: _errorCodeForAbiStatus(sample.expectedStatusCode, actionFixture),
    retryPolicy: _retryPolicyForAbiStatus(sample.expectedStatusCode),
    userVisibleSummaryCode: actionFixture.errorCodes.first,
    diagnosticsSummaryCode: actionFixture.errorCodes.first,
    nextRequiredEvidence: sample.expectedEvidenceSummary,
    objectTypeSummary: sample.abiInputCase,
    objectCountSummary: '0',
    objectVersionSummary: contractCase.evidenceSummary,
    recordedAtSummary: 'not_recorded',
    nativeDebugText: syncBridgeCommandContractForbiddenFragments.join(' '),
    abiStatusCode: sample.expectedStatusCode,
    abiInputCase: sample.abiInputCase,
    hostSampleId: sample.id,
    hostContractCaseId: contractCase.id,
  );
}

String _commandStatusForAbiStatus(String statusCode) {
  switch (statusCode) {
    case 'InvalidState':
      return 'blocked_by_readiness';
    case 'InvalidArgument':
    case 'InternalError':
      return 'fatal_failure';
    case 'SyncError':
      return 'retryable_failure';
    default:
      return 'unexpected_bridge_error';
  }
}

String _errorCodeForAbiStatus(
  String statusCode,
  SyncBridgeCommandContractActionFixture actionFixture,
) {
  if (statusCode == 'SyncError' &&
      actionFixture.errorCodes.contains('network_unreachable')) {
    return 'network_unreachable';
  }
  if (statusCode == 'InvalidState' &&
      actionFixture.errorCodes.contains('backend_unavailable')) {
    return 'backend_unavailable';
  }
  return actionFixture.errorCodes.first;
}

String _retryPolicyForAbiStatus(String statusCode) {
  return statusCode == 'SyncError'
      ? 'retry_after_network_recovery'
      : 'not_retryable';
}

String _safeActionId(String value) {
  return syncBridgeCommandContractActionFixtures.any(
        (fixture) => fixture.actionId == value,
      )
      ? value
      : 'unknown_action';
}

String _safeCommandStatus(String value) {
  return syncBridgeCommandContractCommandStatuses.contains(value)
      ? value
      : 'unexpected_bridge_error';
}

String _safeCommandErrorCode(String value) {
  return syncFfiCommandBoundaryAllCommandErrorCodes().contains(value)
      ? value
      : 'unexpected_bridge_error';
}

String _safeRetryPolicy(String value) {
  return syncBridgeCommandContractRetryPolicies.contains(value)
      ? value
      : 'not_retryable';
}

String _safeSummaryCode(String value) {
  for (final fragment in syncBridgeCommandContractForbiddenFragments) {
    if (value.contains(fragment)) {
      return 'redacted';
    }
  }
  return value.trim().isEmpty ? 'none' : value;
}

String _safeAbiStatusCode(String value) {
  if (value == 'not_recorded') {
    return value;
  }
  final knownStatusCodes = {
    for (final fixture in syncFfiCommandBoundaryAbiStatusCases)
      fixture.statusCode,
  };
  return knownStatusCodes.contains(value) ? value : 'InternalError';
}

String _safeAbiInputCase(String value) {
  if (value == 'none') {
    return value;
  }
  final knownInputs = {
    for (final fixture in syncFfiCommandBoundaryAbiStatusCases) fixture.input,
  };
  return knownInputs.contains(value) ? value : 'unknown_input';
}

String _safeHostSampleId(String value) {
  if (value == 'none') {
    return value;
  }
  final knownSampleIds = {
    for (final sample in syncFfiCommandBoundaryRustHostInputSamples) sample.id,
  };
  return knownSampleIds.contains(value) ? value : 'unknown_sample';
}

String _safeHostContractCaseId(String value) {
  if (value == 'none') {
    return value;
  }
  final knownCaseIds = syncFfiCommandBoundaryRustHostContractCaseIds().toSet();
  return knownCaseIds.contains(value) ? value : 'unknown_contract_case';
}

final class _BridgeFailureCategoryCase {
  const _BridgeFailureCategoryCase({
    required this.error,
    required this.operation,
    required this.expectedCategoryCode,
  });

  final Object error;
  final ManagerBridgeOperation operation;
  final String expectedCategoryCode;
}

final class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure({required this.code});

  @override
  int get statusCode => 2;

  @override
  final String code;

  @override
  String get message => 'native detail omitted';
}
