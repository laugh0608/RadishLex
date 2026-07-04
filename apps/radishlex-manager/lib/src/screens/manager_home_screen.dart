import 'package:flutter/material.dart';

import '../bridge/manager_bridge.dart';
import '../models/manager_models.dart';

class ManagerHomeScreen extends StatefulWidget {
  const ManagerHomeScreen({super.key, required this.bridge});

  final ManagerBridge bridge;

  @override
  State<ManagerHomeScreen> createState() => _ManagerHomeScreenState();
}

class _ManagerHomeScreenState extends State<ManagerHomeScreen> {
  int selectedIndex = 0;
  late Future<ManagerSnapshot> snapshotFuture;

  @override
  void initState() {
    super.initState();
    snapshotFuture = widget.bridge.loadSnapshot();
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<ManagerSnapshot>(
      future: snapshotFuture,
      builder: (context, snapshot) {
        if (snapshot.hasError) {
          return _ManagerLoadFailure(onRetry: _reloadSnapshot);
        }
        final data = snapshot.data;
        if (data == null) {
          return const _ManagerLoading();
        }
        return _ManagerShell(
          snapshot: data,
          selectedIndex: selectedIndex,
          onSelectPage: _selectPage,
          onDeleteTerm: _deleteTerm,
          onImportDictionary: _importDictionary,
          onExportDictionary: _exportDictionary,
        );
      },
    );
  }

  void _selectPage(int index) {
    setState(() {
      selectedIndex = index;
    });
  }

  void _reloadSnapshot() {
    setState(() {
      snapshotFuture = widget.bridge.loadSnapshot();
    });
  }

  void _deleteTerm(UserTerm term) {
    setState(() {
      snapshotFuture = widget.bridge.deleteUserTerm(term.key);
    });
  }

  Future<void> _importDictionary() async {
    final request = await showDialog<_DictionaryImportRequest>(
      context: context,
      builder: (context) => const _DictionaryImportDialog(),
    );
    if (!mounted || request == null) {
      return;
    }

    try {
      final preview = await widget.bridge.inspectDictionaryImport(
        request.filePath,
      );
      if (!mounted) {
        return;
      }
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) =>
            _DictionaryImportPreviewDialog(preview: preview, request: request),
      );
      if (!mounted || confirmed != true) {
        return;
      }

      final result = await widget.bridge.importDictionaryFile(
        filePath: request.filePath,
        sourceName: request.sourceName,
        dryRun: request.dryRun,
      );
      if (!mounted) {
        return;
      }
      _showBridgeMessage(
        result.dryRun
            ? '导入检查完成：${result.totalRecords} 条'
            : '导入完成：${result.importedTerms} / ${result.totalRecords} 条',
      );
      _reloadSnapshot();
    } on Object {
      if (mounted) {
        _showBridgeMessage('管理端 bridge 调用失败');
      }
    }
  }

  Future<void> _exportDictionary() async {
    final filePath = await showDialog<String>(
      context: context,
      builder: (context) => const _DictionaryExportDialog(),
    );
    if (!mounted || filePath == null) {
      return;
    }

    try {
      final result = await widget.bridge.exportDictionaryFile(filePath);
      if (!mounted) {
        return;
      }
      _showBridgeMessage('导出完成：${result.exportedTerms} 条');
    } on Object {
      if (mounted) {
        _showBridgeMessage('管理端 bridge 调用失败');
      }
    }
  }

  void _showBridgeMessage(String message) {
    final messenger = ScaffoldMessenger.of(context);
    messenger.clearSnackBars();
    messenger.showSnackBar(SnackBar(content: Text(message)));
  }
}

class _ManagerShell extends StatelessWidget {
  const _ManagerShell({
    required this.snapshot,
    required this.selectedIndex,
    required this.onSelectPage,
    required this.onDeleteTerm,
    required this.onImportDictionary,
    required this.onExportDictionary,
  });

  final ManagerSnapshot snapshot;
  final int selectedIndex;
  final ValueChanged<int> onSelectPage;
  final ValueChanged<UserTerm> onDeleteTerm;
  final VoidCallback onImportDictionary;
  final VoidCallback onExportDictionary;

