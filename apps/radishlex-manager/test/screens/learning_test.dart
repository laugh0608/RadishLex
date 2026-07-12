import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  testWidgets('learning view exposes aggregate explain data', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.psychology_alt_outlined));
    await tester.pumpAndSettle();

    expect(find.text('rank explain'), findsOneWidget);
    expect(find.text('导入历史'), findsNothing);
    expect(find.text('manager-import'), findsNothing);
    expect(find.text('selection events'), findsOneWidget);
    expect(find.text('manual_user_term'), findsOneWidget);
    expect(find.text('仅展示聚合学习摘要，不展示 P1 原始选择事件、原始输入历史或应用窗口信息。'), findsOneWidget);
    expect(find.text('选择一个候选查看 rank explain 贡献项'), findsOneWidget);
  });

  testWidgets('learning rank explain filters and shows candidate detail', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.psychology_alt_outlined));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const Key('rank-explain-filter')),
      'context',
    );
    await tester.pump();

    expect(find.text('tongbu'), findsOneWidget);
    expect(find.text('luobo'), findsNothing);

    await tester.enterText(find.byKey(const Key('rank-explain-filter')), '');
    await tester.pump();
    await tester.tap(find.text('萝卜词核').last);
    await tester.pump();

    expect(find.text('summary only'), findsOneWidget);
    expect(
      find.text('manual_user_term, frequency_boost, recent_selection'),
      findsOneWidget,
    );
    expect(find.text('不展示 P1 原始事件明细'), findsOneWidget);
  });

  testWidgets('learning view exposes empty aggregate states', (
    WidgetTester tester,
  ) async {
    final snapshot = createManagerFixture().copyWith(
      learningSummary: const LearningSummary(
        userTerms: 0,
        deletedTerms: 0,
        selectionEvents: 0,
        suppressedTerms: 0,
        lastUpdated: '未更新',
      ),
      explanations: const [],
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.psychology_alt_outlined));
    await tester.pumpAndSettle();

    expect(find.text('暂无学习摘要'), findsOneWidget);
    expect(find.text('暂无 rank explain 摘要'), findsOneWidget);
    expect(find.text('选择一个候选查看 rank explain 贡献项'), findsOneWidget);

    await tester.enterText(
      find.byKey(const Key('rank-explain-filter')),
      'luobo',
    );
    await tester.pump();

    expect(find.text('暂无 rank explain 摘要'), findsOneWidget);
  });
}
