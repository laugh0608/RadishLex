import 'dart:io' show Platform;

import 'ffi_manager_bridge.dart';
import 'fixture_manager_bridge.dart';
import 'manager_bridge.dart';

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
  } on Object {
    return FixtureManagerBridge();
  }
}
