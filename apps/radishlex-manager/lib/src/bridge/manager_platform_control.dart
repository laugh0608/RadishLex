import 'manager_bridge.dart';

class ManagerProductPaths {
  const ManagerProductPaths({
    required this.userDbPath,
    required this.settingsFilePath,
    required this.nativeLibraryPath,
  });

  final String userDbPath;
  final String settingsFilePath;
  final String nativeLibraryPath;

  factory ManagerProductPaths.fromPlatformValue(Object? value) {
    if (value is! Map) {
      throw const ManagerPlatformException(
        code: 'platform_paths_invalid',
        message: 'platform runtime path response is not a map',
      );
    }
    final userDbPath = _requiredPath(value, 'userDbPath');
    final settingsFilePath = _requiredPath(value, 'settingsFilePath');
    final nativeLibraryPath = _requiredPath(value, 'nativeLibraryPath');
    final nativeLibraryName = nativeLibraryPath.split('/').last;
    if (nativeLibraryName != 'libradishlex_ime_ffi.dylib' &&
        nativeLibraryName != 'libradishlex_ime_ffi.so') {
      throw const ManagerPlatformException(
        code: 'platform_paths_invalid',
        message: 'platform native library path has an unexpected filename',
      );
    }
    return ManagerProductPaths(
      userDbPath: userDbPath,
      settingsFilePath: settingsFilePath,
      nativeLibraryPath: nativeLibraryPath,
    );
  }
}

class ManagerPrivacyModeState {
  const ManagerPrivacyModeState({required this.present, required this.enabled});

  final bool present;
  final bool enabled;

  factory ManagerPrivacyModeState.fromPlatformValue(Object? value) {
    if (value is! Map ||
        value['present'] is! bool ||
        value['enabled'] is! bool) {
      throw const ManagerPlatformException(
        code: 'privacy_read_failed',
        message: 'platform privacy setting returned an invalid value',
      );
    }
    final present = value['present'] as bool;
    final enabled = value['enabled'] as bool;
    if (!present && enabled) {
      throw const ManagerPlatformException(
        code: 'privacy_read_failed',
        message: 'platform privacy setting returned an inconsistent value',
      );
    }
    return ManagerPrivacyModeState(present: present, enabled: enabled);
  }

  Map<String, bool> toPlatformValue() => {
    'present': present,
    'enabled': enabled,
  };
}

abstract interface class ManagerPlatformControl {
  Future<ManagerProductPaths> resolveProductPaths();

  Future<ManagerPrivacyModeState> readPrivacyModeState();

  Future<void> writePrivacyMode(bool enabled);

  Future<void> restorePrivacyModeState(ManagerPrivacyModeState state);

  Future<void> secureLocalFiles();
}

class ManagerPlatformException implements ManagerBridgeFailure {
  const ManagerPlatformException({required this.code, required this.message});

  @override
  int get statusCode => 2;

  @override
  final String code;

  @override
  final String message;
}

String _requiredPath(Map<dynamic, dynamic> value, String key) {
  final path = value[key];
  if (path is! String || path.trim().isEmpty || !path.startsWith('/')) {
    throw ManagerPlatformException(
      code: 'platform_paths_invalid',
      message: '$key is not an absolute path',
    );
  }
  return path;
}
