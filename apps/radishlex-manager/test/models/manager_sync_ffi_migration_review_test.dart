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
}
