import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class SettingsView extends StatefulWidget {
  const SettingsView({
    super.key,
    required this.settings,
    required this.sync,
    required this.onPreviewDiagnostics,
    required this.onExportDiagnostics,
    required this.onSaveSettingsDraft,
  });

  final ManagerSettings settings;
  final SyncPreflightSummary sync;
  final VoidCallback onPreviewDiagnostics;
  final VoidCallback onExportDiagnostics;
  final ValueChanged<ManagerSettingsDraft> onSaveSettingsDraft;

  @override
  State<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  late final TextEditingController serverEndpointController;
  late bool retainSyncConfig;
  late bool privacyMode;
  late bool diagnosticsExport;
  late bool deploymentEvidenceRecorded;
  late String deploymentEvidenceSource;

  @override
  void initState() {
    super.initState();
    serverEndpointController = TextEditingController();
    _loadDraft(widget.settings.draft);
  }

  @override
  void didUpdateWidget(SettingsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_sameDraft(oldWidget.settings.draft, widget.settings.draft)) {
      _loadDraft(widget.settings.draft);
    }
  }

  @override
  void dispose() {
    serverEndpointController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = widget.settings;
    final diagnostics = settings.runtimeDiagnostics;
    final currentDraft = _currentDraft().normalized();

    return Column(
      children: [
        ManagerSection(
          title: '设置草案',
          child: Column(
            children: [
              TextField(
                key: const Key('settings-server-endpoint'),
                controller: serverEndpointController,
                decoration: const InputDecoration(
                  prefixIcon: Icon(Icons.dns_outlined),
                  labelText: '自部署服务端',
                ),
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 12),
              SwitchListTile(
                key: const Key('settings-privacy-mode'),
                value: privacyMode,
                onChanged: (value) => setState(() {
                  privacyMode = value;
                }),
                secondary: const Icon(Icons.privacy_tip_outlined),
                title: const Text('隐私模式'),
              ),
              SwitchListTile(
                key: const Key('settings-diagnostics-export'),
                value: diagnosticsExport,
                onChanged: (value) => setState(() {
                  diagnosticsExport = value;
                }),
                secondary: const Icon(Icons.bug_report_outlined),
                title: const Text('诊断摘要导出'),
              ),
              CheckboxListTile(
                key: const Key('settings-retain-sync-config'),
                value: retainSyncConfig,
                onChanged: (value) => setState(() {
                  retainSyncConfig = value ?? false;
                }),
                secondary: const Icon(Icons.cloud_done_outlined),
                title: const Text('保留同步配置草案'),
              ),
              CheckboxListTile(
                key: const Key('settings-deployment-evidence'),
                value: deploymentEvidenceRecorded,
                onChanged: (value) => setState(() {
                  deploymentEvidenceRecorded = value ?? false;
                  deploymentEvidenceSource = deploymentEvidenceRecorded
                      ? _deploymentEvidenceSourceOrDefault(
                          deploymentEvidenceSource,
                        )
                      : '';
                }),
                secondary: const Icon(Icons.verified_outlined),
                title: const Text('记录目标部署验证草案'),
              ),
              DropdownButtonFormField<String>(
                key: const Key('settings-deployment-evidence-source'),
                initialValue: deploymentEvidenceRecorded
                    ? _deploymentEvidenceSourceOrDefault(
                        deploymentEvidenceSource,
                      )
                    : null,
                decoration: const InputDecoration(
                  prefixIcon: Icon(Icons.fact_check_outlined),
                  labelText: '部署证据来源',
                ),
                items: managerDeploymentEvidenceSources
                    .map(
                      (source) => DropdownMenuItem(
                        value: source,
                        child: Text(
                          managerDeploymentEvidenceSourceLabel(source),
                        ),
                      ),
                    )
                    .toList(),
                onChanged: deploymentEvidenceRecorded
                    ? (source) => setState(() {
                        deploymentEvidenceSource = source ?? '';
                      })
                    : null,
              ),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  key: const Key('settings-save-button'),
                  onPressed: () => widget.onSaveSettingsDraft(_currentDraft()),
                  icon: const Icon(Icons.save_outlined),
                  label: const Text('保存草案'),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _SettingsSyncGatePreview(draft: currentDraft, sync: widget.sync),
        const SizedBox(height: 16),
        ManagerSection(
          title: '配置来源',
          trailing: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton.icon(
                key: const Key('diagnostics-preview-button'),
                onPressed: widget.onPreviewDiagnostics,
                icon: const Icon(Icons.visibility_outlined),
                label: const Text('预览'),
              ),
              FilledButton.icon(
                key: const Key('diagnostics-export-button'),
                onPressed: widget.onExportDiagnostics,
                icon: const Icon(Icons.ios_share_outlined),
                label: const Text('导出'),
              ),
            ],
          ),
          child: Column(
            children: [
              ManagerKeyValueRow(
                label: 'bridge',
                value: diagnostics.bridgeMode,
              ),
              ManagerKeyValueRow(label: 'userdb', value: diagnostics.userDb),
              ManagerKeyValueRow(
                label: 'native library',
                value: diagnostics.nativeLibrary,
              ),
              ManagerKeyValueRow(
                label: 'sync endpoint',
                value: diagnostics.syncEndpoint,
              ),
              ManagerKeyValueRow(
                label: 'last error',
                value: diagnostics.lastErrorCode,
              ),
            ],
          ),
        ),
      ],
    );
  }

  ManagerSettingsDraft _currentDraft() {
    return ManagerSettingsDraft(
      serverEndpoint: serverEndpointController.text,
      retainSyncConfig: retainSyncConfig,
      privacyMode: privacyMode,
      diagnosticsExport: diagnosticsExport,
      deploymentEvidenceRecorded: deploymentEvidenceRecorded,
      deploymentEvidenceSource: deploymentEvidenceSource,
    );
  }

  void _loadDraft(ManagerSettingsDraft draft) {
    serverEndpointController.text = draft.serverEndpoint;
    retainSyncConfig = draft.retainSyncConfig;
    privacyMode = draft.privacyMode;
    diagnosticsExport = draft.diagnosticsExport;
    deploymentEvidenceRecorded = draft.deploymentEvidenceRecorded;
    deploymentEvidenceSource = draft.deploymentEvidenceSource;
  }
}

