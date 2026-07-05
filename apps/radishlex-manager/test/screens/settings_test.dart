import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

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
  });

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
}
