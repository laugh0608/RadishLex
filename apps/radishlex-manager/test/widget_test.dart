import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  testWidgets('shows local dictionary management first', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    expect(find.text('萝卜词核'), findsWidgets);
    expect(find.text('本地词库'), findsOneWidget);
    expect(find.text('导入历史'), findsOneWidget);
    expect(find.text('设备签名'), findsNothing);
    expect(find.text('luobo'), findsOneWidget);
    expect(find.text('manager-import'), findsWidgets);
    expect(find.text('syncable 3'), findsOneWidget);
    expect(find.text('local-only 128'), findsOneWidget);
    expect(find.text('deleted tombstone'), findsOneWidget);
    expect(
      find.text('选择一个词条查看 key、来源、导入批次、tombstone 和 sync 分类'),
      findsOneWidget,
    );
  });

  testWidgets(
    'dictionary search filters local terms and shows no-match state',
    (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(const RadishLexManagerApp());
      await tester.pumpAndSettle();

      await tester.enterText(
        find.byKey(const Key('dictionary-search-field')),
        'tong',
      );
      await tester.pump();

      expect(find.text('tongbu'), findsOneWidget);
      expect(find.text('bianjie'), findsNothing);

      await tester.enterText(
        find.byKey(const Key('dictionary-search-field')),
        'missing-term',
      );
      await tester.pump();

      expect(find.text('没有匹配 "missing-term" 的词条'), findsOneWidget);
      expect(find.text('tongbu'), findsNothing);
    },
  );

  testWidgets('dictionary view exposes empty local userdb state', (
    WidgetTester tester,
  ) async {
    final snapshot = createManagerFixture().copyWith(
      dictionaryTerms: const [],
      deletedTerms: const [],
      importBatches: const [],
    );

    await tester.pumpWidget(
      RadishLexManagerApp(
        bridge: FixtureManagerBridge(initialSnapshot: snapshot),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('当前本地 userdb 没有可显示词条'), findsOneWidget);
    expect(find.text('暂无 deleted tombstone'), findsOneWidget);
    expect(find.text('暂无导入历史'), findsOneWidget);
    expect(find.byTooltip('删除词条'), findsNothing);
  });

  testWidgets('dictionary term audit detail explains selected local term', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.text('luobo'));
    await tester.pump();

    expect(find.text('active user term'), findsOneWidget);
    expect(find.text('无匹配导入批次'), findsOneWidget);
    expect(find.text('未删除；删除后会写入 tombstone'), findsOneWidget);
    expect(find.text('dictionary.user_terms (1)'), findsOneWidget);
    expect(find.text('backend_unavailable'), findsWidgets);
  });

  testWidgets(
    'dictionary import history filters sorts and selects source terms',
    (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      final snapshot = createManagerFixture().copyWith(
        dictionaryTerms: const [
          UserTerm(
            inputCode: 'luobo',
            text: '萝卜词核',
            reading: 'luo bo ci he',
            weight: 0.92,
            source: 'manual',
            lastUsed: '2026-07-04 10:42',
          ),
          UserTerm(
            inputCode: 'tongbu',
            text: '同步预检',
            reading: 'tong bu yu jian',
            weight: 0.76,
            source: 'selection',
            lastUsed: '2026-07-03 18:12',
          ),
          UserTerm(
            inputCode: 'bianjie',
            text: '边界清晰',
            reading: 'bian jie qing xi',
            weight: 0.71,
            source: 'manager-import',
            lastUsed: '2026-07-02 21:03',
          ),
        ],
      );

      await tester.pumpWidget(
        RadishLexManagerApp(
          bridge: FixtureManagerBridge(initialSnapshot: snapshot),
        ),
      );
      await tester.pumpAndSettle();

      final firstNewestTop = tester.getTopLeft(find.text('#2')).dy;
      final secondNewestTop = tester.getTopLeft(find.text('#1')).dy;
      expect(firstNewestTop, lessThan(secondNewestTop));

      await tester.ensureVisible(
        find.byKey(const Key('dictionary-import-history-sort')),
      );
      await tester.tap(find.byKey(const Key('dictionary-import-history-sort')));
      await tester.pump();

      final firstOldestTop = tester.getTopLeft(find.text('#1')).dy;
      final secondOldestTop = tester.getTopLeft(find.text('#2')).dy;
      expect(firstOldestTop, lessThan(secondOldestTop));

      await tester.enterText(
        find.byKey(const Key('dictionary-import-history-filter')),
        'bootstrap',
      );
      await tester.pump();

      expect(find.text('#1'), findsOneWidget);
      expect(find.text('#2'), findsNothing);

      await tester.enterText(
        find.byKey(const Key('dictionary-import-history-filter')),
        'missing batch',
      );
      await tester.pump();

      expect(find.text('没有匹配当前筛选条件的导入批次'), findsOneWidget);

      await tester.enterText(
        find.byKey(const Key('dictionary-import-history-filter')),
        '',
      );
      await tester.ensureVisible(
        find.byKey(const Key('dictionary-import-history-sort')),
      );
      await tester.tap(find.byKey(const Key('dictionary-import-history-sort')));
      await tester.pump();
      await tester.ensureVisible(find.text('#2'));
      await tester.tap(find.text('#2'));
      await tester.pump();

      expect(find.text('batch #2 / manager-import'), findsOneWidget);
      expect(find.text('bianjie'), findsOneWidget);
      expect(find.text('luobo'), findsNothing);
      expect(find.text('tongbu'), findsNothing);

      await tester.ensureVisible(find.text('bianjie'));
      await tester.tap(find.text('bianjie'));
      await tester.pump();

      expect(
        find.textContaining('#2 / manager-import / 40/42'),
        findsOneWidget,
      );
    },
  );

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
  });

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
  });

  testWidgets('load failure shows structured bridge error code', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(RadishLexManagerApp(bridge: _FailingBridge()));
    await tester.pumpAndSettle();

    expect(find.text('管理端数据加载失败'), findsOneWidget);
    expect(find.text('加载管理数据失败：本地 userdb 错误（userdb_error）'), findsOneWidget);
  });

  testWidgets('settings view previews and exports diagnostics report', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _RecordingManagerBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.tune_outlined));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('diagnostics-preview-button')));
    await tester.pumpAndSettle();

    expect(find.text('诊断摘要预览'), findsOneWidget);
    expect(find.textContaining('runtime.bridge_mode: fixture'), findsOneWidget);
    expect(
      find.textContaining('redaction.user_terms: omitted'),
      findsOneWidget,
    );
    final reportText = tester.widget<SelectableText>(
      find.byKey(const Key('diagnostics-report-text')),
    );
    expect(reportText.data, isNot(contains('萝卜词核')));

    await tester.tap(find.text('关闭'));
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
    await tester.tap(find.byKey(const Key('settings-save-button')));
    await tester.pumpAndSettle();

    expect(find.text('设置草案已保存：sync_disabled_by_policy'), findsOneWidget);

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('sync_disabled_by_policy'), findsWidgets);
    expect(find.text('https://draft.example.invalid'), findsOneWidget);
    final enableButton = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '启用同步'),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('sync gate keeps user sync disabled', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.sync_outlined));
    await tester.pumpAndSettle();

    expect(find.text('backend_unavailable'), findsWidgets);
    expect(find.text('unsupported_signature_algorithm'), findsOneWidget);

    final enableButton = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '启用同步'),
    );
    expect(enableButton.onPressed, isNull);
  });

  testWidgets('delete action goes through manager bridge', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      RadishLexManagerApp(bridge: FixtureManagerBridge()),
    );
    await tester.pumpAndSettle();

    expect(find.text('luobo'), findsOneWidget);
    expect(find.text('luobo / 萝卜词核'), findsNothing);

    await tester.tap(find.byTooltip('删除词条').first);
    await tester.pumpAndSettle();

    expect(find.text('删除词条'), findsOneWidget);
    expect(find.text('luo bo ci he'), findsWidgets);
    expect(find.text('写入 deleted tombstone，避免旧设备或旧备份复活该词条'), findsOneWidget);
    expect(find.text('dictionary.deleted_terms'), findsOneWidget);

    await tester.tap(find.byKey(const Key('dictionary-delete-confirm')));
    await tester.pumpAndSettle();

    expect(find.text('已删除词条：luobo / 萝卜词核'), findsOneWidget);
    expect(find.text('luobo'), findsNothing);
    expect(find.text('luobo / 萝卜词核'), findsOneWidget);
  });

  testWidgets(
    'delete failure stays on dictionary view with structured message',
    (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        RadishLexManagerApp(bridge: _DeleteFailingBridge()),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('删除词条').first);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('dictionary-delete-confirm')));
      await tester.pumpAndSettle();

      expect(find.text('删除词条失败：本地 userdb 错误（userdb_error）'), findsOneWidget);
      expect(find.text('管理端数据加载失败'), findsNothing);
      expect(find.text('luobo'), findsOneWidget);
    },
  );

  testWidgets('dictionary import and export actions call manager bridge', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _RecordingManagerBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('dictionary-import-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('dictionary-import-path')),
      '/tmp/radishlex-import.json',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('dictionary-import-submit')));
    await tester.pumpAndSettle();
    expect(find.text('导入检查'), findsOneWidget);

    await tester.tap(find.byKey(const Key('dictionary-import-confirm')));
    await tester.pumpAndSettle();

    expect(bridge.inspectedPath, '/tmp/radishlex-import.json');
    expect(bridge.importedPath, '/tmp/radishlex-import.json');
    expect(bridge.importedSourceName, 'manager-import');
    expect(bridge.importedDryRun, isTrue);
    expect(find.text('导入检查完成：0 / 0 条，新增 0，更新 0，跳过 0'), findsOneWidget);

    await tester.tap(find.byKey(const Key('dictionary-export-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('dictionary-export-path')),
      '/tmp/radishlex-export.json',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('dictionary-export-submit')));
    await tester.pumpAndSettle();

    expect(bridge.exportedPath, '/tmp/radishlex-export.json');
    expect(
      find.text('导出完成：3 条，dictionary.user_terms.v1 / P2 encrypted sync'),
      findsOneWidget,
    );
  });

  testWidgets('dictionary import refreshes local import history', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _RefreshingImportBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    expect(find.text('暂无导入历史'), findsOneWidget);

    await tester.tap(find.byKey(const Key('dictionary-import-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('dictionary-import-path')),
      '/tmp/radishlex-import.tsv',
    );
    await tester.enterText(
      find.byKey(const Key('dictionary-import-source')),
      'ops-import',
    );
    await tester.tap(find.widgetWithText(SwitchListTile, 'dry run'));
    await tester.pump();
    await tester.tap(find.byKey(const Key('dictionary-import-submit')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('dictionary-import-confirm')));
    await tester.pumpAndSettle();

    expect(bridge.loadCount, greaterThan(1));
    expect(bridge.importedDryRun, isFalse);
    expect(find.text('导入完成：5 / 5 条，新增 4，更新 1，跳过 0'), findsOneWidget);
    expect(find.text('ops-import'), findsOneWidget);
    expect(find.text('#7'), findsOneWidget);
    expect(find.text('2026-07-04 11:05'), findsOneWidget);
    expect(find.text('5/5'), findsOneWidget);
  });

  testWidgets('dictionary import failure shows operation category', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final bridge = _ImportInspectFailingBridge();
    await tester.pumpWidget(RadishLexManagerApp(bridge: bridge));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('dictionary-import-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('dictionary-import-path')),
      '/tmp/private-import.tsv',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('dictionary-import-submit')));
    await tester.pumpAndSettle();

    expect(bridge.inspectedPath, '/tmp/private-import.tsv');
    expect(
      find.text('检查导入词库失败：词库导入或本地 userdb 错误（invalid_argument）'),
      findsOneWidget,
    );
    expect(find.text('导入检查'), findsNothing);
  });
}

