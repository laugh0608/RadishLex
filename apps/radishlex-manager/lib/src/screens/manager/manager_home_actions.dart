import 'package:flutter/material.dart';

import '../../bridge/manager_diagnostics_export.dart';
import '../../bridge/manager_bridge.dart';
import '../../models/manager_models.dart';
import '../dictionary/dictionary_dialogs.dart';
import '../settings/diagnostics_export_dialog.dart';
import '../settings/diagnostics_report_dialog.dart';

class ManagerHomeActions {
  const ManagerHomeActions({
    required this.context,
    required this.bridge,
    required this.currentSnapshot,
    required this.onSnapshotChanged,
    required this.reloadSnapshot,
    required this.showMessage,
  });

  final BuildContext context;
  final ManagerBridge bridge;
  final ManagerSnapshot Function() currentSnapshot;
  final ValueChanged<ManagerSnapshot> onSnapshotChanged;
  final VoidCallback reloadSnapshot;
  final ValueChanged<String> showMessage;

  Future<void> deleteTerm(UserTerm term) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => DictionaryDeleteConfirmDialog(term: term),
    );
    if (!context.mounted || confirmed != true) {
      return;
    }

    try {
      final snapshot = await bridge.deleteUserTerm(term.key);
      if (!context.mounted) {
        return;
      }
      onSnapshotChanged(snapshot);
      showMessage('已删除词条：${term.inputCode} / ${term.text}');
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(
          managerBridgeFailureMessage(
            error,
            ManagerBridgeOperation.deleteUserTerm,
          ),
        );
      }
    }
  }

  Future<void> importDictionary() async {
    final request = await showDialog<DictionaryImportRequest>(
      context: context,
      builder: (context) => const DictionaryImportDialog(),
    );
    if (!context.mounted || request == null) {
      return;
    }

    var operation = ManagerBridgeOperation.inspectDictionaryImport;
    try {
      final preview = await bridge.inspectDictionaryImport(request.filePath);
      if (!context.mounted) {
        return;
      }
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) =>
            DictionaryImportPreviewDialog(preview: preview, request: request),
      );
      if (!context.mounted || confirmed != true) {
        return;
      }

      operation = ManagerBridgeOperation.importDictionaryFile;
      final result = await bridge.importDictionaryFile(
        filePath: request.filePath,
        sourceName: request.sourceName,
        dryRun: request.dryRun,
      );
      if (!context.mounted) {
        return;
      }
      showMessage(dictionaryImportResultMessage(result));
      reloadSnapshot();
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(managerBridgeFailureMessage(error, operation));
      }
    }
  }

  Future<void> exportDictionary() async {
    final filePath = await showDialog<String>(
      context: context,
      builder: (context) => const DictionaryExportDialog(),
    );
    if (!context.mounted || filePath == null) {
      return;
    }

    try {
      final result = await bridge.exportDictionaryFile(filePath);
      if (!context.mounted) {
        return;
      }
      showMessage(dictionaryExportResultMessage(result));
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(
          managerBridgeFailureMessage(
            error,
            ManagerBridgeOperation.exportDictionaryFile,
          ),
        );
      }
    }
  }

  Future<void> previewDiagnostics() async {
    try {
      final report = createManagerDiagnosticsReport(currentSnapshot());
      if (!context.mounted) {
        return;
      }
      await showDialog<void>(
        context: context,
        builder: (context) => DiagnosticsReportDialog(report: report),
      );
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(
          managerBridgeFailureMessage(
            error,
            ManagerBridgeOperation.previewDiagnostics,
          ),
        );
      }
    }
  }

  Future<void> exportDiagnostics() async {
    final filePath = await showDialog<String>(
      context: context,
      builder: (context) => const DiagnosticsExportDialog(),
    );
    if (!context.mounted || filePath == null) {
      return;
    }

    try {
      final snapshot = currentSnapshot();
      final result =
          snapshot.sync.readinessBridgeSnapshot.source ==
              managerSyncReadinessBridgeSourceDefault
          ? await bridge.exportDiagnosticsReport(filePath)
          : writeManagerDiagnosticsReport(
              filePath: filePath,
              report: createManagerDiagnosticsReport(snapshot),
            );
      if (!context.mounted) {
        return;
      }
      showMessage('诊断摘要导出完成：${result.lineCount} 行');
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(
          managerBridgeFailureMessage(
            error,
            ManagerBridgeOperation.exportDiagnostics,
          ),
        );
      }
    }
  }

  Future<void> saveSettingsDraft(ManagerSettingsDraft draft) async {
    try {
      final currentReadiness = currentSnapshot().sync.readinessBridgeSnapshot;
      var snapshot = await bridge.saveSettingsDraft(draft);
      if (currentReadiness.source != managerSyncReadinessBridgeSourceDefault) {
        snapshot = managerSnapshotWithSyncReadiness(snapshot, currentReadiness);
      }
      if (!context.mounted) {
        return;
      }
      onSnapshotChanged(snapshot);
      showMessage('设置草案已保存：${snapshot.sync.state.code}');
    } on Object catch (error) {
      if (context.mounted) {
        showMessage(
          managerBridgeFailureMessage(
            error,
            ManagerBridgeOperation.saveSettingsDraft,
          ),
        );
      }
    }
  }
}

String managerBridgeFailureMessage(
  Object? error,
  ManagerBridgeOperation operation,
) {
  return describeManagerBridgeFailure(error, operation).userMessage;
}

String dictionaryImportResultMessage(DictionaryImportResult result) {
  final skipped = result.skippedDeletedTerms + result.skippedDuplicateTerms;
  final label = result.dryRun ? '导入检查完成' : '导入完成';
  return '$label：${result.importedTerms} / ${result.totalRecords} 条，'
      '新增 ${result.insertedTerms}，更新 ${result.updatedTerms}，跳过 $skipped';
}

String dictionaryExportResultMessage(DictionaryExportResult result) {
  return '导出完成：${result.exportedTerms} 条，${result.format} / ${result.syncClass}';
}
