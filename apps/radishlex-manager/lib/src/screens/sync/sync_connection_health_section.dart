import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class SyncConnectionHealthSection extends StatelessWidget {
  const SyncConnectionHealthSection({super.key, required this.health});

  final SyncConnectionHealth health;

  @override
  Widget build(BuildContext context) {
    return ManagerSection(
      key: const Key('sync-connection-health-section'),
      title: '服务连接健康',
      trailing: ManagerStatusBadge(
        icon: health.canRunReadOnlyProbe
            ? Icons.cloud_queue_outlined
            : Icons.cloud_off_outlined,
        label: health.status.code,
        tone: health.canRunReadOnlyProbe
            ? ManagerBadgeTone.neutral
            : ManagerBadgeTone.warning,
      ),
      child: Column(
        children: [
          ManagerKeyValueRow(label: 'status', value: health.status.label),
          ManagerKeyValueRow(label: 'blocker', value: health.connectionBlocker),
          ManagerKeyValueRow(label: 'endpoint', value: health.endpointStatus),
          ManagerKeyValueRow(
            label: 'access token',
            value: health.accessTokenStatus,
          ),
          ManagerKeyValueRow(label: 'transport', value: health.transportMode),
          ManagerKeyValueRow(
            label: 'server state',
            value: health.serverStateStatus,
          ),
          ManagerKeyValueRow(
            label: 'last remote error',
            value: health.lastRemoteErrorCode,
          ),
          const ManagerKeyValueRow(
            label: 'stop line',
            value: '只读检查不上传 P2 对象，不生成恢复码，不打开设备授权成功路径。',
          ),
        ],
      ),
    );
  }
}
