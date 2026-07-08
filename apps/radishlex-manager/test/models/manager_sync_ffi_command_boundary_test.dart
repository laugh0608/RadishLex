import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_ffi_command_boundary_fixtures.dart';
import '../fixtures/sync_ffi_rust_host_contract_review_fixtures.dart';

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

  test('rust host input samples stay synthetic and status-mapped', () {
    final abiStatusByInput = {
      for (final fixture in syncFfiCommandBoundaryAbiStatusCases)
        fixture.input: fixture.statusCode,
    };
    final knownActionIds = {
      for (final fixture in syncBridgeCommandContractActionFixtures)
        fixture.actionId,
    };
    final encodedSamples = [
      for (final sample in syncFfiCommandBoundaryRustHostInputSamples)
        jsonEncode(syncFfiCommandBoundaryRustHostInputSampleShape(sample)),
    ].join('\n');

    expect(syncFfiCommandBoundaryRustHostInputSamples.length, 9);
    expect(
      syncFfiCommandBoundaryRustHostInputSamples.map(
        (sample) => sample.expectedStatusCode,
      ),
      containsAll(['InvalidArgument', 'InvalidState', 'SyncError']),
    );
    expect(
      syncFfiCommandBoundaryRustHostInputSamples.map(
        (sample) => sample.expectedStatusCode,
      ),
      contains('InternalError'),
    );

    for (final sample in syncFfiCommandBoundaryRustHostInputSamples) {
      expect(knownActionIds, contains(sample.actionId), reason: sample.id);
      expect(
        sample.operationId,
        startsWith('op_test_non_secret_'),
        reason: sample.id,
      );
      expect(
        sample.readinessSnapshotId,
        startsWith('readiness_snapshot_test_'),
        reason: sample.id,
      );
      expect(sample.sourceTag, 'local_smoke', reason: sample.id);
      expect(
        abiStatusByInput[sample.abiInputCase],
        sample.expectedStatusCode,
        reason: sample.id,
      );
      expect(sample.expectedEvidenceSummary, isNot('none'), reason: sample.id);
    }
    expect(
      syncFfiCommandBoundaryRustHostInputSamples.map(
        (sample) => sample.abiInputCase,
      ),
      containsAll([
        'null_pointer',
        'invalid_utf8',
        'same_domain_concurrent_command',
      ]),
    );

    for (final sample in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedSamples,
        isNot(contains(sample.forbiddenFragment)),
        reason: sample.id,
      );
    }
    expect(encodedSamples, isNot(contains('real_keychain')));
    expect(encodedSamples, isNot(contains('real_keystore')));
    expect(encodedSamples, isNot(contains('remote_payload')));
    expect(encodedSamples, isNot(contains('production_upload')));
  });

  test('rust host contract catalog binds samples to smoke plan', () {
    final sampleById = {
      for (final sample in syncFfiCommandBoundaryRustHostInputSamples)
        sample.id: sample,
    };
    final smokeCaseIds = {
      for (final smokeCase in syncFfiCommandBoundaryRustHostSmokeCases)
        smokeCase.id,
    };
    final abiStatusByInput = {
      for (final fixture in syncFfiCommandBoundaryAbiStatusCases)
        fixture.input: fixture.statusCode,
    };
    final validStatusCodes = abiStatusByInput.values.toSet();
    final catalogSmokeCaseIds = {
      for (final fixture in syncFfiCommandBoundaryRustHostContractCases)
        fixture.smokeCaseId,
    };
    final catalogSampleIds = {
      for (final fixture in syncFfiCommandBoundaryRustHostContractCases)
        ...fixture.sampleIds,
    };

    expect(
      syncFfiCommandBoundaryRustHostContractCaseIds(),
      containsAll([
        'contract_reports_command_capability_closed',
        'invalid_request_inputs_return_stable_status',
        'result_handle_copy_then_free',
        'envelope_allowlist_only',
        'forbidden_material_absent_from_native_outputs',
        'panic_boundary_returns_internal_error',
        'sync_domain_command_serialization',
      ]),
    );
    expect(catalogSmokeCaseIds, smokeCaseIds);
    expect(catalogSampleIds, containsAll(sampleById.keys));

    for (final fixture in syncFfiCommandBoundaryRustHostContractCases) {
      final shape = syncFfiCommandBoundaryRustHostContractCaseShape(fixture);

      expect(shape['format'], syncFfiCommandBoundaryRustHostContractFormat);
      expect(
        shape['review_status'],
        syncFfiCommandBoundaryRustHostContractReviewStatus,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(fixture.targetTestName, fixture.id, reason: fixture.id);
      expect(smokeCaseIds, contains(fixture.smokeCaseId), reason: fixture.id);
      expect(
        fixture.implementationStatus,
        'planned_no_native_symbol',
        reason: fixture.id,
      );
      expect(fixture.sampleIdSummary, isNot('none'), reason: fixture.id);
      expect(
        fixture.requiredAbiInputSummary,
        isNot('none'),
        reason: fixture.id,
      );
      expect(fixture.expectedStatusSummary, isNot('none'), reason: fixture.id);
      expect(fixture.evidenceSummary, isNot('none'), reason: fixture.id);

      for (final inputCase in fixture.requiredAbiInputCases) {
        expect(abiStatusByInput, contains(inputCase), reason: fixture.id);
      }
      for (final statusCode in fixture.expectedStatusCodes) {
        expect(validStatusCodes, contains(statusCode), reason: fixture.id);
      }
      for (final sampleId in fixture.sampleIds) {
        final sample = sampleById[sampleId];
        expect(sample, isNotNull, reason: '${fixture.id}: $sampleId');
        expect(
          fixture.requiredAbiInputCases,
          contains(sample!.abiInputCase),
          reason: '${fixture.id}: $sampleId',
        );
        expect(
          fixture.expectedStatusCodes,
          contains(sample.expectedStatusCode),
          reason: '${fixture.id}: $sampleId',
        );
      }
      for (final fragment in syncBridgeCommandContractForbiddenFragments) {
        expect(fixture.evidenceSummary, isNot(contains(fragment)));
        expect(jsonEncode(shape), isNot(contains(fragment)));
      }
    }
  });

  test('rust host catalog keeps rejected material as categories only', () {
    final rejectedCategories = {
      for (final sample in syncBridgeCommandContractRejectedSamples)
        sample.expectedReason,
    };
    final allowedCategories = {'none', ...rejectedCategories};
    final encodedHostCatalog = [
      for (final sample in syncFfiCommandBoundaryRustHostInputSamples)
        jsonEncode(syncFfiCommandBoundaryRustHostInputSampleShape(sample)),
      for (final fixture in syncFfiCommandBoundaryRustHostContractCases)
        jsonEncode(syncFfiCommandBoundaryRustHostContractCaseShape(fixture)),
    ].join('\n');

    for (final sample in syncFfiCommandBoundaryRustHostInputSamples) {
      expect(
        allowedCategories,
        contains(sample.forbiddenMaterialCategory),
        reason: sample.id,
      );
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedHostCatalog,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    expect(encodedHostCatalog, isNot(contains('settings_action_payload')));
    expect(encodedHostCatalog, isNot(contains('bridge_request_payload')));
    expect(encodedHostCatalog, isNot(contains('remote_request_body')));
    expect(encodedHostCatalog, isNot(contains('remote_response_body')));
  });

  test('rust host review items bind contract cases to ffi patterns', () {
    const knownExistingPatterns = {
      'radishlex_ffi_contract_current_version',
      'ffi_status_catch_unwind',
      'ffi_ptr_null_on_error',
      'ffi_release_free_null',
      'read_utf8_rejects_null_and_invalid',
      'read_ffi_bool_rejects_unknown',
      'summary_output_pointer_invalid_argument',
      'error_handle_read_then_free',
      'platform_binding_copies_views_before_release',
      'sync_preflight_summary_output',
      'current_native_symbol_absent',
      'radishlex_status_internal_error',
      'session_owner_thread_invalid_state',
    };
    final contractCaseIds = syncFfiCommandBoundaryRustHostContractCaseIds()
        .toSet();
    final reviewByContractCaseId = {
      for (final item in syncFfiCommandBoundaryRustHostReviewItems)
        item.contractCaseId: item,
    };

    expect(
      syncFfiCommandBoundaryRustHostReviewItemIds(),
      containsAll([
        'contract_reports_command_capability_closed_review',
        'invalid_request_inputs_return_stable_status_review',
        'result_handle_copy_then_free_review',
        'envelope_allowlist_only_review',
        'forbidden_material_absent_from_native_outputs_review',
        'panic_boundary_returns_internal_error_review',
        'sync_domain_command_serialization_review',
      ]),
    );
    expect(reviewByContractCaseId.keys.toSet(), contractCaseIds);
    expect(
      syncFfiCommandBoundaryRustHostReviewStopLines,
      containsAll([
        'add_c_abi_symbol',
        'add_manager_bridge_method',
        'execute_remote_sync',
        'write_settings_action',
        'create_setup_or_authorization_material',
        'connect_go_server',
        'touch_platform_key_backend',
      ]),
    );

    for (final item in syncFfiCommandBoundaryRustHostReviewItems) {
      final shape = syncFfiCommandBoundaryRustHostReviewItemShape(item);

      expect(shape['format'], syncFfiCommandBoundaryRustHostReviewFormat);
      expect(
        shape['review_status'],
        syncFfiCommandBoundaryRustHostReviewStatus,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(contractCaseIds, contains(item.contractCaseId), reason: item.id);
      expect(
        item.implementationStatus,
        'review_ready_no_native_symbol',
        reason: item.id,
      );
      expect(item.reviewTopicSummary, isNot('none'), reason: item.id);
      expect(item.existingPatternSummary, isNot('none'), reason: item.id);
      expect(item.evidenceSummary, isNot('none'), reason: item.id);
      expect(
        knownExistingPatterns,
        containsAll(item.requiredExistingPatterns),
        reason: item.id,
      );
      expect(
        shape['stop_lines'],
        syncFfiCommandBoundaryRustHostReviewStopLines,
        reason: item.id,
      );
    }

    expect(
      reviewByContractCaseId['invalid_request_inputs_return_stable_status']!
          .requiredExistingPatterns,
      containsAll([
        'read_utf8_rejects_null_and_invalid',
        'read_ffi_bool_rejects_unknown',
        'summary_output_pointer_invalid_argument',
      ]),
    );
    expect(
      reviewByContractCaseId['result_handle_copy_then_free']!
          .requiredExistingPatterns,
      containsAll([
        'ffi_ptr_null_on_error',
        'ffi_release_free_null',
        'platform_binding_copies_views_before_release',
      ]),
    );
    expect(
      reviewByContractCaseId['panic_boundary_returns_internal_error']!
          .requiredExistingPatterns,
      containsAll(['ffi_status_catch_unwind']),
    );
    expect(
      reviewByContractCaseId['sync_domain_command_serialization']!
          .requiredReviewTopics,
      containsAll([
        'sync_domain_serialization_guard',
        'operation_id_non_sensitive',
      ]),
    );
  });

  test('rust host review items remain review-only and non-sensitive', () {
    final encodedReviewCatalog = [
      for (final item in syncFfiCommandBoundaryRustHostReviewItems)
        jsonEncode(syncFfiCommandBoundaryRustHostReviewItemShape(item)),
    ].join('\n');

    expect(syncFfiCommandBoundaryCurrentNativeSymbols, isEmpty);
    expect(encodedReviewCatalog, contains('no_native_symbol'));
    for (final symbol in syncFfiCommandBoundaryCandidateSymbols) {
      expect(encodedReviewCatalog, isNot(contains(symbol)), reason: symbol);
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedReviewCatalog,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedReviewCatalog, isNot(contains(fragment)));
    }
    for (final stopLine in syncFfiCommandBoundaryRustHostReviewStopLines) {
      expect(encodedReviewCatalog, contains(stopLine), reason: stopLine);
    }
  });

  test('rust host source checklist binds review items to source patterns', () {
    final reviewById = {
      for (final item in syncFfiCommandBoundaryRustHostReviewItems)
        item.id: item,
    };
    final checklistByReviewId = {
      for (final item in syncFfiCommandBoundaryRustHostSourceChecklistItems)
        item.reviewItemId: item,
    };
    const allowedSourcePrefixes = [
      'crates/ime-ffi/src/',
      'crates/ime-ffi/tests/',
      'apps/radishlex-manager/tool/',
      'apps/radishlex-manager/test/fixtures/',
    ];

    expect(
      syncFfiCommandBoundaryRustHostSourceChecklistItemIds(),
      containsAll([
        'contract_reports_command_capability_closed_source_checklist',
        'invalid_request_inputs_return_stable_status_source_checklist',
        'result_handle_copy_then_free_source_checklist',
        'envelope_allowlist_only_source_checklist',
        'forbidden_material_absent_from_native_outputs_source_checklist',
        'panic_boundary_returns_internal_error_source_checklist',
        'sync_domain_command_serialization_source_checklist',
      ]),
    );
    expect(checklistByReviewId.keys.toSet(), reviewById.keys.toSet());

    for (final item in syncFfiCommandBoundaryRustHostSourceChecklistItems) {
      final review = reviewById[item.reviewItemId];
      final shape = syncFfiCommandBoundaryRustHostSourceChecklistItemShape(
        item,
      );

      expect(review, isNotNull, reason: item.id);
      expect(
        item.patternSourceRefs.keys,
        containsAll(review!.requiredExistingPatterns),
        reason: item.id,
      );
      expect(
        item.implementationStatus,
        'source_checklist_ready_no_native_symbol',
        reason: item.id,
      );
      expect(
        shape['format'],
        syncFfiCommandBoundaryRustHostSourceChecklistFormat,
      );
      expect(
        shape['review_status'],
        syncFfiCommandBoundaryRustHostSourceChecklistStatus,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(
        shape['stop_lines'],
        syncFfiCommandBoundaryRustHostReviewStopLines,
      );
      expect(item.patternSummary, isNot('none'), reason: item.id);
      expect(item.sourceRefSummary, isNot('none'), reason: item.id);
      expect(item.preCheckSummary, isNot('none'), reason: item.id);

      for (final refs in item.patternSourceRefs.values) {
        expect(refs, isNotEmpty, reason: item.id);
        for (final ref in refs) {
          expect(ref, contains('::'), reason: '${item.id}: $ref');
          expect(
            allowedSourcePrefixes.any(ref.startsWith),
            isTrue,
            reason: '${item.id}: $ref',
          );
        }
      }
    }
  });

  test('rust host source checklist remains review-only', () {
    final encodedChecklist = [
      for (final item in syncFfiCommandBoundaryRustHostSourceChecklistItems)
        jsonEncode(
          syncFfiCommandBoundaryRustHostSourceChecklistItemShape(item),
        ),
    ].join('\n');

    expect(syncFfiCommandBoundaryCurrentNativeSymbols, isEmpty);
    expect(encodedChecklist, contains('no_native_symbol'));
    expect(
      encodedChecklist,
      contains('source_checklist_ready_no_native_symbol'),
    );
    for (final symbol in syncFfiCommandBoundaryCandidateSymbols) {
      expect(encodedChecklist, isNot(contains(symbol)), reason: symbol);
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedChecklist,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    expect(encodedChecklist, isNot(contains('request_body')));
    expect(encodedChecklist, isNot(contains('response_body')));
    expect(encodedChecklist, isNot(contains('settings_action_payload')));
    expect(encodedChecklist, isNot(contains('bridge_request_payload')));
    expect(encodedChecklist, isNot(contains('connect_go_server_now')));
    expect(encodedChecklist, isNot(contains('touch_platform_key_backend_now')));
  });

  test('rust host test design binds source checklist to assertions', () {
    final contractById = {
      for (final item in syncFfiCommandBoundaryRustHostContractCases)
        item.id: item,
    };
    final sourceChecklistById = {
      for (final item in syncFfiCommandBoundaryRustHostSourceChecklistItems)
        item.id: item,
    };
    final reviewById = {
      for (final item in syncFfiCommandBoundaryRustHostReviewItems)
        item.id: item,
    };
    final sampleById = {
      for (final sample in syncFfiCommandBoundaryRustHostInputSamples)
        sample.id: sample,
    };
    final designByContractCaseId = {
      for (final item in syncFfiCommandBoundaryRustHostTestDesignItems)
        item.contractCaseId: item,
    };

    expect(
      syncFfiCommandBoundaryRustHostTestDesignItemIds(),
      containsAll([
        'contract_reports_command_capability_closed_test_design',
        'invalid_request_inputs_return_stable_status_test_design',
        'result_handle_copy_then_free_test_design',
        'envelope_allowlist_only_test_design',
        'forbidden_material_absent_from_native_outputs_test_design',
        'panic_boundary_returns_internal_error_test_design',
        'sync_domain_command_serialization_test_design',
      ]),
    );
    expect(designByContractCaseId.keys.toSet(), contractById.keys.toSet());

    for (final item in syncFfiCommandBoundaryRustHostTestDesignItems) {
      final contract = contractById[item.contractCaseId];
      final checklist = sourceChecklistById[item.sourceChecklistItemId];
      final shape = syncFfiCommandBoundaryRustHostTestDesignItemShape(item);

      expect(contract, isNotNull, reason: item.id);
      expect(checklist, isNotNull, reason: item.id);
      expect(
        reviewById[checklist!.reviewItemId]?.contractCaseId,
        item.contractCaseId,
        reason: item.id,
      );
      expect(shape['format'], syncFfiCommandBoundaryRustHostTestDesignFormat);
      expect(
        shape['review_status'],
        syncFfiCommandBoundaryRustHostTestDesignStatus,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(item.sampleIds, contract!.sampleIds, reason: item.id);
      expect(
        item.expectedStatusCodes,
        contract.expectedStatusCodes,
        reason: item.id,
      );
      expect(
        item.implementationGuards,
        syncFfiCommandBoundaryRustHostReviewStopLines,
        reason: item.id,
      );
      expect(
        item.implementationStatus,
        'test_design_ready_no_native_symbol',
        reason: item.id,
      );
      expect(item.sampleIdSummary, isNot('none'), reason: item.id);
      expect(item.expectedStatusSummary, isNot('none'), reason: item.id);
      expect(item.assertionSummary, isNot('none'), reason: item.id);
      expect(item.forbiddenOutputSummary, isNot('none'), reason: item.id);
      expect(item.guardSummary, isNot('none'), reason: item.id);
      expect(checklist.patternSourceRefs, isNotEmpty, reason: item.id);
      expect(checklist.sourceRefSummary, isNot('none'), reason: item.id);

      for (final sampleId in item.sampleIds) {
        final sample = sampleById[sampleId];
        expect(sample, isNotNull, reason: '${item.id}: $sampleId');
        expect(
          item.expectedStatusCodes,
          contains(sample!.expectedStatusCode),
          reason: '${item.id}: $sampleId',
        );
      }
    }
  });

  test('rust host test design remains review-only and non-sensitive', () {
    final encodedDesign = [
      for (final item in syncFfiCommandBoundaryRustHostTestDesignItems)
        jsonEncode(syncFfiCommandBoundaryRustHostTestDesignItemShape(item)),
    ].join('\n');

    expect(syncFfiCommandBoundaryCurrentNativeSymbols, isEmpty);
    expect(encodedDesign, contains('test_design_ready_no_native_symbol'));
    expect(encodedDesign, contains('no_native_symbol'));
    for (final symbol in syncFfiCommandBoundaryCandidateSymbols) {
      expect(encodedDesign, isNot(contains(symbol)), reason: symbol);
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedDesign,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedDesign, isNot(contains(fragment)));
    }
    expect(encodedDesign, isNot(contains('settings_action_payload')));
    expect(encodedDesign, isNot(contains('bridge_request_payload')));
    expect(encodedDesign, isNot(contains('connect_go_server_now')));
    expect(encodedDesign, isNot(contains('touch_platform_key_backend_now')));
  });

  test('rust host c abi review matrix covers test design items', () {
    final testDesignItemIds = syncFfiCommandBoundaryRustHostTestDesignItemIds()
        .toSet();
    final sourceChecklistItemIds =
        syncFfiCommandBoundaryRustHostSourceChecklistItemIds().toSet();
    final reviewDesignItemIds = {
      for (final item in syncFfiRustHostContractReviewItems)
        ...item.testDesignItemIds,
    };

    expect(
      syncFfiRustHostContractReviewItemIds(),
      containsAll([
        'request_struct_layout_review',
        'result_struct_layout_review',
        'release_and_error_lifecycle_review',
        'panic_and_status_boundary_review',
        'command_context_serialization_review',
        'forbidden_material_contract_review',
      ]),
    );
    expect(reviewDesignItemIds, containsAll(testDesignItemIds));

    for (final item in syncFfiRustHostContractReviewItems) {
      final shape = syncFfiRustHostContractReviewItemShape(item);

      expect(shape['format'], syncFfiRustHostContractReviewFormat);
      expect(shape['review_status'], syncFfiRustHostContractReviewStatus);
      expect(
        shape['decision_record_path'],
        syncFfiRustHostContractReviewDecisionRecordPath,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(
        item.implementationStatus,
        syncFfiRustHostContractReviewImplementationStatus,
        reason: item.id,
      );
      expect(
        item.implementationGuards,
        syncFfiCommandBoundaryRustHostReviewStopLines,
        reason: item.id,
      );
      expect(item.testDesignSummary, isNot('none'), reason: item.id);
      expect(item.sourceChecklistSummary, isNot('none'), reason: item.id);
      expect(item.decisionSummary, isNot('none'), reason: item.id);
      expect(item.evidenceSummary, isNot('none'), reason: item.id);
      expect(item.forbiddenOutputSummary, isNot('none'), reason: item.id);
      expect(item.guardSummary, isNot('none'), reason: item.id);

      for (final designItemId in item.testDesignItemIds) {
        expect(testDesignItemIds, contains(designItemId), reason: item.id);
      }
      for (final checklistItemId in item.sourceChecklistItemIds) {
        expect(
          sourceChecklistItemIds,
          contains(checklistItemId),
          reason: item.id,
        );
      }
    }
  });

  test('rust host c abi review matrix remains non-executable', () {
    final encodedReview = [
      for (final item in syncFfiRustHostContractReviewItems)
        jsonEncode(syncFfiRustHostContractReviewItemShape(item)),
    ].join('\n');

    expect(syncFfiCommandBoundaryCurrentNativeSymbols, isEmpty);
    expect(encodedReview, contains('no_native_symbol'));
    expect(encodedReview, contains('0006-manager-sync-c-abi'));
    expect(encodedReview, contains('c_abi_contract_review_ready'));
    for (final symbol in syncFfiCommandBoundaryCandidateSymbols) {
      expect(encodedReview, isNot(contains(symbol)), reason: symbol);
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedReview,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedReview, isNot(contains(fragment)));
    }
    expect(encodedReview, isNot(contains('settings_action_payload')));
    expect(encodedReview, isNot(contains('bridge_request_payload')));
    expect(encodedReview, isNot(contains('remote_request_body')));
    expect(encodedReview, isNot(contains('remote_response_body')));
    expect(encodedReview, isNot(contains('connect_go_server_now')));
    expect(encodedReview, isNot(contains('touch_platform_key_backend_now')));
  });

  test('rust host implementation review package binds c abi matrix', () {
    final contractReviewItemIds = syncFfiRustHostContractReviewItemIds()
        .toSet();
    final implementationReviewByContractId = {
      for (final item in syncFfiRustHostImplementationReviewItems)
        item.contractReviewItemId: item,
    };
    const allowedArtifactPrefixes = [
      'crates/ime-ffi/src/manager_sync_command.rs::',
    ];
    const allowedSourcePrefixes = [
      'crates/ime-ffi/src/',
      'apps/radishlex-manager/tool/',
      'apps/radishlex-manager/test/fixtures/',
    ];

    expect(
      syncFfiRustHostImplementationReviewItemIds(),
      containsAll([
        'request_struct_layout_implementation_review',
        'result_struct_layout_implementation_review',
        'release_and_error_lifecycle_implementation_review',
        'panic_and_status_boundary_implementation_review',
        'command_context_serialization_implementation_review',
        'forbidden_material_contract_implementation_review',
      ]),
    );
    expect(
      implementationReviewByContractId.keys.toSet(),
      contractReviewItemIds,
    );

    for (final item in syncFfiRustHostImplementationReviewItems) {
      final shape = syncFfiRustHostImplementationReviewItemShape(item);

      expect(shape['format'], syncFfiRustHostImplementationReviewFormat);
      expect(shape['review_status'], syncFfiRustHostImplementationReviewStatus);
      expect(
        shape['decision_record_path'],
        syncFfiRustHostContractReviewDecisionRecordPath,
      );
      expect(
        shape['target_test_file'],
        syncFfiCommandBoundaryRustHostContractTargetTestFile,
      );
      expect(
        contractReviewItemIds,
        contains(item.contractReviewItemId),
        reason: item.id,
      );
      expect(
        item.implementationStatus,
        syncFfiRustHostContractReviewImplementationStatus,
        reason: item.id,
      );
      expect(
        item.implementationGuards,
        syncFfiCommandBoundaryRustHostReviewStopLines,
        reason: item.id,
      );
      expect(item.proposedArtifactSummary, isNot('none'), reason: item.id);
      expect(item.reusedSourceRefSummary, isNot('none'), reason: item.id);
      expect(item.implementationNoteSummary, isNot('none'), reason: item.id);
      expect(item.unresolvedQuestionSummary, isNot('none'), reason: item.id);
      expect(item.guardSummary, isNot('none'), reason: item.id);

      for (final artifact in item.proposedRustArtifacts) {
        expect(artifact, contains('::'), reason: '${item.id}: $artifact');
        expect(
          allowedArtifactPrefixes.any(artifact.startsWith),
          isTrue,
          reason: '${item.id}: $artifact',
        );
        expect(artifact, contains('Draft'), reason: '${item.id}: $artifact');
      }
      for (final sourceRef in item.reusedSourceRefs) {
        expect(sourceRef, contains('::'), reason: '${item.id}: $sourceRef');
        expect(
          allowedSourcePrefixes.any(sourceRef.startsWith),
          isTrue,
          reason: '${item.id}: $sourceRef',
        );
      }
    }
  });

  test('rust host implementation review package remains non-executable', () {
    final encodedImplementationReview = [
      for (final item in syncFfiRustHostImplementationReviewItems)
        jsonEncode(syncFfiRustHostImplementationReviewItemShape(item)),
    ].join('\n');

    expect(syncFfiCommandBoundaryCurrentNativeSymbols, isEmpty);
    expect(encodedImplementationReview, contains('no_native_symbol'));
    expect(
      encodedImplementationReview,
      contains('rust_host_implementation_review_ready_no_native_symbol'),
    );
    for (final symbol in syncFfiCommandBoundaryCandidateSymbols) {
      expect(
        encodedImplementationReview,
        isNot(contains(symbol)),
        reason: symbol,
      );
    }
    for (final rejected in syncBridgeCommandContractRejectedSamples) {
      expect(
        encodedImplementationReview,
        isNot(contains(rejected.forbiddenFragment)),
        reason: rejected.id,
      );
    }
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedImplementationReview, isNot(contains(fragment)));
    }
    expect(
      encodedImplementationReview,
      isNot(contains('settings_action_payload')),
    );
    expect(
      encodedImplementationReview,
      isNot(contains('bridge_request_payload')),
    );
    expect(encodedImplementationReview, isNot(contains('remote_request_body')));
    expect(
      encodedImplementationReview,
      isNot(contains('remote_response_body')),
    );
    expect(
      encodedImplementationReview,
      isNot(contains('connect_go_server_now')),
    );
    expect(
      encodedImplementationReview,
      isNot(contains('touch_platform_key_backend_now')),
    );
  });

  test(
    'rust internal draft result accessor evidence matches result fields',
    () {
      final accessorFieldNames = {
        for (final item in syncFfiRustHostResultAccessorFieldSetItems)
          item.fieldName,
      };
      final encodedFieldSet = [
        for (final item in syncFfiRustHostResultAccessorFieldSetItems)
          jsonEncode(syncFfiRustHostResultAccessorFieldSetItemShape(item)),
      ].join('\n');

      expect(
        accessorFieldNames,
        syncFfiCommandBoundaryResultSummaryFields.toSet(),
      );
      expect(
        syncFfiRustHostResultAccessorFieldSetItems.length,
        syncFfiCommandBoundaryResultSummaryFields.length,
      );
      expect(
        syncFfiRustHostResultAccessorFieldSetItems
            .where((item) => item.fieldName == 'object_count_summary')
            .single
            .numericWidth,
        'u64',
      );
      expect(
        syncFfiRustHostResultAccessorFieldSetItems
            .where((item) => item.numericWidth == 'u64')
            .single
            .fieldName,
        'object_count_summary',
      );
      expect(
        syncFfiRustHostResultAccessorFieldSetItems
            .where((item) => item.valueKind == 'summary_code')
            .map((item) => item.storagePolicy)
            .toSet(),
        {'handle_owned_or_static_summary'},
      );
      expect(
        encodedFieldSet,
        contains(syncFfiRustHostResultAccessorFieldSetReviewStatus),
      );
      expect(encodedFieldSet, contains('planned_not_exported_current_phase'));
      expect(encodedFieldSet, isNot(contains('payload_bytes')));
      expect(encodedFieldSet, isNot(contains('request_body')));
      expect(encodedFieldSet, isNot(contains('response_body')));
      for (final rejected in syncBridgeCommandContractRejectedSamples) {
        expect(
          encodedFieldSet,
          isNot(contains(rejected.forbiddenFragment)),
          reason: rejected.id,
        );
      }
    },
  );

  test('rust internal draft owner worker and gate evidence stays closed', () {
    final ownerShape = syncFfiRustHostCommandContextOwnerScopeReviewShape(
      syncFfiRustHostCommandContextOwnerScopeReview,
    );
    final workerShape = syncFfiRustHostCommandWorkerThreadPolicyReviewShape(
      syncFfiRustHostCommandWorkerThreadPolicyReview,
    );
    final gateShape = syncFfiRustHostGateMigrationReviewShape();
    final encoded = jsonEncode({
      'owner': ownerShape,
      'worker': workerShape,
      'gate': gateShape,
    });

    expect(
      ownerShape['review_status'],
      syncFfiRustHostCommandContextOwnerScopeReviewStatus,
    );
    expect(ownerShape['cross_thread_status'], 'InvalidState');
    expect(ownerShape['owns_flutter_widget_state'], isFalse);
    expect(ownerShape['owns_settings_payload'], isFalse);
    expect(ownerShape['owns_dart_pointer'], isFalse);
    expect(ownerShape['owns_platform_ui_object'], isFalse);

    expect(
      workerShape['review_status'],
      syncFfiRustHostCommandWorkerThreadPolicyReviewStatus,
    );
    expect(
      workerShape['future_worker_policy'],
      'single_serial_manager_sync_worker_before_real_sync',
    );
    expect(workerShape['allows_background_remote_retry'], isFalse);
    expect(workerShape['queues_secret_payload'], isFalse);
    expect(workerShape['blocks_flutter_ui_isolate'], isFalse);

    expect(
      gateShape['review_status'],
      syncFfiRustHostGateMigrationReviewStatus,
    );
    expect(gateShape['ready_for_host_contract_test'], isFalse);
    expect(
      gateShape['required_conditions'],
      containsAll([
        'result_accessor_field_set_reviewed',
        'summary_storage_policy_reviewed',
        'object_summary_numeric_width_reviewed',
        'command_context_owner_scope_reviewed',
        'command_worker_thread_policy_reviewed',
        'debug_redaction_test_shape_reviewed',
        'forbidden_material_redaction_reviewed',
        'native_symbol_export_approved_by_adr',
        'dart_native_binding_approved',
        'manager_bridge_command_approved',
        'host_contract_test_file_approved',
        'ffi_smoke_candidate_symbols_absent_until_approval',
        'real_sync_execution_approved_after_gate',
      ]),
    );
    expect(
      gateShape['required_conditions'],
      containsAll(syncFfiRustHostGateReadinessReviewedConditions),
    );
    expect(
      gateShape['required_conditions'],
      containsAll(syncFfiRustHostGateReadinessBlockingConditions),
    );
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });

  test('rust internal draft gate readiness review separates blockers', () {
    final readinessShape = syncFfiRustHostGateReadinessReviewShape(
      syncFfiRustHostGateReadinessReview,
    );
    final gateShape = syncFfiRustHostGateMigrationReviewShape();
    final encoded = jsonEncode({
      'readiness': readinessShape,
      'gate': gateShape,
    });

    expect(
      readinessShape['review_status'],
      syncFfiRustHostGateReadinessReviewStatus,
    );
    expect(
      readinessShape['readiness_state'],
      syncFfiRustHostGateReadinessState,
    );
    expect(readinessShape['ready_for_host_contract_test'], isFalse);
    expect(
      readinessShape['dart_fake_replay_status'],
      syncFfiRustHostGateMigrationReplayStatus,
    );
    expect(
      readinessShape['reviewed_conditions'],
      syncFfiRustHostGateReadinessReviewedConditions,
    );
    expect(
      readinessShape['blocking_conditions'],
      syncFfiRustHostGateReadinessBlockingConditions,
    );
    expect(
      syncFfiRustHostGateReadinessReviewedConditions.toSet().intersection(
        syncFfiRustHostGateReadinessBlockingConditions.toSet(),
      ),
      isEmpty,
    );
    expect(
      gateShape['required_conditions'],
      containsAll(syncFfiRustHostGateReadinessReviewedConditions),
    );
    expect(
      gateShape['required_conditions'],
      containsAll(syncFfiRustHostGateReadinessBlockingConditions),
    );
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });

  test('rust host gate migration replay cases remain closed', () {
    final gateConditions =
        (syncFfiRustHostGateMigrationReviewShape()['required_conditions']
                as List<String>)
            .toSet();
    final coveredMissingConditions = {
      for (final replayCase in syncFfiRustHostGateMigrationReplayCases)
        ...replayCase.simulatedMissingConditions,
    };
    final encodedReplayCases = [
      for (final replayCase in syncFfiRustHostGateMigrationReplayCases)
        jsonEncode(syncFfiRustHostGateMigrationReplayCaseShape(replayCase)),
    ].join('\n');

    expect(
      coveredMissingConditions,
      containsAll(syncFfiRustHostGateReadinessBlockingConditions),
    );
    expect(
      coveredMissingConditions,
      contains('ffi_smoke_candidate_symbols_absent_until_approval'),
    );
    for (final replayCase in syncFfiRustHostGateMigrationReplayCases) {
      final shape = syncFfiRustHostGateMigrationReplayCaseShape(replayCase);

      expect(
        shape['review_status'],
        syncFfiRustHostGateMigrationReplayStatus,
        reason: replayCase.id,
      );
      expect(
        replayCase.expectedReadinessState,
        syncFfiRustHostGateReadinessState,
        reason: replayCase.id,
      );
      expect(
        syncBridgeCommandContractCommandStatuses,
        contains(replayCase.expectedCommandStatus),
        reason: replayCase.id,
      );
      expect(
        syncFfiCommandBoundaryAllCommandErrorCodes(),
        contains(replayCase.expectedErrorCode),
        reason: replayCase.id,
      );
      expect(
        syncBridgeCommandContractRetryPolicies,
        contains(replayCase.expectedRetryPolicy),
        reason: replayCase.id,
      );
      for (final condition in replayCase.simulatedMissingConditions) {
        expect(gateConditions, contains(condition), reason: replayCase.id);
      }
      expect(shape['can_create_host_contract_test_file'], isFalse);
      expect(shape['can_export_native_symbol'], isFalse);
      expect(shape['can_modify_manager_bridge'], isFalse);
      expect(shape['can_execute_real_sync'], isFalse);
    }

    expect(encodedReplayCases, contains('no_native_symbol'));
    expect(encodedReplayCases, isNot(contains('settings_action_payload')));
    expect(encodedReplayCases, isNot(contains('bridge_request_payload')));
    expect(encodedReplayCases, isNot(contains('remote_request_body')));
    expect(encodedReplayCases, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedReplayCases, isNot(contains(fragment)));
    }
  });

  test('rust host test admission review keeps real host test blocked', () {
    final admissionShape = syncFfiRustHostTestAdmissionReviewShape(
      syncFfiRustHostTestAdmissionReview,
    );
    final readinessShape = syncFfiRustHostGateReadinessReviewShape(
      syncFfiRustHostGateReadinessReview,
    );
    final encoded = jsonEncode({
      'admission': admissionShape,
      'readiness': readinessShape,
    });

    expect(
      admissionShape['review_status'],
      syncFfiRustHostTestAdmissionReviewStatus,
    );
    expect(
      admissionShape['admission_decision'],
      syncFfiRustHostTestAdmissionDecision,
    );
    expect(
      admissionShape['next_decision_required'],
      syncFfiRustHostTestAdmissionNextDecision,
    );
    expect(admissionShape['ready_for_host_contract_test'], isFalse);
    expect(admissionShape['can_create_host_contract_test_file'], isFalse);
    expect(admissionShape['can_export_native_symbol'], isFalse);
    expect(admissionShape['can_modify_manager_bridge'], isFalse);
    expect(admissionShape['can_execute_real_sync'], isFalse);
    expect(
      admissionShape['satisfied_conditions'],
      syncFfiRustHostGateReadinessReviewedConditions,
    );
    expect(
      admissionShape['blocking_conditions'],
      syncFfiRustHostGateReadinessBlockingConditions,
    );
    expect(
      admissionShape['evidence_sources'],
      containsAll([
        'adr_0006_current',
        'manager_sync_command_internal_draft_tests',
        'dart_fake_native_gate_migration_replay',
        'ffi_bridge_smoke_candidate_symbols_absent',
      ]),
    );
    expect(
      syncFfiRustHostGateReadinessReviewedConditions.toSet().intersection(
        syncFfiRustHostGateReadinessBlockingConditions.toSet(),
      ),
      isEmpty,
    );
    expect(encoded, contains('no_native_symbol'));
    expect(
      encoded,
      isNot(contains('radishlex_manager_sync_command_execute_v1')),
    );
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });

  test('rust host test admission gaps cover each remaining blocker', () {
    final gateConditions =
        (syncFfiRustHostGateMigrationReviewShape()['required_conditions']
                as List<String>)
            .toSet();
    final gapConditions = {
      for (final gapItem in syncFfiRustHostTestAdmissionGapItems)
        gapItem.condition,
    };
    final replayBlockerCodes = {
      for (final replayCase in syncFfiRustHostGateMigrationReplayCases)
        replayCase.expectedBlockerCode,
    };
    final encodedGapItems = [
      for (final gapItem in syncFfiRustHostTestAdmissionGapItems)
        jsonEncode(syncFfiRustHostTestAdmissionGapItemShape(gapItem)),
    ].join('\n');

    expect(
      gapConditions,
      syncFfiRustHostGateReadinessBlockingConditions.toSet(),
    );
    expect(
      replayBlockerCodes,
      containsAll(
        syncFfiRustHostTestAdmissionGapItems.map((item) => item.blockerCode),
      ),
    );
    for (final gapItem in syncFfiRustHostTestAdmissionGapItems) {
      final shape = syncFfiRustHostTestAdmissionGapItemShape(gapItem);

      expect(
        shape['review_status'],
        syncFfiRustHostTestAdmissionReviewStatus,
        reason: gapItem.condition,
      );
      expect(gateConditions, contains(gapItem.condition));
      expect(shape['condition'], gapItem.condition);
      expect(shape['blocker_code'], gapItem.blockerCode);
      expect(shape['required_decision'], gapItem.requiredDecision);
      expect(shape['evidence_source'], gapItem.evidenceSource);
      expect(shape['can_create_host_contract_test_file'], isFalse);
      expect(shape['can_export_native_symbol'], isFalse);
      expect(shape['can_modify_manager_bridge'], isFalse);
      expect(shape['can_execute_real_sync'], isFalse);
    }

    expect(encodedGapItems, contains('no_native_symbol'));
    expect(
      encodedGapItems,
      isNot(contains('radishlex_manager_sync_command_execute_v1')),
    );
    expect(encodedGapItems, isNot(contains('settings_action_payload')));
    expect(encodedGapItems, isNot(contains('bridge_request_payload')));
    expect(encodedGapItems, isNot(contains('remote_request_body')));
    expect(encodedGapItems, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encodedGapItems, isNot(contains(fragment)));
    }
  });

  test('rust internal draft storage and debug redaction evidence is safe', () {
    final storageShape = syncFfiRustHostSummaryStorageReviewShape(
      syncFfiRustHostSummaryStorageReview,
    );
    final debugShape = syncFfiRustHostDebugRedactionReviewShape(
      syncFfiRustHostDebugRedactionReview,
    );
    final encoded = jsonEncode({'storage': storageShape, 'debug': debugShape});

    expect(
      storageShape['review_status'],
      syncFfiRustHostSummaryStorageReviewStatus,
    );
    expect(storageShape['current_phase_storage'], 'static_summary_code_table');
    expect(
      storageShape['future_dynamic_storage'],
      'rust_owned_result_handle_storage',
    );
    expect(storageShape['stores_caller_pointer'], isFalse);
    expect(storageShape['stores_provider_message'], isFalse);
    expect(storageShape['stores_transport_payload'], isFalse);

    expect(
      debugShape['review_status'],
      syncFfiRustHostDebugRedactionReviewStatus,
    );
    expect(
      debugShape['debug_targets'],
      containsAll([
        'request_error',
        'command_result',
        'result_accessor_field',
        'owner_scope_review',
        'worker_policy_review',
        'gate_migration_review',
        'host_gate_readiness_review',
        'host_test_admission_review',
      ]),
    );
    expect(
      debugShape['forbidden_categories'],
      containsAll([
        'token_material',
        'recovery_secret_material',
        'join_verifier_material',
        'signature_or_wrapped_sync_material',
        'opaque_transport_content',
        'local_path_material',
      ]),
    );
    expect(encoded, isNot(contains('Bearer ')));
    expect(encoded, isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')));
    expect(encoded, isNot(contains('short_code=')));
    expect(encoded, isNot(contains('signature_bytes')));
    expect(encoded, isNot(contains('wrapped_material_bytes')));
    expect(encoded, isNot(contains('payload_bytes=')));
    expect(encoded, isNot(contains('/synthetic/private')));
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
