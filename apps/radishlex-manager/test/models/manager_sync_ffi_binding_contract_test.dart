import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_native_contract.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_ffi_command_boundary_fixtures.dart';
import '../fixtures/sync_ffi_rust_host_contract_review_fixtures.dart';
import '../fixtures/sync_ffi_rust_host_migration_review_fixtures.dart';

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

  test('fake native binding replays host gate migration readiness', () {
    final exercisedReplayCaseIds = <String>{};

    for (final replayCase in syncFfiRustHostGateMigrationReplayCases) {
      final native = _FakeSyncCommandNativeBinding(
        result: _fakeNativeResultForGateReplay(replayCase),
      );
      final summary = _executeAndCopySyncCommandSummary(
        native,
        const _FakeSyncCommandRequest(
          actionId: 'recovery_setup',
          operationId: 'op_test_non_secret_gate_replay',
          readinessSnapshotId: 'readiness_snapshot_test_gate_replay',
          sourceTag: 'local_smoke',
          deviceBackendGate: 'blocked',
          explicitUserStart: true,
        ),
      );

      exercisedReplayCaseIds.add(replayCase.id);

      expect(native.events, ['execute:recovery_setup', 'copy:1', 'free:1']);
      expect(native.openHandleCount, 0);
      expect(summary.commandStatus, replayCase.expectedCommandStatus);
      expect(summary.errorCode, replayCase.expectedErrorCode);
      expect(summary.retryPolicy, replayCase.expectedRetryPolicy);
      expect(
        summary.nextRequiredEvidence,
        replayCase.expectedNextRequiredEvidence,
      );
      expect(summary.gateReplayCaseId, replayCase.id);
      expect(summary.gateReadinessState, replayCase.expectedReadinessState);
      expect(summary.gateBlockerCode, replayCase.expectedBlockerCode);

      final diagnostics = summary.diagnosticsText;
      expect(diagnostics, contains(replayCase.id));
      expect(diagnostics, contains(replayCase.expectedBlockerCode));
      expect(diagnostics, contains(replayCase.expectedReadinessState));
      for (final condition in replayCase.simulatedMissingConditions) {
        expect(diagnostics, contains(condition), reason: replayCase.id);
      }
      expect(diagnostics, isNot(contains('settings_action_payload')));
      expect(diagnostics, isNot(contains('bridge_request_payload')));
      expect(diagnostics, isNot(contains('remote_request_body')));
      expect(diagnostics, isNot(contains('remote_response_body')));
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(diagnostics, isNot(contains(fragment)), reason: replayCase.id);
      }
    }

    expect(
      exercisedReplayCaseIds,
      syncFfiRustHostGateMigrationReplayCases
          .map((replayCase) => replayCase.id)
          .toSet(),
    );
  });

  test('fake native binding replays host test admission gaps', () {
    final exercisedGapConditions = <String>{};

    for (final gapItem in syncFfiRustHostTestAdmissionGapItems) {
      final native = _FakeSyncCommandNativeBinding(
        result: _fakeNativeResultForAdmissionGap(gapItem),
      );
      final summary = _executeAndCopySyncCommandSummary(
        native,
        const _FakeSyncCommandRequest(
          actionId: 'recovery_setup',
          operationId: 'op_test_non_secret_admission_gap',
          readinessSnapshotId: 'readiness_snapshot_test_admission_gap',
          sourceTag: 'local_smoke',
          deviceBackendGate: 'blocked',
          explicitUserStart: true,
        ),
      );

      exercisedGapConditions.add(gapItem.condition);

      expect(native.events, ['execute:recovery_setup', 'copy:1', 'free:1']);
      expect(native.openHandleCount, 0);
      expect(summary.commandStatus, 'blocked_by_readiness');
      expect(summary.errorCode, 'unexpected_bridge_error');
      expect(summary.retryPolicy, 'not_retryable');
      expect(summary.nextRequiredEvidence, gapItem.condition);
      expect(summary.objectTypeSummary, 'host_test_admission_review');
      expect(summary.gateReadinessState, syncFfiRustHostGateReadinessState);
      expect(summary.gateBlockerCode, gapItem.blockerCode);
      expect(summary.gateMissingConditionSummary, gapItem.condition);

      final diagnostics = summary.diagnosticsText;
      expect(diagnostics, contains(gapItem.condition));
      expect(diagnostics, contains(gapItem.blockerCode));
      expect(diagnostics, contains(gapItem.requiredDecision));
      expect(diagnostics, contains(gapItem.evidenceSource));
      expect(diagnostics, isNot(contains('settings_action_payload')));
      expect(diagnostics, isNot(contains('bridge_request_payload')));
      expect(diagnostics, isNot(contains('remote_request_body')));
      expect(diagnostics, isNot(contains('remote_response_body')));
      expect(
        diagnostics,
        isNot(contains('radishlex_manager_sync_command_execute_v1')),
      );
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(
          diagnostics,
          isNot(contains(fragment)),
          reason: gapItem.condition,
        );
      }
    }

    expect(
      exercisedGapConditions,
      syncFfiRustHostGateReadinessBlockingConditions.toSet(),
    );
  });

  test(
    'fake native binding replays export binding and bridge migration reviews',
    () {
      final replayCases = [
        const _MigrationReviewReplayCase(
          id: 'native_export_approval_review',
          objectTypeSummary: 'native_export_approval_review',
          blockerCode: 'native_symbol_export_not_approved',
          nextRequiredEvidence: 'native_symbol_export_approved_by_adr',
          expectedDiagnostics: [
            syncFfiRustHostExportApprovalReviewStatus,
            syncFfiRustHostExportApprovalDecision,
            'result_accessor_field_set_review',
            'result_handle_copy_then_free_review',
            'ffi_bridge_smoke_candidate_symbols_absent',
          ],
        ),
        const _MigrationReviewReplayCase(
          id: 'dart_binding_migration_review',
          objectTypeSummary: 'dart_binding_migration_review',
          blockerCode: 'dart_native_binding_not_approved',
          nextRequiredEvidence: 'dart_native_binding_approved',
          expectedDiagnostics: [
            syncFfiRustHostDartBindingMigrationReviewStatus,
            syncFfiRustHostDartBindingMigrationDecision,
            'dart_copy_free_contract_reviewed',
            'unknown_native_status_mapping_reviewed',
            'ffi_command_error_mapping_reviewed',
            'native_symbol_export_approved_by_adr',
          ],
        ),
        const _MigrationReviewReplayCase(
          id: 'manager_bridge_migration_review',
          objectTypeSummary: 'manager_bridge_migration_review',
          blockerCode: 'manager_bridge_command_not_approved',
          nextRequiredEvidence: 'manager_bridge_command_approved',
          expectedDiagnostics: [
            syncFfiRustHostManagerBridgeMigrationReviewStatus,
            syncFfiRustHostManagerBridgeMigrationDecision,
            'action_intent_mapping_reviewed',
            'settings_draft_write_absent_reviewed',
            'manager_bridge_command_approved',
            'host_contract_test_file_approved',
          ],
        ),
        const _MigrationReviewReplayCase(
          id: 'host_test_file_approval_review',
          objectTypeSummary: 'host_test_file_approval_review',
          blockerCode: 'host_contract_test_file_not_approved',
          nextRequiredEvidence: 'host_contract_test_file_approved',
          expectedDiagnostics: [
            syncFfiRustHostTestFileApprovalReviewStatus,
            syncFfiRustHostTestFileApprovalDecision,
            'c_abi_symbol_lookup_strategy_reviewed',
            'result_handle_lifecycle_reviewed',
            'dynamic_library_smoke_absence_reviewed',
            'host_test_design_package_current',
          ],
        ),
        const _MigrationReviewReplayCase(
          id: 'real_sync_execution_gate_review',
          objectTypeSummary: 'real_sync_execution_gate_review',
          blockerCode: 'real_sync_execution_not_approved',
          nextRequiredEvidence: 'real_sync_execution_approved_after_gate',
          expectedDiagnostics: [
            syncFfiRustHostRealSyncExecutionGateReviewStatus,
            syncFfiRustHostRealSyncExecutionDecision,
            'command_context_owner_scope_reviewed',
            'readiness_snapshot_binding_reviewed',
            'platform_private_key_backend_production_ready',
            'deployment_evidence_summary_approved',
          ],
        ),
      ];

      for (final replayCase in replayCases) {
        final native = _FakeSyncCommandNativeBinding(
          result: _fakeNativeResultForMigrationReview(replayCase),
        );
        final summary = _executeAndCopySyncCommandSummary(
          native,
          const _FakeSyncCommandRequest(
            actionId: 'recovery_setup',
            operationId: 'op_test_non_secret_migration_review',
            readinessSnapshotId: 'readiness_snapshot_test_migration_review',
            sourceTag: 'local_smoke',
            deviceBackendGate: 'blocked',
            explicitUserStart: true,
          ),
        );

        expect(native.events, ['execute:recovery_setup', 'copy:1', 'free:1']);
        expect(native.openHandleCount, 0);
        expect(summary.commandStatus, 'blocked_by_readiness');
        expect(summary.errorCode, 'unexpected_bridge_error');
        expect(summary.retryPolicy, 'not_retryable');
        expect(summary.nextRequiredEvidence, replayCase.nextRequiredEvidence);
        expect(summary.objectTypeSummary, replayCase.objectTypeSummary);
        expect(summary.gateReadinessState, syncFfiRustHostGateReadinessState);
        expect(summary.gateBlockerCode, replayCase.blockerCode);
        expect(
          summary.gateMissingConditionSummary,
          replayCase.nextRequiredEvidence,
        );

        final diagnostics = summary.diagnosticsText;
        expect(diagnostics, contains(replayCase.id));
        expect(diagnostics, contains(replayCase.blockerCode));
        for (final evidence in replayCase.expectedDiagnostics) {
          expect(diagnostics, contains(evidence), reason: replayCase.id);
        }
        expect(diagnostics, isNot(contains('settings_action_payload')));
        expect(diagnostics, isNot(contains('bridge_request_payload')));
        expect(diagnostics, isNot(contains('remote_request_body')));
        expect(diagnostics, isNot(contains('remote_response_body')));
        expect(
          diagnostics,
          isNot(contains('radishlex_manager_sync_command_execute_v1')),
        );
        for (final fragment in syncBridgeCommandContractForbiddenFragments) {
          expect(diagnostics, isNot(contains(fragment)), reason: replayCase.id);
        }
      }
    },
  );
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
    this.gateReplayCaseId = 'none',
    this.gateReadinessState = 'none',
    this.gateBlockerCode = 'none',
    this.gateMissingConditionSummary = 'none',
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
  final String gateReplayCaseId;
  final String gateReadinessState;
  final String gateBlockerCode;
  final String gateMissingConditionSummary;
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
    required this.gateReplayCaseId,
    required this.gateReadinessState,
    required this.gateBlockerCode,
    required this.gateMissingConditionSummary,
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
        gateReplayCaseId: _safeGateReplayCaseId(result.gateReplayCaseId),
        gateReadinessState: _safeGateReadinessState(result.gateReadinessState),
        gateBlockerCode: _safeSummaryCode(result.gateBlockerCode),
        gateMissingConditionSummary: _safeSummaryCode(
          result.gateMissingConditionSummary,
        ),
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
      gateReplayCaseId: _safeGateReplayCaseId(result.gateReplayCaseId),
      gateReadinessState: _safeGateReadinessState(result.gateReadinessState),
      gateBlockerCode: _safeSummaryCode(result.gateBlockerCode),
      gateMissingConditionSummary: _safeSummaryCode(
        result.gateMissingConditionSummary,
      ),
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
  final String gateReplayCaseId;
  final String gateReadinessState;
  final String gateBlockerCode;
  final String gateMissingConditionSummary;

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
      'gate_replay_case_id': gateReplayCaseId,
      'gate_readiness_state': gateReadinessState,
      'gate_blocker_code': gateBlockerCode,
      'gate_missing_condition_summary': gateMissingConditionSummary,
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

