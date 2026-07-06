import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

import '../fixtures/sync_evidence_bundle_fixtures.dart';
import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  testWidgets('sync gate keeps user sync disabled', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('backend_unavailable'), findsWidgets);
    expect(find.text('平台签名 backend 不可用'), findsOneWidget);
    expect(find.text('设备 production gate 为 blocked'), findsOneWidget);
    expect(find.text('production gate blocked'), findsOneWidget);
    expect(find.text('not_recorded'), findsWidgets);
    expect(
      find.text(
        'access_token_missing, platform_private_key_backend_blocked, deployment_evidence_missing, recovery_code_flow_closed, recovery_record_not_created, recovery_code_save_confirmation_required, device_authorization_flow_closed, join_request_unavailable, authorization_package_prerequisites_blocked, device_revocation_flow_closed, lost_device_risk_notice_required, key_epoch_rotation_not_started, user_sync_entry_closed_current_phase',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'recovery_setup, recovery_restore, device_join, device_revocation',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('recovery_code_generation_closed'),
      findsWidgets,
    );
    expect(find.textContaining('join_request_creation_closed'), findsWidgets);
    expect(find.textContaining('recovery_setup_readiness'), findsOneWidget);
    expect(find.text('manager_default_closed_readiness'), findsOneWidget);
    expect(find.text('true'), findsOneWidget);
    expect(find.text('false'), findsOneWidget);
    expect(find.text('syncable 3'), findsOneWidget);
    expect(find.text('local-only 128'), findsOneWidget);
    expect(find.text('unsupported_signature_algorithm'), findsOneWidget);
    expect(
      find.byKey(const Key('sync-connection-health-section')),
      findsOneWidget,
    );
    expect(find.text('服务连接健康'), findsOneWidget);
    expect(find.text('access_token_missing'), findsWidgets);
    expect(find.text('access token 未记录'), findsOneWidget);
    expect(find.text('external_https'), findsOneWidget);
    expect(find.text('not_checked_access_token_missing'), findsOneWidget);
    expect(find.text('只读检查不上传 P2 对象，不生成恢复码，不打开设备授权成功路径。'), findsOneWidget);
    expect(
      find.byKey(const Key('sync-recovery-readiness-section')),
      findsOneWidget,
    );
    expect(find.text('恢复码准备态'), findsOneWidget);
    expect(find.text('恢复码流程关闭'), findsOneWidget);
    expect(find.text('recovery_code_flow_closed'), findsWidgets);
    expect(find.text('required_before_first_upload'), findsOneWidget);
    expect(find.text('recovery_record_not_created'), findsWidgets);
    expect(find.text('recovery_record_creation_closed'), findsOneWidget);
    expect(find.text('blocked_until_recovery_code_saved'), findsOneWidget);
    expect(find.text('recovery_setup_flow_closed'), findsOneWidget);
    expect(find.text('recovery_code_generation_closed'), findsWidgets);
    expect(
      find.text(
        'platform_private_key_backend_ready, release_deployment_evidence_summary_required, explicit_user_start_required',
      ),
      findsWidgets,
    );
    expect(
      find.text(
        'recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
      ),
      findsOneWidget,
    );
    expect(find.text('setup intent'), findsOneWidget);
    expect(find.text('setup intent blocker'), findsOneWidget);
    expect(find.text('setup intent evidence'), findsOneWidget);
    expect(find.text('setup command'), findsOneWidget);
    expect(find.text('setup command policy'), findsOneWidget);
    expect(find.text('setup command stop'), findsOneWidget);
    expect(find.text('setup request boundary'), findsOneWidget);
    expect(find.text('setup result boundary'), findsOneWidget);
    expect(find.text('setup command errors'), findsOneWidget);
    expect(find.text('no_recovery_code_or_wrapped_material'), findsWidgets);
    expect(
      find.text('no_recovery_code_generation_current_phase'),
      findsWidgets,
    );
    expect(
      find.text('request_summary_only_no_recovery_code_generation'),
      findsWidgets,
    );
    expect(
      find.text('result_summary_only_no_recovery_record_or_wrapped_material'),
      findsWidgets,
    );
    expect(
      find.text(
        'configuration_missing, backend_unavailable, deployment_unverified, recovery_code_required, recovery_record_missing, recovery_record_revoked, local_data_inconsistent',
      ),
      findsWidgets,
    );
    expect(find.text('recovery_restore_flow_closed'), findsOneWidget);
    expect(find.text('recovery_code_input_closed'), findsWidgets);
    expect(find.text('input_not_available_current_phase'), findsOneWidget);
    expect(
      find.text(
        'recovery_code_required, recovery_code_invalid, recovery_record_missing, recovery_record_revoked, authentication_required, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(find.text('not_started'), findsOneWidget);
    expect(find.text('restore request boundary'), findsOneWidget);
    expect(find.text('restore result boundary'), findsOneWidget);
    expect(find.text('restore command errors'), findsOneWidget);
    expect(
      find.text('request_summary_only_no_recovery_code_input'),
      findsWidgets,
    );
    expect(
      find.text('result_summary_only_no_unwrapped_device_material'),
      findsWidgets,
    );
    expect(find.text('恢复码生成、输入、轮换和撤销仍未开放。'), findsOneWidget);
    expect(
      find.byKey(const Key('sync-device-authorization-readiness-section')),
      findsOneWidget,
    );
    expect(find.text('设备授权准备态'), findsOneWidget);
    expect(find.text('设备授权流程关闭'), findsOneWidget);
    expect(find.text('device_authorization_flow_closed'), findsWidgets);
    expect(find.text('join_request_unavailable'), findsWidgets);
    expect(find.text('authorization_package_not_created'), findsWidgets);
    expect(
      find.text('authorization_package_prerequisites_blocked'),
      findsWidgets,
    );
    expect(
      find.text(
        'active_existing_device_required, join_request_pending_required, short_code_match_required',
      ),
      findsOneWidget,
    );
    expect(find.text('closed_current_phase'), findsWidgets);
    expect(find.text('device_join_flow_closed'), findsOneWidget);
    expect(find.text('join_request_creation_closed'), findsWidgets);
    expect(find.text('join intent'), findsOneWidget);
    expect(find.text('join intent blocker'), findsOneWidget);
    expect(find.text('join intent evidence'), findsOneWidget);
    expect(find.text('join command'), findsOneWidget);
    expect(find.text('join command policy'), findsOneWidget);
    expect(find.text('join command stop'), findsOneWidget);
    expect(find.text('join request boundary'), findsOneWidget);
    expect(find.text('join result boundary'), findsOneWidget);
    expect(find.text('join command errors'), findsOneWidget);
    expect(
      find.text('no_short_code_signature_or_wrapped_material'),
      findsWidgets,
    );
    expect(
      find.text('no_join_request_or_authorization_package_current_phase'),
      findsWidgets,
    );
    expect(
      find.text('request_summary_only_no_join_request_or_short_code'),
      findsWidgets,
    );
    expect(
      find.text('result_summary_only_no_authorization_package_or_signature'),
      findsWidgets,
    );
    expect(find.text('short_code_verification_not_started'), findsOneWidget);
    expect(
      find.text(
        'join_request_expired, authorization_rejected, device_revoked, backend_unavailable, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(find.text('device_revocation_flow_closed'), findsWidgets);
    expect(find.text('revocation intent'), findsOneWidget);
    expect(find.text('revocation intent blocker'), findsOneWidget);
    expect(find.text('revocation intent evidence'), findsOneWidget);
    expect(find.text('revocation command'), findsOneWidget);
    expect(find.text('revocation command policy'), findsOneWidget);
    expect(find.text('revocation command stop'), findsOneWidget);
    expect(find.text('revocation request boundary'), findsOneWidget);
    expect(find.text('revocation result boundary'), findsOneWidget);
    expect(find.text('revocation command errors'), findsOneWidget);
    expect(
      find.text('no_signature_key_epoch_or_wrapped_material'),
      findsWidgets,
    );
    expect(find.text('no_device_revocation_current_phase'), findsWidgets);
    expect(
      find.text('request_summary_only_no_device_signature_or_key_epoch'),
      findsWidgets,
    );
    expect(
      find.text(
        'result_summary_only_no_revocation_record_or_key_epoch_material',
      ),
      findsWidgets,
    );
    expect(find.text('active_existing_device_required'), findsOneWidget);
    expect(
      find.text(
        'device_revoked, key_epoch_rotation_required, local_data_inconsistent, network_unreachable',
      ),
      findsOneWidget,
    );
    expect(
      find.text('lost_device_prior_material_not_recallable'),
      findsOneWidget,
    );
    expect(find.text('key_epoch_rotation_not_started'), findsWidgets);
    expect(find.text('设备加入审批、授权成功和撤销操作仍未开放。'), findsOneWidget);
    expect(
      find.text('真实远端同步、恢复码和设备授权仍处于关闭状态；本页只展示本地预检和不可用原因。'),
      findsOneWidget,
    );

    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('sync view uses bridge readiness as read-only source', (
    WidgetTester tester,
  ) async {
    final fixture = createManagerFixture();
    const readyDevice = DeviceSecuritySummary(
      deviceId: 'device-ready-01',
      backendId: 'test-production-ready',
      capabilityStatus: 'ready_for_test',
      productionGate: 'ready',
    );
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      accessTokenConfigured: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    );
    final readiness = managerSyncReadinessBridgeSnapshotFromJson(
      readySyncReadinessBridgeJson(),
    );
    final state = deriveManagerSyncUiState(
      draft: draft,
      device: readyDevice,
      readinessBridgeSnapshot: readiness,
    );
    final snapshot = fixture.copyWith(
      sync: fixture.sync.copyWith(
        state: state,
        device: readyDevice,
        serverEndpoint: managerSyncEndpointLabel(draft),
        reason: managerSyncGateReason(
          state: state,
          draft: draft,
          device: readyDevice,
          readinessBridgeSnapshot: readiness,
        ),
        readinessBridgeSnapshot: readiness,
      ),
      settings: fixture.settings.copyWith(draft: draft, syncConfigured: true),
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('blocked_before_user_sync'), findsWidgets);
    expect(find.text('user_sync_entry_closed_current_phase'), findsWidgets);
    expect(find.text('ffi_native_readiness'), findsOneWidget);
    expect(
      find.text(
        'bridge_recovery_setup_readiness, bridge_recovery_restore_readiness, bridge_device_join_readiness, bridge_device_revocation_readiness',
      ),
      findsOneWidget,
    );
    expect(find.text('recovery_ready'), findsOneWidget);
    expect(find.text('device_authorization_ready'), findsOneWidget);
    expect(
      find.text(
        'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      ),
      findsNothing,
    );
    expect(
      find.text('user_sync_entry_current_phase_open_required'),
      findsWidgets,
    );
    expect(find.text('none'), findsWidgets);
    expect(
      find.textContaining('recovery_code_generation_closed'),
      findsNothing,
    );
    expect(find.text('本地预检通过，真实同步入口仍按当前阶段关闭'), findsOneWidget);
    expect(find.text('本地预检通过；用户可用同步入口仍按当前阶段关闭'), findsOneWidget);

    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });

  for (final scenarioId in representativeSyncReadinessScenarioIds) {
    testWidgets('sync view renders readiness scenario $scenarioId', (
      WidgetTester tester,
    ) async {
      final scenario = syncReadinessScenarioById(scenarioId);

      await tester.pumpWidget(
        RadishLexManagerApp(
          bridge: FixtureManagerBridge(
            initialSnapshot: managerSnapshotForSyncReadinessScenario(scenario),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.sync_outlined));
      await tester.pumpAndSettle();

      _expectVisibleSyncReadinessScenario(scenario);

      final enableButton = tester.widget<FilledButton>(
        find.byKey(const Key('sync-enable-button')),
      );
      expect(enableButton.onPressed, isNull, reason: scenario.id);
    });
  }

  for (final scenarioId in representativeSyncEvidenceBundleScenarioIds) {
    testWidgets('sync view renders sync evidence bundle $scenarioId', (
      WidgetTester tester,
    ) async {
      final scenario = syncEvidenceBundleScenarioById(scenarioId);

      await tester.pumpWidget(
        RadishLexManagerApp(
          bridge: FixtureManagerBridge(
            initialSnapshot: managerSnapshotForSyncEvidenceBundleScenario(
              scenario,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.sync_outlined));
      await tester.pumpAndSettle();

      _expectVisibleSyncEvidenceBundleScenario(scenario);

      final enableButton = tester.widget<FilledButton>(
        find.byKey(const Key('sync-enable-button')),
      );
      expect(enableButton.onPressed, isNull, reason: scenario.id);
    });
  }

  testWidgets('sync view exposes local-only empty category state', (
    WidgetTester tester,
  ) async {
    final fixture = createManagerFixture();
    final draft = fixture.settings.draft.copyWith(
      serverEndpoint: '',
      retainSyncConfig: false,
      deploymentEvidenceRecorded: false,
      deploymentEvidenceSource: '',
    );
    final snapshot = fixture.copyWith(
      sync: fixture.sync.copyWith(
        state: SyncUiState.localOnly,
        serverEndpoint: '未配置',
        reason: '未保留自部署服务端草案；真实远端同步保持关闭',
        syncableObjects: 0,
        localOnlyEvents: 0,
        categories: const [],
      ),
      settings: fixture.settings.copyWith(syncConfigured: false, draft: draft),
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('local_only'), findsWidgets);
    expect(find.text('仅本地管理'), findsOneWidget);
    expect(find.text('未保留自部署服务端草案'), findsOneWidget);
    expect(find.text('连接未配置'), findsOneWidget);
    expect(find.text('not_checked_endpoint_missing'), findsOneWidget);
    expect(find.text('暂无本地 P2 对象分类摘要'), findsOneWidget);
    expect(find.text('syncable 0'), findsOneWidget);
    expect(find.text('local-only 0'), findsOneWidget);

    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });
}

void _expectVisibleSyncReadinessScenario(SyncReadinessScenario scenario) {
  expect(find.text(scenario.expectedEntryState.code), findsWidgets);
  expect(find.text(scenario.expectedEntryBlocker), findsWidgets);
  expect(find.text(scenario.expectedBlockedFlows), findsWidgets);
  expect(find.text(scenario.expectedBridgeSource), findsWidgets);
  expect(
    find.text(scenario.expectedUserSyncEnabled.toString()),
    findsWidgets,
    reason: scenario.id,
  );
  for (final executionStatus in _expectedActionCommandExecutionStatuses(
    scenario.expectedInteractionStatuses,
  )) {
    expect(find.text(executionStatus), findsWidgets, reason: scenario.id);
  }
  _expectActionCommandProtocolPreview(
    reason: scenario.id,
    expectedRequestBoundarySummary:
        scenario.expectedActionCommandRequestBoundaries,
    expectedResultBoundarySummary:
        scenario.expectedActionCommandResultBoundaries,
    expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
  );

  if (scenario.expectedIssueCodes != null) {
    expect(
      find.text(scenario.expectedIssueCodes!),
      findsWidgets,
      reason: scenario.id,
    );
  }
  for (final code in scenario.expectedIssueCodeContains) {
    expect(find.textContaining(code), findsWidgets, reason: scenario.id);
  }

  if (scenario.expectedNextEvidence != null) {
    expect(
      find.text(scenario.expectedNextEvidence!),
      findsWidgets,
      reason: scenario.id,
    );
  }
  for (final code in scenario.expectedNextEvidenceContains) {
    expect(find.textContaining(code), findsWidgets, reason: scenario.id);
  }

  for (final fragment in syncReadinessSensitiveLeakFragments) {
    expect(find.textContaining(fragment), findsNothing, reason: scenario.id);
  }
}

void _expectVisibleSyncEvidenceBundleScenario(
  SyncEvidenceBundleScenario scenario,
) {
  expect(find.text(scenario.expectedEntryState.code), findsWidgets);
  expect(find.text(scenario.expectedEntryBlocker), findsWidgets);
  expect(find.text(scenario.expectedBlockedFlows), findsWidgets);
  expect(find.text(scenario.expectedBridgeSource), findsWidgets);
  expect(find.text(scenario.expectedConnectionStatusCode), findsWidgets);
  expect(find.text(scenario.expectedConnectionBlocker), findsWidgets);
  expect(find.text(scenario.expectedConnectionProbeSource), findsWidgets);
  expect(find.text(syncEvidenceBundleRecordedAt), findsOneWidget);
  expect(find.text(scenario.expectedEndpointStatus), findsWidgets);
  expect(find.text(scenario.expectedAccessTokenStatus), findsWidgets);
  expect(find.text(scenario.expectedTransportMode), findsWidgets);
  expect(find.text(scenario.expectedServerStateStatus), findsWidgets);
  expect(find.text(scenario.expectedAuthStatus), findsWidgets);
  expect(find.text(scenario.expectedHttpStatusText), findsOneWidget);
  expect(find.text(scenario.expectedLocalInsecureTls), findsWidgets);
  expect(find.text(scenario.expectedLastRemoteErrorCode), findsWidgets);
  expect(find.text(scenario.expectedUserSyncEnabled.toString()), findsWidgets);
  for (final executionStatus in _expectedActionCommandExecutionStatuses(
    scenario.expectedInteractionStatuses,
  )) {
    expect(find.text(executionStatus), findsWidgets, reason: scenario.id);
  }
  _expectActionCommandProtocolPreview(
    reason: scenario.id,
    expectedRequestBoundarySummary:
        scenario.expectedActionCommandRequestBoundaries,
    expectedResultBoundarySummary:
        scenario.expectedActionCommandResultBoundaries,
    expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
  );

  for (final fragment in syncEvidenceBundleSensitiveLeakFragments) {
    expect(find.textContaining(fragment), findsNothing, reason: scenario.id);
  }
}

void _expectActionCommandProtocolPreview({
  required String reason,
  required String expectedRequestBoundarySummary,
  required String expectedResultBoundarySummary,
  required String expectedErrorCodeSummary,
}) {
  for (final value in [
    'no_recovery_code_or_wrapped_material',
    'no_recovery_code_input_or_device_secret',
    'no_short_code_signature_or_wrapped_material',
    'no_signature_key_epoch_or_wrapped_material',
    'no_recovery_code_generation_current_phase',
    'no_recovery_code_input_current_phase',
    'no_join_request_or_authorization_package_current_phase',
    'no_device_revocation_current_phase',
    expectedRequestBoundarySummary,
    expectedResultBoundarySummary,
    expectedErrorCodeSummary,
  ]) {
    for (final part in value.split(', ')) {
      expect(find.textContaining(part), findsWidgets, reason: reason);
    }
  }
}

List<String> _expectedActionCommandExecutionStatuses(
  String intentStatusSummary,
) {
  if (intentStatusSummary == 'none') {
    return const ['none'];
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) => entry.substring(entry.indexOf('=') + 1))
      .map(_expectedActionCommandExecutionStatus)
      .toSet()
      .toList(growable: false);
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
