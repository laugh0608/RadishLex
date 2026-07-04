import 'package:flutter/material.dart';

import '../bridge/manager_bridge.dart';
import '../models/manager_models.dart';
import 'dictionary_view.dart';
import 'learning_view.dart';
import 'manager_widgets.dart';
import 'settings_view.dart';
import 'sync_view.dart';

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
          return _ManagerLoadFailure(
            error: snapshot.error,
            onRetry: _reloadSnapshot,
          );
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
          onPreviewDiagnostics: _previewDiagnostics,
          onExportDiagnostics: _exportDiagnostics,
          onSaveSettingsDraft: _saveSettingsDraft,
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
    final request = await showDialog<DictionaryImportRequest>(
      context: context,
      builder: (context) => const DictionaryImportDialog(),
    );
    if (!mounted || request == null) {
      return;
    }

    var operation = ManagerBridgeOperation.inspectDictionaryImport;
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
            DictionaryImportPreviewDialog(preview: preview, request: request),
      );
      if (!mounted || confirmed != true) {
        return;
      }

      operation = ManagerBridgeOperation.importDictionaryFile;
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
    } on Object catch (error) {
      if (mounted) {
        _showBridgeMessage(_bridgeFailureMessage(error, operation));
      }
    }
  }

  Future<void> _exportDictionary() async {
    final filePath = await showDialog<String>(
      context: context,
      builder: (context) => const DictionaryExportDialog(),
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
    } on Object catch (error) {
      if (mounted) {
        _showBridgeMessage(
          _bridgeFailureMessage(
            error,
            ManagerBridgeOperation.exportDictionaryFile,
          ),
        );
      }
    }
  }

  Future<void> _previewDiagnostics() async {
    try {
      final report = await widget.bridge.loadDiagnosticsReport();
      if (!mounted) {
        return;
      }
      await showDialog<void>(
        context: context,
        builder: (context) => DiagnosticsReportDialog(report: report),
      );
    } on Object catch (error) {
      if (mounted) {
        _showBridgeMessage(
          _bridgeFailureMessage(
            error,
            ManagerBridgeOperation.previewDiagnostics,
          ),
        );
      }
    }
  }

  Future<void> _exportDiagnostics() async {
    final filePath = await showDialog<String>(
      context: context,
      builder: (context) => const DiagnosticsExportDialog(),
    );
    if (!mounted || filePath == null) {
      return;
    }

    try {
      final result = await widget.bridge.exportDiagnosticsReport(filePath);
      if (!mounted) {
        return;
      }
      _showBridgeMessage('诊断摘要导出完成：${result.lineCount} 行');
    } on Object catch (error) {
      if (mounted) {
        _showBridgeMessage(
          _bridgeFailureMessage(
            error,
            ManagerBridgeOperation.exportDiagnostics,
          ),
        );
      }
    }
  }

  Future<void> _saveSettingsDraft(ManagerSettingsDraft draft) async {
    try {
      final snapshot = await widget.bridge.saveSettingsDraft(draft);
      if (!mounted) {
        return;
      }
      setState(() {
        snapshotFuture = Future.value(snapshot);
      });
      _showBridgeMessage('设置草案已保存：${snapshot.sync.state.code}');
    } on Object catch (error) {
      if (mounted) {
        _showBridgeMessage(
          _bridgeFailureMessage(
            error,
            ManagerBridgeOperation.saveSettingsDraft,
          ),
        );
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
    required this.onPreviewDiagnostics,
    required this.onExportDiagnostics,
    required this.onSaveSettingsDraft,
  });

  final ManagerSnapshot snapshot;
  final int selectedIndex;
  final ValueChanged<int> onSelectPage;
  final ValueChanged<UserTerm> onDeleteTerm;
  final VoidCallback onImportDictionary;
  final VoidCallback onExportDictionary;
  final VoidCallback onPreviewDiagnostics;
  final VoidCallback onExportDiagnostics;
  final ValueChanged<ManagerSettingsDraft> onSaveSettingsDraft;

  @override
  Widget build(BuildContext context) {
    final pages = [
      DictionaryView(
        snapshot: snapshot,
        onDeleteTerm: onDeleteTerm,
        onImportDictionary: onImportDictionary,
        onExportDictionary: onExportDictionary,
      ),
      LearningView(snapshot: snapshot),
      SyncView(sync: snapshot.sync),
      SettingsView(
        settings: snapshot.settings,
        onPreviewDiagnostics: onPreviewDiagnostics,
        onExportDiagnostics: onExportDiagnostics,
        onSaveSettingsDraft: onSaveSettingsDraft,
      ),
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
  const _ManagerLoadFailure({required this.error, required this.onRetry});

  final Object? error;
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
                const SizedBox(height: 6),
                Text(
                  _bridgeFailureMessage(
                    error,
                    ManagerBridgeOperation.loadSnapshot,
                  ),
                ),
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
        ManagerStatusBadge(
          icon: Icons.shield_outlined,
          label: snapshot.sync.state.code,
          tone: ManagerBadgeTone.warning,
        ),
      ],
    );
  }
}

String _bridgeFailureMessage(Object? error, ManagerBridgeOperation operation) {
  return describeManagerBridgeFailure(error, operation).userMessage;
}
