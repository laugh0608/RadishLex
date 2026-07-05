enum RecoveryEntryStatus {
  flowClosed,
  recoveryCodeRequired,
  saveConfirmationRequired,
  ready,
}

extension RecoveryEntryStatusLabel on RecoveryEntryStatus {
  String get code {
    switch (this) {
      case RecoveryEntryStatus.flowClosed:
        return 'recovery_code_flow_closed';
      case RecoveryEntryStatus.recoveryCodeRequired:
        return 'recovery_code_required';
      case RecoveryEntryStatus.saveConfirmationRequired:
        return 'recovery_code_save_confirmation_required';
      case RecoveryEntryStatus.ready:
        return 'recovery_ready';
    }
  }

  String get label {
    switch (this) {
      case RecoveryEntryStatus.flowClosed:
        return '恢复码流程关闭';
      case RecoveryEntryStatus.recoveryCodeRequired:
        return '等待恢复码';
      case RecoveryEntryStatus.saveConfirmationRequired:
        return '等待保存确认';
      case RecoveryEntryStatus.ready:
        return '恢复码准备完成';
    }
  }
}

enum DeviceAuthorizationEntryStatus {
  flowClosed,
  joinRequestUnavailable,
  authorizationUnavailable,
  ready,
}

extension DeviceAuthorizationEntryStatusLabel
    on DeviceAuthorizationEntryStatus {
  String get code {
    switch (this) {
      case DeviceAuthorizationEntryStatus.flowClosed:
        return 'device_authorization_flow_closed';
      case DeviceAuthorizationEntryStatus.joinRequestUnavailable:
        return 'join_request_unavailable';
      case DeviceAuthorizationEntryStatus.authorizationUnavailable:
        return 'authorization_unavailable';
      case DeviceAuthorizationEntryStatus.ready:
        return 'device_authorization_ready';
    }
  }

  String get label {
    switch (this) {
      case DeviceAuthorizationEntryStatus.flowClosed:
        return '设备授权流程关闭';
      case DeviceAuthorizationEntryStatus.joinRequestUnavailable:
        return '加入请求不可用';
      case DeviceAuthorizationEntryStatus.authorizationUnavailable:
        return '授权不可用';
      case DeviceAuthorizationEntryStatus.ready:
        return '设备授权准备完成';
    }
  }
}

enum JoinRequestStatus { unavailable, pending, expired, authorized }

extension JoinRequestStatusLabel on JoinRequestStatus {
  String get code {
    switch (this) {
      case JoinRequestStatus.unavailable:
        return 'join_request_unavailable';
      case JoinRequestStatus.pending:
        return 'join_request_pending';
      case JoinRequestStatus.expired:
        return 'join_request_expired';
      case JoinRequestStatus.authorized:
        return 'join_request_authorized';
    }
  }

  String get label {
    switch (this) {
      case JoinRequestStatus.unavailable:
        return '加入请求不可用';
      case JoinRequestStatus.pending:
        return '加入请求待处理';
      case JoinRequestStatus.expired:
        return '加入请求已过期';
      case JoinRequestStatus.authorized:
        return '加入请求已授权';
    }
  }
}

class SyncReadinessFlowSummary {
  const SyncReadinessFlowSummary({
    required this.flowId,
    required this.status,
    required this.blocker,
    required this.actionStatus,
    required this.requiredEvidenceCodes,
    required this.errorCodes,
    required this.blocksUserSync,
    required this.sourceTag,
  });

  final String flowId;
  final String status;
  final String blocker;
  final String actionStatus;
  final List<String> requiredEvidenceCodes;
  final List<String> errorCodes;
  final bool blocksUserSync;
  final String sourceTag;

  String get requiredEvidenceSummary {
    return managerSyncCodeSummary(requiredEvidenceCodes);
  }

  String get errorCodeSummary {
    return managerSyncCodeSummary(errorCodes);
  }

