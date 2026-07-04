import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class SyncView extends StatelessWidget {
  const SyncView({super.key, required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ManagerSection(
          title: '同步预检',
          trailing: ManagerStatusBadge(
            icon: Icons.cloud_off_outlined,
            label: sync.state.code,
            tone: ManagerBadgeTone.warning,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              ManagerKeyValueRow(label: 'server', value: sync.serverEndpoint),
              ManagerKeyValueRow(label: 'reason', value: sync.reason),
              ManagerKeyValueRow(
                label: 'syncable objects',
                value: sync.syncableObjects.toString(),
              ),
              ManagerKeyValueRow(
                label: 'local-only events',
                value: sync.localOnlyEvents.toString(),
              ),
              ManagerKeyValueRow(label: 'last upload', value: sync.lastUpload),
              ManagerKeyValueRow(
                label: 'last download',
                value: sync.lastDownload,
              ),
              const SizedBox(height: 14),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: sync.categories
                    .map(
                      (category) => Chip(
                        label: Text('${category.name}: ${category.count}'),
                      ),
                    )
                    .toList(),
              ),
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  FilledButton.icon(
                    onPressed: sync.state.canEnableUserSync ? () {} : null,
                    icon: const Icon(Icons.cloud_upload_outlined),
                    label: const Text('启用同步'),
                  ),
                  OutlinedButton.icon(
                    onPressed: () {},
                    icon: const Icon(Icons.content_copy_outlined),
                    label: const Text('复制诊断摘要'),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '设备签名',
          child: Column(
            children: [
              ManagerKeyValueRow(
                label: 'device id',
                value: sync.device.deviceId,
              ),
              ManagerKeyValueRow(
                label: 'backend',
                value: sync.device.backendId,
              ),
              ManagerKeyValueRow(
                label: 'capability',
                value: sync.device.capabilityStatus,
              ),
              ManagerKeyValueRow(
                label: 'gate',
                value: sync.device.productionGate,
              ),
            ],
          ),
        ),
      ],
    );
  }
}