class _RefreshingImportBridge extends FixtureManagerBridge {
  _RefreshingImportBridge()
    : _snapshot = createManagerFixture().copyWith(importBatches: const []);

  ManagerSnapshot _snapshot;
  int loadCount = 0;
  bool? importedDryRun;

  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    loadCount += 1;
    return _snapshot;
  }

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(
    String filePath,
  ) async {
    return DictionaryImportPreview(
      filePath: filePath,
      format: 'dictionary.user_terms.v1',
      recordCount: 5,
      syncClass: 'P2 encrypted sync',
    );
  }

  @override
  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  }) async {
    importedDryRun = dryRun;
    if (!dryRun) {
      _snapshot = _snapshot.copyWith(
        importBatches: [
          DictionaryImportBatchSummary(
            id: 7,
            sourceName: sourceName,
            totalRecords: 5,
            importedTerms: 5,
            insertedTerms: 4,
            updatedTerms: 1,
            skippedDeletedTerms: 0,
            skippedDuplicateTerms: 0,
            createdAt: '2026-07-04 11:05',
            notes: 'fixture import refresh',
          ),
          ..._snapshot.importBatches,
        ],
      );
    }
    return DictionaryImportResult(
      filePath: filePath,
      sourceName: sourceName,
      totalRecords: 5,
      importedTerms: 5,
      insertedTerms: 4,
      updatedTerms: 1,
      skippedDeletedTerms: 0,
      skippedDuplicateTerms: 0,
      dryRun: dryRun,
    );
  }
}

