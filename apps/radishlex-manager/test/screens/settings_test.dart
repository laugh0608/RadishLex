import 'dart:convert';
import 'dart:io';

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
  testWidgets('settings view exposes configuration diagnostics', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    expect(find.text('配置来源'), findsOneWidget);
    expect(find.text('fixture'), findsOneWidget);
    expect(find.text('RADISHLEX_MANAGER_DB not configured'), findsOneWidget);
    expect(find.text('not loaded'), findsOneWidget);
    expect(find.text('同步门禁草案'), findsOneWidget);
    expect(find.text('平台签名 backend 不可用'), findsOneWidget);
    expect(find.text('设备 production gate 为 blocked'), findsOneWidget);
    expect(find.text('deployment evidence missing'), findsWidgets);
    expect(find.text('backend_unavailable'), findsWidgets);
    expect(find.text('not_recorded'), findsWidgets);
    expect(find.text('access_token_missing'), findsWidgets);
    expect(find.text('not_checked_access_token_missing'), findsOneWidget);
    expect(
      find.text(
        'recovery_setup, recovery_restore, device_join, device_revocation',
      ),
      findsWidgets,
    );
    expect(
      find.textContaining('recovery_code_generation_closed'),
      findsWidgets,
    );
    expect(find.textContaining('recovery_setup_readiness'), findsWidgets);
    expect(find.text('manager_default_closed_readiness'), findsWidgets);
    expect(
      find.text(
        'recovery_setup, recovery_restore, join_request_authorization, device_revocation',
      ),
      findsWidgets,
    );
    expect(
      find.text(
        'recovery_setup=visible, recovery_restore=visible, join_request_authorization=visible, device_revocation=visible',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      ),
      findsWidgets,
    );
    expect(
      find.text(
        'recovery_code_generation_closed, recovery_code_input_closed, join_request_creation_closed, device_revocation_flow_closed',
      ),
      findsWidgets,
    );
    expect(find.text('true'), findsOneWidget);
    expect(find.text('required_before_first_upload'), findsOneWidget);
    expect(find.text('recovery_record_not_created'), findsWidgets);
    expect(find.text('authorization_package_not_created'), findsOneWidget);
    expect(
      find.text('authorization_package_prerequisites_blocked'),
      findsWidgets,
    );
    expect(
      find.text('lost_device_prior_material_not_recallable'),
      findsOneWidget,
    );
    expect(find.text('key_epoch_rotation_not_started'), findsWidgets);
    expect(find.text('recovery_setup_flow_closed'), findsOneWidget);
    expect(find.text('recovery_restore_flow_closed'), findsOneWidget);
    expect(find.text('recovery_code_input_closed'), findsOneWidget);
    expect(
      find.textContaining('platform_private_key_backend_ready'),
      findsWidgets,
    );
    expect(find.textContaining('recovery_code_required'), findsWidgets);
    expect(find.text('device_join_flow_closed'), findsOneWidget);
    expect(find.text('short_code_verification_not_started'), findsOneWidget);
    expect(find.text('device_revocation_flow_closed'), findsWidgets);
    expect(find.text('active_existing_device_required'), findsWidgets);
    expect(find.textContaining('key_epoch_rotation_required'), findsWidgets);
  });

  testWidgets('settings gate preview uses bridge readiness snapshot', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

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

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    expect(find.text('blocked_before_user_sync'), findsWidgets);
    expect(find.text('user_sync_entry_closed_current_phase'), findsWidgets);
    expect(find.text('ffi_native_readiness'), findsWidgets);
    expect(
      find.text(
        'bridge_recovery_setup_readiness, bridge_recovery_restore_readiness, bridge_device_join_readiness, bridge_device_revocation_readiness',
      ),
      findsWidgets,
    );
    expect(find.text('recovery_ready'), findsWidgets);
    expect(find.text('device_authorization_ready'), findsWidgets);
    expect(
      find.text(
        'recovery_setup=closed_current_phase, recovery_restore=closed_current_phase, join_request_authorization=closed_current_phase, device_revocation=closed_current_phase',
      ),
      findsWidgets,
    );
    expect(
      find.text('user_sync_entry_current_phase_open_required'),
      findsWidgets,
    );
    expect(
      find.textContaining('recovery_code_generation_closed'),
      findsNothing,
    );
    expect(find.text('false'), findsWidgets);
  });

  for (final scenarioId in representativeSyncReadinessScenarioIds) {
    testWidgets(
      'settings gate preview renders readiness scenario $scenarioId',
      (WidgetTester tester) async {
        await tester.binding.setSurfaceSize(const Size(1400, 1000));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final scenario = syncReadinessScenarioById(scenarioId);

        await tester.pumpWidget(
          RadishLexManagerApp(
            bridge: FixtureManagerBridge(
              initialSnapshot: managerSnapshotForSyncReadinessScenario(
                scenario,
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byIcon(Icons.tune_outlined));
        await tester.pumpAndSettle();

        _expectSettingsGatePreviewReadinessScenario(scenario);
      },
    );
  }

  for (final scenarioId in representativeSyncEvidenceBundleScenarioIds) {
    testWidgets(
      'settings gate preview renders sync evidence bundle $scenarioId',
      (WidgetTester tester) async {
        await tester.binding.setSurfaceSize(const Size(1400, 1000));
        addTearDown(() => tester.binding.setSurfaceSize(null));
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

        await tester.tap(find.byIcon(Icons.tune_outlined));
        await tester.pumpAndSettle();

        _expectSettingsGatePreviewEvidenceBundleScenario(scenario);
      },
    );
  }

  testWidgets('settings draft save updates sync gate source', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      RadishLexManagerApp(bridge: FixtureManagerBridge()),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('settings-server-endpoint')),
      'https://draft.example.invalid',
    );
    await tester.tap(find.byKey(const Key('settings-privacy-mode')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('settings-access-token-configured')));
    await tester.pump();

    expect(find.text('策略禁用同步'), findsOneWidget);
    expect(find.text('设置草案启用隐私模式'), findsOneWidget);

    await tester.tap(find.byKey(const Key('settings-save-button')));
    await tester.pumpAndSettle();

    expect(find.text('设置草案已保存：sync_disabled_by_policy'), findsOneWidget);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('sync_disabled_by_policy'), findsWidgets);
    expect(find.text('策略禁用同步'), findsOneWidget);
    expect(find.text('设置草案启用隐私模式'), findsOneWidget);
    expect(find.text('https://draft.example.invalid'), findsOneWidget);
    expect(find.text('not_checked_policy_disabled'), findsWidgets);
    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('settings deployment evidence source gates preflight draft', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final fixture = createManagerFixture();
    const readyDevice = DeviceSecuritySummary(
      deviceId: 'device-ready-01',
      backendId: 'test-production-ready',
      capabilityStatus: 'ready_for_test',
      productionGate: 'ready',
    );
    final draft = fixture.settings.draft.copyWith(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      deploymentEvidenceRecorded: false,
      deploymentEvidenceSource: '',
    );
    final state = deriveManagerSyncUiState(draft: draft, device: readyDevice);
    final snapshot = fixture.copyWith(
      sync: fixture.sync.copyWith(
        state: state,
        device: readyDevice,
        serverEndpoint: managerSyncEndpointLabel(draft),
        reason: managerSyncGateReason(
          state: state,
          draft: draft,
          device: readyDevice,
        ),
      ),
      settings: fixture.settings.copyWith(draft: draft, syncConfigured: true),
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    expect(find.text('deployment_unverified'), findsWidgets);
    expect(find.text('部署证据未记录'), findsOneWidget);
    expect(find.text('deployment evidence missing'), findsWidgets);

    await tester.ensureVisible(
      find.byKey(const Key('settings-deployment-evidence')),
    );
    await tester.tap(find.byKey(const Key('settings-deployment-evidence')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const Key('settings-deployment-evidence-source')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('external TLS').last);
    await tester.pumpAndSettle();

    expect(find.text('preflight_ready'), findsWidgets);
    expect(find.text('本地预检通过'), findsOneWidget);
    expect(find.text('blocked_before_user_sync'), findsWidgets);
    expect(find.text('recovery_code_flow_closed'), findsWidgets);
    expect(
      find.textContaining('recovery_code_save_confirmation_required'),
      findsWidgets,
    );
    expect(find.text('blocked_until_recovery_code_saved'), findsWidgets);
    expect(find.text('device_authorization_flow_closed'), findsWidgets);
    expect(find.text('join_request_unavailable'), findsWidgets);
    expect(find.text('authorization_package_not_created'), findsWidgets);
    expect(
      find.text(
        'recovery_setup, recovery_restore, device_join, device_revocation',
      ),
      findsWidgets,
    );
    expect(find.textContaining('recovery_setup_readiness'), findsWidgets);
    expect(find.text('recovery_setup_flow_closed'), findsWidgets);
    expect(find.text('recovery_restore_flow_closed'), findsWidgets);
    expect(find.text('device_join_flow_closed'), findsWidgets);
    expect(find.text('join_request_creation_closed'), findsWidgets);
    expect(find.text('short_code_verification_not_started'), findsWidgets);
    expect(find.text('device_revocation_flow_closed'), findsWidgets);
    expect(find.text('active_existing_device_required'), findsWidgets);
    expect(find.textContaining('authorization_rejected'), findsWidgets);
    expect(
      find.text('lost_device_prior_material_not_recallable'),
      findsWidgets,
    );
    expect(find.text('key_epoch_rotation_not_started'), findsWidgets);
    expect(find.text('deployment evidence external TLS'), findsWidgets);

    await tester.ensureVisible(find.byKey(const Key('settings-save-button')));
    await tester.tap(find.byKey(const Key('settings-save-button')));
    await tester.pumpAndSettle();

    expect(find.text('设置草案已保存：preflight_ready'), findsOneWidget);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('preflight_ready'), findsWidgets);
    expect(find.text('本地预检通过，真实同步入口仍等待恢复码和设备授权'), findsOneWidget);
    expect(find.text('blocked_before_user_sync'), findsWidgets);
    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('settings imports connection health summary into sync gate', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final fixture = createManagerFixture();
    const readyDevice = DeviceSecuritySummary(
      deviceId: 'device-ready-01',
      backendId: 'test-production-ready',
      capabilityStatus: 'ready_for_test',
      productionGate: 'ready',
    );
    final draft = fixture.settings.draft.copyWith(
      serverEndpoint: 'https://localhost:7319',
      retainSyncConfig: true,
      accessTokenConfigured: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
    );
    final snapshot = fixture.copyWith(
      sync: fixture.sync.copyWith(
        state: deriveManagerSyncUiState(draft: draft, device: readyDevice),
        device: readyDevice,
        serverEndpoint: managerSyncEndpointLabel(draft),
        reason: managerSyncGateReason(
          state: deriveManagerSyncUiState(draft: draft, device: readyDevice),
          draft: draft,
          device: readyDevice,
        ),
      ),
      settings: fixture.settings.copyWith(draft: draft, syncConfigured: true),
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const Key('settings-connection-health-summary-json')),
    );
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('settings-connection-health-summary-json')),
      jsonEncode({
        'format': managerSyncConnectionHealthSummaryFormat,
        'redaction_policy': managerSyncConnectionHealthSummaryRedactionPolicy,
        'endpoint_status': 'configured',
        'transport_mode': 'local_https',
        'access_token_status': 'not_configured',
        'connection_status': 'reachable',
        'auth_status': 'not_required_for_local_probe',
        'server_state_status': 'domain_missing_expected',
        'http_status': 404,
        'http_status_class': 'client_error',
        'last_remote_error_code': 'not_found',
        'local_insecure_tls': 'allowed',
      }),
    );
    await tester.tap(
      find.byKey(const Key('settings-import-connection-summary')),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(managerSyncConnectionProbeSourceLocalDockerHttps),
      findsWidgets,
    );
    expect(find.text('domain_missing_expected'), findsWidgets);
    expect(find.text('not_required_for_local_probe'), findsWidgets);
    expect(find.text('404 client_error'), findsWidgets);

    await tester.ensureVisible(find.byKey(const Key('settings-save-button')));
    await tester.tap(find.byKey(const Key('settings-save-button')));
    await tester.pumpAndSettle();

    expect(find.text('设置草案已保存：preflight_ready'), findsOneWidget);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('只读探测已到达服务'), findsOneWidget);
    expect(
      find.text(managerSyncConnectionProbeSourceLocalDockerHttps),
      findsWidgets,
    );
    expect(find.text('domain_missing_expected'), findsOneWidget);
    expect(find.text('not_found'), findsOneWidget);
    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('settings imports readiness summary into sync and diagnostics', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));

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
    final snapshot = fixture.copyWith(
      sync: fixture.sync.copyWith(
        state: deriveManagerSyncUiState(draft: draft, device: readyDevice),
        device: readyDevice,
        serverEndpoint: managerSyncEndpointLabel(draft),
        reason: managerSyncGateReason(
          state: deriveManagerSyncUiState(draft: draft, device: readyDevice),
          draft: draft,
          device: readyDevice,
        ),
      ),
      settings: fixture.settings.copyWith(draft: draft, syncConfigured: true),
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await _pumpUi(tester);

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await _pumpUi(tester);
    await tester.ensureVisible(
      find.byKey(const Key('settings-sync-readiness-summary-json')),
    );
    await _pumpUi(tester);
    await tester.enterText(
      find.byKey(const Key('settings-sync-readiness-summary-json')),
      jsonEncode(partiallyBlockedSyncReadinessBridgeJson()),
    );
    await tester.tap(
      find.byKey(const Key('settings-import-readiness-summary')),
    );
    await _pumpUi(tester);

    expect(find.text('fixture_readiness'), findsWidgets);
    expect(find.text('readiness_summary_in_memory'), findsWidgets);
    expect(find.text('manager_default_closed_readiness'), findsWidgets);
    expect(find.text('recovery_setup, device_join'), findsWidgets);
    expect(find.textContaining('recovery_record_missing'), findsWidgets);
    expect(find.textContaining('join_request_expired'), findsWidgets);
    expect(
      find.text(
        'recovery_setup=blocked, recovery_restore=closed_current_phase, join_request_authorization=blocked, device_revocation=closed_current_phase',
      ),
      findsWidgets,
    );
    expect(find.text('false'), findsWidgets);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await _pumpUi(tester);

    expect(find.text('fixture_readiness'), findsWidgets);
    expect(find.text('recovery_setup, device_join'), findsWidgets);
    final enableButton = tester.widget<FilledButton>(
      find.byKey(const Key('sync-enable-button')),
    );
    expect(enableButton.onPressed, isNull);

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await _pumpUi(tester);
    await tester.ensureVisible(
      find.byKey(const Key('diagnostics-preview-button')),
    );
    await tester.tap(find.byKey(const Key('diagnostics-preview-button')));
    await _pumpUi(tester);

    expect(
      find.textContaining('sync.readiness_bridge_source: fixture_readiness'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.readiness_blocked_flows: recovery_setup, device_join',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.entry_blocker: recovery_record_missing'),
      findsOneWidget,
    );
    final reportText = tester.widget<SelectableText>(
      find.byKey(const Key('diagnostics-report-text')),
    );
    expect(reportText.data, isNot(contains('secret-token')));
    expect(reportText.data, isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')));
    expect(reportText.data, isNot(contains('payload_bytes=abcdef')));

    await tester.tap(find.text('关闭'));
    await _pumpUi(tester);

    final exportFile = File(
      '${Directory.systemTemp.path}/radishlex-manager-readiness-${DateTime.now().microsecondsSinceEpoch}.txt',
    );
    addTearDown(() {
      if (exportFile.existsSync()) {
        exportFile.deleteSync();
      }
    });

    await tester.ensureVisible(
      find.byKey(const Key('diagnostics-export-button')),
    );
    await tester.tap(find.byKey(const Key('diagnostics-export-button')));
    await _pumpUi(tester);
    await tester.enterText(
      find.byKey(const Key('diagnostics-export-path')),
      exportFile.path,
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('diagnostics-export-submit')));
    await _pumpUi(tester);

    expect(find.textContaining('诊断摘要导出完成'), findsOneWidget);
    final exported = exportFile.readAsStringSync();
    expect(
      exported,
      contains('sync.readiness_bridge_source: fixture_readiness'),
    );
    expect(
      exported,
      contains('sync.readiness_blocked_flows: recovery_setup, device_join'),
    );
    expect(exported, isNot(contains('secret-token')));
    expect(exported, isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')));
    expect(exported, isNot(contains('payload_bytes=abcdef')));

    await tester.ensureVisible(
      find.byKey(const Key('settings-clear-readiness-summary')),
    );
    await tester.tap(find.byKey(const Key('settings-clear-readiness-summary')));
    await _pumpUi(tester);

    expect(find.text('default_closed_readiness'), findsWidgets);
    expect(find.text('manager_default_closed_readiness'), findsWidgets);
    expect(find.text('fixture_readiness'), findsNothing);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await _pumpUi(tester);

    expect(find.text('manager_default_closed_readiness'), findsWidgets);
    expect(find.text('fixture_readiness'), findsNothing);
  });

  testWidgets('settings rejects unsafe readiness redaction policy', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      RadishLexManagerApp(bridge: FixtureManagerBridge()),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const Key('settings-sync-readiness-summary-json')),
    );
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('settings-sync-readiness-summary-json')),
      jsonEncode({
        ...readySyncReadinessBridgeJson(),
        'redaction_policy': 'raw_native_payload_with_secret_fields',
      }),
    );
    await tester.tap(
      find.byKey(const Key('settings-import-readiness-summary')),
    );
    await tester.pumpAndSettle();

    expect(
      find.text('readiness_summary_redaction_policy_unsupported'),
      findsOneWidget,
    );
    expect(find.text('manager_default_closed_readiness'), findsWidgets);
    expect(find.text('ffi_native_readiness'), findsNothing);
  });
}

