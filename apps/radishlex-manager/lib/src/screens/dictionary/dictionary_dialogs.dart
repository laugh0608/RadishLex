import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class DictionaryDeleteConfirmDialog extends StatelessWidget {
  const DictionaryDeleteConfirmDialog({super.key, required this.term});

  final UserTerm term;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('删除词条'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ManagerKeyValueRow(label: 'input code', value: term.inputCode),
            ManagerKeyValueRow(label: 'text', value: term.text),
            ManagerKeyValueRow(label: 'reading', value: term.reading),
            ManagerKeyValueRow(label: 'source', value: term.source),
            const ManagerKeyValueRow(
              label: 'delete effect',
              value: '写入 deleted tombstone，避免旧设备或旧备份复活该词条',
            ),
            const ManagerKeyValueRow(
              label: 'sync category',
              value: 'dictionary.deleted_terms',
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          key: const Key('dictionary-delete-cancel'),
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton.icon(
          key: const Key('dictionary-delete-confirm'),
          onPressed: () => Navigator.of(context).pop(true),
          icon: const Icon(Icons.delete_outline),
          label: const Text('删除'),
        ),
      ],
    );
  }
}

class DictionaryRestoreConfirmDialog extends StatelessWidget {
  const DictionaryRestoreConfirmDialog({
    super.key,
    required this.term,
    required this.state,
  });

  final UserTermKey term;
  final String state;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('显式恢复词条'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ManagerKeyValueRow(label: 'input code', value: term.inputCode),
            ManagerKeyValueRow(label: 'text', value: term.text),
            ManagerKeyValueRow(label: 'reading', value: term.reading),
            ManagerKeyValueRow(label: 'current state', value: state),
            const ManagerKeyValueRow(
              label: 'restore effect',
              value: '清除对应 tombstone 或 suppressed 状态，并以新的版本恢复为 active',
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          key: const Key('dictionary-restore-cancel'),
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton.icon(
          key: const Key('dictionary-restore-confirm'),
          onPressed: () => Navigator.of(context).pop(true),
          icon: const Icon(Icons.restore),
          label: const Text('确认恢复'),
        ),
      ],
    );
  }
}

class DictionaryImportRequest {
  const DictionaryImportRequest({
    required this.filePath,
    required this.sourceName,
    required this.dryRun,
  });

  final String filePath;
  final String sourceName;
  final bool dryRun;
}

class DictionaryImportDialog extends StatefulWidget {
  const DictionaryImportDialog({super.key});

  @override
  State<DictionaryImportDialog> createState() => _DictionaryImportDialogState();
}

class _DictionaryImportDialogState extends State<DictionaryImportDialog> {
  final filePathController = TextEditingController();
  final sourceNameController = TextEditingController(text: 'manager-import');
  bool dryRun = true;

  @override
  void dispose() {
    filePathController.dispose();
    sourceNameController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final filePath = filePathController.text.trim();
    final sourceName = sourceNameController.text.trim();
    final canSubmit = filePath.isNotEmpty && sourceName.isNotEmpty;

    return AlertDialog(
      title: const Text('导入词库'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              key: const Key('dictionary-import-path'),
              controller: filePathController,
              decoration: const InputDecoration(
                prefixIcon: Icon(Icons.file_open_outlined),
                labelText: '文件路径',
              ),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 12),
            TextField(
              key: const Key('dictionary-import-source'),
              controller: sourceNameController,
              decoration: const InputDecoration(
                prefixIcon: Icon(Icons.label_outline),
                labelText: 'source name',
              ),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 8),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              value: dryRun,
              onChanged: (value) {
                setState(() {
                  dryRun = value;
                });
              },
              secondary: const Icon(Icons.fact_check_outlined),
              title: const Text('dry run'),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton.icon(
          key: const Key('dictionary-import-submit'),
          onPressed: canSubmit
              ? () => Navigator.of(context).pop(
                  DictionaryImportRequest(
                    filePath: filePath,
                    sourceName: sourceName,
                    dryRun: dryRun,
                  ),
                )
              : null,
          icon: const Icon(Icons.rule_outlined),
          label: const Text('检查导入'),
        ),
      ],
    );
  }
}

class DictionaryImportPreviewDialog extends StatelessWidget {
  const DictionaryImportPreviewDialog({
    super.key,
    required this.preview,
    required this.request,
  });

  final DictionaryImportPreview preview;
  final DictionaryImportRequest request;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('导入检查'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ManagerKeyValueRow(label: 'file', value: preview.filePath),
            ManagerKeyValueRow(label: 'format', value: preview.format),
            ManagerKeyValueRow(
              label: 'records',
              value: preview.recordCount.toString(),
            ),
            ManagerKeyValueRow(label: 'sync class', value: preview.syncClass),
            ManagerKeyValueRow(
              label: 'dry run',
              value: request.dryRun.toString(),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton.icon(
          key: const Key('dictionary-import-confirm'),
          onPressed: () => Navigator.of(context).pop(true),
          icon: const Icon(Icons.playlist_add_check_outlined),
          label: Text(request.dryRun ? '执行检查' : '导入'),
        ),
      ],
    );
  }
}

class DictionaryExportDialog extends StatefulWidget {
  const DictionaryExportDialog({super.key});

  @override
  State<DictionaryExportDialog> createState() => _DictionaryExportDialogState();
}

class _DictionaryExportDialogState extends State<DictionaryExportDialog> {
  final filePathController = TextEditingController();

  @override
  void dispose() {
    filePathController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final filePath = filePathController.text.trim();

    return AlertDialog(
      title: const Text('导出词库'),
      content: SizedBox(
        width: 420,
        child: TextField(
          key: const Key('dictionary-export-path'),
          controller: filePathController,
          decoration: const InputDecoration(
            prefixIcon: Icon(Icons.file_download_outlined),
            labelText: '文件路径',
          ),
          onChanged: (_) => setState(() {}),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton.icon(
          key: const Key('dictionary-export-submit'),
          onPressed: filePath.isNotEmpty
              ? () => Navigator.of(context).pop(filePath)
              : null,
          icon: const Icon(Icons.download_done_outlined),
          label: const Text('导出'),
        ),
      ],
    );
  }
}