class _SettingsSyncGatePreview extends StatelessWidget {
  const _SettingsSyncGatePreview({required this.draft, required this.sync});

  final ManagerSettingsDraft draft;
  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAuditForDraft(
      draft: draft,
      device: sync.device,
    );
    final tone = audit.state.canEnableUserSync
        ? ManagerBadgeTone.success
        : ManagerBadgeTone.warning;

    return ManagerSection(
      title: '同步门禁草案',
      trailing: ManagerStatusBadge(
        icon: Icons.rule_outlined,
        label: audit.state.code,
        tone: tone,
      ),
      child: Column(
        children: [
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              ManagerStatusBadge(
                icon: Icons.fact_check_outlined,
                label: audit.stateLabel,
                tone: tone,
              ),
              Chip(label: Text(managerDeploymentEvidenceLabel(draft))),
              Chip(label: Text(audit.deviceGateLabel)),
            ],
          ),
          const SizedBox(height: 12),
          ManagerKeyValueRow(
            label: 'server draft',
            value: managerSyncEndpointLabel(draft),
          ),
          ManagerKeyValueRow(label: 'state source', value: audit.stateSource),
          ManagerKeyValueRow(
            label: 'deployment evidence',
            value: managerDeploymentEvidenceLabel(draft),
          ),
          ManagerKeyValueRow(
            label: 'device backend',
            value: sync.device.backendId,
          ),
          ManagerKeyValueRow(
            label: 'device gate',
            value: sync.device.productionGate,
          ),
          ManagerKeyValueRow(label: 'stop line', value: audit.actionStopLine),
        ],
      ),
    );
  }
}

bool _sameDraft(ManagerSettingsDraft left, ManagerSettingsDraft right) {
  return left.serverEndpoint == right.serverEndpoint &&
      left.retainSyncConfig == right.retainSyncConfig &&
      left.privacyMode == right.privacyMode &&
      left.diagnosticsExport == right.diagnosticsExport &&
      left.deploymentEvidenceRecorded == right.deploymentEvidenceRecorded &&
      left.deploymentEvidenceSource == right.deploymentEvidenceSource;
}

String _deploymentEvidenceSourceOrDefault(String source) {
  return isValidManagerDeploymentEvidenceSource(source)
      ? source
      : managerDeploymentEvidenceLocalSmoke;
}

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
