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
    required this.runtimeDiagnostics,
  });

  final bool privacyMode;
  final bool diagnosticsExport;
  final bool syncConfigured;
  final ManagerRuntimeDiagnostics runtimeDiagnostics;

  ManagerSettings copyWith({
    bool? privacyMode,
    bool? diagnosticsExport,
    bool? syncConfigured,
    ManagerRuntimeDiagnostics? runtimeDiagnostics,
  }) {
    return ManagerSettings(
      privacyMode: privacyMode ?? this.privacyMode,
      diagnosticsExport: diagnosticsExport ?? this.diagnosticsExport,
      syncConfigured: syncConfigured ?? this.syncConfigured,
      runtimeDiagnostics: runtimeDiagnostics ?? this.runtimeDiagnostics,
    );
  }
}

class ManagerRuntimeDiagnostics {
  const ManagerRuntimeDiagnostics({
    required this.bridgeMode,
    required this.userDb,
    required this.nativeLibrary,
    required this.syncEndpoint,
    required this.lastErrorCode,
  });

  final String bridgeMode;
  final String userDb;
  final String nativeLibrary;
  final String syncEndpoint;
  final String lastErrorCode;
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
