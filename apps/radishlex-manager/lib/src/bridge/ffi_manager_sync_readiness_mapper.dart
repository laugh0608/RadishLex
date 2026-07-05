import '../models/manager_models.dart';

const managerSyncReadinessBridgeSummaryFormat = 'manager_sync_readiness.v1';
const managerSyncReadinessBridgeRedactionPolicy =
    'summary_only_no_tokens_recovery_secret_or_payload_bytes';

ManagerSyncReadinessBridgeSnapshot managerSyncReadinessBridgeSnapshotFromJson(
  Map<String, Object?> json,
) {
  final format = _bridgeString(json, 'format', const {
    managerSyncReadinessBridgeSummaryFormat,
  }, 'unsupported_format');
  final redactionPolicy = _bridgeString(json, 'redaction_policy', const {
    managerSyncReadinessBridgeRedactionPolicy,
  }, 'unsupported_redaction_policy');
  if (format != managerSyncReadinessBridgeSummaryFormat ||
      redactionPolicy != managerSyncReadinessBridgeRedactionPolicy) {
    return _invalidBridgeSnapshot();
  }

  return ManagerSyncReadinessBridgeSnapshot(
    source: _bridgeString(
      json,
      'source',
      _bridgeReadinessSources,
      'unknown_bridge_readiness_source',
    ),
    recoverySetupReadiness: _recoverySetupReadinessFromBridge(
      _bridgeObject(json['recovery_setup']),
    ),
    recoveryRestoreReadiness: _recoveryRestoreReadinessFromBridge(
      _bridgeObject(json['recovery_restore']),
    ),
    deviceJoinReadiness: _deviceJoinReadinessFromBridge(
      _bridgeObject(json['device_join']),
    ),
    deviceRevocationReadiness: _deviceRevocationReadinessFromBridge(
      _bridgeObject(json['device_revocation']),
    ),
  );
}

RecoverySetupReadiness _recoverySetupReadinessFromBridge(
  Map<String, Object?> json,
) {
  return RecoverySetupReadiness(
    status: _bridgeString(
      json,
      'status',
      _recoverySetupStatuses,
      managerClosedRecoverySetupReadiness.status,
    ),
    blocker: _bridgeString(
      json,
      'blocker',
      _recoverySetupBlockers,
      managerClosedRecoverySetupReadiness.blocker,
    ),
    entryActionStatus: _bridgeString(
      json,
      'entry_action_status',
      _bridgeEntryActionStatuses,
      managerClosedRecoverySetupReadiness.entryActionStatus,
    ),
    generatedCodeStatus: _bridgeString(
      json,
      'generated_code_status',
      _generatedCodeStatuses,
      managerClosedRecoverySetupReadiness.generatedCodeStatus,
    ),
    saveConfirmationStatus: _bridgeString(
      json,
      'save_confirmation_status',
      _saveConfirmationStatuses,
      managerClosedRecoverySetupReadiness.saveConfirmationStatus,
    ),
    recoveryRecordStatus: _bridgeString(
      json,
      'recovery_record_status',
      _recoveryRecordStatuses,
      managerClosedRecoverySetupReadiness.recoveryRecordStatus,
    ),
    firstUploadGate: _bridgeString(
      json,
      'first_upload_gate',
      _firstUploadGateStatuses,
      managerClosedRecoverySetupReadiness.firstUploadGate,
    ),
    requiredPrerequisites: _bridgeCodeList(
      json['required_prerequisites'],
      _requiredEvidenceCodes,
      managerClosedRecoverySetupReadiness.requiredPrerequisites,
      'unexpected_bridge_required_evidence',
    ),
    errorCodes: _bridgeCodeList(
      json['error_codes'],
      _readinessErrorCodes,
      managerClosedRecoverySetupReadiness.errorCodes,
      'unexpected_bridge_error_code',
    ),
    sourceTag: 'bridge_recovery_setup_readiness',
  );
}

