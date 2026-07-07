import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_ffi_command_boundary_fixtures.dart';

void main() {
  test('ffi boundary fixture keeps sync commands design-only', () {
    final strategy = syncFfiCommandBoundaryStrategyShape();

    expect(strategy['format'], syncFfiCommandBoundaryFormat);
    expect(strategy['review_status'], syncFfiCommandBoundaryReviewStatus);
    expect(
      strategy['recommended_strategy'],
      syncFfiCommandBoundaryRecommendedStrategy,
    );
    expect(strategy['current_native_symbols'], isEmpty);
    expect(
      syncFfiCommandBoundaryCandidateSymbols,
      contains('radishlex_manager_sync_command_execute_v1'),
    );
    expect(
      syncFfiCommandBoundaryRejectedStrategies,
      contains('json_tunnel_request_response'),
    );
    expect(
      syncFfiCommandBoundaryRejectedStrategies,
      contains('dedicated_symbols_before_contract_review'),
    );
  });

  test('ffi request and result shapes expose only safe summaries', () {
    for (final fixture in syncBridgeCommandContractActionFixtures) {
      final request = syncFfiCommandBoundarySafeRequestShape(fixture);
      final result = syncFfiCommandBoundarySafeResultShape(fixture);
      final encoded = '${jsonEncode(request)}\n${jsonEncode(result)}';

      expect(request['action_id'], fixture.actionId);
      expect(result['action_id'], fixture.actionId);
      expect(
        request.keys,
        containsAll([
          'format',
          'review_status',
          'action_id',
          'common_field_policies',
          'action_safe_fields',
          'transient_secret_policy',
        ]),
      );
      expect(
        result.keys,
        containsAll([
          'format',
          'review_status',
          'action_id',
          'summary_field_policies',
        ]),
      );
      final commonFieldPolicies =
          request['common_field_policies'] as Map<String, String>;
      expect(
        commonFieldPolicies.keys,
        containsAll(syncFfiCommandBoundaryRequestCommonFields),
      );
      expect(
        commonFieldPolicies['operation_id'],
        'borrowed_utf8_view_call_scope',
      );
      expect(commonFieldPolicies['explicit_user_start'], 'u8_bool');
      expect(request['action_safe_fields'], fixture.requestSafeFields);
      final summaryFieldPolicies =
          result['summary_field_policies'] as Map<String, String>;
      expect(
        summaryFieldPolicies.keys,
        containsAll(syncFfiCommandBoundaryResultSummaryFields),
      );
      expect(summaryFieldPolicies.values.toSet(), {'safe_summary'});

      for (final field in [
        ...fixture.opaqueMaterialFields,
        ...fixture.transportPayloadFields,
      ]) {
        expect(encoded, isNot(contains(field)), reason: fixture.actionId);
      }
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(encoded, isNot(contains(fragment)), reason: fixture.actionId);
      }
    }
  });

  test(
    'ownership and transient secret rules cover copy release boundaries',
    () {
      expect(
        syncFfiCommandBoundaryOwnershipRules,
        containsAll([
          'request_allocated_by_caller_call_scope_only',
          'borrowed_utf8_views_copied_before_rust_async_boundary',
          'rust_owned_result_handle',
          'dart_copies_summary_then_frees_result',
          'result_views_invalid_after_free',
          'free_null_is_noop',
          'error_read_then_free',
          'no_pointer_in_settings_diagnostics_or_widget_state',
        ]),
      );
      expect(
        syncFfiCommandBoundaryTransientSecretRules,
        containsAll([
          'user_explicit_operation_only',
          'borrowed_view_call_scope',
          'rust_short_lifecycle_internal_copy_only',
          'not_in_settings',
          'not_in_diagnostics',
          'not_in_logs',
          'not_in_test_golden',
          'one_time_display_material_separate_lifecycle',
        ]),
      );
      for (final rule in syncFfiCommandBoundaryTransientSecretRules) {
        expect(rule, isNot(contains('payload')));
        expect(rule, isNot(contains('wrapped_material')));
      }
    },
  );

  test('abi status cases separate ffi errors from command envelopes', () {
    final statusByInput = {
      for (final fixture in syncFfiCommandBoundaryAbiStatusCases)
        fixture.input: fixture.statusCode,
    };

    expect(statusByInput['null_pointer'], 'InvalidArgument');
    expect(statusByInput['invalid_utf8'], 'InvalidArgument');
    expect(statusByInput['unknown_schema_version'], 'InvalidArgument');
    expect(
      statusByInput['sync_command_not_enabled_current_phase'],
      'InvalidState',
    );
    expect(statusByInput['panic_boundary_caught'], 'InternalError');

    final commandErrorCodes = syncFfiCommandBoundaryAllCommandErrorCodes();
    for (final fixture in syncBridgeCommandContractActionFixtures) {
      expect(commandErrorCodes, containsAll(fixture.errorCodes));
    }
    expect(commandErrorCodes, contains('version_conflict'));
    expect(commandErrorCodes, contains('unexpected_bridge_error'));
    expect(commandErrorCodes, isNot(contains('provider_exception_raw')));
    expect(commandErrorCodes, isNot(contains('http_response_body')));
  });

  test('rust and dart smoke plans cover ownership and redaction evidence', () {
    expect(
      syncFfiCommandBoundaryAllSmokeCaseIds(),
      containsAll([
        'contract_reports_command_capability_closed',
        'invalid_request_inputs_return_stable_status',
        'result_handle_copy_then_free',
        'envelope_allowlist_only',
        'forbidden_material_absent_from_native_outputs',
        'panic_boundary_returns_internal_error',
        'sync_domain_command_serialization',
        'native_command_capability_missing_keeps_ui_closed',
        'dart_copies_result_then_releases_handle',
        'forbidden_material_not_visible_to_manager',
        'transient_secret_not_persisted',
        'unknown_native_status_downgrades_to_safe_blocker',
        'ffi_and_command_errors_map_to_bridge_categories',
      ]),
    );

    for (final smokeCase in [
      ...syncFfiCommandBoundaryRustHostSmokeCases,
      ...syncFfiCommandBoundaryDartFfiSmokeCases,
    ]) {
      expect(smokeCase.expectedEvidence, isNotEmpty, reason: smokeCase.id);
      expect(smokeCase.evidenceSummary, isNot('none'));
      expect(smokeCase.evidenceSummary, isNot(contains('real_keychain')));
      expect(smokeCase.evidenceSummary, isNot(contains('real_keystore')));
      expect(smokeCase.evidenceSummary, isNot(contains('remote_payload')));
      expect(smokeCase.evidenceSummary, isNot(contains('production_upload')));
    }
  });

  test(
    'ffi boundary rejected samples keep forbidden material out of outputs',
    () {
      final safeBoundaryText = [
        jsonEncode(syncFfiCommandBoundaryStrategyShape()),
        for (final fixture in syncBridgeCommandContractActionFixtures)
          jsonEncode(syncFfiCommandBoundarySafeRequestShape(fixture)),
        for (final fixture in syncBridgeCommandContractActionFixtures)
          jsonEncode(syncFfiCommandBoundarySafeResultShape(fixture)),
        managerSyncCodeSummary(syncFfiCommandBoundaryOwnershipRules),
        managerSyncCodeSummary(syncFfiCommandBoundaryTransientSecretRules),
        managerSyncCodeSummary(
          syncFfiCommandBoundaryRustHostSmokeCases.map((fixture) => fixture.id),
        ),
        managerSyncCodeSummary(
          syncFfiCommandBoundaryDartFfiSmokeCases.map((fixture) => fixture.id),
        ),
      ].join('\n');

      for (final sample in syncBridgeCommandContractRejectedSamples) {
        expect(
          syncBridgeCommandContractActionFixtures.map(
            (fixture) => fixture.actionId,
          ),
          contains(sample.actionId),
        );
        expect(
          safeBoundaryText,
          isNot(contains(sample.forbiddenFragment)),
          reason: sample.id,
        );
      }
      expect(safeBoundaryText, isNot(contains('Bearer ')));
      expect(safeBoundaryText, isNot(contains('request_body')));
      expect(safeBoundaryText, isNot(contains('response_body')));
      expect(safeBoundaryText, isNot(contains('/synthetic/private')));
    },
  );
}
