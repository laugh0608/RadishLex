import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  const readyDevice = DeviceSecuritySummary(
    deviceId: 'device-ready-01',
    backendId: 'test-production-ready',
    capabilityStatus: 'ready_for_test',
    productionGate: 'ready',
  );

  const blockedDevice = DeviceSecuritySummary(
    deviceId: 'device-blocked-01',
    backendId: 'android-keystore-v1',
    capabilityStatus: 'unsupported_signature_algorithm',
    productionGate: 'blocked',
  );

  test('entry gate keeps privacy mode ahead of deployment evidence', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: true,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
    );

    final gate = deriveManagerSyncEntryGate(draft: draft, device: readyDevice);

    expect(gate.entryState, SyncEntryState.syncDisabledByPolicy);
    expect(gate.entryBlocker, 'sync_disabled_by_policy');
    expect(gate.localEvidenceSource, managerDeploymentEvidenceExternalTls);
    expect(gate.userSyncEnabled, isFalse);
    expect(gate.uiState, SyncUiState.syncDisabledByPolicy);
    expect(gate.productionBlockers, contains('sync_disabled_by_policy'));
  });

  test(
    'entry gate reports blocked platform backend before evidence checks',
    () {
      const draft = ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
      );

      final audit = managerSyncGateAuditForDraft(
        draft: draft,
        device: blockedDevice,
      );

      expect(audit.state, SyncUiState.backendUnavailable);
      expect(audit.entryGate.entryState, SyncEntryState.backendUnavailable);
      expect(audit.entryGate.entryBlocker, 'backend_unavailable');
      expect(
        audit.entryGate.productionBlockers,
        contains('platform_private_key_backend_blocked'),
      );
      expect(audit.entryGate.userSyncEnabled, isFalse);
    },
  );

  test('local smoke supports development preflight without user sync', () {
    const draft = ManagerSettingsDraft(
      serverEndpoint: 'https://sync.example.invalid',
      retainSyncConfig: true,
      privacyMode: false,
      diagnosticsExport: false,
      deploymentEvidenceRecorded: true,
      deploymentEvidenceSource: managerDeploymentEvidenceLocalSmoke,
    );

    final gate = deriveManagerSyncEntryGate(draft: draft, device: readyDevice);

    expect(gate.entryState, SyncEntryState.localSmokeReady);
    expect(gate.entryBlocker, 'release_deployment_evidence_required');
    expect(gate.localEvidenceSource, managerDeploymentEvidenceLocalSmoke);
    expect(gate.uiState, SyncUiState.preflightReady);
    expect(gate.userSyncEnabled, isFalse);
    expect(
      gate.productionBlockers,
      containsAll([
        'release_deployment_evidence_required',
        'recovery_code_flow_closed',
        'device_authorization_flow_closed',
        'user_sync_entry_closed_current_phase',
      ]),
    );
  });

  test(
    'external evidence remains blocked before recovery and authorization',
    () {
      const draft = ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: false,
        deploymentEvidenceRecorded: true,
        deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
      );

      final audit = managerSyncGateAuditForDraft(
        draft: draft,
        device: readyDevice,
      );

      expect(audit.state, SyncUiState.preflightReady);
      expect(audit.entryGate.entryState, SyncEntryState.blockedBeforeUserSync);
      expect(audit.entryGate.entryBlocker, 'blocked_before_user_sync');
      expect(
        audit.entryGate.localEvidenceSource,
        managerDeploymentEvidenceExternalTls,
      );
      expect(
        audit.entryGate.productionBlockers,
        contains('release_deployment_evidence_summary_required'),
      );
      expect(audit.entryGate.userSyncEnabled, isFalse);
    },
  );
}
