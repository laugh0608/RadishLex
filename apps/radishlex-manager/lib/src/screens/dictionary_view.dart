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
  final importFilterController = TextEditingController();
  int? selectedBatchId;
  bool newestImportFirst = true;

  @override
  void dispose() {
    searchController.dispose();
    importFilterController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final searchQuery = searchController.text.trim();
    final selectedBatch = _selectedBatch(widget.snapshot.importBatches);
    final visibleTerms = _filterTerms(
      widget.snapshot.dictionaryTerms,
      selectedBatch,
    );
    final visibleImportBatches = _filterImportBatches(
      widget.snapshot.importBatches,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ManagerSection(
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
              if (selectedBatch != null) ...[
                const SizedBox(height: 10),
                InputChip(
                  key: const Key('dictionary-selected-import-batch'),
                  avatar: const Icon(Icons.filter_alt_outlined, size: 18),
                  label: Text(
                    'batch #${selectedBatch.id} / ${selectedBatch.sourceName}',
                  ),
                  onDeleted: () => setState(() {
                    selectedBatchId = null;
                  }),
                ),
              ],
              const SizedBox(height: 14),
              if (widget.snapshot.dictionaryTerms.isEmpty)
                const _DictionaryEmptyState(
                  icon: Icons.library_books_outlined,
                  message: '当前本地 userdb 没有可显示词条',
                )
              else if (visibleTerms.isEmpty)
                _DictionaryEmptyState(
                  icon: Icons.search_off_outlined,
                  message: _emptyTermsMessage(searchQuery, selectedBatch),
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
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '导入历史',
          trailing: Text('${widget.snapshot.importBatches.length} batches'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _DictionarySyncImpact(sync: widget.snapshot.sync),
              const SizedBox(height: 14),
              _DictionaryImportHistoryControls(
                controller: importFilterController,
                newestImportFirst: newestImportFirst,
                onFilterChanged: (_) => setState(() {}),
                onToggleSort: () => setState(() {
                  newestImportFirst = !newestImportFirst;
                }),
              ),
              const SizedBox(height: 14),
              _DictionaryImportHistory(
                batches: visibleImportBatches,
                totalBatchCount: widget.snapshot.importBatches.length,
                selectedBatchId: selectedBatchId,
                onSelectBatch: (batch) => setState(() {
                  selectedBatchId = batch.id;
                }),
              ),
            ],
          ),
        ),
      ],
    );
  }

  DictionaryImportBatchSummary? _selectedBatch(
    List<DictionaryImportBatchSummary> batches,
  ) {
    for (final batch in batches) {
      if (batch.id == selectedBatchId) {
        return batch;
      }
    }
    return null;
  }

  List<UserTerm> _filterTerms(
    List<UserTerm> terms,
    DictionaryImportBatchSummary? selectedBatch,
  ) {
    final query = searchController.text.trim().toLowerCase();
    return terms
        .where((term) {
          final matchesQuery =
              query.isEmpty ||
              term.inputCode.toLowerCase().contains(query) ||
              term.text.toLowerCase().contains(query) ||
              term.reading.toLowerCase().contains(query) ||
              term.source.toLowerCase().contains(query);
          final matchesBatch =
              selectedBatch == null || term.source == selectedBatch.sourceName;
          return matchesQuery && matchesBatch;
        })
        .toList(growable: false);
  }

  List<DictionaryImportBatchSummary> _filterImportBatches(
    List<DictionaryImportBatchSummary> batches,
  ) {
    final query = importFilterController.text.trim().toLowerCase();
    final filtered = batches.where((batch) {
      return query.isEmpty ||
          batch.id.toString().contains(query) ||
          batch.sourceName.toLowerCase().contains(query) ||
          batch.createdAt.toLowerCase().contains(query) ||
          batch.notes.toLowerCase().contains(query);
    }).toList();

    filtered.sort((left, right) {
      final createdAtCompare = left.createdAt.compareTo(right.createdAt);
      final idCompare = left.id.compareTo(right.id);
      final result = createdAtCompare == 0 ? idCompare : createdAtCompare;
      return newestImportFirst ? -result : result;
    });
    return filtered;
  }
}

String _emptyTermsMessage(
  String searchQuery,
  DictionaryImportBatchSummary? selectedBatch,
) {
  if (selectedBatch != null && searchQuery.isNotEmpty) {
    return 'batch #${selectedBatch.id} 中没有匹配 "$searchQuery" 的词条';
  }
  if (selectedBatch != null) {
    return 'batch #${selectedBatch.id} / ${selectedBatch.sourceName} 暂无匹配词条';
  }
  return '没有匹配 "$searchQuery" 的词条';
}

