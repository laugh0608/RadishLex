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
          ManagerKeyValueRow(
            label: 'join flow',
            value: authorization.joinReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'join blocker',
            value: authorization.joinReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'short code',
            value: authorization.joinReadiness.shortCodeVerificationStatus,
          ),
          ManagerKeyValueRow(
            label: 'join errors',
            value: authorization.joinReadiness.errorCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'revocation flow',
            value: authorization.revocationReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'revocation blocker',
            value: authorization.revocationReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'active device',
            value: authorization.revocationReadiness.activeDeviceRequirement,
          ),
          ManagerKeyValueRow(
            label: 'revocation errors',
            value: authorization.revocationReadiness.errorCodeSummary,
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
