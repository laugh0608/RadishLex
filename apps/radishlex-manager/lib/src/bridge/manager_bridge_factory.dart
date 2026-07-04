import 'dart:io' show Platform;

import 'ffi_manager_bridge.dart';
import 'fixture_manager_bridge.dart';
import 'manager_bridge.dart';
import '../models/manager_models.dart';

ManagerBridge createDefaultManagerBridge({Map<String, String>? environment}) {
  final env = environment ?? Platform.environment;
  final dbPath = env['RADISHLEX_MANAGER_DB']?.trim();
  if (dbPath == null || dbPath.isEmpty) {
    return FixtureManagerBridge();
  }

  try {
    return FfiManagerBridge(
      dbPath: dbPath,
      libraryPath: env['RADISHLEX_MANAGER_FFI_LIBRARY'],
      serverEndpoint: env['RADISHLEX_MANAGER_SYNC_SERVER'],
    );
  } on Object catch (error) {
    return FixtureManagerBridge.withDiagnostics(
      diagnostics: ManagerRuntimeDiagnostics(
        bridgeMode: 'fixture_fallback',
        userDb: 'RADISHLEX_MANAGER_DB configured',
        nativeLibrary: _nativeLibraryStatus(env),
        syncEndpoint: _syncEndpointStatus(env),
        lastErrorCode: _factoryFailureCode(error),
      ),
    );
  }
}

String _nativeLibraryStatus(Map<String, String> environment) {
  final libraryPath = environment['RADISHLEX_MANAGER_FFI_LIBRARY']?.trim();
  return libraryPath == null || libraryPath.isEmpty
      ? 'default dynamic library lookup failed'
      : 'RADISHLEX_MANAGER_FFI_LIBRARY load failed';
}

String _syncEndpointStatus(Map<String, String> environment) {
  final endpoint = environment['RADISHLEX_MANAGER_SYNC_SERVER']?.trim();
  return endpoint == null || endpoint.isEmpty
      ? 'RADISHLEX_MANAGER_SYNC_SERVER not configured'
      : 'RADISHLEX_MANAGER_SYNC_SERVER configured';
}

String _factoryFailureCode(Object error) {
  if (error is ManagerBridgeFailure) {
    return error.code;
  }
  return 'ffi_library_load_failed';
}
