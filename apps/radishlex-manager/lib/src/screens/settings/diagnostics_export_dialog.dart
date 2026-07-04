import 'package:flutter/material.dart';

class DiagnosticsExportDialog extends StatefulWidget {
  const DiagnosticsExportDialog({super.key});

  @override
  State<DiagnosticsExportDialog> createState() =>
      _DiagnosticsExportDialogState();
}

class _DiagnosticsExportDialogState extends State<DiagnosticsExportDialog> {
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
      title: const Text('导出诊断摘要'),
      content: SizedBox(
        width: 420,
        child: TextField(
          key: const Key('diagnostics-export-path'),
          controller: filePathController,
          decoration: const InputDecoration(
            prefixIcon: Icon(Icons.description_outlined),
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
          key: const Key('diagnostics-export-submit'),
          onPressed: filePath.isNotEmpty
              ? () => Navigator.of(context).pop(filePath)
              : null,
          icon: const Icon(Icons.ios_share_outlined),
          label: const Text('导出'),
        ),
      ],
    );
  }
}
