import 'manager_snapshot.dart';
import 'manager_sync_models.dart';

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
