import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  const readyDevice = DeviceSecuritySummary(
    deviceId: 'device-ready-01',
    backendId: 'test-production-ready',
    capabilityStatus: 'ready_for_test',
    productionGate: 'ready',
  );

  test('action command preview maps readiness scenario catalog', () {
    for (final scenario in syncReadinessScenarioCatalog()) {
      final snapshot = managerSnapshotForSyncReadinessScenario(scenario);
      final gate = deriveManagerSyncEntryGate(
        draft: snapshot.settings.draft,
        device: snapshot.sync.device,
        readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
      );

      _expectActionCommandPreviewPlan(
        gate.interactionEntryPlan.actionCommandPreviewPlan,
        expectedIntentStatusSummary: scenario.expectedInteractionStatuses,
        expectedBlockerSummary: scenario.expectedInteractionBlockers,
        expectedRequestBoundarySummary:
            scenario.expectedActionCommandRequestBoundaries,
        expectedResultBoundarySummary:
            scenario.expectedActionCommandResultBoundaries,
        expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
        expectedRequestAllowedFieldSummary:
            scenario.expectedActionRequestAllowedFields,
        expectedResultAllowedFieldSummary:
            scenario.expectedActionResultAllowedFields,
        expectedForbiddenMaterialSummary:
            scenario.expectedActionForbiddenMaterials,
      );
    }
  });

  test('action command preview maps evidence bundle catalog', () {
    for (final scenario in syncEvidenceBundleScenarioCatalog()) {
      final snapshot = managerSnapshotForSyncEvidenceBundleScenario(scenario);
      final gate = deriveManagerSyncEntryGate(
        draft: snapshot.settings.draft,
        device: snapshot.sync.device,
        readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
      );

      _expectActionCommandPreviewPlan(
        gate.interactionEntryPlan.actionCommandPreviewPlan,
        expectedIntentStatusSummary: scenario.expectedInteractionStatuses,
        expectedBlockerSummary: scenario.expectedInteractionBlockers,
        expectedRequestBoundarySummary:
            scenario.expectedActionCommandRequestBoundaries,
        expectedResultBoundarySummary:
            scenario.expectedActionCommandResultBoundaries,
        expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
        expectedRequestAllowedFieldSummary:
            scenario.expectedActionRequestAllowedFields,
        expectedResultAllowedFieldSummary:
            scenario.expectedActionResultAllowedFields,
        expectedForbiddenMaterialSummary:
            scenario.expectedActionForbiddenMaterials,
      );
    }
  });

  test('action command preview reports blocked future action intent', () {
    final readiness = managerSyncReadinessBridgeSnapshotFromJson({
      'format': managerSyncReadinessBridgeSummaryFormat,
      'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
      'source': 'ffi_native_readiness',
      'recovery_setup': {
        'status': 'recovery_setup_blocked',
        'blocker': 'backend_unavailable',
        'entry_action_status': 'blocked_by_backend',
        'required_prerequisites': ['platform_private_key_backend_ready'],
        'error_codes': ['backend_unavailable'],
      },
    });

    final gate = deriveManagerSyncEntryGate(
      draft: const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        accessTokenConfigured: true,
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      ),
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );

    final intent = gate.interactionEntryPlan.intentFor('recovery_setup');
    expect(intent.visibilityStatus, 'visible');
    expect(intent.intentStatus, 'blocked');
    expect(intent.blocker, 'backend_unavailable');
    expect(
      intent.requiredEvidenceSummary,
      'platform_private_key_backend_ready',
    );

    final command = gate.interactionEntryPlan.actionCommandPreviewPlan
        .previewFor('recovery_setup');
    expect(command.intentStatus, 'blocked');
    expect(command.executionStatus, 'blocked_by_readiness');
    expect(command.blocker, 'backend_unavailable');
    expect(
      command.requiredEvidenceSummary,
      'platform_private_key_backend_ready',
    );
    expect(command.dataPolicy, 'no_recovery_code_or_wrapped_material');
    expect(command.stopLine, 'no_recovery_code_generation_current_phase');
    expect(
      command.requestBoundary,
      'request_summary_only_no_recovery_code_generation',
    );
    expect(
      command.resultBoundary,
      'result_summary_only_no_recovery_record_or_wrapped_material',
    );
    expect(
      command.errorCodeSummary,
      'configuration_missing, backend_unavailable, deployment_unverified, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
    );
    expect(
      command.requestPreview.requestStatus,
      'request_blocked_by_readiness',
    );
    expect(
      command.requestPreview.allowedFieldSummary,
      'deployment_evidence_source_tag, device_backend_gate, explicit_user_start, save_confirmation_status',
    );
    expect(command.resultPreview.resultStatus, 'result_blocked_by_readiness');
    expect(
      command.resultPreview.allowedFieldSummary,
      'status_code, recovery_record_status, first_upload_gate, next_required_evidence',
    );
    expect(
      command.requestPreview.forbiddenMaterialSummary,
      syncActionForbiddenMaterialSummary,
    );
  });

  test('action command preview supports confirmation-only future intent', () {
    final plan = managerSyncInteractionEntryPlanFromReadinessFlows(
      readinessFlows: const [
        SyncReadinessFlowSummary(
          flowId: 'recovery_setup',
          status: 'recovery_setup_blocked',
          blocker: 'none',
          actionStatus: 'available',
          requiredEvidenceCodes: ['blocked_until_recovery_code_saved'],
          errorCodes: [],
          blocksUserSync: true,
          sourceTag: 'unit_test_readiness',
        ),
      ],
      userSyncEnabled: true,
    );

    final intent = plan.intentFor('recovery_setup');
    expect(intent.visibilityStatus, 'visible');
    expect(intent.intentStatus, 'requires_confirmation');
    expect(intent.blocker, 'user_confirmation_required');
    expect(
      intent.requiredEvidenceSummary,
      'blocked_until_recovery_code_saved, explicit_user_confirmation_required',
    );

    final command = plan.actionCommandPreviewPlan.previewFor('recovery_setup');
    expect(command.intentStatus, 'requires_confirmation');
    expect(command.executionStatus, 'blocked_until_user_confirmation');
    expect(command.blocker, 'user_confirmation_required');
    expect(
      command.requiredEvidenceSummary,
      'blocked_until_recovery_code_saved, explicit_user_confirmation_required',
    );
    expect(command.stopLine, 'no_recovery_code_generation_current_phase');
    expect(
      command.requestBoundary,
      'request_summary_only_no_recovery_code_generation',
    );
    expect(
      command.resultBoundary,
      'result_summary_only_no_recovery_record_or_wrapped_material',
    );
    expect(
      command.errorCodeSummary,
      'configuration_missing, backend_unavailable, deployment_unverified, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
    );
    expect(
      command.requestPreview.requestStatus,
      'request_blocked_until_user_confirmation',
    );
    expect(
      command.resultPreview.resultStatus,
      'result_blocked_until_user_confirmation',
    );
  });
}

