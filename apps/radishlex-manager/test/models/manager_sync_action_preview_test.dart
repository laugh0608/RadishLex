import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
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

  test('action command preview covers protocol status matrix', () {
    for (final action in syncActionProtocolExpectations) {
      for (final scenario in syncActionProtocolStatusScenarios) {
        final plan = SyncInteractionEntryPlan(
          intents: [
            SyncInteractionActionIntent(
              actionId: action.actionId,
              visibilityStatus: 'visible',
              intentStatus: scenario.intentStatus,
              blocker: scenario.blocker,
              requiredEvidenceCodes: scenario.requiredEvidenceCodes,
              sourceTag: 'unit_test_action_protocol',
            ),
          ],
        ).actionCommandPreviewPlan;
        final command = plan.previewFor(action.actionId);

        expect(command.actionId, action.actionId, reason: scenario.id);
        expect(command.visibilityStatus, 'visible', reason: scenario.id);
        expect(
          command.intentStatus,
          scenario.intentStatus,
          reason: scenario.id,
        );
        expect(
          command.executionStatus,
          scenario.expectedExecutionStatus,
          reason: '${action.actionId} ${scenario.id}',
        );
        expect(command.blocker, scenario.blocker, reason: scenario.id);
        expect(
          command.requiredEvidenceSummary,
          scenario.requiredEvidenceSummary,
          reason: scenario.id,
        );
        expect(command.dataPolicy, action.dataPolicy, reason: scenario.id);
        expect(command.stopLine, action.stopLine, reason: scenario.id);
        expect(
          command.requestBoundary,
          action.requestBoundary,
          reason: scenario.id,
        );
        expect(
          command.resultBoundary,
          action.resultBoundary,
          reason: scenario.id,
        );
        expect(
          command.errorCodeSummary,
          action.errorCodeSummary,
          reason: scenario.id,
        );
        expect(
          command.requestPreview.requestStatus,
          scenario.expectedRequestStatus,
          reason: '${action.actionId} ${scenario.id}',
        );
        expect(
          command.requestPreview.boundary,
          action.requestBoundary,
          reason: scenario.id,
        );
        expect(
          command.requestPreview.allowedFieldSummary,
          action.requestAllowedFieldSummary,
          reason: scenario.id,
        );
        expect(
          command.requestPreview.forbiddenMaterialSummary,
          syncActionForbiddenMaterialSummary,
          reason: scenario.id,
        );
        expect(
          command.resultPreview.resultStatus,
          scenario.expectedResultStatus,
          reason: '${action.actionId} ${scenario.id}',
        );
        expect(
          command.resultPreview.boundary,
          action.resultBoundary,
          reason: scenario.id,
        );
        expect(
          command.resultPreview.allowedFieldSummary,
          action.resultAllowedFieldSummary,
          reason: scenario.id,
        );
        expect(
          command.resultPreview.forbiddenMaterialSummary,
          syncActionForbiddenMaterialSummary,
          reason: scenario.id,
        );
      }
    }
  });

  test('action command preview keeps missing intent non-executable', () {
    final command = const SyncActionCommandPreviewPlan(
      previews: [],
    ).previewFor('recovery_setup');

    expect(command.actionId, 'recovery_setup');
    expect(command.visibilityStatus, 'hidden');
    expect(command.intentStatus, 'blocked');
    expect(command.executionStatus, 'blocked_by_missing_intent');
    expect(command.blocker, 'interaction_intent_missing');
    expect(command.requiredEvidenceSummary, 'interaction_intent_missing');
    expect(command.dataPolicy, managerSyncActionCommandPreviewDataPolicy);
    expect(command.stopLine, 'no_bridge_command_without_interaction_intent');
    expect(command.requestBoundary, 'no_request_without_interaction_intent');
    expect(command.resultBoundary, 'no_result_without_interaction_intent');
    expect(command.errorCodeSummary, 'interaction_intent_missing');
    expect(
      command.requestPreview.requestStatus,
      'request_blocked_by_missing_intent',
    );
    expect(command.requestPreview.allowedFieldSummary, 'none');
    expect(
      command.requestPreview.forbiddenMaterialSummary,
      'no_secret_material_or_payload',
    );
    expect(
      command.resultPreview.resultStatus,
      'result_blocked_by_missing_intent',
    );
    expect(command.resultPreview.allowedFieldSummary, 'none');
    expect(
      command.resultPreview.forbiddenMaterialSummary,
      'no_secret_material_or_payload',
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
    syncActionExpectedExecutionSummary(expectedIntentStatusSummary),
  );
  expect(plan.blockerSummary, expectedBlockerSummary);
  expect(plan.dataPolicySummary, syncActionCommandDataPolicySummary);
  expect(plan.stopLineSummary, syncActionCommandStopLineSummary);
  expect(plan.requestBoundarySummary, expectedRequestBoundarySummary);
  expect(plan.resultBoundarySummary, expectedResultBoundarySummary);
  expect(plan.errorCodeSummary, expectedErrorCodeSummary);
  expect(
    plan.requestStatusSummary,
    syncActionExpectedRequestStatusSummary(expectedIntentStatusSummary),
  );
  expect(plan.requestAllowedFieldSummary, expectedRequestAllowedFieldSummary);
  expect(
    plan.requestForbiddenMaterialSummary,
    expectedForbiddenMaterialSummary,
  );
  expect(
    plan.resultStatusSummary,
    syncActionExpectedResultStatusSummary(expectedIntentStatusSummary),
  );
  expect(plan.resultAllowedFieldSummary, expectedResultAllowedFieldSummary);
  expect(plan.resultForbiddenMaterialSummary, expectedForbiddenMaterialSummary);
}
