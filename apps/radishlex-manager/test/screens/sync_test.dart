import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

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
        'access_token_missing, platform_private_key_backend_blocked, deployment_evidence_missing, recovery_code_flow_closed, device_authorization_flow_closed, user_sync_entry_closed_current_phase',
      ),
      findsOneWidget,
    );
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
    expect(find.text('not_started'), findsOneWidget);
    expect(find.text('恢复码生成、输入、轮换和撤销仍未开放。'), findsOneWidget);
    expect(
      find.byKey(const Key('sync-device-authorization-readiness-section')),
      findsOneWidget,
    );
    expect(find.text('设备授权准备态'), findsOneWidget);
    expect(find.text('设备授权流程关闭'), findsOneWidget);
    expect(find.text('device_authorization_flow_closed'), findsWidgets);
    expect(find.text('join_request_unavailable'), findsOneWidget);
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