_FakeNativeSyncCommandResult _fakeNativeResultForGateReplay(
  SyncFfiRustHostGateMigrationReplayCase replayCase,
) {
  return _FakeNativeSyncCommandResult(
    actionId: 'recovery_setup',
    commandStatus: replayCase.expectedCommandStatus,
    errorCode: replayCase.expectedErrorCode,
    retryPolicy: replayCase.expectedRetryPolicy,
    userVisibleSummaryCode: replayCase.expectedErrorCode,
    diagnosticsSummaryCode: replayCase.expectedErrorCode,
    nextRequiredEvidence: replayCase.expectedNextRequiredEvidence,
    objectTypeSummary: 'host_gate_readiness_review',
    objectCountSummary: '0',
    objectVersionSummary: replayCase.missingConditionSummary,
    recordedAtSummary: 'not_recorded',
    nativeDebugText: syncBridgeCommandContractForbiddenFragments.join(' '),
    abiStatusCode: 'InvalidState',
    abiInputCase: 'sync_command_not_enabled_current_phase',
    gateReplayCaseId: replayCase.id,
    gateReadinessState: replayCase.expectedReadinessState,
    gateBlockerCode: replayCase.expectedBlockerCode,
    gateMissingConditionSummary: replayCase.missingConditionSummary,
  );
}

