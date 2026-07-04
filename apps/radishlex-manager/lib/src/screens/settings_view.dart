import 'package:flutter/material.dart';

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
                }),
                secondary: const Icon(Icons.verified_outlined),
                title: const Text('记录目标部署验证草案'),
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
    );
  }

  void _loadDraft(ManagerSettingsDraft draft) {
    serverEndpointController.text = draft.serverEndpoint;
    retainSyncConfig = draft.retainSyncConfig;
    privacyMode = draft.privacyMode;
    diagnosticsExport = draft.diagnosticsExport;
    deploymentEvidenceRecorded = draft.deploymentEvidenceRecorded;
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
      left.deploymentEvidenceRecorded == right.deploymentEvidenceRecorded;
}

class DiagnosticsReportDialog extends StatelessWidget {
  const DiagnosticsReportDialog({super.key, required this.report});

  final ManagerDiagnosticsReport report;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('诊断摘要预览'),
      content: SizedBox(
        width: 560,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxHeight: 420),
          child: SingleChildScrollView(
            child: SelectableText(
              report.toRedactedText(),
              key: const Key('diagnostics-report-text'),
            ),
          ),
        ),
      ),
      actions: [
        FilledButton.icon(
          onPressed: () => Navigator.of(context).pop(),
          icon: const Icon(Icons.check),
          label: const Text('关闭'),
        ),
      ],
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
