import 'package:flutter/material.dart';

import '../bridge/manager_bridge.dart';
import '../models/manager_models.dart';
import 'dictionary_view.dart';
import 'learning_view.dart';
import 'manager/manager_home_actions.dart';
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
        final actions = ManagerHomeActions(
          context: context,
          bridge: widget.bridge,
          currentSnapshot: () => data,
          onSnapshotChanged: _setSnapshot,
          reloadSnapshot: _reloadSnapshot,
          showMessage: _showBridgeMessage,
        );
        return _ManagerShell(
          snapshot: data,
          selectedIndex: selectedIndex,
          onSelectPage: _selectPage,
          onRefresh: _reloadSnapshot,
          onDeleteTerm: actions.deleteTerm,
          onRestoreTerm: actions.restoreTerm,
          onImportDictionary: actions.importDictionary,
          onExportDictionary: actions.exportDictionary,
          onPreviewDiagnostics: actions.previewDiagnostics,
          onExportDiagnostics: actions.exportDiagnostics,
          onSaveSettingsDraft: actions.saveSettingsDraft,
          onStartSyncQualification: widget.bridge.startSyncQualification,
          onImportSyncReadinessSummary: (readinessBridgeSnapshot) =>
              _setSnapshot(
                managerSnapshotWithSyncReadiness(data, readinessBridgeSnapshot),
              ),
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

  void _setSnapshot(ManagerSnapshot snapshot) {
    setState(() {
      snapshotFuture = Future.value(snapshot);
    });
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
    required this.onRefresh,
    required this.onDeleteTerm,
    required this.onRestoreTerm,
    required this.onImportDictionary,
    required this.onExportDictionary,
    required this.onPreviewDiagnostics,
    required this.onExportDiagnostics,
    required this.onSaveSettingsDraft,
    required this.onStartSyncQualification,
    required this.onImportSyncReadinessSummary,
  });

  final ManagerSnapshot snapshot;
  final int selectedIndex;
  final ValueChanged<int> onSelectPage;
  final VoidCallback onRefresh;
  final ValueChanged<UserTerm> onDeleteTerm;
  final void Function(UserTermKey term, String state) onRestoreTerm;
  final VoidCallback onImportDictionary;
  final VoidCallback onExportDictionary;
  final VoidCallback onPreviewDiagnostics;
  final VoidCallback onExportDiagnostics;
  final ValueChanged<ManagerSettingsDraft> onSaveSettingsDraft;
  final ManagerSyncQualificationRun Function(
    ManagerSyncQualificationRequest request,
  )
  onStartSyncQualification;
  final ValueChanged<ManagerSyncReadinessBridgeSnapshot>
  onImportSyncReadinessSummary;

  @override
  Widget build(BuildContext context) {
    final pages = [
      DictionaryView(
        snapshot: snapshot,
        onDeleteTerm: onDeleteTerm,
        onRestoreTerm: onRestoreTerm,
        onImportDictionary: onImportDictionary,
        onExportDictionary: onExportDictionary,
      ),
      LearningView(snapshot: snapshot),
      SyncView(
        sync: snapshot.sync,
        settingsDraft: snapshot.settings.draft,
        onStartQualification: onStartSyncQualification,
      ),
      SettingsView(
        settings: snapshot.settings,
        sync: snapshot.sync,
        onPreviewDiagnostics: onPreviewDiagnostics,
        onExportDiagnostics: onExportDiagnostics,
        onSaveSettingsDraft: onSaveSettingsDraft,
        onImportSyncReadinessSummary: onImportSyncReadinessSummary,
      ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final useRail = constraints.maxWidth >= 820;
        final content = _ManagerPageFrame(
          title: _destinations[selectedIndex].label,
          snapshot: snapshot,
          onRefresh: onRefresh,
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
                  managerBridgeFailureMessage(
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
    required this.onRefresh,
    required this.child,
  });

  final String title;
  final ManagerSnapshot snapshot;
  final VoidCallback onRefresh;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 18, 24, 12),
            child: _Header(
              title: title,
              snapshot: snapshot,
              onRefresh: onRefresh,
            ),
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
  const _Header({
    required this.title,
    required this.snapshot,
    required this.onRefresh,
  });

  final String title;
  final ManagerSnapshot snapshot;
  final VoidCallback onRefresh;

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
        IconButton(
          key: const Key('manager-refresh-button'),
          onPressed: onRefresh,
          tooltip: '刷新管理数据',
          icon: const Icon(Icons.refresh),
        ),
        const SizedBox(width: 8),
        ManagerStatusBadge(
          icon: Icons.shield_outlined,
          label: snapshot.sync.state.code,
          tone: ManagerBadgeTone.warning,
        ),
      ],
    );
  }
}
