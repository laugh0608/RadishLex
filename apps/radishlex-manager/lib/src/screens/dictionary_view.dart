import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'dictionary/import_history_section.dart';
import 'manager_widgets.dart';

class DictionaryView extends StatefulWidget {
  const DictionaryView({
    super.key,
    required this.snapshot,
    required this.onDeleteTerm,
    required this.onRestoreTerm,
    required this.onImportDictionary,
    required this.onExportDictionary,
  });

  final ManagerSnapshot snapshot;
  final ValueChanged<UserTerm> onDeleteTerm;
  final void Function(UserTermKey term, String state) onRestoreTerm;
  final VoidCallback onImportDictionary;
  final VoidCallback onExportDictionary;

  @override
  State<DictionaryView> createState() => _DictionaryViewState();
}

class _DictionaryViewState extends State<DictionaryView> {
  final searchController = TextEditingController();
  final importFilterController = TextEditingController();
  int? selectedBatchId;
  UserTermKey? selectedTermKey;
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
    final selectedTerm = _selectedTerm(visibleTerms);
    final selectedTermImportBatch = _importBatchForTerm(
      selectedTerm,
      widget.snapshot.importBatches,
    );
    final selectedTermTombstone = _tombstoneForTerm(
      selectedTerm,
      widget.snapshot.deletedTerms,
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
                  selectedTermKey: selectedTermKey,
                  onSelectTerm: (term) => setState(() {
                    selectedTermKey = term.key;
                  }),
                  onDeleteTerm: widget.onDeleteTerm,
                  onRestoreTerm: widget.onRestoreTerm,
                ),
              const SizedBox(height: 14),
              _DeletedTermsStrip(
                deletedTerms: widget.snapshot.deletedTerms,
                onRestoreTerm: widget.onRestoreTerm,
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '词条审计详情',
          child: _DictionaryTermAuditPanel(
            term: selectedTerm,
            importBatch: selectedTermImportBatch,
            tombstone: selectedTermTombstone,
            sync: widget.snapshot.sync,
          ),
        ),
        const SizedBox(height: 16),
        DictionaryImportHistorySection(
          batches: widget.snapshot.importBatches,
          visibleBatches: visibleImportBatches,
          sync: widget.snapshot.sync,
          controller: importFilterController,
          newestImportFirst: newestImportFirst,
          selectedBatchId: selectedBatchId,
          onFilterChanged: (_) => setState(() {}),
          onToggleSort: () => setState(() {
            newestImportFirst = !newestImportFirst;
          }),
          onSelectBatch: (batch) => setState(() {
            selectedBatchId = batch.id;
          }),
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

  UserTerm? _selectedTerm(List<UserTerm> terms) {
    final key = selectedTermKey;
    if (key == null) {
      return null;
    }
    for (final term in terms) {
      if (term.key == key) {
        return term;
      }
    }
    return null;
  }

  DictionaryImportBatchSummary? _importBatchForTerm(
    UserTerm? term,
    List<DictionaryImportBatchSummary> batches,
  ) {
    if (term == null) {
      return null;
    }
    final importBatchId = term.importBatchId;
    if (importBatchId == null) {
      return null;
    }
    for (final batch in batches) {
      if (batch.id == importBatchId) {
        return batch;
      }
    }
    return null;
  }

  DeletedTerm? _tombstoneForTerm(
    UserTerm? term,
    List<DeletedTerm> deletedTerms,
  ) {
    if (term == null) {
      return null;
    }
    for (final deletedTerm in deletedTerms) {
      if (deletedTerm.inputCode == term.inputCode &&
          deletedTerm.text == term.text &&
          deletedTerm.reading == term.reading) {
        return deletedTerm;
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
              selectedBatch == null || term.importBatchId == selectedBatch.id;
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

class _DictionaryTermAuditPanel extends StatelessWidget {
  const _DictionaryTermAuditPanel({
    required this.term,
    required this.importBatch,
    required this.tombstone,
    required this.sync,
  });

  final UserTerm? term;
  final DictionaryImportBatchSummary? importBatch;
  final DeletedTerm? tombstone;
  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    final selectedTerm = term;
    if (selectedTerm == null) {
      return const _DictionaryEmptyState(
        icon: Icons.info_outline,
        message: '选择一个词条查看 key、来源、导入批次、tombstone 和 sync 分类',
      );
    }

    final batch = importBatch;
    final deletedTerm = tombstone;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            ManagerStatusBadge(
              icon: Icons.key_outlined,
              label: '${selectedTerm.status} user term',
              tone: selectedTerm.status == 'active'
                  ? ManagerBadgeTone.success
                  : ManagerBadgeTone.warning,
            ),
            ManagerStatusBadge(
              icon: Icons.sync_outlined,
              label: sync.state.code,
              tone: ManagerBadgeTone.warning,
            ),
          ],
        ),
        const SizedBox(height: 10),
        ManagerKeyValueRow(label: 'input code', value: selectedTerm.inputCode),
        ManagerKeyValueRow(label: 'text', value: selectedTerm.text),
        ManagerKeyValueRow(label: 'reading', value: selectedTerm.reading),
        ManagerKeyValueRow(label: 'source', value: selectedTerm.source),
        ManagerKeyValueRow(label: 'status', value: selectedTerm.status),
        ManagerKeyValueRow(
          label: 'weight',
          value: selectedTerm.weight.toStringAsFixed(2),
        ),
        ManagerKeyValueRow(label: 'last used', value: selectedTerm.lastUsed),
        ManagerKeyValueRow(
          label: 'import batch',
          value: _importBatchAuditLabel(batch),
        ),
        ManagerKeyValueRow(
          label: 'tombstone',
          value: deletedTerm == null
              ? '未删除；删除后会写入 tombstone'
              : '已存在 tombstone：${deletedTerm.deletedAt}',
        ),
        ManagerKeyValueRow(
          label: 'sync category',
          value:
              'dictionary.user_terms '
              '(${_categoryCount(sync, "dictionary.user_terms")})',
        ),
      ],
    );
  }
}