  List<String> get issueCodes {
    return [if (blocker != 'none') blocker, ...errorCodes];
  }

  String get issueCodeSummary {
    return managerSyncCodeSummary(issueCodes);
  }
}

class RecoveryEntryGate {
  const RecoveryEntryGate({
    required this.status,
    required this.blocker,
    required this.canGenerateCode,
    required this.canRestoreDevice,
    required this.requiresSaveConfirmation,
    required this.saveConfirmationRequirement,
    required this.recoveryRecordStatus,
    required this.recoveryRecordBlocker,
    required this.firstUploadGate,
    required this.readinessBlockers,
    required this.setupReadiness,
    required this.restoreReadiness,
  });

  final RecoveryEntryStatus status;
  final String blocker;
  final bool canGenerateCode;
  final bool canRestoreDevice;
  final bool requiresSaveConfirmation;
  final String saveConfirmationRequirement;
  final String recoveryRecordStatus;
  final String recoveryRecordBlocker;
  final String firstUploadGate;
  final List<String> readinessBlockers;
  final RecoverySetupReadiness setupReadiness;
  final RecoveryRestoreReadiness restoreReadiness;

  String get generateStatus {
    return canGenerateCode ? 'available' : 'closed_current_phase';
  }

  String get restoreStatus {
    return canRestoreDevice ? 'available' : 'closed_current_phase';
  }

  String get confirmationStatus {
    return requiresSaveConfirmation ? 'required' : 'not_started';
  }

  String get readinessBlockerSummary {
    return readinessBlockers.isEmpty ? 'none' : readinessBlockers.join(', ');
  }

  List<SyncReadinessFlowSummary> get readinessFlowSummaries {
    return [setupReadiness.toFlowSummary(), restoreReadiness.toFlowSummary()];
  }
}

class RecoverySetupReadiness {
  const RecoverySetupReadiness({
    required this.status,
    required this.blocker,
    required this.entryActionStatus,
    required this.generatedCodeStatus,
    required this.saveConfirmationStatus,
    required this.recoveryRecordStatus,
    required this.firstUploadGate,
    required this.requiredPrerequisites,
    required this.errorCodes,
    this.sourceTag = 'recovery_setup_readiness',
  });

  final String status;
  final String blocker;
  final String entryActionStatus;
  final String generatedCodeStatus;
  final String saveConfirmationStatus;
  final String recoveryRecordStatus;
  final String firstUploadGate;
  final List<String> requiredPrerequisites;
  final List<String> errorCodes;
  final String sourceTag;

  String get prerequisiteSummary {
    return requiredPrerequisites.isEmpty
        ? 'none'
        : requiredPrerequisites.join(', ');
  }

  String get errorCodeSummary {
    return errorCodes.isEmpty ? 'none' : errorCodes.join(', ');
  }

  SyncReadinessFlowSummary toFlowSummary() {
    return SyncReadinessFlowSummary(
      flowId: 'recovery_setup',
      status: status,
      blocker: blocker,
      actionStatus: entryActionStatus,
      requiredEvidenceCodes: requiredPrerequisites,
      errorCodes: errorCodes,
      blocksUserSync: blocker != 'none',
      sourceTag: sourceTag,
    );
  }
}

class RecoveryRestoreReadiness {
  const RecoveryRestoreReadiness({
    required this.status,
    required this.blocker,
    required this.entryActionStatus,
    required this.codeInputStatus,
    required this.recoveryRecordLookupStatus,
    required this.attemptLimitStatus,
    required this.deviceRegistrationStatus,
    required this.errorCodes,
    this.sourceTag = 'recovery_restore_readiness',
  });

  final String status;
  final String blocker;
  final String entryActionStatus;
  final String codeInputStatus;
  final String recoveryRecordLookupStatus;
  final String attemptLimitStatus;
  final String deviceRegistrationStatus;
  final List<String> errorCodes;
  final String sourceTag;