class _DictionaryImportHistory extends StatelessWidget {
  const _DictionaryImportHistory({
    required this.batches,
    required this.totalBatchCount,
    required this.selectedBatchId,
    required this.onSelectBatch,
  });

  final List<DictionaryImportBatchSummary> batches;
  final int totalBatchCount;
  final int? selectedBatchId;
  final ValueChanged<DictionaryImportBatchSummary> onSelectBatch;

  @override
  Widget build(BuildContext context) {
    if (batches.isEmpty) {
      return _DictionaryEmptyState(
        icon: Icons.history_toggle_off_outlined,
        message: totalBatchCount == 0 ? '暂无导入历史' : '没有匹配当前筛选条件的导入批次',
      );
    }

    return SingleChildScrollView(
      key: const Key('dictionary-import-history'),
      scrollDirection: Axis.horizontal,
      child: DataTable(
        showCheckboxColumn: false,
        headingTextStyle: Theme.of(context).textTheme.labelMedium,
        columns: const [
          DataColumn(label: Text('batch')),
          DataColumn(label: Text('source')),
          DataColumn(label: Text('created at')),
          DataColumn(label: Text('records')),
          DataColumn(label: Text('inserted')),
          DataColumn(label: Text('updated')),
          DataColumn(label: Text('skipped')),
          DataColumn(label: Text('notes')),
        ],
        rows: batches
            .map(
              (batch) => DataRow(
                selected: batch.id == selectedBatchId,
                onSelectChanged: (_) => onSelectBatch(batch),
                cells: [
                  DataCell(Text('#${batch.id}')),
                  DataCell(Text(batch.sourceName)),
                  DataCell(Text(batch.createdAt)),
                  DataCell(
                    Text('${batch.importedTerms}/${batch.totalRecords}'),
                  ),
                  DataCell(Text(batch.insertedTerms.toString())),
                  DataCell(Text(batch.updatedTerms.toString())),
                  DataCell(
                    Text(
                      'deleted ${batch.skippedDeletedTerms}, '
                      'duplicate ${batch.skippedDuplicateTerms}',
                    ),
                  ),
                  DataCell(Text(batch.notes.isEmpty ? '无' : batch.notes)),
                ],
              ),
            )
            .toList(),
      ),
    );
  }
}

class _DictionarySyncImpact extends StatelessWidget {
  const _DictionarySyncImpact({required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Text(
          '本地 sync preflight 影响',
          style: Theme.of(context).textTheme.labelLarge,
        ),
        Chip(
          avatar: const Icon(Icons.inventory_2_outlined, size: 18),
          label: Text('syncable ${sync.syncableObjects}'),
        ),
        Chip(
          avatar: const Icon(Icons.lock_clock_outlined, size: 18),
          label: Text('local-only ${sync.localOnlyEvents}'),
        ),
        ...sync.categories.map(
          (category) => Chip(label: Text('${category.name} ${category.count}')),
        ),
      ],
    );
  }
}

class _DictionaryImportHistoryControls extends StatelessWidget {
  const _DictionaryImportHistoryControls({
    required this.controller,
    required this.newestImportFirst,
    required this.onFilterChanged,
    required this.onToggleSort,
  });

  final TextEditingController controller;
  final bool newestImportFirst;
  final ValueChanged<String> onFilterChanged;
  final VoidCallback onToggleSort;

  @override
  Widget build(BuildContext context) {
    final filterField = TextField(
      key: const Key('dictionary-import-history-filter'),
      controller: controller,
      decoration: const InputDecoration(
        prefixIcon: Icon(Icons.search),
        labelText: '筛选 batch / source / created at / notes',
      ),
      onChanged: onFilterChanged,
    );
    final sortButton = OutlinedButton.icon(
      key: const Key('dictionary-import-history-sort'),
      onPressed: onToggleSort,
      icon: const Icon(Icons.swap_vert),
      label: Text(newestImportFirst ? '最新优先' : '最早优先'),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 560) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              filterField,
              const SizedBox(height: 10),
              Align(alignment: Alignment.centerLeft, child: sortButton),
            ],
          );
        }

        return Row(
          children: [
            Expanded(child: filterField),
            const SizedBox(width: 10),
            sortButton,
          ],
        );
      },
    );
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
