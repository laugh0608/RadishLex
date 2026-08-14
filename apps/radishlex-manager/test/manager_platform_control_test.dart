import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/bridge/manager_platform_control.dart';
import 'package:radishlex_manager/src/bridge/method_channel_manager_platform_control.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('method channel resolves product paths and privacy read-back', () async {
    const channel = MethodChannel('test.radishlex.manager.runtime');
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          switch (call.method) {
            case 'resolveProductPaths':
              return <String, Object?>{
                'userDbPath':
                    '/Users/test/Library/Application Support/RadishLex/userdb.sqlite3',
                'settingsFilePath':
                    '/Users/test/Library/Application Support/RadishLex/manager-settings.json',
                'nativeLibraryPath':
                    '/Applications/RadishLex Manager.app/Contents/Frameworks/libradishlex_ime_ffi.dylib',
              };
            case 'readPrivacyModeState':
              return <String, bool>{'present': false, 'enabled': false};
            case 'writePrivacyMode':
              return <String, bool>{
                'present': true,
                'enabled':
                    (call.arguments as Map<Object?, Object?>)['enabled']!
                        as bool,
              };
            case 'restorePrivacyModeState':
              return call.arguments;
            case 'secureLocalFiles':
              return null;
          }
          throw PlatformException(code: 'unexpected_method');
        });
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    final control = MethodChannelManagerPlatformControl(channel: channel);
    final paths = await control.resolveProductPaths();
    expect(paths.userDbPath, endsWith('/RadishLex/userdb.sqlite3'));
    expect(
      paths.nativeLibraryPath,
      endsWith('/Frameworks/libradishlex_ime_ffi.dylib'),
    );
    final privacy = await control.readPrivacyModeState();
    expect(privacy.present, isFalse);
    expect(privacy.enabled, isFalse);
    await control.writePrivacyMode(false);
    await control.restorePrivacyModeState(privacy);
    await control.secureLocalFiles();
    expect(calls.map((call) => call.method), [
      'resolveProductPaths',
      'readPrivacyModeState',
      'writePrivacyMode',
      'restorePrivacyModeState',
      'secureLocalFiles',
    ]);
  });

  test('product paths reject relative or unexpected native paths', () {
    expect(
      () => ManagerProductPaths.fromPlatformValue({
        'userDbPath': 'relative.sqlite3',
        'settingsFilePath': '/tmp/settings.json',
        'nativeLibraryPath': '/tmp/libradishlex_ime_ffi.dylib',
      }),
      throwsA(
        isA<ManagerPlatformException>().having(
          (failure) => failure.code,
          'code',
          'platform_paths_invalid',
        ),
      ),
    );
  });

  test('product paths accept the fixed Linux shared library basename', () {
    final paths = ManagerProductPaths.fromPlatformValue({
      'userDbPath': '/home/test/.local/share/radishlex/userdb.sqlite3',
      'settingsFilePath': '/home/test/.config/radishlex/settings.json',
      'nativeLibraryPath': '/opt/radishlex-manager/lib/libradishlex_ime_ffi.so',
    });

    expect(paths.nativeLibraryPath, endsWith('/libradishlex_ime_ffi.so'));
  });

  test('product paths reject an arbitrary shared library basename', () {
    expect(
      () => ManagerProductPaths.fromPlatformValue({
        'userDbPath': '/home/test/.local/share/radishlex/userdb.sqlite3',
        'settingsFilePath': '/home/test/.config/radishlex/settings.json',
        'nativeLibraryPath': '/opt/radishlex-manager/lib/untrusted.so',
      }),
      throwsA(isA<ManagerPlatformException>()),
    );
  });

  test('privacy state rejects enabled value without a stored key', () {
    expect(
      () => ManagerPrivacyModeState.fromPlatformValue({
        'present': false,
        'enabled': true,
      }),
      throwsA(isA<ManagerPlatformException>()),
    );
  });
}
