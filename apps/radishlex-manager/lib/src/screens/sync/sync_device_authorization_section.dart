import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class SyncDeviceAuthorizationSection extends StatelessWidget {
  const SyncDeviceAuthorizationSection({
    super.key,
    required this.authorization,
  });

  final DeviceAuthorizationEntryGate authorization;

  @override
  Widget build(BuildContext context) {
    return ManagerSection(
      key: const Key('sync-device-authorization-readiness-section'),
      title: '设备授权准备态',
      trailing: ManagerStatusBadge(
        icon: Icons.phonelink_lock_outlined,
        label: authorization.status.code,
        tone: ManagerBadgeTone.warning,
      ),
      child: Column(
        children: [
          ManagerKeyValueRow(
            label: 'status',
            value: authorization.status.label,
          ),
          ManagerKeyValueRow(label: 'blocker', value: authorization.blocker),
          ManagerKeyValueRow(
            label: 'join request',
            value: authorization.joinRequestStatus.code,
          ),
          ManagerKeyValueRow(
            label: 'create request',
            value: authorization.createJoinRequestStatus,
          ),
          ManagerKeyValueRow(
            label: 'approve request',
            value: authorization.approveJoinRequestStatus,
          ),
          ManagerKeyValueRow(
            label: 'authorization package',
            value: authorization.authorizationPackageStatus,
          ),
          ManagerKeyValueRow(
            label: 'package blocker',
            value: authorization.authorizationPackageBlocker,
          ),
          ManagerKeyValueRow(
            label: 'package preconditions',
            value: authorization.authorizationPackagePreconditions,
          ),
          ManagerKeyValueRow(
            label: 'revoke device',
            value: authorization.revokeDeviceStatus,
          ),
          ManagerKeyValueRow(
            label: 'lost device risk',
            value: authorization.lostDeviceRiskNotice,
          ),
          ManagerKeyValueRow(
            label: 'key epoch',
            value: authorization.keyEpochStatus,
          ),
          ManagerKeyValueRow(
            label: 'readiness blockers',
            value: authorization.readinessBlockerSummary,
          ),
          const ManagerKeyValueRow(
            label: 'stop line',
            value: '设备加入审批、授权成功和撤销操作仍未开放。',
          ),
        ],
      ),
    );
  }
}