  String get errorCodeSummary {
    return errorCodes.isEmpty ? 'none' : errorCodes.join(', ');
  }

  SyncReadinessFlowSummary toFlowSummary() {
    return SyncReadinessFlowSummary(
      flowId: 'recovery_restore',
      status: status,
      blocker: blocker,
      actionStatus: entryActionStatus,
      requiredEvidenceCodes: [
        codeInputStatus,
        recoveryRecordLookupStatus,
        deviceRegistrationStatus,
      ],
      errorCodes: errorCodes,
      blocksUserSync: blocker != 'none',
      sourceTag: sourceTag,
    );
  }
}

class DeviceAuthorizationEntryGate {
  const DeviceAuthorizationEntryGate({
    required this.status,
    required this.blocker,
    required this.joinRequestStatus,
    required this.canCreateJoinRequest,
    required this.canApproveJoinRequest,
    required this.canRevokeDevice,
    required this.authorizationPackageStatus,
    required this.authorizationPackageBlocker,
    required this.authorizationPackagePreconditions,
    required this.lostDeviceRiskNotice,
    required this.keyEpochStatus,
    required this.readinessBlockers,
    required this.joinReadiness,
    required this.revocationReadiness,
  });

  final DeviceAuthorizationEntryStatus status;
  final String blocker;
  final JoinRequestStatus joinRequestStatus;
  final bool canCreateJoinRequest;
  final bool canApproveJoinRequest;
  final bool canRevokeDevice;
  final String authorizationPackageStatus;
  final String authorizationPackageBlocker;
  final String authorizationPackagePreconditions;
  final String lostDeviceRiskNotice;
  final String keyEpochStatus;
  final List<String> readinessBlockers;
  final DeviceJoinReadiness joinReadiness;
  final DeviceRevocationReadiness revocationReadiness;

  String get createJoinRequestStatus {
    return canCreateJoinRequest ? 'available' : 'closed_current_phase';
  }

  String get approveJoinRequestStatus {
    return canApproveJoinRequest ? 'available' : 'closed_current_phase';
  }

  String get revokeDeviceStatus {
    return canRevokeDevice ? 'available' : 'closed_current_phase';
  }

  String get readinessBlockerSummary {
    return readinessBlockers.isEmpty ? 'none' : readinessBlockers.join(', ');
  }

  List<SyncReadinessFlowSummary> get readinessFlowSummaries {
    return [joinReadiness.toFlowSummary(), revocationReadiness.toFlowSummary()];
  }
}

class DeviceJoinReadiness {
  const DeviceJoinReadiness({
    required this.status,
    required this.blocker,
    required this.entryActionStatus,
    required this.joinRequestStatus,
    required this.shortCodeVerificationStatus,
    required this.authorizationPackageStatus,
    required this.authorizationPackagePreconditions,
    required this.errorCodes,
    this.sourceTag = 'device_join_readiness',
  });

  final String status;
  final String blocker;
  final String entryActionStatus;
  final JoinRequestStatus joinRequestStatus;
  final String shortCodeVerificationStatus;
  final String authorizationPackageStatus;
  final String authorizationPackagePreconditions;
  final List<String> errorCodes;
  final String sourceTag;

  String get errorCodeSummary {
    return errorCodes.isEmpty ? 'none' : errorCodes.join(', ');
  }

  SyncReadinessFlowSummary toFlowSummary() {
    return SyncReadinessFlowSummary(
      flowId: 'device_join',
      status: status,
      blocker: blocker,
      actionStatus: entryActionStatus,
      requiredEvidenceCodes: [
        joinRequestStatus.code,
        shortCodeVerificationStatus,
        authorizationPackagePreconditions,
      ],
      errorCodes: errorCodes,
      blocksUserSync: blocker != 'none',
      sourceTag: sourceTag,
    );
  }
}

