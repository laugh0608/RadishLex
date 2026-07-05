import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/app.dart';
import 'package:radishlex_manager/src/bridge/fixture_manager_bridge.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
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
    expect(find.text('字段 57'), findsOneWidget);
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
      find.textContaining('sync.recovery_blocker: recovery_code_flow_closed'),
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
      find.textContaining('settings.access_token: not_configured'),
      findsOneWidget,
    );
    expect(
      find.textContaining('sync.connection_status: access_token_missing'),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'sync.server_state_status: not_checked_access_token_missing',
      ),
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
