import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';

Map<String, Object?> readySyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'ffi_native_readiness',
    'recovery_setup': {
      'status': 'recovery_setup_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'generated_code_status': 'generated_once',
      'save_confirmation_status': 'confirmed',
      'recovery_record_status': 'recovery_record_active',
      'first_upload_gate': 'ready_for_encrypted_p2_upload',
      'required_prerequisites': <String>[],
      'error_codes': <String>[],
    },
    'recovery_restore': {
      'status': 'recovery_restore_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'code_input_status': 'validated',
      'recovery_record_lookup_status': 'recovery_record_active',
      'attempt_limit_status': 'available',
      'device_registration_status': 'ready_after_recovery_success',
      'error_codes': <String>[],
    },
    'device_join': {
      'status': 'device_join_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'join_request_status': 'join_request_authorized',
      'short_code_verification_status': 'verified',
      'authorization_package_status': 'authorization_package_ready',
      'authorization_package_preconditions': 'satisfied',
      'error_codes': <String>[],
    },
    'device_revocation': {
      'status': 'device_revocation_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_ready',
      'error_codes': <String>[],
    },
  };
}

Map<String, Object?> partiallyBlockedSyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'fixture_readiness',
    'recovery_setup': {
      'status': 'recovery_setup_blocked',
      'blocker': 'recovery_record_missing',
      'entry_action_status': 'blocked_by_recovery',
      'generated_code_status': 'not_generated',
      'save_confirmation_status': 'required_before_first_upload',
      'recovery_record_status': 'recovery_record_missing',
      'first_upload_gate': 'blocked_until_recovery_record_active',
      'required_prerequisites': [
        'platform_private_key_backend_ready',
        'release_deployment_evidence_summary_required',
        'explicit_user_start_required',
      ],
      'error_codes': ['recovery_record_missing', 'network_unreachable'],
    },
    'recovery_restore': {
      'status': 'recovery_restore_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'code_input_status': 'available',
      'recovery_record_lookup_status': 'recovery_record_active',
      'attempt_limit_status': 'available',
      'device_registration_status': 'ready_after_recovery_success',
      'error_codes': <String>[],
    },
    'device_join': {
      'status': 'device_join_blocked',
      'blocker': 'join_request_expired',
      'entry_action_status': 'blocked_by_recovery',
      'join_request_status': 'join_request_expired',
      'short_code_verification_status': 'short_code_match_required',
      'authorization_package_status': 'authorization_package_blocked',
      'authorization_package_preconditions': [
        'join_request_pending_required',
        'short_code_match_required',
      ],
      'error_codes': ['join_request_expired', 'authorization_rejected'],
    },
    'device_revocation': {
      'status': 'device_revocation_ready',
      'blocker': 'none',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_ready',
      'error_codes': <String>[],
    },
  };
}

Map<String, Object?> unsafeSyncReadinessBridgeJson() {
  return {
    'format': managerSyncReadinessBridgeSummaryFormat,
    'redaction_policy': managerSyncReadinessBridgeRedactionPolicy,
    'source': 'ffi_native_readiness_with_token=secret-token',
    'token': 'secret-token',
    'server_endpoint': 'https://user:secret-token@sync.example.invalid',
    'request_body': '{"recovery_code":"RADISHLEX-RECOVERY-CODE-SECRET"}',
    'recovery_setup': {
      'status': 'native_status:secret-token',
      'blocker': 'recovery_record_missing',
      'entry_action_status': 'available',
      'generated_code_status': 'RADISHLEX-RECOVERY-CODE-SECRET',
      'save_confirmation_status': '/synthetic/private/recovery.txt',
      'recovery_record_status': 'recovery_record_missing',
      'first_upload_gate': 'wrapped_material_bytes=abcdef',
      'required_prerequisites': [
        'platform_private_key_backend_ready',
        'wrapped_material_bytes=abcdef',
        42,
      ],
      'error_codes': [
        'recovery_record_missing',
        'signature_bytes=abcdef',
        false,
      ],
    },
    'recovery_restore': {
      'status': 'recovery_restore_blocked',
      'blocker': 'recovery_code_invalid',
      'entry_action_status': 'available',
      'code_input_status': 'RADISHLEX-RECOVERY-CODE-SECRET',
      'recovery_record_lookup_status': 'recovery_record_missing',
      'attempt_limit_status': 'rate_limited',
      'device_registration_status': 'blocked_until_recovery_success',
      'error_codes': 'recovery_code_invalid, token=secret-token',
    },
    'device_join': {
      'status': 'device_join_blocked',
      'blocker': 'join_request_expired',
      'entry_action_status': 'available',
      'join_request_status': 'join_request_pending short_code=123456',
      'short_code_verification_status': 'short_code=123456',
      'authorization_package_status': 'authorization_package_blocked',
      'authorization_package_preconditions':
          'active_existing_device_required, /synthetic/private/join.txt',
      'error_codes': ['network_unreachable', 'payload_bytes=abcdef'],
    },
    'device_revocation': {
      'status': 'device_revocation_blocked',
      'blocker': 'key_epoch_rotation_required',
      'entry_action_status': 'available',
      'revoke_device_status': 'available',
      'active_device_requirement': 'satisfied',
      'lost_device_risk_notice': 'acknowledged',
      'key_epoch_status': 'key_epoch_rotation_required',
      'error_codes': ['key_epoch_rotation_required', 'private_key=abcdef'],
    },
  };
}