class DeviceRevocationReadiness {
  const DeviceRevocationReadiness({
    required this.status,
    required this.blocker,
    required this.entryActionStatus,
    required this.revokeDeviceStatus,
    required this.activeDeviceRequirement,
    required this.lostDeviceRiskNotice,
    required this.keyEpochStatus,
    required this.errorCodes,
    this.sourceTag = 'device_revocation_readiness',
  });

  final String status;
  final String blocker;
  final String entryActionStatus;
  final String revokeDeviceStatus;
  final String activeDeviceRequirement;
  final String lostDeviceRiskNotice;
  final String keyEpochStatus;
  final List<String> errorCodes;
  final String sourceTag;

  String get errorCodeSummary {
    return errorCodes.isEmpty ? 'none' : errorCodes.join(', ');
  }

  SyncReadinessFlowSummary toFlowSummary() {
    return SyncReadinessFlowSummary(
      flowId: 'device_revocation',
      status: status,
      blocker: blocker,
      actionStatus: entryActionStatus,
      requiredEvidenceCodes: [
        activeDeviceRequirement,
        lostDeviceRiskNotice,
        keyEpochStatus,
      ],
      errorCodes: errorCodes,
      blocksUserSync: blocker != 'none',
      sourceTag: sourceTag,
    );
  }
}

String managerSyncCodeSummary(Iterable<String> codes) {
  final uniqueCodes = <String>{};
  for (final code in codes) {
    final trimmed = code.trim();
    if (trimmed.isEmpty || trimmed == 'none') {
      continue;
    }
    for (final part in trimmed.split(',')) {
      final normalized = part.trim();
      if (normalized.isEmpty || normalized == 'none') {
        continue;
      }
      uniqueCodes.add(normalized);
    }
  }
  return uniqueCodes.isEmpty ? 'none' : uniqueCodes.join(', ');
}

RecoveryEntryGate managerRecoveryEntryGateFromReadiness({
  required RecoverySetupReadiness setupReadiness,
  required RecoveryRestoreReadiness restoreReadiness,
}) {
  final blocker = _firstSyncCode([
    setupReadiness.blocker,
    restoreReadiness.blocker,
  ]);
  return RecoveryEntryGate(
    status: _recoveryEntryStatusFromReadiness(
      setupReadiness: setupReadiness,
      restoreReadiness: restoreReadiness,
      blocker: blocker,
    ),
    blocker: blocker,
    canGenerateCode: false,
    canRestoreDevice: false,
    requiresSaveConfirmation:
        setupReadiness.saveConfirmationStatus == 'required_before_first_upload',
    saveConfirmationRequirement: setupReadiness.saveConfirmationStatus,
    recoveryRecordStatus: setupReadiness.recoveryRecordStatus,
    recoveryRecordBlocker: _recoveryRecordBlocker(
      setupReadiness.recoveryRecordStatus,
    ),
    firstUploadGate: setupReadiness.firstUploadGate,
    readinessBlockers: _recoveryReadinessBlockers(
      setupReadiness: setupReadiness,
      restoreReadiness: restoreReadiness,
    ),
    setupReadiness: setupReadiness,
    restoreReadiness: restoreReadiness,
  );
}

