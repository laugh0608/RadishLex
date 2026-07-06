import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  test('readiness scenario catalog drives diagnostics summaries', () {
    for (final scenario in syncReadinessScenarioCatalog()) {
      final report = createManagerDiagnosticsReport(
        managerSnapshotForSyncReadinessScenario(scenario),
      );
      final text = report.toRedactedText();

      expect(
        _diagnosticsValue(report, 'sync.readiness_bridge_source'),
        scenario.expectedBridgeSource,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.entry_blocker'),
        scenario.expectedEntryBlocker,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.readiness_blocked_flows'),
        scenario.expectedBlockedFlows,
        reason: scenario.id,
      );
      if (scenario.expectedIssueCodes != null) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_issue_codes'),
          scenario.expectedIssueCodes,
          reason: scenario.id,
        );
      }
      for (final code in scenario.expectedIssueCodeContains) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_issue_codes'),
          contains(code),
          reason: scenario.id,
        );
      }
      for (final fragment in scenario.forbiddenIssueCodeFragments) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_issue_codes'),
          isNot(contains(fragment)),
          reason: scenario.id,
        );
      }
      if (scenario.expectedNextEvidence != null) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_next_required_evidence'),
          scenario.expectedNextEvidence,
          reason: scenario.id,
        );
      }
      for (final code in scenario.expectedNextEvidenceContains) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_next_required_evidence'),
          contains(code),
          reason: scenario.id,
        );
      }
      for (final fragment in scenario.forbiddenNextEvidenceFragments) {
        expect(
          _diagnosticsValue(report, 'sync.readiness_next_required_evidence'),
          isNot(contains(fragment)),
          reason: scenario.id,
        );
      }
      expect(
        _diagnosticsValue(report, 'sync.interaction_statuses'),
        scenario.expectedInteractionStatuses,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.interaction_blockers'),
        scenario.expectedInteractionBlockers,
        reason: scenario.id,
      );
      _expectDiagnosticsActionCommandPreview(
        report,
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
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.user_sync_enabled'),
        scenario.expectedUserSyncEnabled.toString(),
        reason: scenario.id,
      );
      expect(text, isNot(contains('secret-token')), reason: scenario.id);
      expect(
        text,
        isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')),
        reason: scenario.id,
      );
      expect(text, isNot(contains('/synthetic/private')), reason: scenario.id);
      expect(
        text,
        isNot(contains('payload_bytes=abcdef')),
        reason: scenario.id,
      );
      expect(
        text,
        isNot(contains('wrapped_material_bytes=abcdef')),
        reason: scenario.id,
      );
      expect(
        text,
        isNot(contains('signature_bytes=abcdef')),
        reason: scenario.id,
      );
      expect(text, isNot(contains('private_key=abcdef')), reason: scenario.id);
      expect(text, isNot(contains('short_code=')), reason: scenario.id);
    }
  });

  test('sync evidence bundle catalog drives diagnostics summaries', () {
    for (final scenario in syncEvidenceBundleScenarioCatalog()) {
      final report = createManagerDiagnosticsReport(
        managerSnapshotForSyncEvidenceBundleScenario(scenario),
      );
      final text = report.toRedactedText();

      expect(
        _diagnosticsValue(report, 'sync.entry_state'),
        scenario.expectedEntryState.code,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.entry_blocker'),
        scenario.expectedEntryBlocker,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.readiness_bridge_source'),
        scenario.expectedBridgeSource,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.readiness_blocked_flows'),
        scenario.expectedBlockedFlows,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.interaction_statuses'),
        scenario.expectedInteractionStatuses,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.interaction_blockers'),
        scenario.expectedInteractionBlockers,
        reason: scenario.id,
      );
      _expectDiagnosticsActionCommandPreview(
        report,
        expectedIntentStatusSummary: scenario.expectedInteractionStatuses,
        expectedBlockerSummary: scenario.expectedInteractionBlockers,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.user_sync_enabled'),
        scenario.expectedUserSyncEnabled.toString(),
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.connection_status'),
        scenario.expectedConnectionStatusCode,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.connection_blocker'),
        scenario.expectedConnectionBlocker,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.connection_probe_source'),
        scenario.expectedConnectionProbeSource,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.connection_probe_recorded_at'),
        syncEvidenceBundleRecordedAt,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.endpoint_status'),
        scenario.expectedEndpointStatus,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.access_token_status'),
        scenario.expectedAccessTokenStatus,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.transport_mode'),
        scenario.expectedTransportMode,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.server_state_status'),
        scenario.expectedServerStateStatus,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.connection_auth_status'),
        scenario.expectedAuthStatus,
        reason: scenario.id,
      );
      expect(
        '${_diagnosticsValue(report, 'sync.connection_http_status')} '
        '${_diagnosticsValue(report, 'sync.connection_http_status_class')}',
        scenario.expectedHttpStatusText,
        reason: scenario.id,
      );
      expect(
        _diagnosticsValue(report, 'sync.last_remote_error_code'),
        scenario.expectedLastRemoteErrorCode,
        reason: scenario.id,
      );
      for (final fragment in syncEvidenceBundleSensitiveLeakFragments) {
        expect(text, isNot(contains(fragment)), reason: scenario.id);
      }
    }
  });

  testWidgets('settings view previews and exports diagnostics report', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _DiagnosticsRecordingBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    await tester.ensureVisible(
      find.byKey(const Key('diagnostics-preview-button')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('diagnostics-preview-button')));
    await tester.pumpAndSettle();

    expect(find.text('诊断摘要预览'), findsOneWidget);
    expect(find.text('分组 6'), findsOneWidget);
    expect(find.text('字段 124'), findsOneWidget);
    expect(
      find.byKey(const Key('diagnostics-section-sync_gate')),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('diagnostics-item-sync.state_source')),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('diagnostics-item-sync.entry_state')),
      findsOneWidget,
    );
    expect(find.textContaining('runtime.bridge_mode: fixture'), findsOneWidget);
    expect(
      find.textContaining('sync.state_source: 设备 production gate 为 blocked'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.entry_state: backend_unavailable'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.local_evidence_source: not_recorded'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_blocked_flows: recovery_setup, recovery_restore, device_join, device_revocation',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_issue_codes: recovery_code_generation_closed, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent, recovery_code_input_closed, recovery_code_invalid, authentication_required, network_unreachable, join_request_creation_closed, join_request_expired, authorization_rejected, device_revoked, backend_unavailable, device_revocation_flow_closed, key_epoch_rotation_required',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_next_required_evidence: platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required, input_not_available_current_phase, not_checked_current_phase, blocked_until_recovery_success, join_request_unavailable, short_code_verification_not_started, active_existing_device_required, join_request_pending_required, short_code_match_required, lost_device_prior_material_not_recallable, key_epoch_rotation_not_started',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_source_tags: recovery_setup_readiness, recovery_restore_readiness, device_join_readiness, device_revocation_readiness',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_bridge_source: manager_default_closed_readiness',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.readiness_user_sync_blocked: true'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_actions: recovery_setup, recovery_restore, join_request_authorization, device_revocation',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_visibility: recovery_setup=visible, recovery_restore=visible, join_request_authorization=visible, device_revocation=visible',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_statuses: recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_blockers: recovery_code_generation_closed, recovery_code_input_closed, join_request_creation_closed, device_revocation_flow_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_required_evidence: platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required, input_not_available_current_phase, not_checked_current_phase, blocked_until_recovery_success, join_request_unavailable, short_code_verification_not_started, active_existing_device_required, join_request_pending_required, short_code_match_required, lost_device_prior_material_not_recallable, key_epoch_rotation_not_started',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.interaction_source_tags: recovery_setup_readiness, recovery_restore_readiness, device_join_readiness, device_revocation_readiness',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_format: manager_sync_action_command_preview.v1',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_intent_statuses: recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_execution_statuses: recovery_setup=not_executable_current_phase, recovery_restore=not_executable_current_phase, join_request_authorization=not_executable_current_phase, device_revocation=not_executable_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_data_policy: $syncActionCommandDataPolicySummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_stop_lines: $syncActionCommandStopLineSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_request_boundaries: $syncActionCommandRequestBoundarySummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_result_boundaries: $syncActionCommandResultBoundarySummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_command_error_codes: $syncActionCommandErrorCodeSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_request_statuses: ${syncActionExpectedRequestStatusSummary(syncReadinessClosedInteractionStatuses)}',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_request_allowed_fields: $syncActionRequestAllowedFieldSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_request_forbidden_material: $syncActionForbiddenMaterialSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_result_statuses: ${syncActionExpectedResultStatusSummary(syncReadinessClosedInteractionStatuses)}',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_result_allowed_fields: $syncActionResultAllowedFieldSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.action_result_forbidden_material: $syncActionForbiddenMaterialSummary',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.recovery_blocker: recovery_code_flow_closed'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_save_confirmation: required_before_first_upload',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_record_status: recovery_record_not_created',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_record_blocker: recovery_record_creation_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_first_upload_gate: blocked_until_recovery_code_saved',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_setup_status: recovery_setup_flow_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_setup_blocker: recovery_code_generation_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_setup_action_status: read_only_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_setup_error_codes: recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_restore_status: recovery_restore_flow_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.recovery_restore_error_codes: recovery_code_required, recovery_code_invalid, recovery_record_missing, recovery_record_revoked, authentication_required, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_authorization_blocker: device_authorization_flow_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.join_request_status: join_request_unavailable'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.authorization_package_status: authorization_package_not_created',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.authorization_package_blocker: authorization_package_prerequisites_blocked',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_revocation_status: closed_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.lost_device_risk: lost_device_prior_material_not_recallable',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.key_epoch_status: key_epoch_rotation_not_started',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.device_join_status: device_join_flow_closed'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_join_blocker: join_request_creation_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_join_short_code: short_code_verification_not_started',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_join_error_codes: join_request_expired, authorization_rejected, device_revoked, backend_unavailable, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_revocation_flow_status: device_revocation_flow_closed',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.device_revocation_error_codes: device_revoked, key_epoch_rotation_required, local_data_inconsistent, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('settings.access_token: not_configured'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_status: access_token_missing'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_probe_source: not_recorded'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_probe_recorded_at: not_recorded'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.server_state_status: not_checked_access_token_missing',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_auth_status: not_checked'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_http_status: 0'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_http_status_class: not_checked'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_local_insecure_tls: not_checked'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.last_remote_error_code: none'),
      findsOneWidget,
    );
    expect(
      find.textContaining('redaction.user_terms: omitted'),
      findsOneWidget,
    );
    final reportText = tester.widget<SelectableText>(
      find.byKey(const Key('diagnostics-report-text')),
    );
    expect(reportText.data, isNot(contains('萝卜词核')));

    await tester.tap(find.byKey(const Key('diagnostics-section-sync_gate')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const Key('diagnostics-item-sync.state_source')),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('diagnostics-item-runtime.bridge_mode')),
      findsNothing,
    );

    await tester.tap(find.byKey(const Key('diagnostics-section-all')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('diagnostics-report-filter')),
      'redaction.tokens',
    );
    await tester.pumpAndSettle();

    expect(
      find.byKey(const Key('diagnostics-item-redaction.tokens')),
      findsOneWidget,
    );
    expect(
      find.byKey(const Key('diagnostics-item-sync.state_source')),
      findsNothing,
    );

    final clipboardCalls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        clipboardCalls.add(call);
        return null;
      },
    );
    addTearDown(() {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      );
    });

    await tester.tap(find.byKey(const Key('diagnostics-copy-button')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('诊断摘要已复制'), findsOneWidget);
    final clipboardSetDataCalls = clipboardCalls
        .where((call) => call.method == 'Clipboard.setData')
        .toList();
    expect(clipboardSetDataCalls, hasLength(1));
    expect(
      clipboardSetDataCalls.single.arguments,
      containsPair('text', contains('sync.entry_blocker')),
    );

    await tester.tap(find.text('关闭'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(
      find.byKey(const Key('diagnostics-export-button')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('diagnostics-export-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('diagnostics-export-path')),
      '/tmp/radishlex-manager-diagnostics.txt',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('diagnostics-export-submit')));
    await tester.pumpAndSettle();

    expect(
      bridge.diagnosticsExportPath,
      '/tmp/radishlex-manager-diagnostics.txt',
    );
    expect(find.text('诊断摘要导出完成：12 行'), findsOneWidget);
  });
}

String _diagnosticsValue(ManagerDiagnosticsReport report, String key) {
  for (final section in report.sections) {
    for (final item in section.items) {
      if (item.key == key) {
        return item.value;
      }
    }
  }
  throw StateError('diagnostics item missing: $key');
}

void _expectDiagnosticsActionCommandPreview(
  ManagerDiagnosticsReport report, {
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
  required String reason,
}) {
  expect(
    _diagnosticsValue(report, 'sync.action_command_format'),
    managerSyncActionCommandPreviewFormat,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_actions'),
    'recovery_setup, recovery_restore, join_request_authorization, device_revocation',
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_intent_statuses'),
    expectedIntentStatusSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_execution_statuses'),
    syncActionExpectedExecutionSummary(expectedIntentStatusSummary),
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_blockers'),
    expectedBlockerSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_data_policy'),
    syncActionCommandDataPolicySummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_stop_lines'),
    syncActionCommandStopLineSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_request_boundaries'),
    expectedRequestBoundarySummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_result_boundaries'),
    expectedResultBoundarySummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_command_error_codes'),
    expectedErrorCodeSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_request_statuses'),
    syncActionExpectedRequestStatusSummary(expectedIntentStatusSummary),
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_request_allowed_fields'),
    expectedRequestAllowedFieldSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_request_forbidden_material'),
    expectedForbiddenMaterialSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_result_statuses'),
    syncActionExpectedResultStatusSummary(expectedIntentStatusSummary),
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_result_allowed_fields'),
    expectedResultAllowedFieldSummary,
    reason: reason,
  );
  expect(
    _diagnosticsValue(report, 'sync.action_result_forbidden_material'),
    expectedForbiddenMaterialSummary,
    reason: reason,
  );
}

class _DiagnosticsRecordingBridge extends FixtureManagerBridge {
  String? diagnosticsExportPath;

  @override
  Future<ManagerDiagnosticsExportResult> exportDiagnosticsReport(
    String filePath,
  ) async {
    diagnosticsExportPath = filePath;
    return const ManagerDiagnosticsExportResult(
      filePath: '/tmp/radishlex-manager-diagnostics.txt',
      format: 'manager.diagnostics.v1',
      lineCount: 12,
      itemCount: 24,
      redactionPolicy: 'summary_only_no_terms_paths_tokens_or_payload_bytes',
    );
  }
}