String _importBatchAuditLabel(DictionaryImportBatchSummary? batch) {
  if (batch == null) {
    return '无匹配导入批次';
  }
  return '#${batch.id} / ${batch.sourceName} / '
      '${batch.importedTerms}/${batch.totalRecords} / ${batch.createdAt}';
}

int _categoryCount(SyncPreflightSummary sync, String categoryName) {
  for (final category in sync.categories) {
    if (category.name == categoryName) {
      return category.count;
    }
  }
  return 0;
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

class _DictionaryTermsTable extends StatelessWidget {
  const _DictionaryTermsTable({
    required this.terms,
    required this.selectedTermKey,
    required this.onSelectTerm,
    required this.onDeleteTerm,
    required this.onRestoreTerm,
  });

  final List<UserTerm> terms;
  final UserTermKey? selectedTermKey;
  final ValueChanged<UserTerm> onSelectTerm;
  final ValueChanged<UserTerm> onDeleteTerm;
  final void Function(UserTermKey term, String state) onRestoreTerm;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: DataTable(
        showCheckboxColumn: false,
        headingTextStyle: Theme.of(context).textTheme.labelMedium,
        columns: const [
          DataColumn(label: Text('input code')),
          DataColumn(label: Text('text')),
          DataColumn(label: Text('reading')),
          DataColumn(label: Text('weight')),
          DataColumn(label: Text('source')),
          DataColumn(label: Text('status')),
          DataColumn(label: Text('')),
        ],
        rows: terms
            .map(
              (term) => DataRow(
                selected: term.key == selectedTermKey,
                onSelectChanged: (_) => onSelectTerm(term),
                cells: [
                  DataCell(Text(term.inputCode)),
                  DataCell(Text(term.text)),
                  DataCell(Text(term.reading)),
                  DataCell(Text(term.weight.toStringAsFixed(2))),
                  DataCell(Text(term.source)),
                  DataCell(Text(term.status)),
                  DataCell(
                    IconButton(
                      tooltip: term.status == 'suppressed' ? '显式恢复词条' : '删除词条',
                      onPressed: term.status == 'suppressed'
                          ? () => onRestoreTerm(term.key, term.status)
                          : () => onDeleteTerm(term),
                      icon: Icon(
                        term.status == 'suppressed'
                            ? Icons.restore
                            : Icons.delete_outline,
                      ),
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
  const _DeletedTermsStrip({
    required this.deletedTerms,
    required this.onRestoreTerm,
  });

  final List<DeletedTerm> deletedTerms;
  final void Function(UserTermKey term, String state) onRestoreTerm;

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
          (term) => InputChip(
            avatar: const Icon(Icons.block, size: 18),
            label: Text('${term.inputCode} / ${term.text}'),
            tooltip: '${term.deletedAt} / ${term.reason}',
            onPressed: () => onRestoreTerm(term.key, 'deleted'),
            deleteIcon: const Icon(Icons.restore, size: 18),
            onDeleted: () => onRestoreTerm(term.key, 'deleted'),
          ),
        ),
        if (deletedTerms.isEmpty) const Text('暂无 deleted tombstone'),
      ],
    );
  }
}
