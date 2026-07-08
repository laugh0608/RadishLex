import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import '../fixtures/sync_bridge_command_contract_fixtures.dart';
import '../fixtures/sync_ffi_rust_host_contract_review_fixtures.dart';
import '../fixtures/sync_ffi_rust_host_migration_review_fixtures.dart';

void main() {
  test('rust host export approval review maps symbols without exporting', () {
    final resultFieldNames = {
      for (final fixture in syncFfiRustHostResultAccessorFieldSetItems)
        fixture.fieldName,
      'result_handle_lifecycle',
    };
    final exportShape = syncFfiRustHostExportApprovalReviewShape(
      syncFfiRustHostExportApprovalReview,
    );
    final symbolShapes = exportShape['symbol_reviews'] as List<Object?>;
    final encoded = jsonEncode(exportShape);

    expect(
      exportShape['review_status'],
      syncFfiRustHostExportApprovalReviewStatus,
    );
    expect(
      exportShape['export_decision'],
      syncFfiRustHostExportApprovalDecision,
    );
    expect(exportShape['can_export_native_symbol'], isFalse);
    expect(exportShape['can_create_host_contract_test_file'], isFalse);
    expect(exportShape['can_modify_dart_native_binding'], isFalse);
    expect(exportShape['can_modify_manager_bridge'], isFalse);
    expect(
      exportShape['required_evidence'],
      containsAll([
        'adr_0006_current',
        'result_accessor_field_set_review',
        'result_handle_copy_then_free_review',
        'panic_boundary_review',
        'ffi_bridge_smoke_candidate_symbols_absent',
      ]),
    );
    expect(
      symbolShapes.map(
        (shape) => (shape as Map<String, Object?>)['symbol_name'],
      ),
      containsAll([
        'radishlex_manager_sync_command_execute_v1',
        'radishlex_manager_sync_command_result_action_id',
        'radishlex_manager_sync_command_result_status',
        'radishlex_manager_sync_command_result_error_code',
        'radishlex_manager_sync_command_result_retry_policy',
        'radishlex_manager_sync_command_result_summary',
        'radishlex_manager_sync_command_result_free',
      ]),
    );
    for (final shape in symbolShapes.cast<Map<String, Object?>>()) {
      expect(shape['review_status'], syncFfiRustHostExportApprovalReviewStatus);
      expect(shape['export_state'], 'planned_not_exported_current_phase');
      expect(shape['export_approved'], isFalse);
      expect(shape['smoke_requirement'], 'symbol_absent_until_export_approval');
      expect(shape['release_responsibility'], isNotEmpty);
      expect(shape['error_boundary'], isNotEmpty);
      expect(shape['panic_boundary'], isNotEmpty);
      for (final field in shape['required_contract_fields'] as List<String>) {
        expect(
          resultFieldNames,
          contains(field),
          reason: '${shape['symbol_name']}',
        );
      }
    }

    expect(encoded, contains('no_native_symbol'));
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });

  test('rust host binding and bridge migration reviews remain blocked', () {
    final bindingShape = syncFfiRustHostDartBindingMigrationReviewShape(
      syncFfiRustHostDartBindingMigrationReview,
    );
    final bridgeShape = syncFfiRustHostManagerBridgeMigrationReviewShape(
      syncFfiRustHostManagerBridgeMigrationReview,
    );
    final encoded = jsonEncode({
      'binding': bindingShape,
      'bridge': bridgeShape,
    });

    expect(
      bindingShape['review_status'],
      syncFfiRustHostDartBindingMigrationReviewStatus,
    );
    expect(
      bindingShape['migration_decision'],
      syncFfiRustHostDartBindingMigrationDecision,
    );
    expect(
      bindingShape['reviewed_conditions'],
      containsAll([
        'dart_copy_free_contract_reviewed',
        'unknown_native_status_mapping_reviewed',
        'ffi_command_error_mapping_reviewed',
        'forbidden_material_redaction_reviewed',
      ]),
    );
    expect(
      bindingShape['blocking_conditions'],
      containsAll([
        'native_symbol_export_approved_by_adr',
        'dart_native_binding_approved',
        'manager_bridge_command_approved',
      ]),
    );
    expect(bindingShape['can_modify_dart_native_binding'], isFalse);
    expect(bindingShape['can_call_native_command'], isFalse);
    expect(bindingShape['can_hold_native_pointer_after_copy'], isFalse);
    expect(bindingShape['can_write_settings_action'], isFalse);
    expect(bindingShape['can_modify_manager_bridge'], isFalse);

    expect(
      bridgeShape['review_status'],
      syncFfiRustHostManagerBridgeMigrationReviewStatus,
    );
    expect(
      bridgeShape['migration_decision'],
      syncFfiRustHostManagerBridgeMigrationDecision,
    );
    expect(
      bridgeShape['reviewed_conditions'],
      containsAll([
        'action_intent_mapping_reviewed',
        'settings_draft_write_absent_reviewed',
        'diagnostics_redaction_reviewed',
      ]),
    );
    expect(
      bridgeShape['blocking_conditions'],
      containsAll([
        'manager_bridge_command_approved',
        'host_contract_test_file_approved',
        'real_sync_execution_approved_after_gate',
      ]),
    );
    expect(bridgeShape['can_modify_manager_bridge'], isFalse);
    expect(bridgeShape['can_create_bridge_command_request'], isFalse);
    expect(bridgeShape['can_write_settings_action'], isFalse);
    expect(bridgeShape['can_execute_real_sync'], isFalse);

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

  test('rust host test file approval review keeps test file uncreated', () {
    final shape = syncFfiRustHostTestFileApprovalReviewShape(
      syncFfiRustHostTestFileApprovalReview,
    );
    final encoded = jsonEncode(shape);

    expect(shape['review_status'], syncFfiRustHostTestFileApprovalReviewStatus);
    expect(shape['approval_decision'], syncFfiRustHostTestFileApprovalDecision);
    expect(
      shape['target_test_file'],
      'crates/ime-ffi/tests/manager_sync_command_boundary.rs',
    );
    expect(
      shape['planned_test_cases'],
      containsAll([
        'contract_reports_command_capability_closed',
        'result_handle_copy_then_free',
        'forbidden_material_absent_from_native_outputs',
        'sync_domain_command_serialization',
      ]),
    );
    expect(
      shape['reviewed_conditions'],
      containsAll([
        'c_abi_symbol_lookup_strategy_reviewed',
        'capability_missing_behavior_reviewed',
        'dynamic_library_smoke_absence_reviewed',
      ]),
    );
    expect(
      shape['blocking_conditions'],
      containsAll([
        'native_symbol_export_approved_by_adr',
        'dart_native_binding_approved',
        'manager_bridge_command_approved',
        'host_contract_test_file_approved',
      ]),
    );
    expect(
      shape['required_evidence'],
      containsAll([
        'host_test_design_package_current',
        'c_abi_contract_review_matrix_current',
        'ffi_bridge_smoke_candidate_symbols_absent',
      ]),
    );
    expect(shape['can_create_host_contract_test_file'], isFalse);
    expect(shape['can_export_native_symbol'], isFalse);
    expect(shape['can_call_dynamic_library_symbol'], isFalse);
    expect(shape['can_modify_dart_native_binding'], isFalse);

    expect(encoded, contains('no_native_symbol'));
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });

  test('rust host real sync execution gate blocks remote side effects', () {
    final shape = syncFfiRustHostRealSyncExecutionGateReviewShape(
      syncFfiRustHostRealSyncExecutionGateReview,
    );
    final encoded = jsonEncode(shape);

    expect(
      shape['review_status'],
      syncFfiRustHostRealSyncExecutionGateReviewStatus,
    );
    expect(
      shape['execution_decision'],
      syncFfiRustHostRealSyncExecutionDecision,
    );
    expect(
      shape['reviewed_conditions'],
      containsAll([
        'command_context_owner_scope_reviewed',
        'command_worker_thread_policy_reviewed',
        'readiness_snapshot_binding_reviewed',
        'deployment_evidence_summary_reviewed',
      ]),
    );
    expect(
      shape['blocking_conditions'],
      containsAll([
        'host_contract_test_file_approved',
        'platform_private_key_backend_production_ready',
        'recovery_authorization_interaction_tests_passed',
        'deployment_evidence_summary_approved',
        'real_sync_execution_approved_after_gate',
      ]),
    );
    expect(
      shape['required_evidence'],
      containsAll([
        'platform_private_key_backend_strategy_current',
        'manager_recovery_device_auth_flow_current',
        'sync_server_production_deployment_runbook_current',
        'local_docker_https_smoke_development_only',
      ]),
    );
    expect(shape['can_execute_real_sync'], isFalse);
    expect(shape['can_connect_go_server'], isFalse);
    expect(shape['can_touch_platform_key_backend'], isFalse);
    expect(shape['can_generate_recovery_code'], isFalse);
    expect(shape['can_create_join_request'], isFalse);
    expect(shape['can_revoke_device'], isFalse);

    expect(encoded, contains('no_remote_sync'));
    expect(encoded, isNot(contains('settings_action_payload')));
    expect(encoded, isNot(contains('bridge_request_payload')));
    expect(encoded, isNot(contains('remote_request_body')));
    expect(encoded, isNot(contains('remote_response_body')));
    for (final fragment in syncBridgeCommandContractForbiddenFragments) {
      expect(encoded, isNot(contains(fragment)));
    }
  });
}
