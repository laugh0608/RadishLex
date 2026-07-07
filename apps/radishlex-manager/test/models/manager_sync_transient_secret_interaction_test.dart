import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_transient_secret_interaction_fixtures.dart';

void main() {
  test('transient secret interactions cover bridge contract fields', () {
    final contractTransientFields = {
      for (final fixture in syncBridgeCommandContractActionFixtures)
        ...fixture.transientSecretFields,
    };
    final coveredFields = syncTransientSecretCoveredContractFields().toSet();
    final actionIds = {
      for (final fixture in syncBridgeCommandContractActionFixtures)
        fixture.actionId,
    };

    expect(
      syncTransientSecretInteractionFixtures.map((fixture) => fixture.actionId),
      containsAll(['recovery_setup', 'recovery_restore']),
    );
    expect(coveredFields, contractTransientFields);

    for (final fixture in syncTransientSecretInteractionFixtures) {
      expect(actionIds, contains(fixture.actionId), reason: fixture.id);
      expect(
        fixture.currentPhaseStatus,
        anyOf(
          'display_not_available_current_phase',
          'input_not_available_current_phase',
          'confirmation_not_available_current_phase',
        ),
        reason: fixture.id,
      );
      expect(
        fixture.allowedUiStateSummary,
        isNot(contains('secret')),
        reason: fixture.id,
      );
      expect(
        fixture.allowedDiagnosticsSummary,
        isNot(contains('secret')),
        reason: fixture.id,
      );
      expect(fixture.completionEvidenceSummary, isNot('none'));
    }
  });

  test('transient secret lifecycle forbids persistence and raw material', () {
    expect(
      syncTransientSecretLifecycleRules,
      containsAll([
        'user_explicit_operation_only',
        'clear_on_submit_cancel_navigation_or_timeout',
        'not_in_settings',
        'not_in_diagnostics',
        'not_in_logs',
        'not_in_test_golden',
      ]),
    );
    expect(
      syncTransientSecretForbiddenPersistenceTargets,
      containsAll([
        'settings_json',
        'diagnostics_export',
        'widget_text_snapshot',
        'log_line',
        'crash_report',
        'route_arguments',
      ]),
    );

    final encoded = [
      for (final fixture in syncTransientSecretInteractionFixtures)
        jsonEncode(syncTransientSecretInteractionShape(fixture)),
    ].join('\n');

    expect(encoded, contains(syncTransientSecretInteractionFormat));
    expect(encoded, contains(syncTransientSecretInteractionReviewStatus));
    expect(encoded, contains(syncTransientSecretInteractionCurrentPhaseStatus));
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encoded,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    expect(encoded, isNot(contains('recovery_code=')));
    expect(encoded, isNot(contains('private_key')));
    expect(encoded, isNot(contains('provider_exception_raw')));
    expect(encoded, isNot(contains('http_response_body')));
  });

  test('current phase prohibits transient secret operations', () {
    final prohibitedByAction = {
      for (final fixture in syncBridgeCommandContractActionFixtures)
        fixture.actionId: fixture.currentPhaseProhibitedOperations,
    };

    for (final fixture in syncTransientSecretInteractionFixtures) {
      final prohibited = prohibitedByAction[fixture.actionId] ?? const [];
      expect(prohibited, isNotEmpty, reason: fixture.id);
      expect(
        fixture.prohibitedOperations.toSet().intersection(prohibited.toSet()),
        isNotEmpty,
        reason: fixture.id,
      );
    }

    final setup = syncTransientSecretInteractionFixtures.firstWhere(
      (fixture) => fixture.id == 'recovery_setup_one_time_display',
    );
    final restore = syncTransientSecretInteractionFixtures.firstWhere(
      (fixture) => fixture.id == 'recovery_restore_code_input',
    );

    expect(setup.prohibitedOperations, contains('generate_recovery_code'));
    expect(setup.prohibitedOperations, contains('unlock_first_upload'));
    expect(restore.prohibitedOperations, contains('input_recovery_code'));
    expect(restore.prohibitedOperations, contains('unwrap_device_material'));
  });

  test('recovery visible layer fixtures stay on status copy only', () {
    final interactionIds = {
      for (final fixture in syncTransientSecretInteractionFixtures) fixture.id,
    };
    final actionIds = {
      for (final fixture in syncTransientSecretInteractionFixtures)
        fixture.actionId,
    };

    expect(syncRecoveryVisibleLayerFixtures, hasLength(2));
    expect(
      syncRecoveryVisibleLayerFixtures.map((fixture) => fixture.actionId),
      ['recovery_setup', 'recovery_restore'],
    );

    final encoded = [
      for (final fixture in syncRecoveryVisibleLayerFixtures)
        jsonEncode(syncRecoveryVisibleLayerShape(fixture)),
    ].join('\n');

    expect(encoded, contains(syncRecoveryVisibleLayerFormat));
    expect(encoded, contains(syncRecoveryVisibleLayerReviewStatus));
    expect(encoded, contains('display_not_available_current_phase'));
    expect(encoded, contains('input_not_available_current_phase'));
    expect(encoded, contains('required_before_first_upload'));
    expect(encoded, contains('not_started'));
    expect(encoded, contains('blocked_until_recovery_success'));
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('http_response_body')));

    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encoded,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }

    for (final fixture in syncRecoveryVisibleLayerFixtures) {
      expect(actionIds, contains(fixture.actionId), reason: fixture.id);
      expect(
        interactionIds,
        contains(fixture.interactionFixtureId),
        reason: fixture.id,
      );
      expect(fixture.uiStateSummary, isNot('none'), reason: fixture.id);
      expect(fixture.uiStatusSummary, isNot('none'), reason: fixture.id);
      expect(
        fixture.diagnosticsKeySummary,
        startsWith('sync.recovery_'),
        reason: fixture.id,
      );
      expect(
        fixture.diagnosticsStatusSummary,
        isNot(contains('secret')),
        reason: fixture.id,
      );
      expect(
        fixture.blockedOperationSummary,
        contains('secret'),
        reason: fixture.id,
      );
    }
  });

  test('recovery visible fixtures match current UI and diagnostics status', () {
    final snapshot = createManagerFixture();
    final gateAudit = managerSyncGateAuditForDraft(
      draft: snapshot.settings.draft,
      device: snapshot.sync.device,
      readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
    );
    final recovery = gateAudit.entryGate.recovery;
    final diagnostics = _diagnosticsByKey(
      createManagerDiagnosticsReport(snapshot),
    );

    final uiStatusByAction = {
      'recovery_setup': {
        recovery.setupReadiness.oneTimeDisplayStatus,
        recovery.setupReadiness.saveConfirmationStatus,
        recovery.recoveryRecordStatus,
        recovery.firstUploadGate,
      },
      'recovery_restore': {
        recovery.restoreReadiness.codeInputStatus,
        recovery.restoreReadiness.recoveryRecordLookupStatus,
        recovery.restoreReadiness.attemptLimitStatus,
        recovery.restoreReadiness.deviceRegistrationStatus,
      },
    };

    for (final fixture in syncRecoveryVisibleLayerFixtures) {
      expect(
        uiStatusByAction[fixture.actionId],
        containsAll(fixture.uiExpectedStatusCodes),
        reason: fixture.id,
      );
      for (final key in fixture.diagnosticsKeys) {
        expect(diagnostics, contains(key), reason: '${fixture.id}: $key');
      }
      expect(
        diagnostics.values,
        containsAll(fixture.diagnosticsExpectedStatusCodes),
        reason: fixture.id,
      );
    }
  });
}

Map<String, String> _diagnosticsByKey(ManagerDiagnosticsReport report) {
  return {
    for (final section in report.sections)
      for (final item in section.items) item.key: item.value,
  };
}