void _expectActionCommandPreviewPlan(
  SyncActionCommandPreviewPlan plan, {
  required String expectedIntentStatusSummary,
  required String expectedBlockerSummary,
  String expectedRequestBoundarySummary =
      syncActionCommandRequestBoundarySummary,
  String expectedResultBoundarySummary = syncActionCommandResultBoundarySummary,
  String expectedErrorCodeSummary = syncActionCommandErrorCodeSummary,
  String expectedRequestAllowedFieldSummary =
      syncActionRequestAllowedFieldSummary,
  String expectedResultAllowedFieldSummary =
      syncActionResultAllowedFieldSummary,
  String expectedForbiddenMaterialSummary = syncActionForbiddenMaterialSummary,
}) {
  expect(
    plan.actionIdSummary,
    'recovery_setup, recovery_restore, join_request_authorization, device_revocation',
  );
  expect(
    plan.visibilitySummary,
    'recovery_setup=visible, recovery_restore=visible, join_request_authorization=visible, device_revocation=visible',
  );
  expect(plan.intentStatusSummary, expectedIntentStatusSummary);
  expect(
    plan.executionStatusSummary,
    _expectedActionCommandExecutionSummary(expectedIntentStatusSummary),
  );
  expect(plan.blockerSummary, expectedBlockerSummary);
  expect(plan.dataPolicySummary, syncActionCommandDataPolicySummary);
  expect(plan.stopLineSummary, syncActionCommandStopLineSummary);
  expect(plan.requestBoundarySummary, expectedRequestBoundarySummary);
  expect(plan.resultBoundarySummary, expectedResultBoundarySummary);
  expect(plan.errorCodeSummary, expectedErrorCodeSummary);
  expect(
    plan.requestStatusSummary,
    _expectedActionRequestStatusSummary(expectedIntentStatusSummary),
  );
  expect(plan.requestAllowedFieldSummary, expectedRequestAllowedFieldSummary);
  expect(
    plan.requestForbiddenMaterialSummary,
    expectedForbiddenMaterialSummary,
  );
  expect(
    plan.resultStatusSummary,
    _expectedActionResultStatusSummary(expectedIntentStatusSummary),
  );
  expect(plan.resultAllowedFieldSummary, expectedResultAllowedFieldSummary);
  expect(plan.resultForbiddenMaterialSummary, expectedForbiddenMaterialSummary);
}

String _expectedActionCommandExecutionSummary(String intentStatusSummary) {
  if (intentStatusSummary == 'none') {
    return 'none';
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) {
        final separator = entry.indexOf('=');
        final actionId = entry.substring(0, separator);
        final intentStatus = entry.substring(separator + 1);
        return '$actionId=${_expectedActionCommandExecutionStatus(intentStatus)}';
      })
      .join(', ');
}

String _expectedActionCommandExecutionStatus(String intentStatus) {
  switch (intentStatus) {
    case 'ready':
      return 'ready_for_future_bridge_command';
    case 'requires_confirmation':
      return 'blocked_until_user_confirmation';
    case 'blocked':
      return 'blocked_by_readiness';
    case 'closed_current_phase':
      return 'not_executable_current_phase';
    default:
      return 'blocked_by_unknown_intent_status';
  }
}

String _expectedActionRequestStatusSummary(String intentStatusSummary) {
  return _expectedActionStatusSummary(
    intentStatusSummary,
    _expectedActionRequestStatus,
  );
}

String _expectedActionResultStatusSummary(String intentStatusSummary) {
  return _expectedActionStatusSummary(
    intentStatusSummary,
    _expectedActionResultStatus,
  );
}

String _expectedActionStatusSummary(
  String intentStatusSummary,
  String Function(String intentStatus) mapStatus,
) {
  if (intentStatusSummary == 'none') {
    return 'none';
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) {
        final separator = entry.indexOf('=');
        final actionId = entry.substring(0, separator);
        final intentStatus = entry.substring(separator + 1);
        return '$actionId=${mapStatus(intentStatus)}';
      })
      .join(', ');
}

String _expectedActionRequestStatus(String intentStatus) {
  switch (_expectedActionCommandExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'request_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'request_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'request_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'request_not_built_current_phase';
    default:
      return 'request_blocked_by_unknown_execution_status';
  }
}

String _expectedActionResultStatus(String intentStatus) {
  switch (_expectedActionCommandExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'result_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'result_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'result_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'result_not_available_current_phase';
    default:
      return 'result_blocked_by_unknown_execution_status';
  }
}
