import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge_factory.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
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

  testWidgets('refresh reloads a snapshot changed by the input runtime', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _ExternallyUpdatedBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    bridge.simulateInputRuntimeSelection();
    await tester.tap(find.byKey(const Key('manager-refresh-button')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('学习'));
    await tester.pumpAndSettle();

    expect(bridge.loadCount, 2);
    expect(find.text('selection events 129'), findsOneWidget);
  });
}

class _ExternallyUpdatedBridge extends FixtureManagerBridge {
  ManagerSnapshot _snapshot = createManagerFixture();
  int loadCount = 0;

  void simulateInputRuntimeSelection() {
    _snapshot = _snapshot.copyWith(
      generatedAt: '2026-07-18 13:26',
      learningSummary: _snapshot.learningSummary.copyWith(
        userTerms: 4,
        selectionEvents: 129,
        lastUpdated: '2026-07-18 13:26',
      ),
    );
  }

  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    loadCount += 1;
    return _snapshot;
  }
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
