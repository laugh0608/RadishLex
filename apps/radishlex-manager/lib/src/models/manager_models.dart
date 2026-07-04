enum SyncUiState {
  localOnly,
  preflightReady,
  serverConfigured,
  backendUnavailable,
  deploymentUnverified,
  syncDisabledByPolicy,
  readyForUserSync,
}

extension SyncUiStateLabel on SyncUiState {
  String get code {
    switch (this) {
      case SyncUiState.localOnly:
        return 'local_only';
      case SyncUiState.preflightReady:
        return 'preflight_ready';
      case SyncUiState.serverConfigured:
        return 'server_configured';
      case SyncUiState.backendUnavailable:
        return 'backend_unavailable';
      case SyncUiState.deploymentUnverified:
        return 'deployment_unverified';
      case SyncUiState.syncDisabledByPolicy:
        return 'sync_disabled_by_policy';
      case SyncUiState.readyForUserSync:
        return 'ready_for_user_sync';
    }
  }

  bool get canEnableUserSync => this == SyncUiState.readyForUserSync;
}

class ManagerSnapshot {
  const ManagerSnapshot({
    required this.generatedAt,
    required this.dictionaryTerms,
    required this.deletedTerms,
    required this.importBatches,
    required this.learningSummary,
    required this.explanations,
    required this.sync,
    required this.settings,
  });

  final String generatedAt;
  final List<UserTerm> dictionaryTerms;
  final List<DeletedTerm> deletedTerms;
  final List<DictionaryImportBatchSummary> importBatches;
  final LearningSummary learningSummary;
  final List<RankerExplanation> explanations;
  final SyncPreflightSummary sync;
  final ManagerSettings settings;

  ManagerSnapshot copyWith({
    String? generatedAt,
    List<UserTerm>? dictionaryTerms,
    List<DeletedTerm>? deletedTerms,
    List<DictionaryImportBatchSummary>? importBatches,
    LearningSummary? learningSummary,
    List<RankerExplanation>? explanations,
    SyncPreflightSummary? sync,
    ManagerSettings? settings,
  }) {
    return ManagerSnapshot(
      generatedAt: generatedAt ?? this.generatedAt,
      dictionaryTerms: dictionaryTerms ?? this.dictionaryTerms,
      deletedTerms: deletedTerms ?? this.deletedTerms,
      importBatches: importBatches ?? this.importBatches,
      learningSummary: learningSummary ?? this.learningSummary,
      explanations: explanations ?? this.explanations,
      sync: sync ?? this.sync,
      settings: settings ?? this.settings,
    );
  }
}

class UserTerm {
  const UserTerm({
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.weight,
    required this.source,
    required this.lastUsed,
  });

  final String inputCode;
  final String text;
  final String reading;
  final double weight;
  final String source;
  final String lastUsed;

  UserTermKey get key =>
      UserTermKey(inputCode: inputCode, text: text, reading: reading);
}

class UserTermKey {
  const UserTermKey({
    required this.inputCode,
    required this.text,
    required this.reading,
  });

  final String inputCode;
  final String text;
  final String reading;

  @override
  bool operator ==(Object other) {
    return other is UserTermKey &&
        other.inputCode == inputCode &&
        other.text == text &&
        other.reading == reading;
  }

  @override
  int get hashCode => Object.hash(inputCode, text, reading);
}

class DeletedTerm {
  const DeletedTerm({
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.deletedAt,
  });

  final String inputCode;
  final String text;
  final String reading;
  final String deletedAt;
}

class DictionaryImportBatchSummary {
  const DictionaryImportBatchSummary({
    required this.id,
    required this.sourceName,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.createdAt,
    required this.notes,
  });

  final int id;
  final String sourceName;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final String createdAt;
  final String notes;
}

class LearningSummary {
  const LearningSummary({
    required this.userTerms,
    required this.deletedTerms,
    required this.selectionEvents,
    required this.suppressedTerms,
    required this.lastUpdated,
  });

  final int userTerms;
  final int deletedTerms;
  final int selectionEvents;
  final int suppressedTerms;
  final String lastUpdated;

  LearningSummary copyWith({
    int? userTerms,
    int? deletedTerms,
    int? selectionEvents,
    int? suppressedTerms,
    String? lastUpdated,
  }) {
    return LearningSummary(
      userTerms: userTerms ?? this.userTerms,
      deletedTerms: deletedTerms ?? this.deletedTerms,
      selectionEvents: selectionEvents ?? this.selectionEvents,
      suppressedTerms: suppressedTerms ?? this.suppressedTerms,
      lastUpdated: lastUpdated ?? this.lastUpdated,
    );
  }
}