  @override
  Widget build(BuildContext context) {
    final pages = [
      _DictionaryView(
        snapshot: snapshot,
        onDeleteTerm: onDeleteTerm,
        onImportDictionary: onImportDictionary,
        onExportDictionary: onExportDictionary,
      ),
      _LearningView(snapshot: snapshot),
      _SyncView(sync: snapshot.sync),
      _SettingsView(settings: snapshot.settings),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final useRail = constraints.maxWidth >= 820;
        final content = _ManagerPageFrame(
          title: _destinations[selectedIndex].label,
          snapshot: snapshot,
          child: pages[selectedIndex],
        );

        if (useRail) {
          return Scaffold(
            body: Row(
              children: [
                NavigationRail(
                  selectedIndex: selectedIndex,
                  onDestinationSelected: onSelectPage,
                  labelType: NavigationRailLabelType.all,
                  destinations: _destinations
                      .map(
                        (destination) => NavigationRailDestination(
                          icon: Icon(destination.icon),
                          selectedIcon: Icon(destination.selectedIcon),
                          label: Text(destination.label),
                        ),
                      )
                      .toList(),
                ),
                const VerticalDivider(width: 1),
                Expanded(child: content),
              ],
            ),
          );
        }

        return Scaffold(
          body: content,
          bottomNavigationBar: NavigationBar(
            selectedIndex: selectedIndex,
            onDestinationSelected: onSelectPage,
            destinations: _destinations
                .map(
                  (destination) => NavigationDestination(
                    icon: Icon(destination.icon),
                    selectedIcon: Icon(destination.selectedIcon),
                    label: destination.label,
                  ),
                )
                .toList(),
          ),
        );
      },
    );
  }
}

class _ManagerLoading extends StatelessWidget {
  const _ManagerLoading();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Center(
        child: SizedBox.square(
          dimension: 28,
          child: CircularProgressIndicator(strokeWidth: 3),
        ),
      ),
    );
  }
}

class _ManagerLoadFailure extends StatelessWidget {
  const _ManagerLoadFailure({required this.onRetry});

  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Card(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.error_outline),
                const SizedBox(height: 12),
                const Text('管理端数据加载失败'),
                const SizedBox(height: 12),
                FilledButton.icon(
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: const Text('重试'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _Destination {
  const _Destination({
    required this.label,
    required this.icon,
    required this.selectedIcon,
  });

  final String label;
  final IconData icon;
  final IconData selectedIcon;
}

const _destinations = [
  _Destination(
    label: '词库',
    icon: Icons.library_books_outlined,
    selectedIcon: Icons.library_books,
  ),
  _Destination(
    label: '学习',
    icon: Icons.psychology_alt_outlined,
    selectedIcon: Icons.psychology_alt,
  ),
  _Destination(
    label: '同步',
    icon: Icons.sync_outlined,
    selectedIcon: Icons.sync,
  ),
  _Destination(
    label: '设置',
    icon: Icons.tune_outlined,
    selectedIcon: Icons.tune,
  ),
];

class _ManagerPageFrame extends StatelessWidget {
  const _ManagerPageFrame({
    required this.title,
    required this.snapshot,
    required this.child,
  });

  final String title;
  final ManagerSnapshot snapshot;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 18, 24, 12),
            child: _Header(title: title, snapshot: snapshot),
          ),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
              child: child,
            ),
          ),
        ],
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.title, required this.snapshot});

  final String title;
  final ManagerSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Row(
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('萝卜词核', style: theme.textTheme.labelLarge),
              const SizedBox(height: 4),
              Text(title, style: theme.textTheme.headlineSmall),
            ],
          ),
        ),
        _StatusBadge(
          icon: Icons.shield_outlined,
          label: snapshot.sync.state.code,
          tone: _BadgeTone.warning,
        ),
      ],
    );
  }
}

