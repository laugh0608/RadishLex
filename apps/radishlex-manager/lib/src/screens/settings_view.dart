import 'dart:convert';

import 'package:flutter/material.dart';

import '../bridge/ffi_manager_sync_readiness_mapper.dart';
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
    required this.onImportSyncReadinessSummary,
  });

  final ManagerSettings settings;
  final SyncPreflightSummary sync;
  final VoidCallback onPreviewDiagnostics;
  final VoidCallback onExportDiagnostics;
  final ValueChanged<ManagerSettingsDraft> onSaveSettingsDraft;
  final ValueChanged<ManagerSyncReadinessBridgeSnapshot>
  onImportSyncReadinessSummary;

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
  late ManagerSyncReadinessBridgeSnapshot syncReadinessBridgeSnapshot;
  late final TextEditingController readinessSummaryController;
  String readinessSummaryError = '';

  @override
  void initState() {
    super.initState();
    serverEndpointController = TextEditingController();
    connectionSummaryController = TextEditingController();
    readinessSummaryController = TextEditingController();
    syncReadinessBridgeSnapshot = widget.sync.readinessBridgeSnapshot;
    _loadDraft(widget.settings.draft);
  }

  @override
  void didUpdateWidget(SettingsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_sameDraft(oldWidget.settings.draft, widget.settings.draft)) {
      _loadDraft(widget.settings.draft);
    }
    if (oldWidget.sync.readinessBridgeSnapshot !=
        widget.sync.readinessBridgeSnapshot) {
      syncReadinessBridgeSnapshot = widget.sync.readinessBridgeSnapshot;
      readinessSummaryError = '';
      readinessSummaryController.clear();
    }
  }

  @override
  void dispose() {
    serverEndpointController.dispose();
    connectionSummaryController.dispose();
    readinessSummaryController.dispose();
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
        _SettingsSyncGatePreview(
          draft: currentDraft,
          sync: widget.sync,
          readinessBridgeSnapshot: syncReadinessBridgeSnapshot,
        ),
        const SizedBox(height: 16),
        _ConnectionHealthSummaryImportSection(
          record: syncConnectionProbeRecord,
          controller: connectionSummaryController,
          error: connectionSummaryError,
          onImport: _importConnectionSummary,
          onClear: _clearConnectionSummary,
        ),
        const SizedBox(height: 16),
        _SyncReadinessSummaryImportSection(
          draft: currentDraft,
          sync: widget.sync,
          readinessBridgeSnapshot: syncReadinessBridgeSnapshot,
          controller: readinessSummaryController,
          error: readinessSummaryError,
          onImport: _importSyncReadinessSummary,
          onClear: _clearSyncReadinessSummary,
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
      final json = _decodeSummaryObject(connectionSummaryController.text);
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

  void _importSyncReadinessSummary() {
    try {
      final json = _decodeSummaryObject(readinessSummaryController.text);
      final imported = importManagerSyncReadinessBridgeSummaryFromJson(json);
      if (!imported.accepted) {
        setState(() {
          readinessSummaryError = imported.errorCode;
        });
        return;
      }

      setState(() {
        syncReadinessBridgeSnapshot = imported.snapshot;
        readinessSummaryError = '';
        readinessSummaryController.clear();
      });
      widget.onImportSyncReadinessSummary(imported.snapshot);
    } on FormatException {
      setState(() {
        readinessSummaryError = 'readiness_summary_json_invalid';
      });
    } on Object {
      setState(() {
        readinessSummaryError = 'readiness_summary_import_failed';
      });
    }
  }

  void _clearSyncReadinessSummary() {
    setState(() {
      syncReadinessBridgeSnapshot = managerDefaultSyncReadinessBridgeSnapshot;
      readinessSummaryError = '';
      readinessSummaryController.clear();
    });
    widget.onImportSyncReadinessSummary(
      managerDefaultSyncReadinessBridgeSnapshot,
    );
  }
}

Map<String, Object?> _decodeSummaryObject(String text) {
  final decoded = jsonDecode(text);
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
  return json;
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

class _SyncReadinessSummaryImportSection extends StatelessWidget {
  const _SyncReadinessSummaryImportSection({
    required this.draft,
    required this.sync,
    required this.readinessBridgeSnapshot,
    required this.controller,
    required this.error,
    required this.onImport,
    required this.onClear,
  });

  final ManagerSettingsDraft draft;
  final SyncPreflightSummary sync;
  final ManagerSyncReadinessBridgeSnapshot readinessBridgeSnapshot;
  final TextEditingController controller;
  final String error;
  final VoidCallback onImport;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAuditForDraft(
      draft: draft,
      device: sync.device,
      readinessBridgeSnapshot: readinessBridgeSnapshot,
    );
    final hasImported =
        readinessBridgeSnapshot.source !=
        managerSyncReadinessBridgeSourceDefault;

    return ManagerSection(
      key: const Key('settings-sync-readiness-summary-section'),
      title: '同步 readiness 摘要导入',
      trailing: ManagerStatusBadge(
        icon: hasImported ? Icons.rule_folder_outlined : Icons.rule_outlined,
        label: readinessBridgeSnapshot.source,
        tone: hasImported ? ManagerBadgeTone.neutral : ManagerBadgeTone.warning,
      ),
      child: Column(
        children: [
          ManagerKeyValueRow(
            label: 'source',
            value: readinessBridgeSnapshot.source,
          ),
          ManagerKeyValueRow(
            label: 'import mode',
            value: hasImported
                ? 'readiness_summary_in_memory'
                : 'default_closed_readiness',
          ),
          ManagerKeyValueRow(
            label: 'clear target',
            value: managerSyncReadinessBridgeSourceDefault,
          ),
          ManagerKeyValueRow(
            label: 'blocked flows',
            value: audit.entryGate.readinessBlockedFlowSummary,
          ),
          ManagerKeyValueRow(
            label: 'issue codes',
            value: audit.entryGate.readinessIssueCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'next evidence',
            value: audit.entryGate.readinessNextRequiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction status',
            value: audit.entryGate.interactionEntryPlan.intentStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction blockers',
            value: audit.entryGate.interactionEntryPlan.blockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'command execution',
            value: audit
                .entryGate
                .interactionEntryPlan
                .actionCommandPreviewPlan
                .executionStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'command policy',
            value: audit
                .entryGate
                .interactionEntryPlan
                .actionCommandPreviewPlan
                .dataPolicySummary,
          ),
          ManagerKeyValueRow(
            label: 'command stop lines',
            value: audit
                .entryGate
                .interactionEntryPlan
                .actionCommandPreviewPlan
                .stopLineSummary,
          ),
          ManagerKeyValueRow(
            label: 'user sync enabled',
            value: audit.entryGate.userSyncEnabled.toString(),
          ),
          const SizedBox(height: 12),
          TextField(
            key: const Key('settings-sync-readiness-summary-json'),
            controller: controller,
            minLines: 3,
            maxLines: 6,
            decoration: InputDecoration(
              prefixIcon: const Icon(Icons.rule_folder_outlined),
              labelText: 'manager_sync_readiness.v1 JSON',
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
                  key: const Key('settings-clear-readiness-summary'),
                  onPressed: hasImported ? onClear : null,
                  icon: const Icon(Icons.backspace_outlined),
                  label: const Text('清除摘要'),
                ),
                FilledButton.icon(
                  key: const Key('settings-import-readiness-summary'),
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
  const _SettingsSyncGatePreview({
    required this.draft,
    required this.sync,
    required this.readinessBridgeSnapshot,
  });

  final ManagerSettingsDraft draft;
  final SyncPreflightSummary sync;
  final ManagerSyncReadinessBridgeSnapshot readinessBridgeSnapshot;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAuditForDraft(
      draft: draft,
      device: sync.device,
      readinessBridgeSnapshot: readinessBridgeSnapshot,
    );
    final connection = audit.entryGate.connectionHealth;
    final interactionPlan = audit.entryGate.interactionEntryPlan;
    final actionCommandPlan =
        audit.entryGate.interactionEntryPlan.actionCommandPreviewPlan;
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
            label: 'readiness blocked flows',
            value: audit.entryGate.readinessBlockedFlowSummary,
          ),
          ManagerKeyValueRow(
            label: 'readiness issue codes',
            value: audit.entryGate.readinessIssueCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'readiness next evidence',
            value: audit.entryGate.readinessNextRequiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'readiness source tags',
            value: audit.entryGate.readinessSourceTagSummary,
          ),
          ManagerKeyValueRow(
            label: 'readiness bridge source',
            value: audit.entryGate.readinessBridgeSource,
          ),
          ManagerKeyValueRow(
            label: 'readiness user sync blocked',
            value: audit.entryGate.readinessUserSyncBlocked.toString(),
          ),
          ManagerKeyValueRow(
            label: 'interaction actions',
            value: interactionPlan.actionIdSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction visibility',
            value: interactionPlan.visibilitySummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction status',
            value: interactionPlan.intentStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction blockers',
            value: interactionPlan.blockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction evidence',
            value: interactionPlan.requiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'interaction sources',
            value: interactionPlan.sourceTagSummary,
          ),
          ManagerKeyValueRow(
            label: 'command format',
            value: managerSyncActionCommandPreviewFormat,
          ),
          ManagerKeyValueRow(
            label: 'command actions',
            value: actionCommandPlan.actionIdSummary,
          ),
          ManagerKeyValueRow(
            label: 'command execution',
            value: actionCommandPlan.executionStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'command blockers',
            value: actionCommandPlan.blockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'command evidence',
            value: actionCommandPlan.requiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'command policy',
            value: actionCommandPlan.dataPolicySummary,
          ),
          ManagerKeyValueRow(
            label: 'command stop lines',
            value: actionCommandPlan.stopLineSummary,
          ),
          ManagerKeyValueRow(
            label: 'command request boundary',
            value: actionCommandPlan.requestBoundarySummary,
          ),
          ManagerKeyValueRow(
            label: 'command result boundary',
            value: actionCommandPlan.resultBoundarySummary,
          ),
          ManagerKeyValueRow(
            label: 'command errors',
            value: actionCommandPlan.errorCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'request status',
            value: actionCommandPlan.requestStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'request fields',
            value: actionCommandPlan.requestAllowedFieldSummary,
          ),
          ManagerKeyValueRow(
            label: 'request forbidden material',
            value: actionCommandPlan.requestForbiddenMaterialSummary,
          ),
          ManagerKeyValueRow(
            label: 'result status',
            value: actionCommandPlan.resultStatusSummary,
          ),
          ManagerKeyValueRow(
            label: 'result fields',
            value: actionCommandPlan.resultAllowedFieldSummary,
          ),
          ManagerKeyValueRow(
            label: 'result forbidden material',
            value: actionCommandPlan.resultForbiddenMaterialSummary,
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
            label: 'recovery setup flow',
            value: audit.entryGate.recovery.setupReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'recovery setup blocker',
            value: audit.entryGate.recovery.setupReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'recovery setup prerequisites',
            value: audit.entryGate.recovery.setupReadiness.prerequisiteSummary,
          ),
          ManagerKeyValueRow(
            label: 'recovery setup errors',
            value: audit.entryGate.recovery.setupReadiness.errorCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'recovery restore flow',
            value: audit.entryGate.recovery.restoreReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'recovery restore blocker',
            value: audit.entryGate.recovery.restoreReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'recovery restore code input',
            value: audit.entryGate.recovery.restoreReadiness.codeInputStatus,
          ),
          ManagerKeyValueRow(
            label: 'recovery restore errors',
            value: audit.entryGate.recovery.restoreReadiness.errorCodeSummary,
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
            label: 'device join flow',
            value: audit.entryGate.deviceAuthorization.joinReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'device join blocker',
            value: audit.entryGate.deviceAuthorization.joinReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'device join short code',
            value: audit
                .entryGate
                .deviceAuthorization
                .joinReadiness
                .shortCodeVerificationStatus,
          ),
          ManagerKeyValueRow(
            label: 'device join errors',
            value: audit
                .entryGate
                .deviceAuthorization
                .joinReadiness
                .errorCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'device revocation flow',
            value:
                audit.entryGate.deviceAuthorization.revocationReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'device revocation blocker',
            value:
                audit.entryGate.deviceAuthorization.revocationReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'device revocation active device',
            value: audit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .activeDeviceRequirement,
          ),
          ManagerKeyValueRow(
            label: 'device revocation errors',
            value: audit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .errorCodeSummary,
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
