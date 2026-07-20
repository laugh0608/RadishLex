import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/manager_settings_store.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';

void main() {
  test('settings persist only credential presence, never secret material', () {
    final tempDir = Directory.systemTemp.createTempSync(
      'radishlex-manager-secret-boundary-',
    );
    addTearDown(() => tempDir.deleteSync(recursive: true));
    final settingsFile = '${tempDir.path}/manager-settings.json';
    final store = ManagerSettingsStore(filePath: settingsFile);

    store.save(
      const ManagerSettingsDraft(
        serverEndpoint: 'https://sync.example.invalid',
        retainSyncConfig: true,
        privacyMode: false,
        diagnosticsExport: true,
        deploymentEvidenceRecorded: false,
        accessTokenConfigured: true,
      ),
    );

    final persisted = File(settingsFile).readAsStringSync();
    expect(persisted, contains('"access_token_configured": true'));
    expect(persisted, isNot(contains('access_token_value')));
    expect(persisted, isNot(contains('recovery_code')));
    expect(persisted, isNot(contains('short_code')));
    expect(persisted, isNot(contains('private_key')));
    expect(persisted, isNot(contains('wrapped_material')));
  });

  test('diagnostics expose status codes without transient secret material', () {
    final report = createManagerDiagnosticsReport(createManagerFixture());
    final text = report.toRedactedText();

    expect(text, contains('settings.access_token: not_configured'));
    expect(text, contains('sync.recovery_setup_display_status:'));
    expect(text, contains('sync.recovery_restore_code_input:'));
    expect(text, contains('sync.device_join_short_code:'));
    expect(text, contains('sync.interaction_statuses:'));
    for (final fragment in [
      'Bearer ',
      'access_token_value',
      'recovery_code=',
      'short_code=',
      'private_key=',
      'signature_bytes=',
      'wrapped_material=',
      'request_body',
      'response_body',
    ]) {
      expect(text, isNot(contains(fragment)), reason: fragment);
    }
  });

  test('privacy mode closes sync entry before any interaction can execute', () {
    final snapshot = createManagerFixture();
    final audit = managerSyncGateAuditForDraft(
      draft: snapshot.settings.draft.copyWith(privacyMode: true),
      device: snapshot.sync.device,
      readinessBridgeSnapshot: snapshot.sync.readinessBridgeSnapshot,
    );

    expect(audit.state, SyncUiState.syncDisabledByPolicy);
    expect(audit.entryGate.userSyncEnabled, isFalse);
    expect(audit.entryGate.entryBlocker, 'sync_disabled_by_policy');
    expect(
      audit.entryGate.interactionEntryPlan.intents,
      everyElement(
        isA<SyncInteractionActionIntent>().having(
          (intent) => intent.intentStatus,
          'status',
          'closed_current_phase',
        ),
      ),
    );
  });
}