class RankerExplanation {
  const RankerExplanation({
    required this.inputCode,
    required this.candidate,
    required this.score,
    required this.signals,
  });

  final String inputCode;
  final String candidate;
  final double score;
  final List<String> signals;
}

class SyncPreflightSummary {
  const SyncPreflightSummary({
    required this.state,
    required this.serverEndpoint,
    required this.reason,
    required this.syncableObjects,
    required this.localOnlyEvents,
    required this.lastUpload,
    required this.lastDownload,
    required this.categories,
    required this.device,
  });

  final SyncUiState state;
  final String serverEndpoint;
  final String reason;
  final int syncableObjects;
  final int localOnlyEvents;
  final String lastUpload;
  final String lastDownload;
  final List<SyncCategorySummary> categories;
  final DeviceSecuritySummary device;

  SyncPreflightSummary copyWith({
    SyncUiState? state,
    String? serverEndpoint,
    String? reason,
    int? syncableObjects,
    int? localOnlyEvents,
    String? lastUpload,
    String? lastDownload,
    List<SyncCategorySummary>? categories,
    DeviceSecuritySummary? device,
  }) {
    return SyncPreflightSummary(
      state: state ?? this.state,
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      reason: reason ?? this.reason,
      syncableObjects: syncableObjects ?? this.syncableObjects,
      localOnlyEvents: localOnlyEvents ?? this.localOnlyEvents,
      lastUpload: lastUpload ?? this.lastUpload,
      lastDownload: lastDownload ?? this.lastDownload,
      categories: categories ?? this.categories,
      device: device ?? this.device,
    );
  }
}

class SyncCategorySummary {
  const SyncCategorySummary({required this.name, required this.count});

  final String name;
  final int count;
}

class DeviceSecuritySummary {
  const DeviceSecuritySummary({
    required this.deviceId,
    required this.backendId,
    required this.capabilityStatus,
    required this.productionGate,
  });

  final String deviceId;
  final String backendId;
  final String capabilityStatus;
  final String productionGate;
}

class ManagerSettings {
  const ManagerSettings({
    required this.privacyMode,
    required this.diagnosticsExport,
    required this.syncConfigured,
    required this.draft,
    required this.runtimeDiagnostics,
  });

  final bool privacyMode;
  final bool diagnosticsExport;
  final bool syncConfigured;
  final ManagerSettingsDraft draft;
  final ManagerRuntimeDiagnostics runtimeDiagnostics;

  ManagerSettings copyWith({
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? syncConfigured,
    ManagerSettingsDraft? draft,
    ManagerRuntimeDiagnostics? runtimeDiagnostics,
  }) {
    return ManagerSettings(
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      syncConfigured: syncConfigured ?? this.syncConfigured,
      draft: draft ?? this.draft,
      runtimeDiagnostics: runtimeDiagnostics ?? this.runtimeDiagnostics,
    );
  }
}

class ManagerSettingsDraft {
  const ManagerSettingsDraft({
    required this.serverEndpoint,
    required this.retainSyncConfig,
    required this.privacyMode,
    required this.diagnosticsExport,
    required this.deploymentEvidenceRecorded,
  });

  const ManagerSettingsDraft.empty()
    : serverEndpoint = '',
      retainSyncConfig = false,
      privacyMode = false,
      diagnosticsExport = false,
      deploymentEvidenceRecorded = false;

  final String serverEndpoint;
  final bool retainSyncConfig;
  final bool privacyMode;
  final bool diagnosticsExport;
  final bool deploymentEvidenceRecorded;

  bool get hasServerEndpoint =>
      retainSyncConfig && serverEndpoint.trim().isNotEmpty;

  ManagerSettingsDraft copyWith({
    String? serverEndpoint,
    bool? retainSyncConfig,
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? deploymentEvidenceRecorded,
  }) {
    return ManagerSettingsDraft(
      serverEndpoint: serverEndpoint ?? this.serverEndpoint,
      retainSyncConfig: retainSyncConfig ?? this.retainSyncConfig,
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      deploymentEvidenceRecorded:
          deploymentEvidenceRecorded ?? this.deploymentEvidenceRecorded,
    );
  }

  ManagerSettingsDraft normalized() {
    return copyWith(serverEndpoint: serverEndpoint.trim());
  }
}

class ManagerRuntimeDiagnostics {
  const ManagerRuntimeDiagnostics({
    required this.bridgeMode,
    required this.userDb,
    required this.nativeLibrary,
    required this.settingsStore,
    required this.syncEndpoint,
    required this.lastErrorCode,
  });

  final String bridgeMode;
  final String userDb;
  final String nativeLibrary;
  final String settingsStore;
  final String syncEndpoint;
  final String lastErrorCode;
}

