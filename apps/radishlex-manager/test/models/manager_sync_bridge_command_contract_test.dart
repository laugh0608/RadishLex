import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  test('bridge command contract fixtures cover the four preview actions', () {
    expect(
      managerSyncCodeSummary(
        syncBridgeCommandContractActionFixtures.map(
          (fixture) => fixture.actionId,
        ),
      ),
      'recovery_setup, recovery_restore, join_request_authorization, device_revocation',
    );

    for (final preview in syncActionProtocolExpectations) {
      final fixture = syncBridgeCommandContractFixtureFor(preview.actionId);

      expect(
        fixture.requestSafeFieldSummary,
        preview.requestAllowedFieldSummary,
      );
      expect(fixture.resultSafeFieldSummary, preview.resultAllowedFieldSummary);
      expect(fixture.errorCodeSummary, preview.errorCodeSummary);
      expect(fixture.currentPhaseProhibitedOperations, isNotEmpty);
    }
  });

  test('safe request and result design shapes expose only summary fields', () {
    for (final fixture in syncBridgeCommandContractActionFixtures) {
      final request = syncBridgeCommandContractSafeRequestShape(fixture);
      final result = syncBridgeCommandContractSafeResultShape(fixture);
      final requestText = jsonEncode(request);
      final resultText = jsonEncode(result);

      expect(request['review_status'], syncBridgeCommandContractReviewStatus);
      expect(result['review_status'], syncBridgeCommandContractReviewStatus);
      expect(request['action_id'], fixture.actionId);
      expect(result['action_id'], fixture.actionId);
      expect(
        request.keys,
        containsAll([
          'format',
          'review_status',
          'action_id',
          'command_status',
          'readiness_snapshot_binding',
          'operation_id_policy',
          ...fixture.requestSafeFields,
        ]),
      );
      expect(
        result.keys,
        containsAll([
          'format',
          'review_status',
          'action_id',
          'command_status',
          'retry_policy',
          ...fixture.resultSafeFields,
        ]),
      );

      for (final field in fixture.transientSecretFields) {
        expect(request, isNot(contains(field)), reason: fixture.actionId);
        expect(result, isNot(contains(field)), reason: fixture.actionId);
      }
      for (final field in fixture.opaqueMaterialFields) {
        expect(requestText, isNot(contains(field)), reason: fixture.actionId);
        expect(resultText, isNot(contains(field)), reason: fixture.actionId);
      }
      for (final field in fixture.transportPayloadFields) {
        expect(requestText, isNot(contains(field)), reason: fixture.actionId);
        expect(resultText, isNot(contains(field)), reason: fixture.actionId);
      }
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(
          requestText,
          isNot(contains(fragment)),
          reason: fixture.actionId,
        );
        expect(resultText, isNot(contains(fragment)), reason: fixture.actionId);
      }
    }
  });

  test('error envelopes keep allowlisted codes and required fields', () {
    for (final fixture in syncBridgeCommandContractActionFixtures) {
      for (final errorCode in fixture.errorCodes) {
        final envelope = syncBridgeCommandContractErrorEnvelope(
          fixture,
          errorCode,
        );
        final envelopeText = jsonEncode(envelope);

        expect(
          envelope.keys,
          containsAll(syncBridgeCommandContractEnvelopeRequiredFields),
          reason: '${fixture.actionId} $errorCode',
        );
        expect(envelope['action_id'], fixture.actionId);
        expect(
          syncBridgeCommandContractCommandStatuses,
          contains(envelope['command_status']),
        );
        expect(fixture.errorCodes, contains(envelope['error_code']));
        expect(
          syncBridgeCommandContractRetryPolicies,
          contains(envelope['retry_policy']),
        );
        expect(envelope['user_visible_summary_code'], errorCode);
        expect(envelope['diagnostics_summary_code'], errorCode);
        for (final fragment in syncBridgeCommandContractForbiddenFragments) {
          expect(envelopeText, isNot(contains(fragment)), reason: errorCode);
        }
      }
    }
  });

  test('contract fixtures separate transient, opaque and transport fields', () {
    for (final fixture in syncBridgeCommandContractActionFixtures) {
      expect(fixture.transientSecretFields, isNotEmpty);
      expect(fixture.opaqueMaterialFields, isNotEmpty);
      expect(fixture.transportPayloadFields, isNotEmpty);
      expect(fixture.idempotencyStatuses, isNotEmpty);

      final diagnosticsAllowed =
          syncBridgeCommandContractDiagnosticsAllowedFields.join(', ');
      for (final field in [
        ...fixture.transientSecretFields,
        ...fixture.opaqueMaterialFields,
        ...fixture.transportPayloadFields,
      ]) {
        expect(diagnosticsAllowed, isNot(contains(field)));
      }
    }
  });

  test('rejected design samples cover forbidden material categories', () {
    final rejectedText = syncBridgeCommandContractRejectedSamples
        .map((sample) => jsonEncode(sample.payload))
        .join('\n');

    for (final sample in syncBridgeCommandContractRejectedSamples) {
      expect(
        syncBridgeCommandContractActionFixtures.map(
          (fixture) => fixture.actionId,
        ),
        contains(sample.actionId),
      );
      expect(
        jsonEncode(sample.payload),
        contains(sample.forbiddenFragment),
        reason: sample.id,
      );
      expect(sample.expectedReason, isNotEmpty);
    }
    for (final fragment in [
      'secret-token',
      'RADISHLEX-RECOVERY-CODE-SECRET',
      'short_code=',
      'signature_bytes',
      'wrapped_material_bytes',
      'payload_bytes=abcdef',
      'request_body',
      'response_body',
      '/synthetic/private',
    ]) {
      expect(rejectedText, contains(fragment), reason: fragment);
    }
  });

  test('representative diagnostics stay on preview summaries only', () {
    final snapshots = [
      managerSnapshotForSyncReadinessScenario(
        syncReadinessScenarioById('all_ready_current_phase_closed'),
      ),
      managerSnapshotForSyncReadinessScenario(
        syncReadinessScenarioById('unknown_native_status_sanitized'),
      ),
      managerSnapshotForSyncEvidenceBundleScenario(
        syncEvidenceBundleScenarioById(
          'local_https_reachable_recovery_record_missing',
        ),
      ),
    ];

    for (final snapshot in snapshots) {
      final report = createManagerDiagnosticsReport(snapshot);
      final text = report.toRedactedText();

      expect(
        _diagnosticsValue(report, 'sync.action_command_format'),
        managerSyncActionCommandPreviewFormat,
      );
      expect(
        _diagnosticsValue(report, 'sync.action_request_allowed_fields'),
        syncActionRequestAllowedFieldSummary,
      );
      expect(
        _diagnosticsValue(report, 'sync.action_result_allowed_fields'),
        syncActionResultAllowedFieldSummary,
      );
      expect(
        _diagnosticsValue(report, 'sync.action_request_forbidden_material'),
        syncActionForbiddenMaterialSummary,
      );
      expect(
        _diagnosticsValue(report, 'sync.action_result_forbidden_material'),
        syncActionForbiddenMaterialSummary,
      );
      expect(text, isNot(contains('future_manager_bridge_command_request')));
      expect(text, isNot(contains('future_manager_bridge_command_result')));
      for (final fixture in syncBridgeCommandContractActionFixtures) {
        for (final field in [
          ...fixture.transientSecretFields,
          ...fixture.transportPayloadFields,
          ...fixture.currentPhaseProhibitedOperations,
        ]) {
          expect(text, isNot(contains(field)), reason: field);
        }
      }
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(text, isNot(contains(fragment)), reason: fragment);
      }
    }
  });
}

String _diagnosticsValue(ManagerDiagnosticsReport report, String key) {
  return report.sections
      .expand((section) => section.items)
      .singleWhere((item) => item.key == key)
      .value;
}
