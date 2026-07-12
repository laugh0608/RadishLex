import 'dart:io' show File;

import '../models/manager_models.dart';

ManagerDiagnosticsExportResult writeManagerDiagnosticsReport({
  required String filePath,
  required ManagerDiagnosticsReport report,
}) {
  final text = report.toRedactedText();
  File(filePath).writeAsStringSync(text);
  final lineCount = text.trimRight().isEmpty
      ? 0
      : text.trimRight().split('\n').length;

  return ManagerDiagnosticsExportResult(
    filePath: filePath,
    format: report.format,
    lineCount: lineCount,
    itemCount: report.itemCount,
    redactionPolicy: report.redactionPolicy,
  );
}