RecoveryRestoreReadiness _recoveryRestoreReadinessFromBridge(
  Map<String, Object?> json,
) {
  return RecoveryRestoreReadiness(
    status: _bridgeString(
      json,
      'status',
      _recoveryRestoreStatuses,
      managerClosedRecoveryRestoreReadiness.status,
    ),
    blocker: _bridgeString(
      json,
      'blocker',
      _recoveryRestoreBlockers,
      managerClosedRecoveryRestoreReadiness.blocker,
    ),
    entryActionStatus: _bridgeString(
      json,
      'entry_action_status',
      _bridgeEntryActionStatuses,
      managerClosedRecoveryRestoreReadiness.entryActionStatus,
    ),
    codeInputStatus: _bridgeString(
      json,
      'code_input_status',
      _recoveryCodeInputStatuses,
      managerClosedRecoveryRestoreReadiness.codeInputStatus,
    ),
    recoveryRecordLookupStatus: _bridgeString(
      json,
      'recovery_record_lookup_status',
      _recoveryRecordLookupStatuses,
      managerClosedRecoveryRestoreReadiness.recoveryRecordLookupStatus,
    ),
    attemptLimitStatus: _bridgeString(
      json,
      'attempt_limit_status',
      _attemptLimitStatuses,
      managerClosedRecoveryRestoreReadiness.attemptLimitStatus,
    ),
    deviceRegistrationStatus: _bridgeString(
      json,
      'device_registration_status',
      _deviceRegistrationStatuses,
      managerClosedRecoveryRestoreReadiness.deviceRegistrationStatus,
    ),
    errorCodes: _bridgeCodeList(
      json['error_codes'],
      _readinessErrorCodes,
      managerClosedRecoveryRestoreReadiness.errorCodes,
      'unexpected_bridge_error_code',
    ),
    sourceTag: 'bridge_recovery_restore_readiness',
  );
}

DeviceJoinReadiness _deviceJoinReadinessFromBridge(Map<String, Object?> json) {
  return DeviceJoinReadiness(
    status: _bridgeString(
      json,
      'status',
      _deviceJoinStatuses,
      managerClosedDeviceJoinReadiness.status,
    ),
    blocker: _bridgeString(
      json,
      'blocker',
      _deviceJoinBlockers,
      managerClosedDeviceJoinReadiness.blocker,
    ),
    entryActionStatus: _bridgeString(
      json,
      'entry_action_status',
      _bridgeEntryActionStatuses,
      managerClosedDeviceJoinReadiness.entryActionStatus,
    ),
    joinRequestStatus: _joinRequestStatusFromBridge(
      json['join_request_status'],
    ),
    shortCodeVerificationStatus: _bridgeString(
      json,
      'short_code_verification_status',
      _shortCodeVerificationStatuses,
      managerClosedDeviceJoinReadiness.shortCodeVerificationStatus,
    ),
    authorizationPackageStatus: _bridgeString(
      json,
      'authorization_package_status',
      _authorizationPackageStatuses,
      managerClosedDeviceJoinReadiness.authorizationPackageStatus,
    ),
    authorizationPackagePreconditions: managerSyncCodeSummary(
      _bridgeCodeList(
        json['authorization_package_preconditions'],
        _authorizationPackagePreconditions,
        _splitCodes(
          managerClosedDeviceJoinReadiness.authorizationPackagePreconditions,
        ),
        'unexpected_bridge_required_evidence',
      ),
    ),
    errorCodes: _bridgeCodeList(
      json['error_codes'],
      _readinessErrorCodes,
      managerClosedDeviceJoinReadiness.errorCodes,
      'unexpected_bridge_error_code',
    ),
    sourceTag: 'bridge_device_join_readiness',
  );
}