class _DictionaryView extends StatelessWidget {
  const _DictionaryView({
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
  Widget build(BuildContext context) {
    return _Section(
      title: '本地词库',
      trailing: Wrap(
        spacing: 8,
        children: [
          OutlinedButton.icon(
            key: const Key('dictionary-import-button'),
            onPressed: onImportDictionary,
            icon: const Icon(Icons.upload_file_outlined),
            label: const Text('导入'),
          ),
          FilledButton.icon(
            key: const Key('dictionary-export-button'),
            onPressed: onExportDictionary,
            icon: const Icon(Icons.download_outlined),
            label: const Text('导出'),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const TextField(
            decoration: InputDecoration(
              prefixIcon: Icon(Icons.search),
              labelText: '搜索 input code / text',
            ),
          ),
          const SizedBox(height: 14),
          SingleChildScrollView(
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
              rows: snapshot.dictionaryTerms
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
          ),
          const SizedBox(height: 14),
          _DeletedTermsStrip(deletedTerms: snapshot.deletedTerms),
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
      ],
    );
  }
}

class _DictionaryImportRequest {
  const _DictionaryImportRequest({
    required this.filePath,
    required this.sourceName,
    required this.dryRun,
  });

  final String filePath;
  final String sourceName;
  final bool dryRun;
}

class _DictionaryImportDialog extends StatefulWidget {
  const _DictionaryImportDialog();

  @override
  State<_DictionaryImportDialog> createState() =>
      _DictionaryImportDialogState();
}

class _DictionaryImportDialogState extends State<_DictionaryImportDialog> {
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
                  _DictionaryImportRequest(
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

class _DictionaryImportPreviewDialog extends StatelessWidget {
  const _DictionaryImportPreviewDialog({
    required this.preview,
    required this.request,
  });

  final DictionaryImportPreview preview;
  final _DictionaryImportRequest request;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('导入检查'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _KeyValueRow(label: 'file', value: preview.filePath),
            _KeyValueRow(label: 'format', value: preview.format),
            _KeyValueRow(
              label: 'records',
              value: preview.recordCount.toString(),
            ),
            _KeyValueRow(label: 'sync class', value: preview.syncClass),
            _KeyValueRow(label: 'dry run', value: request.dryRun.toString()),
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

class _DictionaryExportDialog extends StatefulWidget {
  const _DictionaryExportDialog();

  @override
  State<_DictionaryExportDialog> createState() =>
      _DictionaryExportDialogState();
}

class _DictionaryExportDialogState extends State<_DictionaryExportDialog> {
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

class _LearningView extends StatelessWidget {
  const _LearningView({required this.snapshot});

  final ManagerSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final summary = snapshot.learningSummary;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            _MetricTile(
              icon: Icons.library_add_check_outlined,
              label: 'user terms',
              value: summary.userTerms.toString(),
            ),
            _MetricTile(
              icon: Icons.history_outlined,
              label: 'selection events',
              value: summary.selectionEvents.toString(),
            ),
            _MetricTile(
              icon: Icons.block_outlined,
              label: 'suppressed',
              value: summary.suppressedTerms.toString(),
            ),
            _MetricTile(
              icon: Icons.delete_sweep_outlined,
              label: 'deleted',
              value: summary.deletedTerms.toString(),
            ),
          ],
        ),
        const SizedBox(height: 16),
        _Section(
          title: 'rank explain',
          trailing: Text('updated ${summary.lastUpdated}'),
          child: Column(
            children: snapshot.explanations
                .map((explanation) => _RankerExplanationRow(explanation))
                .toList(),
          ),
        ),
      ],
    );
  }
}

class _RankerExplanationRow extends StatelessWidget {
  const _RankerExplanationRow(this.explanation);

  final RankerExplanation explanation;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 104,
            child: Text(
              explanation.inputCode,
              style: Theme.of(context).textTheme.labelLarge,
            ),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(explanation.candidate),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: explanation.signals
                      .map((signal) => Chip(label: Text(signal)))
                      .toList(),
                ),
              ],
            ),
          ),
          _StatusBadge(
            icon: Icons.trending_up,
            label: explanation.score.toStringAsFixed(2),
            tone: _BadgeTone.success,
          ),
        ],
      ),
    );
  }
}