DeviceAuthorizationEntryGate managerDeviceAuthorizationEntryGateFromReadiness({
  required DeviceJoinReadiness joinReadiness,
  required DeviceRevocationReadiness revocationReadiness,
}) {
  final blocker = _firstSyncCode([
    joinReadiness.blocker,
    revocationReadiness.blocker,
  ]);
  return DeviceAuthorizationEntryGate(
    status: _deviceAuthorizationEntryStatusFromReadiness(
      joinReadiness: joinReadiness,
      revocationReadiness: revocationReadiness,
      blocker: blocker,
    ),
    blocker: blocker,
    joinRequestStatus: joinReadiness.joinRequestStatus,
    canCreateJoinRequest: false,
    canApproveJoinRequest: false,
    canRevokeDevice: false,
    authorizationPackageStatus: joinReadiness.authorizationPackageStatus,
    authorizationPackageBlocker: _authorizationPackageBlocker(joinReadiness),
    authorizationPackagePreconditions:
        joinReadiness.authorizationPackagePreconditions,
    lostDeviceRiskNotice: revocationReadiness.lostDeviceRiskNotice,
    keyEpochStatus: revocationReadiness.keyEpochStatus,
    readinessBlockers: _deviceAuthorizationReadinessBlockers(
      joinReadiness: joinReadiness,
      revocationReadiness: revocationReadiness,
    ),
    joinReadiness: joinReadiness,
    revocationReadiness: revocationReadiness,
  );
}

RecoveryEntryStatus _recoveryEntryStatusFromReadiness({
  required RecoverySetupReadiness setupReadiness,
  required RecoveryRestoreReadiness restoreReadiness,
  required String blocker,
}) {
  if (blocker == 'none') {
    return RecoveryEntryStatus.ready;
  }
  if (setupReadiness.status == 'recovery_setup_flow_closed' ||
      restoreReadiness.status == 'recovery_restore_flow_closed') {
    return RecoveryEntryStatus.flowClosed;
  }
  if (setupReadiness.saveConfirmationStatus != 'confirmed') {
    return RecoveryEntryStatus.saveConfirmationRequired;
  }
  if (setupReadiness.errorCodes.contains('recovery_code_required') ||
      restoreReadiness.errorCodes.contains('recovery_code_required')) {
    return RecoveryEntryStatus.recoveryCodeRequired;
  }
  return RecoveryEntryStatus.flowClosed;
}

DeviceAuthorizationEntryStatus _deviceAuthorizationEntryStatusFromReadiness({
  required DeviceJoinReadiness joinReadiness,
  required DeviceRevocationReadiness revocationReadiness,
  required String blocker,
}) {
  if (blocker == 'none') {
    return DeviceAuthorizationEntryStatus.ready;
  }
  if (joinReadiness.status == 'device_join_flow_closed' ||
      revocationReadiness.status == 'device_revocation_flow_closed') {
    return DeviceAuthorizationEntryStatus.flowClosed;
  }
  if (joinReadiness.joinRequestStatus == JoinRequestStatus.unavailable) {
    return DeviceAuthorizationEntryStatus.joinRequestUnavailable;
  }
  return DeviceAuthorizationEntryStatus.authorizationUnavailable;
}

String _firstSyncCode(Iterable<String> codes) {
  for (final code in codes) {
    final normalized = managerSyncCodeSummary([code]);
    if (normalized != 'none') {
      return normalized;
    }
  }
  return 'none';
}

String _recoveryRecordBlocker(String recoveryRecordStatus) {
  switch (recoveryRecordStatus) {
    case 'recovery_record_active':
      return 'none';
    case 'recovery_record_missing':
      return 'recovery_record_missing';
    case 'recovery_record_revoked':
      return 'recovery_record_revoked';
    default:
      return 'recovery_record_creation_closed';
  }
}

String _authorizationPackageBlocker(DeviceJoinReadiness joinReadiness) {
  if (joinReadiness.authorizationPackageStatus ==
      'authorization_package_ready') {
    return 'none';
  }
  if (joinReadiness.authorizationPackagePreconditions == 'satisfied') {
    return joinReadiness.blocker == 'none' ? 'none' : joinReadiness.blocker;
  }
  return 'authorization_package_prerequisites_blocked';
}

List<String> _recoveryReadinessBlockers({
  required RecoverySetupReadiness setupReadiness,
  required RecoveryRestoreReadiness restoreReadiness,
}) {
  return _syncCodes([
    setupReadiness.blocker,
    restoreReadiness.blocker,
    if (setupReadiness.recoveryRecordStatus != 'recovery_record_active')
      setupReadiness.recoveryRecordStatus,
    if (setupReadiness.saveConfirmationStatus == 'required_before_first_upload')
      'recovery_code_save_confirmation_required',
  ]);
}