DeviceRevocationReadiness _deviceRevocationReadinessFromBridge(
  Map<String, Object?> json,
) {
  return DeviceRevocationReadiness(
    status: _bridgeString(
      json,
      'status',
      _deviceRevocationStatuses,
      managerClosedDeviceRevocationReadiness.status,
    ),
    blocker: _bridgeString(
      json,
      'blocker',
      _deviceRevocationBlockers,
      managerClosedDeviceRevocationReadiness.blocker,
    ),
    entryActionStatus: _bridgeString(
      json,
      'entry_action_status',
      _bridgeEntryActionStatuses,
      managerClosedDeviceRevocationReadiness.entryActionStatus,
    ),
    revokeDeviceStatus: _bridgeString(
      json,
      'revoke_device_status',
      _revokeDeviceStatuses,
      managerClosedDeviceRevocationReadiness.revokeDeviceStatus,
    ),
    activeDeviceRequirement: _bridgeString(
      json,
      'active_device_requirement',
      _activeDeviceRequirementStatuses,
      managerClosedDeviceRevocationReadiness.activeDeviceRequirement,
    ),
    lostDeviceRiskNotice: _bridgeString(
      json,
      'lost_device_risk_notice',
      _lostDeviceRiskStatuses,
      managerClosedDeviceRevocationReadiness.lostDeviceRiskNotice,
    ),
    keyEpochStatus: _bridgeString(
      json,
      'key_epoch_status',
      _keyEpochStatuses,
      managerClosedDeviceRevocationReadiness.keyEpochStatus,
    ),
    errorCodes: _bridgeCodeList(
      json['error_codes'],
      _readinessErrorCodes,
      managerClosedDeviceRevocationReadiness.errorCodes,
      'unexpected_bridge_error_code',
    ),
    sourceTag: 'bridge_device_revocation_readiness',
  );
}

ManagerSyncReadinessBridgeSnapshot _invalidBridgeSnapshot() {
  return ManagerSyncReadinessBridgeSnapshot(
    source: 'bridge_readiness_summary_invalid',
    recoverySetupReadiness: RecoverySetupReadiness(
      status: managerClosedRecoverySetupReadiness.status,
      blocker: managerClosedRecoverySetupReadiness.blocker,
      entryActionStatus: managerClosedRecoverySetupReadiness.entryActionStatus,
      generatedCodeStatus:
          managerClosedRecoverySetupReadiness.generatedCodeStatus,
      saveConfirmationStatus:
          managerClosedRecoverySetupReadiness.saveConfirmationStatus,
      recoveryRecordStatus:
          managerClosedRecoverySetupReadiness.recoveryRecordStatus,
      firstUploadGate: managerClosedRecoverySetupReadiness.firstUploadGate,
      requiredPrerequisites:
          managerClosedRecoverySetupReadiness.requiredPrerequisites,
      errorCodes: const ['bridge_readiness_summary_invalid'],
      sourceTag: 'bridge_recovery_setup_readiness',
    ),
    recoveryRestoreReadiness: RecoveryRestoreReadiness(
      status: managerClosedRecoveryRestoreReadiness.status,
      blocker: managerClosedRecoveryRestoreReadiness.blocker,
      entryActionStatus:
          managerClosedRecoveryRestoreReadiness.entryActionStatus,
      codeInputStatus: managerClosedRecoveryRestoreReadiness.codeInputStatus,
      recoveryRecordLookupStatus:
          managerClosedRecoveryRestoreReadiness.recoveryRecordLookupStatus,
      attemptLimitStatus:
          managerClosedRecoveryRestoreReadiness.attemptLimitStatus,
      deviceRegistrationStatus:
          managerClosedRecoveryRestoreReadiness.deviceRegistrationStatus,
      errorCodes: const ['bridge_readiness_summary_invalid'],
      sourceTag: 'bridge_recovery_restore_readiness',
    ),
    deviceJoinReadiness: DeviceJoinReadiness(
      status: managerClosedDeviceJoinReadiness.status,
      blocker: managerClosedDeviceJoinReadiness.blocker,
      entryActionStatus: managerClosedDeviceJoinReadiness.entryActionStatus,
      joinRequestStatus: managerClosedDeviceJoinReadiness.joinRequestStatus,
      shortCodeVerificationStatus:
          managerClosedDeviceJoinReadiness.shortCodeVerificationStatus,
      authorizationPackageStatus:
          managerClosedDeviceJoinReadiness.authorizationPackageStatus,
      authorizationPackagePreconditions:
          managerClosedDeviceJoinReadiness.authorizationPackagePreconditions,
      errorCodes: const ['bridge_readiness_summary_invalid'],
      sourceTag: 'bridge_device_join_readiness',
    ),
    deviceRevocationReadiness: DeviceRevocationReadiness(
      status: managerClosedDeviceRevocationReadiness.status,
      blocker: managerClosedDeviceRevocationReadiness.blocker,
      entryActionStatus:
          managerClosedDeviceRevocationReadiness.entryActionStatus,
      revokeDeviceStatus:
          managerClosedDeviceRevocationReadiness.revokeDeviceStatus,
      activeDeviceRequirement:
          managerClosedDeviceRevocationReadiness.activeDeviceRequirement,
      lostDeviceRiskNotice:
          managerClosedDeviceRevocationReadiness.lostDeviceRiskNotice,
      keyEpochStatus: managerClosedDeviceRevocationReadiness.keyEpochStatus,
      errorCodes: const ['bridge_readiness_summary_invalid'],
      sourceTag: 'bridge_device_revocation_readiness',
    ),
  );
}