_FakeNativeSyncCommandResult _fakeNativeResultForAdmissionGap(
  SyncFfiRustHostTestAdmissionGapItem gapItem,
) {
  return _FakeNativeSyncCommandResult(
    actionId: 'recovery_setup',
    commandStatus: 'blocked_by_readiness',
    errorCode: 'unexpected_bridge_error',
    retryPolicy: 'not_retryable',
    userVisibleSummaryCode: 'unexpected_bridge_error',
    diagnosticsSummaryCode: 'unexpected_bridge_error',
    nextRequiredEvidence: gapItem.condition,
    objectTypeSummary: 'host_test_admission_review',
    objectCountSummary: '0',
    objectVersionSummary:
        '${gapItem.requiredDecision}|${gapItem.evidenceSource}',
    recordedAtSummary: 'not_recorded',
    nativeDebugText: syncBridgeCommandContractForbiddenFragments.join(' '),
    abiStatusCode: 'InvalidState',
    abiInputCase: 'sync_command_not_enabled_current_phase',
    gateReadinessState: syncFfiRustHostGateReadinessState,
    gateBlockerCode: gapItem.blockerCode,
    gateMissingConditionSummary: gapItem.condition,
  );
}

_FakeNativeSyncCommandResult _fakeNativeResultForMigrationReview(
  _MigrationReviewReplayCase replayCase,
) {
  return _FakeNativeSyncCommandResult(
    actionId: 'recovery_setup',
    commandStatus: 'blocked_by_readiness',
    errorCode: 'unexpected_bridge_error',
    retryPolicy: 'not_retryable',
    userVisibleSummaryCode: 'unexpected_bridge_error',
    diagnosticsSummaryCode: 'unexpected_bridge_error',
    nextRequiredEvidence: replayCase.nextRequiredEvidence,
    objectTypeSummary: replayCase.objectTypeSummary,
    objectCountSummary: '0',
    objectVersionSummary: replayCase.expectedDiagnostics.join('|'),
    recordedAtSummary: 'not_recorded',
    nativeDebugText: syncBridgeCommandContractForbiddenFragments.join(' '),
    abiStatusCode: 'InvalidState',
    abiInputCase: replayCase.id,
    gateReadinessState: syncFfiRustHostGateReadinessState,
    gateBlockerCode: replayCase.blockerCode,
    gateMissingConditionSummary: replayCase.nextRequiredEvidence,
  );
}

final class _MigrationReviewReplayCase {
  const _MigrationReviewReplayCase({
    required this.id,
    required this.objectTypeSummary,
    required this.blockerCode,
    required this.nextRequiredEvidence,
    required this.expectedDiagnostics,
  });

  final String id;
  final String objectTypeSummary;
  final String blockerCode;
  final String nextRequiredEvidence;
  final List<String> expectedDiagnostics;
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

String _safeGateReplayCaseId(String value) {
  if (value == 'none') {
    return value;
  }
  final knownReplayIds = {
    for (final replayCase in syncFfiRustHostGateMigrationReplayCases)
      replayCase.id,
  };
  return knownReplayIds.contains(value) ? value : 'unknown_gate_replay_case';
}

String _safeGateReadinessState(String value) {
  return value == syncFfiRustHostGateReadinessState
      ? value
      : 'host_gate_blocked_no_native_symbol';
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
