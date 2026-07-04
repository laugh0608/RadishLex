import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class SyncView extends StatelessWidget {
  const SyncView({super.key, required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAudit(state: sync.state, device: sync.device);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ManagerSection(
          title: '同步预检',
          trailing: _SyncStateBadge(state: sync.state),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _SyncGateSummary(sync: sync),
              const SizedBox(height: 14),
              ManagerKeyValueRow(label: 'server', value: sync.serverEndpoint),
              ManagerKeyValueRow(label: 'reason', value: sync.reason),
              ManagerKeyValueRow(
                label: 'state source',
                value: audit.stateSource,
              ),
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
              _SyncCategorySummary(categories: sync.categories),
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  FilledButton.icon(
                    key: const Key('sync-enable-button'),
                    onPressed: null,
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
              const SizedBox(height: 8),
              Text(
                audit.actionStopLine,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '设备签名',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
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
              const SizedBox(height: 14),
              _DeviceGateExplanation(device: sync.device),
            ],
          ),
        ),
      ],
    );
  }
}

class _SyncStateBadge extends StatelessWidget {
  const _SyncStateBadge({required this.state});

  final SyncUiState state;

  @override
  Widget build(BuildContext context) {
    return ManagerStatusBadge(
      icon: state.canEnableUserSync
          ? Icons.cloud_done_outlined
          : Icons.cloud_off_outlined,
      label: state.code,
      tone: state.canEnableUserSync
          ? ManagerBadgeTone.success
          : ManagerBadgeTone.warning,
    );
  }
}

class _SyncGateSummary extends StatelessWidget {
  const _SyncGateSummary({required this.sync});

  final SyncPreflightSummary sync;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAudit(state: sync.state, device: sync.device);

    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        ManagerStatusBadge(
          icon: Icons.rule_outlined,
          label: audit.stateLabel,
          tone: sync.state.canEnableUserSync
              ? ManagerBadgeTone.success
              : ManagerBadgeTone.warning,
        ),
        Chip(
          avatar: const Icon(Icons.inventory_2_outlined, size: 18),
          label: Text('syncable ${sync.syncableObjects}'),
        ),
        Chip(
          avatar: const Icon(Icons.lock_clock_outlined, size: 18),
          label: Text('local-only ${sync.localOnlyEvents}'),
        ),
      ],
    );
  }
}

class _SyncCategorySummary extends StatelessWidget {
  const _SyncCategorySummary({required this.categories});

  final List<SyncCategorySummary> categories;

  @override
  Widget build(BuildContext context) {
    if (categories.isEmpty) {
      return const _SyncEmptyState(
        icon: Icons.inventory_2_outlined,
        message: '暂无本地 P2 对象分类摘要',
      );
    }

    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: categories
          .map(
            (category) =>
                Chip(label: Text('${category.name}: ${category.count}')),
          )
          .toList(),
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

class _SyncEmptyState extends StatelessWidget {
  const _SyncEmptyState({required this.icon, required this.message});

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.onSurfaceVariant;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Row(
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 10),
          Flexible(
            child: Text(
              message,
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: color),
            ),
          ),
        ],
      ),
    );
  }
}
