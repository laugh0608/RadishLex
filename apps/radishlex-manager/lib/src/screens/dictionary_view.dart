import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class DictionaryView extends StatefulWidget {
  const DictionaryView({
    super.key,
    required this.snapshot,
    required this.onDeleteTerm,
    required this.onImportDictionary,
    required this.onExportDictionary,
  });

  final ManagerSnapshot snapshot;
  final ValueChanged<UserTerm> onDeleteTerm;
  final VoidCallback onImportDictionary;
  final VoidCallback onExportDictionary;

  @override
  State<DictionaryView> createState() => _DictionaryViewState();
}

class _DictionaryViewState extends State<DictionaryView> {
  final searchController = TextEditingController();

  @override
  void dispose() {
    searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final searchQuery = searchController.text.trim();
    final visibleTerms = _filterTerms(widget.snapshot.dictionaryTerms);

    return ManagerSection(
      title: '本地词库',
      trailing: Wrap(
        spacing: 8,
        children: [
          OutlinedButton.icon(
            key: const Key('dictionary-import-button'),
            onPressed: widget.onImportDictionary,
            icon: const Icon(Icons.upload_file_outlined),
            label: const Text('导入'),
          ),
          FilledButton.icon(
            key: const Key('dictionary-export-button'),
            onPressed: widget.onExportDictionary,
            icon: const Icon(Icons.download_outlined),
            label: const Text('导出'),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          TextField(
            key: const Key('dictionary-search-field'),
            controller: searchController,
            decoration: const InputDecoration(
              prefixIcon: Icon(Icons.search),
              labelText: '搜索 input code / text / reading / source',
            ),
            onChanged: (_) => setState(() {}),
          ),
          const SizedBox(height: 14),
          if (widget.snapshot.dictionaryTerms.isEmpty)
            const _DictionaryEmptyState(
              icon: Icons.library_books_outlined,
              message: '当前本地 userdb 没有可显示词条',
            )
          else if (visibleTerms.isEmpty)
            _DictionaryEmptyState(
              icon: Icons.search_off_outlined,
              message: '没有匹配 "$searchQuery" 的词条',
            )
          else
            _DictionaryTermsTable(
              terms: visibleTerms,
              onDeleteTerm: widget.onDeleteTerm,
            ),
          const SizedBox(height: 14),
          _DeletedTermsStrip(deletedTerms: widget.snapshot.deletedTerms),
        ],
      ),
    );
  }

  List<UserTerm> _filterTerms(List<UserTerm> terms) {
    final query = searchController.text.trim().toLowerCase();
    if (query.isEmpty) {
      return terms;
    }

    return terms
        .where((term) {
          return term.inputCode.toLowerCase().contains(query) ||
              term.text.toLowerCase().contains(query) ||
              term.reading.toLowerCase().contains(query) ||
              term.source.toLowerCase().contains(query);
        })
        .toList(growable: false);
  }
}

class _DictionaryTermsTable extends StatelessWidget {
  const _DictionaryTermsTable({
    required this.terms,
    required this.onDeleteTerm,
  });

  final List<UserTerm> terms;
  final ValueChanged<UserTerm> onDeleteTerm;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: DataTable(
        headingTextStyle: Theme.of(context).textTheme.labelMedium,
        columns: const [
          DataColumn(label: Text('input code')),
          DataColumn(label: Text('text')),
          DataColumn(label: Text('reading')),
          DataColumn(label: Text('weight')),
          DataColumn(label: Text('source')),
          DataColumn(label: Text('last used')),
          DataColumn(label: Text('')),
        ],
        rows: terms
            .map(
              (term) => DataRow(
                cells: [
                  DataCell(Text(term.inputCode)),
                  DataCell(Text(term.text)),
                  DataCell(Text(term.reading)),
                  DataCell(Text(term.weight.toStringAsFixed(2))),
                  DataCell(Text(term.source)),
                  DataCell(Text(term.lastUsed)),
                  DataCell(
                    IconButton(
                      tooltip: '删除词条',
                      onPressed: () => onDeleteTerm(term),
                      icon: const Icon(Icons.delete_outline),
                    ),
                  ),
                ],
              ),
            )
            .toList(),
      ),
    );
  }
}

class _DictionaryEmptyState extends StatelessWidget {
  const _DictionaryEmptyState({required this.icon, required this.message});

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.onSurfaceVariant;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 20),
      child: Row(
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 10),
          Flexible(
            child: Text(
              message,
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: color),
            ),
          ),
        ],
      ),
    );
  }
}

class _DeletedTermsStrip extends StatelessWidget {
  const _DeletedTermsStrip({required this.deletedTerms});

  final List<DeletedTerm> deletedTerms;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Text(
          'deleted tombstone',
          style: Theme.of(context).textTheme.labelLarge,
        ),
        ...deletedTerms.map(
          (term) => Chip(
            avatar: const Icon(Icons.block, size: 18),
            label: Text('${term.inputCode} / ${term.text}'),
          ),
        ),
        if (deletedTerms.isEmpty) const Text('暂无 deleted tombstone'),
      ],
    );
  }
}

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
