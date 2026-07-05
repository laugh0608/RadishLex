import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'sync/device_signature_section.dart';
import 'sync/sync_preflight_section.dart';

class SyncView extends StatelessWidget {
  const SyncView({super.key, required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SyncPreflightSection(sync: sync),
        const SizedBox(height: 16),
        DeviceSignatureSection(device: sync.device),
      ],
    );
  }
}