Future<void> _pumpUi(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 250));
}

void _expectSettingsGatePreviewReadinessScenario(
  SyncReadinessScenario scenario,
) {
  expect(find.text(scenario.expectedEntryState.code), findsWidgets);
  expect(find.text(scenario.expectedEntryBlocker), findsWidgets);
  expect(find.text(scenario.expectedBlockedFlows), findsWidgets);
  expect(find.text(scenario.expectedBridgeSource), findsWidgets);
  expect(find.text(scenario.expectedInteractionStatuses), findsWidgets);
  expect(find.text(scenario.expectedInteractionBlockers), findsWidgets);
  expect(
    find.text(
      _expectedActionCommandExecutionSummary(
        scenario.expectedInteractionStatuses,
      ),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  expect(
    find.text(
      _expectedActionRequestStatusSummary(scenario.expectedInteractionStatuses),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  expect(
    find.text(
      _expectedActionResultStatusSummary(scenario.expectedInteractionStatuses),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  _expectActionCommandProtocolPreview(
    reason: scenario.id,
    expectedRequestBoundarySummary:
        scenario.expectedActionCommandRequestBoundaries,
    expectedResultBoundarySummary:
        scenario.expectedActionCommandResultBoundaries,
    expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
    expectedRequestAllowedFieldSummary:
        scenario.expectedActionRequestAllowedFields,
    expectedResultAllowedFieldSummary:
        scenario.expectedActionResultAllowedFields,
    expectedForbiddenMaterialSummary: scenario.expectedActionForbiddenMaterials,
  );
  expect(
    find.text(scenario.expectedUserSyncEnabled.toString()),
    findsWidgets,
    reason: scenario.id,
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

void _expectSettingsGatePreviewEvidenceBundleScenario(
  SyncEvidenceBundleScenario scenario,
) {
  expect(find.text(scenario.expectedEntryState.code), findsWidgets);
  expect(find.text(scenario.expectedEntryBlocker), findsWidgets);
  expect(find.text(scenario.expectedBlockedFlows), findsWidgets);
  expect(find.text(scenario.expectedBridgeSource), findsWidgets);
  expect(find.text(scenario.expectedInteractionStatuses), findsWidgets);
  expect(find.text(scenario.expectedInteractionBlockers), findsWidgets);
  expect(
    find.text(
      _expectedActionCommandExecutionSummary(
        scenario.expectedInteractionStatuses,
      ),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  expect(
    find.text(
      _expectedActionRequestStatusSummary(scenario.expectedInteractionStatuses),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  expect(
    find.text(
      _expectedActionResultStatusSummary(scenario.expectedInteractionStatuses),
    ),
    findsWidgets,
    reason: scenario.id,
  );
  _expectActionCommandProtocolPreview(
    reason: scenario.id,
    expectedRequestBoundarySummary:
        scenario.expectedActionCommandRequestBoundaries,
    expectedResultBoundarySummary:
        scenario.expectedActionCommandResultBoundaries,
    expectedErrorCodeSummary: scenario.expectedActionCommandErrorCodes,
    expectedRequestAllowedFieldSummary:
        scenario.expectedActionRequestAllowedFields,
    expectedResultAllowedFieldSummary:
        scenario.expectedActionResultAllowedFields,
    expectedForbiddenMaterialSummary: scenario.expectedActionForbiddenMaterials,
  );
  expect(find.text(scenario.expectedConnectionStatusCode), findsWidgets);
  expect(find.text(scenario.expectedConnectionProbeSource), findsWidgets);
  expect(find.text(syncEvidenceBundleRecordedAt), findsWidgets);
  expect(find.text(scenario.expectedConnectionBlocker), findsWidgets);
  expect(find.text(scenario.expectedTransportMode), findsWidgets);
  expect(find.text(scenario.expectedServerStateStatus), findsWidgets);
  expect(find.text(scenario.expectedAuthStatus), findsWidgets);
  expect(find.text(scenario.expectedHttpStatusText), findsWidgets);
  expect(find.text(scenario.expectedLastRemoteErrorCode), findsWidgets);
  expect(find.text(scenario.expectedUserSyncEnabled.toString()), findsWidgets);

  for (final fragment in syncEvidenceBundleSensitiveLeakFragments) {
    expect(find.textContaining(fragment), findsNothing, reason: scenario.id);
  }
}

void _expectActionCommandProtocolPreview({
  required String reason,
  required String expectedRequestBoundarySummary,
  required String expectedResultBoundarySummary,
  required String expectedErrorCodeSummary,
  required String expectedRequestAllowedFieldSummary,
  required String expectedResultAllowedFieldSummary,
  required String expectedForbiddenMaterialSummary,
}) {
  for (final value in [
    syncActionCommandDataPolicySummary,
    syncActionCommandStopLineSummary,
    expectedRequestBoundarySummary,
    expectedResultBoundarySummary,
    expectedErrorCodeSummary,
    expectedRequestAllowedFieldSummary,
    expectedResultAllowedFieldSummary,
    expectedForbiddenMaterialSummary,
  ]) {
    expect(find.text(value), findsWidgets, reason: reason);
  }
}

String _expectedActionCommandExecutionSummary(String intentStatusSummary) {
  if (intentStatusSummary == 'none') {
    return 'none';
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) {
        final separator = entry.indexOf('=');
        final actionId = entry.substring(0, separator);
        final intentStatus = entry.substring(separator + 1);
        return '$actionId=${_expectedActionCommandExecutionStatus(intentStatus)}';
      })
      .join(', ');
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

String _expectedActionRequestStatusSummary(String intentStatusSummary) {
  return _expectedActionStatusSummary(
    intentStatusSummary,
    _expectedActionRequestStatus,
  );
}

String _expectedActionResultStatusSummary(String intentStatusSummary) {
  return _expectedActionStatusSummary(
    intentStatusSummary,
    _expectedActionResultStatus,
  );
}

String _expectedActionStatusSummary(
  String intentStatusSummary,
  String Function(String intentStatus) mapStatus,
) {
  if (intentStatusSummary == 'none') {
    return 'none';
  }
  return intentStatusSummary
      .split(', ')
      .map((entry) {
        final separator = entry.indexOf('=');
        final actionId = entry.substring(0, separator);
        final intentStatus = entry.substring(separator + 1);
        return '$actionId=${mapStatus(intentStatus)}';
      })
      .join(', ');
}

String _expectedActionRequestStatus(String intentStatus) {
  switch (_expectedActionCommandExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'request_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'request_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'request_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'request_not_built_current_phase';
    default:
      return 'request_blocked_by_unknown_execution_status';
  }
}

String _expectedActionResultStatus(String intentStatus) {
  switch (_expectedActionCommandExecutionStatus(intentStatus)) {
    case 'ready_for_future_bridge_command':
      return 'result_shape_ready_for_future_bridge';
    case 'blocked_until_user_confirmation':
      return 'result_blocked_until_user_confirmation';
    case 'blocked_by_readiness':
      return 'result_blocked_by_readiness';
    case 'not_executable_current_phase':
      return 'result_not_available_current_phase';
    default:
      return 'result_blocked_by_unknown_execution_status';
  }
}
