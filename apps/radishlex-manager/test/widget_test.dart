import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge_factory.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  testWidgets('load failure shows structured bridge error code', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(RadishLexManagerApp(bridge: _FailingBridge()));
    await tester.pumpAndSettle();

    expect(find.text('管理端数据加载失败'), findsOneWidget);
    expect(find.text('加载管理数据失败：本地 userdb 错误（userdb_error）'), findsOneWidget);
  });

  testWidgets('demo runtime keeps a persistent synthetic data banner', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(),
        runtimeMode: ManagerRuntimeMode.demo,
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(Banner), findsOneWidget);
    expect(tester.widget<Banner>(find.byType(Banner)).message, '合成演示数据');
  });
}

class _FailingBridge extends FixtureManagerBridge {
  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    throw const _TestBridgeFailure(code: 'userdb_error');
  }
}

class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure({required this.code});

  @override
  int get statusCode => 4;

  @override
  final String code;

  @override
  String get message => 'private path omitted';
}
