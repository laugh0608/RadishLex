import 'dart:convert';

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
  late bool accessTokenConfigured;
  late String deploymentEvidenceSource;
  late ManagerSyncConnectionProbeRecord syncConnectionProbeRecord;
  late final TextEditingController connectionSummaryController;
  String connectionSummaryError = '';

  @override
  void initState() {
    super.initState();
    serverEndpointController = TextEditingController();
    connectionSummaryController = TextEditingController();
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
    connectionSummaryController.dispose();
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
                key: const Key('settings-access-token-configured'),
                value: accessTokenConfigured,
                onChanged: (value) => setState(() {
                  accessTokenConfigured = value ?? false;
                }),
                secondary: const Icon(Icons.password_outlined),
                title: const Text('access token 已配置'),
                subtitle: const Text('仅记录存在性，不保存 token 文本。'),
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
        _ConnectionHealthSummaryImportSection(
          record: syncConnectionProbeRecord,
          controller: connectionSummaryController,
          error: connectionSummaryError,
          onImport: _importConnectionSummary,
          onClear: _clearConnectionSummary,
        ),
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
      accessTokenConfigured: accessTokenConfigured,
      deploymentEvidenceSource: deploymentEvidenceSource,
      syncConnectionProbeRecord: syncConnectionProbeRecord,
    );
  }

  void _loadDraft(ManagerSettingsDraft draft) {
    serverEndpointController.text = draft.serverEndpoint;
    retainSyncConfig = draft.retainSyncConfig;
    privacyMode = draft.privacyMode;
    diagnosticsExport = draft.diagnosticsExport;
    deploymentEvidenceRecorded = draft.deploymentEvidenceRecorded;
    accessTokenConfigured = draft.accessTokenConfigured;
    deploymentEvidenceSource = draft.deploymentEvidenceSource;
    syncConnectionProbeRecord = draft.syncConnectionProbeRecord;
    connectionSummaryError = '';
    connectionSummaryController.clear();
  }

  void _importConnectionSummary() {
    try {
      final decoded = jsonDecode(connectionSummaryController.text);
      if (decoded is! Map) {
        throw const FormatException('summary root must be an object');
      }

      final json = <String, Object?>{};
      for (final entry in decoded.entries) {
        final key = entry.key;
        if (key is! String) {
          throw const FormatException('summary keys must be strings');
        }
        json[key] = entry.value;
      }

      final summary = SyncConnectionProbeSummary.fromJson(json);
      setState(() {
        syncConnectionProbeRecord = managerSyncConnectionProbeRecordFromSummary(
          summary,
          source: managerSyncConnectionProbeSourceForSummary(summary),
          recordedAt: DateTime.now().toUtc().toIso8601String(),
        );
        connectionSummaryError = '';
        connectionSummaryController.clear();
      });
    } on FormatException {
      setState(() {
        connectionSummaryError = 'probe_summary_json_invalid';
      });
    } on Object {
      setState(() {
        connectionSummaryError = 'probe_summary_import_failed';
      });
    }
  }

  void _clearConnectionSummary() {
    setState(() {
      syncConnectionProbeRecord =
          const ManagerSyncConnectionProbeRecord.empty();
      connectionSummaryError = '';
      connectionSummaryController.clear();
    });
  }
}

class _ConnectionHealthSummaryImportSection extends StatelessWidget {
  const _ConnectionHealthSummaryImportSection({
    required this.record,
    required this.controller,
    required this.error,
    required this.onImport,
    required this.onClear,
  });

