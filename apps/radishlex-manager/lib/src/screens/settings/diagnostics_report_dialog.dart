import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../models/manager_models.dart';

class DiagnosticsReportDialog extends StatefulWidget {
  const DiagnosticsReportDialog({super.key, required this.report});

  final ManagerDiagnosticsReport report;

  @override
  State<DiagnosticsReportDialog> createState() =>
      _DiagnosticsReportDialogState();
}

class _DiagnosticsReportDialogState extends State<DiagnosticsReportDialog> {
  final filterController = TextEditingController();
  String? selectedSection;

  @override
  void dispose() {
    filterController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final report = widget.report;
    final visibleSections = _filteredSections();

    return AlertDialog(
      title: const Text('诊断摘要预览'),
      content: SizedBox(
        width: 720,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxHeight: 560),
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _DiagnosticsReportSummary(report: report),
                const SizedBox(height: 12),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    FilterChip(
                      key: const Key('diagnostics-section-all'),
                      selected: selectedSection == null,
                      label: Text('全部 ${report.itemCount}'),
                      onSelected: (_) => setState(() {
                        selectedSection = null;
                      }),
                    ),
                    for (final section in report.sections)
                      FilterChip(
                        key: Key('diagnostics-section-${section.title}'),
                        selected: selectedSection == section.title,
                        label: Text('${section.title} ${section.items.length}'),
                        onSelected: (_) => setState(() {
                          selectedSection = section.title;
                        }),
                      ),
                  ],
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('diagnostics-report-filter'),
                  controller: filterController,
                  decoration: InputDecoration(
                    prefixIcon: const Icon(Icons.search),
                    labelText: '筛选字段',
                    suffixIcon: filterController.text.isEmpty
                        ? null
                        : IconButton(
                            key: const Key('diagnostics-report-filter-clear'),
                            onPressed: () => setState(filterController.clear),
                            icon: const Icon(Icons.close),
                          ),
                  ),
                  onChanged: (_) => setState(() {}),
                ),
                const SizedBox(height: 12),
                if (visibleSections.isEmpty)
                  const Padding(
                    padding: EdgeInsets.symmetric(vertical: 24),
                    child: Text('没有匹配的诊断字段'),
                  )
                else
                  for (final section in visibleSections) ...[
                    _DiagnosticsReportSectionView(section: section),
                    const SizedBox(height: 12),
                  ],
                const Divider(height: 28),
                Text('脱敏文本', style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border.all(
                      color: Theme.of(context).colorScheme.outlineVariant,
                    ),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: SelectableText(
                      report.toRedactedText(),
                      key: const Key('diagnostics-report-text'),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
      actions: [
        OutlinedButton.icon(
          key: const Key('diagnostics-copy-button'),
          onPressed: _copyReport,
          icon: const Icon(Icons.copy_outlined),
          label: const Text('复制'),
        ),
        FilledButton.icon(
          onPressed: () => Navigator.of(context).pop(),
          icon: const Icon(Icons.check),
          label: const Text('关闭'),
        ),
      ],
    );
  }

  List<ManagerDiagnosticsSection> _filteredSections() {
    final query = filterController.text.trim().toLowerCase();
    final sections = selectedSection == null
        ? widget.report.sections
        : widget.report.sections.where(
            (section) => section.title == selectedSection,
          );

    final visibleSections = <ManagerDiagnosticsSection>[];
    for (final section in sections) {
      final items = _filteredItems(section, query);
      if (items.isNotEmpty) {
        visibleSections.add(
          ManagerDiagnosticsSection(title: section.title, items: items),
        );
      }
    }
    return visibleSections;
  }

  List<ManagerDiagnosticsItem> _filteredItems(
    ManagerDiagnosticsSection section,
    String query,
  ) {
    if (query.isEmpty) {
      return section.items;
    }
    if (section.title.toLowerCase().contains(query)) {
      return section.items;
    }
    return section.items.where((item) {
      return item.key.toLowerCase().contains(query) ||
          item.value.toLowerCase().contains(query) ||
          item.kind.toLowerCase().contains(query);
    }).toList();
  }

  Future<void> _copyReport() async {
    await Clipboard.setData(
      ClipboardData(text: widget.report.toRedactedText()),
    );
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(const SnackBar(content: Text('诊断摘要已复制')));
  }
}

class _DiagnosticsReportSummary extends StatelessWidget {
  const _DiagnosticsReportSummary({required this.report});

  final ManagerDiagnosticsReport report;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        Chip(label: Text(report.format)),
        Chip(label: Text('分组 ${report.sections.length}')),
        Chip(label: Text('字段 ${report.itemCount}')),
        Chip(label: Text(report.redactionPolicy)),
      ],
    );
  }
}

class _DiagnosticsReportSectionView extends StatelessWidget {
  const _DiagnosticsReportSectionView({required this.section});

  final ManagerDiagnosticsSection section;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;

    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border.all(color: colors.outlineVariant),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    section.title,
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                ),
                Chip(label: Text('字段 ${section.items.length}')),
              ],
            ),
            const SizedBox(height: 8),
            for (final item in section.items) _DiagnosticsReportItemRow(item),
          ],
        ),
      ),
    );
  }
}

class _DiagnosticsReportItemRow extends StatelessWidget {
  const _DiagnosticsReportItemRow(this.item);

  final ManagerDiagnosticsItem item;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;

    return Container(
      key: Key('diagnostics-item-${item.key}'),
      width: double.infinity,
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: colors.surfaceContainerHighest.withValues(alpha: 0.42),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SelectableText(
                  item.key,
                  style: Theme.of(context).textTheme.labelLarge,
                ),
              ),
              const SizedBox(width: 8),
              Chip(label: Text(item.kind)),
            ],
          ),
          const SizedBox(height: 4),
          SelectableText(item.value),
        ],
      ),
    );
  }
}