Map<String, Object?> _bridgeObject(Object? value) {
  if (value is! Map) {
    return const {};
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    final key = entry.key;
    if (key is String) {
      result[key] = entry.value;
    }
  }
  return result;
}

String _bridgeString(
  Map<String, Object?> json,
  String key,
  Set<String> allowedValues,
  String fallback,
) {
  final value = json[key];
  if (value is! String) {
    return fallback;
  }
  final candidate = value.trim();
  return allowedValues.contains(candidate) ? candidate : fallback;
}

List<String> _bridgeCodeList(
  Object? value,
  Set<String> allowedValues,
  List<String> fallback,
  String unknownFallback,
) {
  if (value == null) {
    return fallback;
  }

  final codes = <String>{};
  var hasUnknown = false;
  final rawValues = value is List ? value : [value];
  for (final raw in rawValues) {
    if (raw is! String) {
      hasUnknown = true;
      continue;
    }
    for (final part in raw.split(',')) {
      final candidate = part.trim();
      if (candidate.isEmpty || candidate == 'none') {
        continue;
      }
      if (allowedValues.contains(candidate)) {
        codes.add(candidate);
      } else {
        hasUnknown = true;
      }
    }
  }
  if (hasUnknown) {
    codes.add(unknownFallback);
  }
  return codes.toList(growable: false);
}

List<String> _splitCodes(String value) {
  final summary = managerSyncCodeSummary([value]);
  return summary == 'none' ? const [] : summary.split(', ');
}

JoinRequestStatus _joinRequestStatusFromBridge(Object? value) {
  if (value is! String) {
    return managerClosedDeviceJoinReadiness.joinRequestStatus;
  }
  switch (value.trim()) {
    case 'join_request_pending':
      return JoinRequestStatus.pending;
    case 'join_request_expired':
      return JoinRequestStatus.expired;
    case 'join_request_authorized':
      return JoinRequestStatus.authorized;
    default:
      return JoinRequestStatus.unavailable;
  }
}

const _bridgeReadinessSources = {
  managerSyncReadinessBridgeSourceDefault,
  'ffi_native_readiness',
  'fixture_readiness',
  'injected_test_readiness',
};

const _bridgeEntryActionStatuses = {
  'read_only_current_phase',
  'available',
  'blocked_by_backend',
  'blocked_by_deployment',
  'blocked_by_recovery',
  'closed_current_phase',
};

const _readinessErrorCodes = {
  'configuration_missing',
  'authentication_required',
  'backend_unavailable',
  'deployment_unverified',
  'recovery_code_required',
  'recovery_code_invalid',
  'recovery_record_missing',
  'recovery_record_revoked',
  'join_request_expired',
  'authorization_rejected',
  'device_revoked',
  'network_unreachable',
  'version_conflict',
  'local_data_inconsistent',
  'key_epoch_rotation_required',
  'bridge_readiness_summary_invalid',
};

const _requiredEvidenceCodes = {
  'platform_private_key_backend_ready',
  'release_deployment_evidence_summary_required',
  'explicit_user_start_required',
  'input_not_available_current_phase',
  'not_checked_current_phase',
  'blocked_until_recovery_success',
  'join_request_unavailable',
  'short_code_verification_not_started',
  'active_existing_device_required',
  'join_request_pending_required',
  'short_code_match_required',
  'lost_device_prior_material_not_recallable',
  'key_epoch_rotation_not_started',
  'satisfied',
  'available',
  'verified',
  'acknowledged',
  'key_epoch_ready',
};

