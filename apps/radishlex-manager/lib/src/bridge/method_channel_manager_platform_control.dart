import 'package:flutter/services.dart';

import 'manager_platform_control.dart';

class MethodChannelManagerPlatformControl implements ManagerPlatformControl {
  MethodChannelManagerPlatformControl({MethodChannel? channel})
    : _channel = channel ?? const MethodChannel(_channelName);

  static const _channelName = 'dev.radishlex.manager/runtime';
  final MethodChannel _channel;

  @override
  Future<ManagerProductPaths> resolveProductPaths() async {
    try {
      final value = await _channel.invokeMethod<Object?>('resolveProductPaths');
      return ManagerProductPaths.fromPlatformValue(value);
    } on ManagerPlatformException {
      rethrow;
    } on PlatformException catch (error) {
      throw ManagerPlatformException(
        code: error.code.isEmpty ? 'platform_paths_unavailable' : error.code,
        message: 'macOS product paths are unavailable',
      );
    } on MissingPluginException {
      throw const ManagerPlatformException(
        code: 'platform_bridge_unavailable',
        message: 'macOS runtime bridge is unavailable',
      );
    }
  }

  @override
  Future<ManagerPrivacyModeState> readPrivacyModeState() async {
    try {
      return ManagerPrivacyModeState.fromPlatformValue(
        await _channel.invokeMethod<Object?>('readPrivacyModeState'),
      );
    } on ManagerPlatformException {
      rethrow;
    } on PlatformException {
      throw const ManagerPlatformException(
        code: 'privacy_read_failed',
        message: 'macOS privacy preference could not be read',
      );
    }
  }

  @override
  Future<void> writePrivacyMode(bool enabled) async {
    try {
      final state = ManagerPrivacyModeState.fromPlatformValue(
        await _channel.invokeMethod<Object?>('writePrivacyMode', {
          'enabled': enabled,
        }),
      );
      if (!state.present || state.enabled != enabled) {
        throw const ManagerPlatformException(
          code: 'privacy_write_failed',
          message: 'macOS privacy preference read-back did not match',
        );
      }
    } on ManagerPlatformException {
      rethrow;
    } on PlatformException {
      throw const ManagerPlatformException(
        code: 'privacy_write_failed',
        message: 'macOS privacy preference could not be updated',
      );
    }
  }

  @override
  Future<void> restorePrivacyModeState(ManagerPrivacyModeState state) async {
    try {
      final restored = ManagerPrivacyModeState.fromPlatformValue(
        await _channel.invokeMethod<Object?>(
          'restorePrivacyModeState',
          state.toPlatformValue(),
        ),
      );
      if (restored.present != state.present ||
          restored.enabled != state.enabled) {
        throw const ManagerPlatformException(
          code: 'privacy_rollback_failed',
          message: 'macOS privacy preference rollback did not match',
        );
      }
    } on ManagerPlatformException {
      rethrow;
    } on PlatformException {
      throw const ManagerPlatformException(
        code: 'privacy_rollback_failed',
        message: 'macOS privacy preference could not be restored',
      );
    }
  }

  @override
  Future<void> secureLocalFiles() async {
    try {
      await _channel.invokeMethod<void>('secureLocalFiles');
    } on PlatformException {
      throw const ManagerPlatformException(
        code: 'local_file_permissions_failed',
        message: 'manager local file permissions could not be enforced',
      );
    }
  }
}
