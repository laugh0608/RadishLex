import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class SyncRecoverySection extends StatelessWidget {
  const SyncRecoverySection({
    super.key,
    required this.recovery,
    required this.interactionPlan,
  });

  final RecoveryEntryGate recovery;
  final SyncInteractionEntryPlan interactionPlan;

  @override
  Widget build(BuildContext context) {
    final setupIntent = interactionPlan.intentFor('recovery_setup');
    final restoreIntent = interactionPlan.intentFor('recovery_restore');

    return ManagerSection(
      key: const Key('sync-recovery-readiness-section'),
      title: '恢复码准备态',
      trailing: ManagerStatusBadge(
        icon: Icons.key_off_outlined,
        label: recovery.status.code,
        tone: ManagerBadgeTone.warning,
      ),
      child: Column(
        children: [
          ManagerKeyValueRow(label: 'status', value: recovery.status.label),
          ManagerKeyValueRow(label: 'blocker', value: recovery.blocker),
          ManagerKeyValueRow(
            label: 'generate code',
            value: recovery.generateStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore device',
            value: recovery.restoreStatus,
          ),
          ManagerKeyValueRow(
            label: 'confirmation',
            value: recovery.confirmationStatus,
          ),
          ManagerKeyValueRow(
            label: 'save confirmation',
            value: recovery.saveConfirmationRequirement,
          ),
          ManagerKeyValueRow(
            label: 'recovery record',
            value: recovery.recoveryRecordStatus,
          ),
          ManagerKeyValueRow(
            label: 'record blocker',
            value: recovery.recoveryRecordBlocker,
          ),
          ManagerKeyValueRow(
            label: 'first upload gate',
            value: recovery.firstUploadGate,
          ),
          ManagerKeyValueRow(
            label: 'readiness blockers',
            value: recovery.readinessBlockerSummary,
          ),
          ManagerKeyValueRow(
            label: 'setup intent',
            value: setupIntent.intentStatus,
          ),
          ManagerKeyValueRow(
            label: 'setup intent blocker',
            value: setupIntent.blocker,
          ),
          ManagerKeyValueRow(
            label: 'setup intent evidence',
            value: setupIntent.requiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'restore intent',
            value: restoreIntent.intentStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore intent blocker',
            value: restoreIntent.blocker,
          ),
          ManagerKeyValueRow(
            label: 'restore intent evidence',
            value: restoreIntent.requiredEvidenceSummary,
          ),
          ManagerKeyValueRow(
            label: 'setup flow',
            value: recovery.setupReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'setup display status',
            value: recovery.setupReadiness.oneTimeDisplayStatus,
          ),
          ManagerKeyValueRow(
            label: 'setup blocker',
            value: recovery.setupReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'setup save confirmation',
            value: recovery.setupReadiness.saveConfirmationStatus,
          ),
          ManagerKeyValueRow(
            label: 'setup prerequisites',
            value: recovery.setupReadiness.prerequisiteSummary,
          ),
          ManagerKeyValueRow(
            label: 'setup errors',
            value: recovery.setupReadiness.errorCodeSummary,
          ),
          ManagerKeyValueRow(
            label: 'restore flow',
            value: recovery.restoreReadiness.status,
          ),
          ManagerKeyValueRow(
            label: 'restore blocker',
            value: recovery.restoreReadiness.blocker,
          ),
          ManagerKeyValueRow(
            label: 'code input',
            value: recovery.restoreReadiness.codeInputStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore lookup',
            value: recovery.restoreReadiness.recoveryRecordLookupStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore attempt limit',
            value: recovery.restoreReadiness.attemptLimitStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore device registration',
            value: recovery.restoreReadiness.deviceRegistrationStatus,
          ),
          ManagerKeyValueRow(
            label: 'restore errors',
            value: recovery.restoreReadiness.errorCodeSummary,
          ),
          const ManagerKeyValueRow(
            label: 'stop line',
            value: '恢复码生成、输入、轮换和撤销仍未开放。',
          ),
        ],
      ),
    );
  }
}