SyncUiState deriveManagerSyncUiState({
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  if (draft.privacyMode) {
    return SyncUiState.syncDisabledByPolicy;
  }
  if (!draft.hasServerEndpoint) {
    return SyncUiState.localOnly;
  }
  if (device.productionGate != 'ready') {
    return SyncUiState.backendUnavailable;
  }
  if (!draft.deploymentEvidenceRecorded) {
    return SyncUiState.deploymentUnverified;
  }
  return SyncUiState.preflightReady;
}

String managerSyncGateReason({
  required SyncUiState state,
  required ManagerSettingsDraft draft,
  required DeviceSecuritySummary device,
}) {
  switch (state) {
    case SyncUiState.localOnly:
      return '未保留自部署服务端草案；真实远端同步保持关闭';
    case SyncUiState.syncDisabledByPolicy:
      return '隐私模式已启用；真实远端同步保持关闭';
    case SyncUiState.backendUnavailable:
      return '平台私钥 backend 未解除生产门禁：${device.productionGate}';
    case SyncUiState.deploymentUnverified:
      return '目标部署运行证据未记录；真实远端同步保持关闭';
    case SyncUiState.preflightReady:
      return '本地预检通过；用户可用同步入口仍等待后续阶段开放';
    case SyncUiState.serverConfigured:
      return '已保留服务端草案；仍未进入生产可用同步';
    case SyncUiState.readyForUserSync:
      return '用户可用同步入口尚未在当前阶段开放';
  }
}

String managerSyncEndpointLabel(ManagerSettingsDraft draft) {
  return draft.hasServerEndpoint ? draft.serverEndpoint.trim() : '未配置';
}

class ManagerDiagnosticsReport {
  const ManagerDiagnosticsReport({
    required this.generatedAt,
    required this.format,
    required this.redactionPolicy,
    required this.sections,
  });

  final String generatedAt;
  final String format;
  final String redactionPolicy;
  final List<ManagerDiagnosticsSection> sections;

  int get itemCount =>
      sections.fold(0, (count, section) => count + section.items.length);

  String toRedactedText() {
    final buffer = StringBuffer()
      ..writeln('RadishLex Manager Diagnostics')
      ..writeln('format: $format')
      ..writeln('generated_at: $generatedAt')
      ..writeln('redaction_policy: $redactionPolicy');

    for (final section in sections) {
      buffer
        ..writeln()
        ..writeln('[${section.title}]');
      for (final item in section.items) {
        buffer.writeln('${item.key}: ${item.value}');
      }
    }

    return buffer.toString();
  }
}

class ManagerDiagnosticsSection {
  const ManagerDiagnosticsSection({required this.title, required this.items});

  final String title;
  final List<ManagerDiagnosticsItem> items;
}

class ManagerDiagnosticsItem {
  const ManagerDiagnosticsItem({
    required this.key,
    required this.value,
    required this.kind,
  });

  final String key;
  final String value;
  final String kind;
}

class ManagerDiagnosticsExportResult {
  const ManagerDiagnosticsExportResult({
    required this.filePath,
    required this.format,
    required this.lineCount,
    required this.itemCount,
    required this.redactionPolicy,
  });

  final String filePath;
  final String format;
  final int lineCount;
  final int itemCount;
  final String redactionPolicy;
}

class DictionaryImportPreview {
  const DictionaryImportPreview({
    required this.filePath,
    required this.format,
    required this.recordCount,
    required this.syncClass,
  });

  final String filePath;
  final String format;
  final int recordCount;
  final String syncClass;
}

class DictionaryImportResult {
  const DictionaryImportResult({
    required this.filePath,
    required this.sourceName,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.dryRun,
  });

  final String filePath;
  final String sourceName;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final bool dryRun;
}

class DictionaryExportResult {
  const DictionaryExportResult({
    required this.filePath,
    required this.exportedTerms,
    required this.format,
    required this.syncClass,
  });

  final String filePath;
  final int exportedTerms;
  final String format;
  final String syncClass;
}

ManagerDiagnosticsReport createManagerDiagnosticsReport(
  ManagerSnapshot snapshot,
) {
  final diagnostics = snapshot.settings.runtimeDiagnostics;
  final latestImportBatch = snapshot.importBatches.isEmpty
      ? null
      : snapshot.importBatches.first;

  return ManagerDiagnosticsReport(
    generatedAt: snapshot.generatedAt,
    format: 'manager.diagnostics.v1',
    redactionPolicy: 'summary_only_no_terms_paths_tokens_or_payload_bytes',
    sections: [
      ManagerDiagnosticsSection(
        title: 'runtime',
        items: [
          _diagnosticsItem(
            'runtime.bridge_mode',
            diagnostics.bridgeMode,
            'configuration',
          ),
          _diagnosticsItem(
            'runtime.userdb',
            diagnostics.userDb,
            'configuration',
          ),
          _diagnosticsItem(
            'runtime.native_library',
            diagnostics.nativeLibrary,
            'configuration',
          ),
          _diagnosticsItem(
            'runtime.settings_store',
            diagnostics.settingsStore,
            'configuration',
          ),
          _diagnosticsItem(
            'runtime.sync_endpoint',
            diagnostics.syncEndpoint,
            'configuration',
          ),
          _diagnosticsItem(
            'runtime.last_error_code',
            diagnostics.lastErrorCode,
            'error_code',
          ),
        ],
      ),
      ManagerDiagnosticsSection(
        title: 'settings_draft',
        items: [
          _diagnosticsItem(
            'settings.retain_sync_config',
            snapshot.settings.draft.retainSyncConfig.toString(),
            'configuration',
          ),
          _diagnosticsItem(
            'settings.server_endpoint',
            snapshot.settings.draft.hasServerEndpoint
                ? 'configured'
                : 'not_configured',
            'configuration',
          ),
          _diagnosticsItem(
            'settings.privacy_mode',
            snapshot.settings.draft.privacyMode.toString(),
            'configuration',
          ),
          _diagnosticsItem(
            'settings.diagnostics_export',
            snapshot.settings.draft.diagnosticsExport.toString(),
            'configuration',
          ),
          _diagnosticsItem(
            'settings.deployment_evidence',
            snapshot.settings.draft.deploymentEvidenceRecorded.toString(),
            'configuration',
          ),
        ],
      ),
      ManagerDiagnosticsSection(
        title: 'local_data',
        items: [
          _diagnosticsItem(
            'local.user_terms',
            snapshot.learningSummary.userTerms.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'local.deleted_terms',
            snapshot.learningSummary.deletedTerms.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'local.selection_events',
            snapshot.learningSummary.selectionEvents.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'local.suppressed_terms',
            snapshot.learningSummary.suppressedTerms.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'local.import_batches',
            snapshot.importBatches.length.toString(),
            'aggregate_count',
          ),
        ],
      ),
      ManagerDiagnosticsSection(
        title: 'latest_import_batch',
        items: [
          _diagnosticsItem(
            'import_batch.present',
            (latestImportBatch != null).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.total_records',
            (latestImportBatch?.totalRecords ?? 0).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.imported_terms',
            (latestImportBatch?.importedTerms ?? 0).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.inserted_terms',
            (latestImportBatch?.insertedTerms ?? 0).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.updated_terms',
            (latestImportBatch?.updatedTerms ?? 0).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.skipped_deleted_terms',
            (latestImportBatch?.skippedDeletedTerms ?? 0).toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'import_batch.skipped_duplicate_terms',
            (latestImportBatch?.skippedDuplicateTerms ?? 0).toString(),
            'aggregate_count',
          ),
        ],
      ),
      ManagerDiagnosticsSection(
        title: 'sync_gate',
        items: [
          _diagnosticsItem('sync.state', snapshot.sync.state.code, 'gate'),
          _diagnosticsItem(
            'sync.syncable_objects',
            snapshot.sync.syncableObjects.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'sync.local_only_events',
            snapshot.sync.localOnlyEvents.toString(),
            'aggregate_count',
          ),
          _diagnosticsItem(
            'device.backend',
            snapshot.sync.device.backendId,
            'gate',
          ),
          _diagnosticsItem(
            'device.capability',
            snapshot.sync.device.capabilityStatus,
            'gate',
          ),
          _diagnosticsItem(
            'device.production_gate',
            snapshot.sync.device.productionGate,
            'gate',
          ),
        ],
      ),
      const ManagerDiagnosticsSection(
        title: 'redaction',
        items: [
          ManagerDiagnosticsItem(
            key: 'redaction.user_terms',
            value: 'omitted',
            kind: 'policy',
          ),
          ManagerDiagnosticsItem(
            key: 'redaction.file_paths',
            value: 'omitted',
            kind: 'policy',
          ),
          ManagerDiagnosticsItem(
            key: 'redaction.tokens',
            value: 'omitted',
            kind: 'policy',
          ),
          ManagerDiagnosticsItem(
            key: 'redaction.payload_bytes',
            value: 'omitted',
            kind: 'policy',
          ),
        ],
      ),
    ],
  );
}

ManagerDiagnosticsItem _diagnosticsItem(String key, String value, String kind) {
  return ManagerDiagnosticsItem(key: key, value: value, kind: kind);
}