List<String> _deviceAuthorizationReadinessBlockers({
  required DeviceJoinReadiness joinReadiness,
  required DeviceRevocationReadiness revocationReadiness,
}) {
  return _syncCodes([
    joinReadiness.blocker,
    if (joinReadiness.joinRequestStatus != JoinRequestStatus.authorized)
      joinReadiness.joinRequestStatus.code,
    _authorizationPackageBlocker(joinReadiness),
    revocationReadiness.blocker,
    if (revocationReadiness.lostDeviceRiskNotice != 'acknowledged')
      'lost_device_risk_notice_required',
    if (revocationReadiness.keyEpochStatus != 'key_epoch_ready')
      revocationReadiness.keyEpochStatus,
  ]);
}

List<String> _syncCodes(Iterable<String> codes) {
  final summary = managerSyncCodeSummary(codes);
  return summary == 'none'
      ? const []
      : summary.split(', ').toList(growable: false);
}

const managerClosedRecoverySetupReadiness = RecoverySetupReadiness(
  status: 'recovery_setup_flow_closed',
  blocker: 'recovery_code_generation_closed',
  entryActionStatus: 'read_only_current_phase',
  generatedCodeStatus: 'not_generated',
  saveConfirmationStatus: 'required_before_first_upload',
  recoveryRecordStatus: 'recovery_record_not_created',
  firstUploadGate: 'blocked_until_recovery_code_saved',
  requiredPrerequisites: [
    'platform_private_key_backend_ready',
    'release_deployment_evidence_summary_required',
    'explicit_user_start_required',
  ],
  errorCodes: [
    'recovery_code_required',
    'recovery_record_missing',
    'recovery_record_revoked',
    'local_data_inconsistent',
  ],
);

const managerClosedRecoveryRestoreReadiness = RecoveryRestoreReadiness(
  status: 'recovery_restore_flow_closed',
  blocker: 'recovery_code_input_closed',
  entryActionStatus: 'read_only_current_phase',
  codeInputStatus: 'input_not_available_current_phase',
  recoveryRecordLookupStatus: 'not_checked_current_phase',
  attemptLimitStatus: 'not_started',
  deviceRegistrationStatus: 'blocked_until_recovery_success',
  errorCodes: [
    'recovery_code_required',
    'recovery_code_invalid',
    'recovery_record_missing',
    'recovery_record_revoked',
    'authentication_required',
    'network_unreachable',
  ],
);

const managerClosedDeviceJoinReadiness = DeviceJoinReadiness(
  status: 'device_join_flow_closed',
  blocker: 'join_request_creation_closed',
  entryActionStatus: 'read_only_current_phase',
  joinRequestStatus: JoinRequestStatus.unavailable,
  shortCodeVerificationStatus: 'short_code_verification_not_started',
  authorizationPackageStatus: 'authorization_package_not_created',
  authorizationPackagePreconditions:
      'active_existing_device_required, join_request_pending_required, short_code_match_required',
  errorCodes: [
    'join_request_expired',
    'authorization_rejected',
    'device_revoked',
    'backend_unavailable',
    'network_unreachable',
  ],
);

const managerClosedDeviceRevocationReadiness = DeviceRevocationReadiness(
  status: 'device_revocation_flow_closed',
  blocker: 'device_revocation_flow_closed',
  entryActionStatus: 'read_only_current_phase',
  revokeDeviceStatus: 'closed_current_phase',
  activeDeviceRequirement: 'active_existing_device_required',
  lostDeviceRiskNotice: 'lost_device_prior_material_not_recallable',
  keyEpochStatus: 'key_epoch_rotation_not_started',
  errorCodes: [
    'device_revoked',
    'key_epoch_rotation_required',
    'local_data_inconsistent',
    'network_unreachable',
  ],
);