const _recoverySetupStatuses = {
  'recovery_setup_flow_closed',
  'recovery_setup_ready',
  'recovery_setup_blocked',
};

const _recoverySetupBlockers = {
  'none',
  'recovery_code_generation_closed',
  'recovery_code_required',
  'recovery_record_missing',
  'recovery_record_revoked',
  'backend_unavailable',
  'deployment_unverified',
  'local_data_inconsistent',
};

const _generatedCodeStatuses = {
  'not_generated',
  'generated_once',
  'not_available_current_phase',
  'redacted_available_once',
};

const _saveConfirmationStatuses = {
  'required_before_first_upload',
  'confirmed',
  'not_required',
};

const _recoveryRecordStatuses = {
  'recovery_record_not_created',
  'recovery_record_active',
  'recovery_record_missing',
  'recovery_record_revoked',
};

const _firstUploadGateStatuses = {
  'blocked_until_recovery_code_saved',
  'blocked_until_recovery_record_active',
  'ready_for_encrypted_p2_upload',
};

const _recoveryRestoreStatuses = {
  'recovery_restore_flow_closed',
  'recovery_restore_ready',
  'recovery_restore_blocked',
};

const _recoveryRestoreBlockers = {
  'none',
  'recovery_code_input_closed',
  'recovery_code_required',
  'recovery_code_invalid',
  'recovery_record_missing',
  'recovery_record_revoked',
  'authentication_required',
  'network_unreachable',
  'local_data_inconsistent',
};

const _recoveryCodeInputStatuses = {
  'input_not_available_current_phase',
  'available',
  'validated',
  'invalid',
  'not_persisted',
};

const _recoveryRecordLookupStatuses = {
  'not_checked_current_phase',
  'available',
  'recovery_record_active',
  'recovery_record_missing',
  'recovery_record_revoked',
};

const _attemptLimitStatuses = {'not_started', 'available', 'rate_limited'};

const _deviceRegistrationStatuses = {
  'blocked_until_recovery_success',
  'ready_after_recovery_success',
  'device_registration_pending',
  'device_registration_ready',
};

const _deviceJoinStatuses = {
  'device_join_flow_closed',
  'device_join_ready',
  'device_join_blocked',
};

const _deviceJoinBlockers = {
  'none',
  'join_request_creation_closed',
  'join_request_expired',
  'authorization_rejected',
  'device_revoked',
  'backend_unavailable',
  'network_unreachable',
  'local_data_inconsistent',
};

const _shortCodeVerificationStatuses = {
  'short_code_verification_not_started',
  'short_code_match_required',
  'verified',
  'mismatch',
};

const _authorizationPackageStatuses = {
  'authorization_package_not_created',
  'authorization_package_ready',
  'authorization_package_blocked',
};

const _authorizationPackagePreconditions = {
  'active_existing_device_required',
  'join_request_pending_required',
  'short_code_match_required',
  'platform_private_key_backend_ready',
  'authorization_package_prerequisites_blocked',
  'satisfied',
};

const _deviceRevocationStatuses = {
  'device_revocation_flow_closed',
  'device_revocation_ready',
  'device_revocation_blocked',
};

const _deviceRevocationBlockers = {
  'none',
  'device_revocation_flow_closed',
  'device_revoked',
  'key_epoch_rotation_required',
  'backend_unavailable',
  'network_unreachable',
  'local_data_inconsistent',
};

const _revokeDeviceStatuses = {'closed_current_phase', 'available'};

const _activeDeviceRequirementStatuses = {
  'active_existing_device_required',
  'satisfied',
};

const _lostDeviceRiskStatuses = {
  'lost_device_prior_material_not_recallable',
  'acknowledged',
};

const _keyEpochStatuses = {
  'key_epoch_rotation_not_started',
  'key_epoch_rotation_required',
  'key_epoch_ready',
};
