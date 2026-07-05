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
}

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
);
