import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class DeviceSignatureSection extends StatelessWidget {
  const DeviceSignatureSection({super.key, required this.device});

  final DeviceSecuritySummary device;

  @override
  Widget build(BuildContext context) {
    return ManagerSection(
      title: '设备签名',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ManagerKeyValueRow(label: 'device id', value: device.deviceId),
          ManagerKeyValueRow(label: 'backend', value: device.backendId),
          ManagerKeyValueRow(
            label: 'capability',
            value: device.capabilityStatus,
          ),
          ManagerKeyValueRow(label: 'gate', value: device.productionGate),
          const SizedBox(height: 14),
          _DeviceGateExplanation(device: device),
        ],
      ),
    );
  }
}

class _DeviceGateExplanation extends StatelessWidget {
  const _DeviceGateExplanation({required this.device});

  final DeviceSecuritySummary device;

  @override
  Widget build(BuildContext context) {
    final gateReady = managerDeviceGateReady(device);
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        ManagerStatusBadge(
          icon: gateReady ? Icons.verified_outlined : Icons.gpp_bad_outlined,
          label: managerDeviceGateLabel(device),
          tone: gateReady ? ManagerBadgeTone.success : ManagerBadgeTone.warning,
        ),
        Chip(label: Text('backend ${device.backendId}')),
        Chip(label: Text('capability ${device.capabilityStatus}')),
      ],
    );
  }
}
