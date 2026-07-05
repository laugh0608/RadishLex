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
  final syncGateAudit = managerSyncGateAuditForDraft(
    draft: snapshot.settings.draft,
    device: snapshot.sync.device,
  );

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
            'settings.access_token',
            managerSyncAccessTokenStatus(snapshot.settings.draft),
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
            managerDeploymentEvidenceLabel(snapshot.settings.draft),
            'configuration',
          ),
          _diagnosticsItem(
            'settings.deployment_evidence_source',
            snapshot.settings.draft.hasDeploymentEvidence
                ? snapshot.settings.draft.deploymentEvidenceSource
                : 'not_recorded',
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
            'sync.state_label',
            syncGateAudit.stateLabel,
            'gate',
          ),
          _diagnosticsItem(
            'sync.state_source',
            syncGateAudit.stateSource,
            'gate',
          ),
          _diagnosticsItem(
            'sync.entry_state',
            syncGateAudit.entryGate.entryState.code,
            'gate',
          ),
          _diagnosticsItem(
            'sync.entry_blocker',
            syncGateAudit.entryGate.entryBlocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.local_evidence_source',
            syncGateAudit.entryGate.localEvidenceSource,
            'gate',
          ),
          _diagnosticsItem(
            'sync.production_blockers',
            syncGateAudit.entryGate.productionBlockerSummary,
            'gate',
          ),
          _diagnosticsItem(
            'sync.user_sync_enabled',
            syncGateAudit.entryGate.userSyncEnabled.toString(),
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_status',
            syncGateAudit.entryGate.recovery.status.code,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_blocker',
            syncGateAudit.entryGate.recovery.blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_save_confirmation',
            syncGateAudit.entryGate.recovery.saveConfirmationRequirement,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_record_status',
            syncGateAudit.entryGate.recovery.recoveryRecordStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_record_blocker',
            syncGateAudit.entryGate.recovery.recoveryRecordBlocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_first_upload_gate',
            syncGateAudit.entryGate.recovery.firstUploadGate,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_readiness_blockers',
            syncGateAudit.entryGate.recovery.readinessBlockerSummary,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_setup_status',
            syncGateAudit.entryGate.recovery.setupReadiness.status,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_setup_blocker',
            syncGateAudit.entryGate.recovery.setupReadiness.blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_setup_action_status',
            syncGateAudit.entryGate.recovery.setupReadiness.entryActionStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_setup_prerequisites',
            syncGateAudit.entryGate.recovery.setupReadiness.prerequisiteSummary,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_setup_error_codes',
            syncGateAudit.entryGate.recovery.setupReadiness.errorCodeSummary,
            'error_code',
          ),
          _diagnosticsItem(
            'sync.recovery_restore_status',
            syncGateAudit.entryGate.recovery.restoreReadiness.status,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_restore_blocker',
            syncGateAudit.entryGate.recovery.restoreReadiness.blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_restore_code_input',
            syncGateAudit.entryGate.recovery.restoreReadiness.codeInputStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.recovery_restore_error_codes',
            syncGateAudit.entryGate.recovery.restoreReadiness.errorCodeSummary,
            'error_code',
          ),
          _diagnosticsItem(
            'sync.device_authorization_status',
            syncGateAudit.entryGate.deviceAuthorization.status.code,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_authorization_blocker',
            syncGateAudit.entryGate.deviceAuthorization.blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.join_request_status',
            syncGateAudit.entryGate.deviceAuthorization.joinRequestStatus.code,
            'gate',
          ),
          _diagnosticsItem(
            'sync.authorization_package_status',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .authorizationPackageStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.authorization_package_blocker',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .authorizationPackageBlocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.authorization_package_preconditions',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .authorizationPackagePreconditions,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_revocation_status',
            syncGateAudit.entryGate.deviceAuthorization.revokeDeviceStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.lost_device_risk',
            syncGateAudit.entryGate.deviceAuthorization.lostDeviceRiskNotice,
            'gate',
          ),
          _diagnosticsItem(
            'sync.key_epoch_status',
            syncGateAudit.entryGate.deviceAuthorization.keyEpochStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_authorization_readiness_blockers',
            syncGateAudit.entryGate.deviceAuthorization.readinessBlockerSummary,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_join_status',
            syncGateAudit.entryGate.deviceAuthorization.joinReadiness.status,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_join_blocker',
            syncGateAudit.entryGate.deviceAuthorization.joinReadiness.blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_join_action_status',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .joinReadiness
                .entryActionStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_join_short_code',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .joinReadiness
                .shortCodeVerificationStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_join_error_codes',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .joinReadiness
                .errorCodeSummary,
            'error_code',
          ),
          _diagnosticsItem(
            'sync.device_revocation_flow_status',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .status,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_revocation_blocker',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .blocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_revocation_active_requirement',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .activeDeviceRequirement,
            'gate',
          ),
          _diagnosticsItem(
            'sync.device_revocation_error_codes',
            syncGateAudit
                .entryGate
                .deviceAuthorization
                .revocationReadiness
                .errorCodeSummary,
            'error_code',
          ),
          _diagnosticsItem(
            'sync.connection_status',
            syncGateAudit.entryGate.connectionHealth.status.code,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_blocker',
            syncGateAudit.entryGate.connectionHealth.connectionBlocker,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_probe_source',
            syncGateAudit.entryGate.connectionHealth.probeSource,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_probe_recorded_at',
            syncGateAudit.entryGate.connectionHealth.probeRecordedAt,
            'timestamp',
          ),
          _diagnosticsItem(
            'sync.endpoint_status',
            syncGateAudit.entryGate.connectionHealth.endpointStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.access_token_status',
            syncGateAudit.entryGate.connectionHealth.accessTokenStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.transport_mode',
            syncGateAudit.entryGate.connectionHealth.transportMode,
            'gate',
          ),
          _diagnosticsItem(
            'sync.server_state_status',
            syncGateAudit.entryGate.connectionHealth.serverStateStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_auth_status',
            syncGateAudit.entryGate.connectionHealth.authStatus,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_http_status',
            syncGateAudit.entryGate.connectionHealth.httpStatus.toString(),
            'status_code',
          ),
          _diagnosticsItem(
            'sync.connection_http_status_class',
            syncGateAudit.entryGate.connectionHealth.httpStatusClass,
            'gate',
          ),
          _diagnosticsItem(
            'sync.connection_local_insecure_tls',
            syncGateAudit.entryGate.connectionHealth.localInsecureTls,
            'gate',
          ),
          _diagnosticsItem(
            'sync.last_remote_error_code',
            syncGateAudit.entryGate.connectionHealth.lastRemoteErrorCode,
            'error_code',
          ),
          _diagnosticsItem(
            'sync.action_stop_line',
            syncGateAudit.actionStopLine,
            'gate',
          ),
          _diagnosticsItem(
            'sync.deployment_evidence',
            managerDeploymentEvidenceLabel(snapshot.settings.draft),
            'gate',
          ),
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
          _diagnosticsItem(
            'device.production_gate_label',
            syncGateAudit.deviceGateLabel,
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