class _SyncView extends StatelessWidget {
  const _SyncView({required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _Section(
          title: '同步预检',
          trailing: _StatusBadge(
            icon: Icons.cloud_off_outlined,
            label: sync.state.code,
            tone: _BadgeTone.warning,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _KeyValueRow(label: 'server', value: sync.serverEndpoint),
              _KeyValueRow(label: 'reason', value: sync.reason),
              _KeyValueRow(
                label: 'syncable objects',
                value: sync.syncableObjects.toString(),
              ),
              _KeyValueRow(
                label: 'local-only events',
                value: sync.localOnlyEvents.toString(),
              ),
              _KeyValueRow(label: 'last upload', value: sync.lastUpload),
              _KeyValueRow(label: 'last download', value: sync.lastDownload),
              const SizedBox(height: 14),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: sync.categories
                    .map(
                      (category) => Chip(
                        label: Text('${category.name}: ${category.count}'),
                      ),
                    )
                    .toList(),
              ),
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  FilledButton.icon(
                    onPressed: sync.state.canEnableUserSync ? () {} : null,
                    icon: const Icon(Icons.cloud_upload_outlined),
                    label: const Text('启用同步'),
                  ),
                  OutlinedButton.icon(
                    onPressed: () {},
                    icon: const Icon(Icons.content_copy_outlined),
                    label: const Text('复制诊断摘要'),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Section(
          title: '设备签名',
          child: Column(
            children: [
              _KeyValueRow(label: 'device id', value: sync.device.deviceId),
              _KeyValueRow(label: 'backend', value: sync.device.backendId),
              _KeyValueRow(
                label: 'capability',
                value: sync.device.capabilityStatus,
              ),
              _KeyValueRow(label: 'gate', value: sync.device.productionGate),
            ],
          ),
        ),
      ],
    );
  }
}

class _SettingsView extends StatelessWidget {
  const _SettingsView({required this.settings});

  final ManagerSettings settings;

  @override
  Widget build(BuildContext context) {
    return _Section(
      title: '设置',
      child: Column(
        children: [
          const TextField(
            decoration: InputDecoration(
              prefixIcon: Icon(Icons.dns_outlined),
              labelText: '自部署服务端',
            ),
          ),
          const SizedBox(height: 12),
          SwitchListTile(
            value: settings.privacyMode,
            onChanged: (_) {},
            secondary: const Icon(Icons.privacy_tip_outlined),
            title: const Text('隐私模式'),
          ),
          SwitchListTile(
            value: settings.diagnosticsExport,
            onChanged: (_) {},
            secondary: const Icon(Icons.bug_report_outlined),
            title: const Text('诊断摘要导出'),
          ),
          CheckboxListTile(
            value: settings.syncConfigured,
            onChanged: (_) {},
            secondary: const Icon(Icons.cloud_done_outlined),
            title: const Text('保留同步配置草案'),
          ),
        ],
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.title, required this.child, this.trailing});

  final String title;
  final Widget child;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                ?trailing,
              ],
            ),
            const SizedBox(height: 14),
            child,
          ],
        ),
      ),
    );
  }
}

class _MetricTile extends StatelessWidget {
  const _MetricTile({
    required this.icon,
    required this.label,
    required this.value,
  });

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 190,
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              Icon(icon),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(label, style: Theme.of(context).textTheme.labelMedium),
                    const SizedBox(height: 2),
                    Text(value, style: Theme.of(context).textTheme.titleLarge),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _KeyValueRow extends StatelessWidget {
  const _KeyValueRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 148,
            child: Text(label, style: Theme.of(context).textTheme.labelLarge),
          ),
          Expanded(child: Text(value)),
        ],
      ),
    );
  }
}

enum _BadgeTone { success, warning }

class _StatusBadge extends StatelessWidget {
  const _StatusBadge({
    required this.icon,
    required this.label,
    required this.tone,
  });

  final IconData icon;
  final String label;
  final _BadgeTone tone;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final background = tone == _BadgeTone.success
        ? const Color(0xFFE7F3EC)
        : const Color(0xFFFFF1D8);
    final foreground = tone == _BadgeTone.success
        ? const Color(0xFF1F6B45)
        : const Color(0xFF8A4F00);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: colors.outlineVariant),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 16, color: foreground),
          const SizedBox(width: 6),
          Text(label, style: TextStyle(color: foreground)),
        ],
      ),
    );
  }
}
