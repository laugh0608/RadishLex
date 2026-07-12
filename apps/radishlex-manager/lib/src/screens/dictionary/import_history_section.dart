import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class DictionaryImportHistorySection extends StatelessWidget {
  const DictionaryImportHistorySection({
    super.key,
    required this.batches,
    required this.visibleBatches,
    required this.sync,
    required this.controller,
    required this.newestImportFirst,
    required this.selectedBatchId,
    required this.onFilterChanged,
    required this.onToggleSort,
    required this.onSelectBatch,
  });

  final List<DictionaryImportBatchSummary> batches;
  final List<DictionaryImportBatchSummary> visibleBatches;
  final SyncPreflightSummary sync;
  final TextEditingController controller;
  final bool newestImportFirst;
  final int? selectedBatchId;
  final ValueChanged<String> onFilterChanged;
  final VoidCallback onToggleSort;
  final ValueChanged<DictionaryImportBatchSummary> onSelectBatch;

  @override
  Widget build(BuildContext context) {
    return ManagerSection(
      title: '导入历史',
      trailing: Text('${batches.length} batches'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _DictionarySyncImpact(sync: sync),
          const SizedBox(height: 14),
          _DictionaryImportHistoryControls(
            controller: controller,
            newestImportFirst: newestImportFirst,
            onFilterChanged: onFilterChanged,
            onToggleSort: onToggleSort,
          ),
          const SizedBox(height: 14),
          _DictionaryImportHistory(
            batches: visibleBatches,
            totalBatchCount: batches.length,
            selectedBatchId: selectedBatchId,
            onSelectBatch: onSelectBatch,
          ),
        ],
      ),
    );
  }
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
      return _DictionaryImportHistoryEmptyState(
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

class _DictionaryImportHistoryEmptyState extends StatelessWidget {
  const _DictionaryImportHistoryEmptyState({
    required this.icon,
    required this.message,
  });

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