class _RecordingManagerBridge extends FixtureManagerBridge {
  String? inspectedPath;
  String? importedPath;
  String? importedSourceName;
  bool? importedDryRun;
  String? exportedPath;
  String? diagnosticsExportPath;

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(
    String filePath,
  ) async {
    inspectedPath = filePath;
    return super.inspectDictionaryImport(filePath);
  }

  @override
  Future<DictionaryImportResult> importDictionaryFile({
    required String filePath,
    required String sourceName,
    required bool dryRun,
  }) async {
    importedPath = filePath;
    importedSourceName = sourceName;
    importedDryRun = dryRun;
    return super.importDictionaryFile(
      filePath: filePath,
      sourceName: sourceName,
      dryRun: dryRun,
    );
  }

  @override
  Future<DictionaryExportResult> exportDictionaryFile(String filePath) async {
    exportedPath = filePath;
    return super.exportDictionaryFile(filePath);
  }

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

class _FailingBridge extends FixtureManagerBridge {
  @override
  Future<ManagerSnapshot> loadSnapshot() async {
    throw const _TestBridgeFailure(code: 'userdb_error');
  }
}

class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure({required this.code, this.statusCode = 4});

  @override
  final int statusCode;

  @override
  final String code;

  @override
  String get message => 'private path omitted';
}

class _DeleteFailingBridge extends FixtureManagerBridge {
  @override
  Future<ManagerSnapshot> deleteUserTerm(UserTermKey term) async {
    throw const _TestBridgeFailure(code: 'userdb_error');
  }
}

class _ImportInspectFailingBridge extends FixtureManagerBridge {
  String? inspectedPath;

  @override
  Future<DictionaryImportPreview> inspectDictionaryImport(
    String filePath,
  ) async {
    inspectedPath = filePath;
    throw const _TestBridgeFailure(code: 'invalid_argument', statusCode: 2);
  }
}
