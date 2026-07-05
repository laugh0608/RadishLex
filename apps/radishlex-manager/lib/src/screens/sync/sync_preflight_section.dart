import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';
import 'sync_empty_state.dart';

class SyncPreflightSection extends StatelessWidget {
  const SyncPreflightSection({
    super.key,
    required this.sync,
    required this.settingsDraft,
  });

  final SyncPreflightSummary sync;
  final ManagerSettingsDraft settingsDraft;

  @override
  Widget build(BuildContext context) {
    final audit = managerSyncGateAuditForDraft(
      draft: settingsDraft,
      device: sync.device,
    );

    return ManagerSection(
      title: '同步预检',
      trailing: _SyncStateBadge(audit: audit),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _SyncGateSummary(sync: sync, audit: audit),
          const SizedBox(height: 14),
          ManagerKeyValueRow(label: 'server', value: sync.serverEndpoint),
          ManagerKeyValueRow(label: 'reason', value: sync.reason),
          ManagerKeyValueRow(label: 'state source', value: audit.stateSource),
          ManagerKeyValueRow(
            label: 'entry state',
            value: audit.entryGate.entryState.code,
          ),
          ManagerKeyValueRow(
            label: 'entry blocker',
            value: audit.entryGate.entryBlocker,
          ),
          ManagerKeyValueRow(
            label: 'local evidence',
            value: audit.entryGate.localEvidenceSource,
          ),
          ManagerKeyValueRow(
            label: 'production blockers',
            value: audit.entryGate.productionBlockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'user sync enabled',
            value: audit.entryGate.userSyncEnabled.toString(),
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
          ManagerKeyValueRow(label: 'last download', value: sync.lastDownload),
          const SizedBox(height: 14),
          _SyncCategorySummary(categories: sync.categories),
          const SizedBox(height: 16),
          const _SyncPreflightActions(),
          const SizedBox(height: 8),
          Text(
            audit.actionStopLine,
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

class _SyncStateBadge extends StatelessWidget {
  const _SyncStateBadge({required this.audit});

  final ManagerSyncGateAudit audit;

  @override
  Widget build(BuildContext context) {
    return ManagerStatusBadge(
      icon: audit.entryGate.userSyncEnabled
          ? Icons.cloud_done_outlined
          : Icons.cloud_off_outlined,
      label: audit.state.code,
      tone: audit.entryGate.userSyncEnabled
          ? ManagerBadgeTone.success
          : ManagerBadgeTone.warning,
    );
  }
}

class _SyncGateSummary extends StatelessWidget {
  const _SyncGateSummary({required this.sync, required this.audit});

  final SyncPreflightSummary sync;
  final ManagerSyncGateAudit audit;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        ManagerStatusBadge(
          icon: Icons.rule_outlined,
          label: audit.stateLabel,
          tone: audit.entryGate.userSyncEnabled
              ? ManagerBadgeTone.success
              : ManagerBadgeTone.warning,
        ),
        Chip(
          avatar: const Icon(Icons.fact_check_outlined, size: 18),
          label: Text(audit.entryGate.entryState.code),
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
      return const SyncEmptyState(
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

class _SyncPreflightActions extends StatelessWidget {
  const _SyncPreflightActions();

  @override
  Widget build(BuildContext context) {
    return Wrap(
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
    );
  }
}
