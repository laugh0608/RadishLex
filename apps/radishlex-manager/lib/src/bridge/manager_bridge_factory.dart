import 'ffi_manager_bridge.dart';
import 'fixture_manager_bridge.dart';
import 'manager_bridge.dart';
import 'manager_platform_control.dart';
import 'method_channel_manager_platform_control.dart';
import 'unavailable_manager_bridge.dart';

enum ManagerRuntimeMode {
  product,
  demo;

  static ManagerRuntimeMode? parse(String value) {
    switch (value.trim().toLowerCase()) {
      case 'product':
        return ManagerRuntimeMode.product;
      case 'demo':
        return ManagerRuntimeMode.demo;
      default:
        return null;
    }
  }
}

class ManagerBootstrap {
  const ManagerBootstrap({required this.mode, required this.bridge});

  final ManagerRuntimeMode mode;
  final ManagerBridge bridge;
}

Future<ManagerBootstrap> createDefaultManagerBootstrap({
  String mode = const String.fromEnvironment(
    'RADISHLEX_MANAGER_MODE',
    defaultValue: 'product',
  ),
  ManagerPlatformControl? platformControl,
}) async {
  final runtimeMode = ManagerRuntimeMode.parse(mode);
  if (runtimeMode == ManagerRuntimeMode.demo) {
    return ManagerBootstrap(
      mode: ManagerRuntimeMode.demo,
      bridge: FixtureManagerBridge(),
    );
  }
  if (runtimeMode == null) {
    return const ManagerBootstrap(
      mode: ManagerRuntimeMode.product,
      bridge: UnavailableManagerBridge(
        ManagerStartupException(
          code: 'runtime_mode_invalid',
          message: 'manager runtime mode must be product or demo',
        ),
      ),
    );
  }

  final platform = platformControl ?? MethodChannelManagerPlatformControl();
  try {
    final paths = await platform.resolveProductPaths();
    return ManagerBootstrap(
      mode: ManagerRuntimeMode.product,
      bridge: FfiManagerBridge(
        dbPath: paths.userDbPath,
        libraryPath: paths.nativeLibraryPath,
        settingsFilePath: paths.settingsFilePath,
        platformControl: platform,
      ),
    );
  } on ManagerBridgeFailure catch (failure) {
    return ManagerBootstrap(
      mode: ManagerRuntimeMode.product,
      bridge: UnavailableManagerBridge(failure),
    );
  } on Object {
    return const ManagerBootstrap(
      mode: ManagerRuntimeMode.product,
      bridge: UnavailableManagerBridge(
        ManagerStartupException(
          code: 'ffi_library_load_failed',
          message: 'bundled RadishLex native library could not be loaded',
        ),
      ),
    );
  }
}

class ManagerStartupException implements ManagerBridgeFailure {
  const ManagerStartupException({required this.code, required this.message});

  @override
  int get statusCode => 2;

  @override
  final String code;

  @override
  final String message;
}
