import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
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
    expect(find.text('设备签名'), findsNothing);
    expect(find.text('luobo'), findsOneWidget);
    expect(find.text('deleted tombstone'), findsOneWidget);
  });

  testWidgets('learning view exposes aggregate explain data', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const RadishLexManagerApp());
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.psychology_alt_outlined));
    await tester.pumpAndSettle();

    expect(find.text('rank explain'), findsOneWidget);
    expect(find.text('import batches'), findsOneWidget);
    expect(find.text('manager-import'), findsOneWidget);
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

    expect(find.text('luobo'), findsNothing);
    expect(find.text('luobo / 萝卜词核'), findsOneWidget);
  });

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
    expect(find.text('导入检查完成：0 条'), findsOneWidget);

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
    expect(find.text('导出完成：3 条'), findsOneWidget);
  });
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
    throw const _TestBridgeFailure();
  }
}

class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure();

  @override
  int get statusCode => 4;

  @override
  String get code => 'userdb_error';

  @override
  String get message => 'private path omitted';
}
