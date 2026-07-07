import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

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
}