const managerReadyRecoverySetupReadiness = RecoverySetupReadiness(
  status: 'recovery_setup_ready',
  blocker: 'none',
  entryActionStatus: 'available',
  generatedCodeStatus: 'generated_once',
  saveConfirmationStatus: 'confirmed',
  recoveryRecordStatus: 'recovery_record_active',
  firstUploadGate: 'ready_for_encrypted_p2_upload',
  requiredPrerequisites: [],
  errorCodes: [],
);

const managerReadyRecoveryRestoreReadiness = RecoveryRestoreReadiness(
  status: 'recovery_restore_ready',
  blocker: 'none',
  entryActionStatus: 'available',
  codeInputStatus: 'available',
  recoveryRecordLookupStatus: 'available',
  attemptLimitStatus: 'available',
  deviceRegistrationStatus: 'ready_after_recovery_success',
  errorCodes: [],
);

const managerReadyDeviceJoinReadiness = DeviceJoinReadiness(
  status: 'device_join_ready',
  blocker: 'none',
  entryActionStatus: 'available',
  joinRequestStatus: JoinRequestStatus.authorized,
  shortCodeVerificationStatus: 'verified',
  authorizationPackageStatus: 'authorization_package_ready',
  authorizationPackagePreconditions: 'satisfied',
  errorCodes: [],
);

const managerReadyDeviceRevocationReadiness = DeviceRevocationReadiness(
  status: 'device_revocation_ready',
  blocker: 'none',
  entryActionStatus: 'available',
  revokeDeviceStatus: 'available',
  activeDeviceRequirement: 'satisfied',
  lostDeviceRiskNotice: 'acknowledged',
  keyEpochStatus: 'key_epoch_ready',
  errorCodes: [],
);

const managerClosedRecoveryEntryGate = RecoveryEntryGate(
  status: RecoveryEntryStatus.flowClosed,
  blocker: 'recovery_code_flow_closed',
  canGenerateCode: false,
  canRestoreDevice: false,
  requiresSaveConfirmation: false,
  saveConfirmationRequirement: 'required_before_first_upload',
  recoveryRecordStatus: 'recovery_record_not_created',
  recoveryRecordBlocker: 'recovery_record_creation_closed',
  firstUploadGate: 'blocked_until_recovery_code_saved',
  readinessBlockers: [
    'recovery_code_flow_closed',
    'recovery_record_not_created',
    'recovery_code_save_confirmation_required',
  ],
  setupReadiness: managerClosedRecoverySetupReadiness,
  restoreReadiness: managerClosedRecoveryRestoreReadiness,
);

const managerClosedDeviceAuthorizationEntryGate = DeviceAuthorizationEntryGate(
  status: DeviceAuthorizationEntryStatus.flowClosed,
  blocker: 'device_authorization_flow_closed',
  joinRequestStatus: JoinRequestStatus.unavailable,
  canCreateJoinRequest: false,
  canApproveJoinRequest: false,
  canRevokeDevice: false,
  authorizationPackageStatus: 'authorization_package_not_created',
  authorizationPackageBlocker: 'authorization_package_prerequisites_blocked',
  authorizationPackagePreconditions:
      'active_existing_device_required, join_request_pending_required, short_code_match_required',
  lostDeviceRiskNotice: 'lost_device_prior_material_not_recallable',
  keyEpochStatus: 'key_epoch_rotation_not_started',
  readinessBlockers: [
    'device_authorization_flow_closed',
    'join_request_unavailable',
    'authorization_package_prerequisites_blocked',
    'device_revocation_flow_closed',
    'lost_device_risk_notice_required',
    'key_epoch_rotation_not_started',
  ],
  joinReadiness: managerClosedDeviceJoinReadiness,
  revocationReadiness: managerClosedDeviceRevocationReadiness,
);
