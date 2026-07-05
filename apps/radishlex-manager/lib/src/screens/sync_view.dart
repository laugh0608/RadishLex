import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'sync/device_signature_section.dart';
import 'sync/sync_connection_health_section.dart';
import 'sync/sync_device_authorization_section.dart';
import 'sync/sync_preflight_section.dart';
import 'sync/sync_recovery_section.dart';

class SyncView extends StatelessWidget {
  const SyncView({super.key, required this.sync, required this.settingsDraft});

  final SyncPreflightSummary sync;
  final ManagerSettingsDraft settingsDraft;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAuditForDraft(
      draft: settingsDraft,
      device: sync.device,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SyncPreflightSection(sync: sync, audit: audit),
        const SizedBox(height: 16),
        SyncConnectionHealthSection(health: audit.entryGate.connectionHealth),
        const SizedBox(height: 16),
        SyncRecoverySection(recovery: audit.entryGate.recovery),
        const SizedBox(height: 16),
        SyncDeviceAuthorizationSection(
          authorization: audit.entryGate.deviceAuthorization,
        ),
        const SizedBox(height: 16),
        DeviceSignatureSection(device: sync.device),
      ],
    );
  }
}