  final ManagerSyncConnectionProbeRecord record;
  final TextEditingController controller;
  final String error;
  final VoidCallback onImport;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    final hasRecord = record.isRecorded;
    return ManagerSection(
      key: const Key('settings-connection-health-summary-section'),
      title: '连接健康摘要回填',
      trailing: ManagerStatusBadge(
        icon: hasRecord
            ? Icons.cloud_done_outlined
            : Icons.cloud_upload_outlined,
        label: hasRecord ? record.connectionStatus : 'not_recorded',
        tone: hasRecord ? ManagerBadgeTone.neutral : ManagerBadgeTone.warning,
      ),
      child: Column(
        children: [
          ManagerKeyValueRow(
            label: 'source',
            value: hasRecord ? record.source : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'recorded at',
            value: hasRecord ? record.recordedAt : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'connection',
            value: hasRecord ? record.connectionStatus : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'server state',
            value: hasRecord ? record.serverStateStatus : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'auth',
            value: hasRecord ? record.authStatus : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'http',
            value: hasRecord
                ? '${record.httpStatus} ${record.httpStatusClass}'
                : 'not_recorded',
          ),
          ManagerKeyValueRow(
            label: 'last remote error',
            value: hasRecord ? record.lastRemoteErrorCode : 'not_recorded',
          ),
          const SizedBox(height: 12),
          TextField(
            key: const Key('settings-connection-health-summary-json'),
            controller: controller,
            minLines: 3,
            maxLines: 6,
            decoration: InputDecoration(
              prefixIcon: const Icon(Icons.data_object_outlined),
              labelText: 'sync_connection_health.v1 JSON',
              errorText: error.isEmpty ? null : error,
            ),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton.icon(
                  key: const Key('settings-clear-connection-summary'),
                  onPressed: hasRecord ? onClear : null,
                  icon: const Icon(Icons.backspace_outlined),
                  label: const Text('清除摘要'),
                ),
                FilledButton.icon(
                  key: const Key('settings-import-connection-summary'),
                  onPressed: onImport,
                  icon: const Icon(Icons.input_outlined),
                  label: const Text('导入摘要'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
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
    final connection = audit.entryGate.connectionHealth;
    final tone = audit.entryGate.userSyncEnabled
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
          ManagerKeyValueRow(
            label: 'access token',
            value: managerSyncAccessTokenStatus(draft),
          ),
          ManagerKeyValueRow(label: 'state source', value: audit.stateSource),
          ManagerKeyValueRow(
            label: 'entry state',
            value: audit.entryGate.entryState.code,
          ),
          ManagerKeyValueRow(
            label: 'entry blocker',
            value: audit.entryGate.entryBlocker,
          ),
          ManagerKeyValueRow(
            label: 'local evidence',
            value: audit.entryGate.localEvidenceSource,
          ),
          ManagerKeyValueRow(
            label: 'production blockers',
            value: audit.entryGate.productionBlockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'user sync enabled',
            value: audit.entryGate.userSyncEnabled.toString(),
          ),
          ManagerKeyValueRow(
            label: 'recovery status',
            value: audit.entryGate.recovery.status.code,
          ),
          ManagerKeyValueRow(
            label: 'recovery blocker',
            value: audit.entryGate.recovery.blocker,
          ),
          ManagerKeyValueRow(
            label: 'recovery save confirmation',
            value: audit.entryGate.recovery.saveConfirmationRequirement,
          ),
          ManagerKeyValueRow(
            label: 'recovery record',
            value: audit.entryGate.recovery.recoveryRecordStatus,
          ),
          ManagerKeyValueRow(
            label: 'recovery record blocker',
            value: audit.entryGate.recovery.recoveryRecordBlocker,
          ),
          ManagerKeyValueRow(
            label: 'recovery first upload',
            value: audit.entryGate.recovery.firstUploadGate,
          ),
          ManagerKeyValueRow(
            label: 'recovery blockers',
            value: audit.entryGate.recovery.readinessBlockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'authorization status',
            value: audit.entryGate.deviceAuthorization.status.code,
          ),
          ManagerKeyValueRow(
            label: 'authorization blocker',
            value: audit.entryGate.deviceAuthorization.blocker,
          ),
          ManagerKeyValueRow(
            label: 'join request',
            value: audit.entryGate.deviceAuthorization.joinRequestStatus.code,
          ),
          ManagerKeyValueRow(
            label: 'authorization package',
            value:
                audit.entryGate.deviceAuthorization.authorizationPackageStatus,
          ),
          ManagerKeyValueRow(
            label: 'package blocker',
            value:
                audit.entryGate.deviceAuthorization.authorizationPackageBlocker,
          ),
          ManagerKeyValueRow(
            label: 'package preconditions',
            value: audit
                .entryGate
                .deviceAuthorization
                .authorizationPackagePreconditions,
          ),
          ManagerKeyValueRow(
            label: 'device revocation',
            value: audit.entryGate.deviceAuthorization.revokeDeviceStatus,
          ),
          ManagerKeyValueRow(
            label: 'lost device risk',
            value: audit.entryGate.deviceAuthorization.lostDeviceRiskNotice,
          ),
          ManagerKeyValueRow(
            label: 'key epoch',
            value: audit.entryGate.deviceAuthorization.keyEpochStatus,
          ),
          ManagerKeyValueRow(
            label: 'authorization blockers',
            value: audit.entryGate.deviceAuthorization.readinessBlockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'connection status',
            value: connection.status.code,
          ),
          ManagerKeyValueRow(
            label: 'connection source',
            value: connection.probeSource,
          ),
          ManagerKeyValueRow(
            label: 'connection recorded',
            value: connection.probeRecordedAt,
          ),
          ManagerKeyValueRow(
            label: 'connection blocker',
            value: connection.connectionBlocker,
          ),
          ManagerKeyValueRow(
            label: 'transport mode',
            value: connection.transportMode,
          ),
          ManagerKeyValueRow(
            label: 'server state',
            value: connection.serverStateStatus,
          ),
          ManagerKeyValueRow(
            label: 'auth status',
            value: connection.authStatus,
          ),
          ManagerKeyValueRow(
            label: 'http status',
            value: '${connection.httpStatus} ${connection.httpStatusClass}',
          ),
          ManagerKeyValueRow(
            label: 'last remote error',
            value: connection.lastRemoteErrorCode,
          ),
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
      left.accessTokenConfigured == right.accessTokenConfigured &&
      left.deploymentEvidenceSource == right.deploymentEvidenceSource &&
      left.syncConnectionProbeRecord == right.syncConnectionProbeRecord;
}

String _deploymentEvidenceSourceOrDefault(String source) {
  return isValidManagerDeploymentEvidenceSource(source)
      ? source
      : managerDeploymentEvidenceLocalSmoke;
}
