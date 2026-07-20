import Cocoa
import CoreFoundation
import Darwin
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  private var runtimeBridge: RadishLexManagerRuntimeBridge?

  override func awakeFromNib() {
    _ = umask(0o077)
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    runtimeBridge = RadishLexManagerRuntimeBridge(
      binaryMessenger: flutterViewController.engine.binaryMessenger
    )

    super.awakeFromNib()
  }
}

private final class RadishLexManagerRuntimeBridge {
  private static let channelName = "dev.radishlex.manager/runtime"
  private static let privacyDomain = "org.radishlex.inputmethod.macos" as CFString
  private static let privacyKey = "RadishLexPrivacyMode" as CFString
  private static let nativeLibraryName = "libradishlex_ime_ffi.dylib"

  private let channel: FlutterMethodChannel

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: Self.channelName,
      binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      self?.handle(call, result: result)
    }
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    do {
      switch call.method {
      case "resolveProductPaths":
        result(try resolveProductPaths())
      case "readPrivacyModeState":
        result(privacyModeResult(try readPrivacyModeState()))
      case "writePrivacyMode":
        guard
          let arguments = call.arguments as? [String: Any],
          let enabled = arguments["enabled"] as? Bool
        else {
          throw RuntimeBridgeFailure(
            code: "invalid_argument",
            message: "privacy mode requires a boolean enabled value"
          )
        }
        try writePrivacyMode(enabled)
        result(privacyModeResult(try readPrivacyModeState()))
      case "restorePrivacyModeState":
        guard
          let arguments = call.arguments as? [String: Any],
          let present = arguments["present"] as? Bool,
          let enabled = arguments["enabled"] as? Bool,
          present || !enabled
        else {
          throw RuntimeBridgeFailure(
            code: "invalid_argument",
            message: "privacy rollback requires a consistent prior state"
          )
        }
        try restorePrivacyModeState(PrivacyModeState(present: present, enabled: enabled))
        result(privacyModeResult(try readPrivacyModeState()))
      case "secureLocalFiles":
        try secureLocalFiles()
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    } catch let failure as RuntimeBridgeFailure {
      result(FlutterError(code: failure.code, message: failure.message, details: nil))
    } catch {
      result(
        FlutterError(
          code: "platform_bridge_error",
          message: "macOS manager runtime operation failed",
          details: nil
        )
      )
    }
  }

  private func resolveProductPaths() throws -> [String: String] {
    let paths = try productPaths()
    try secureLocalFiles()

    guard try itemType(at: paths.nativeLibrary) == S_IFREG else {
      throw RuntimeBridgeFailure(
        code: "ffi_library_load_failed",
        message: "bundled RadishLex native library is missing or invalid"
      )
    }

    return [
      "userDbPath": paths.userDb.path,
      "settingsFilePath": paths.settingsFile.path,
      "nativeLibraryPath": paths.nativeLibrary.path,
    ]
  }

  private func secureLocalFiles() throws {
    let paths = try productPaths()
    try secureSupportDirectory(paths.supportDirectory)
    try rejectInvalidLocalFile(paths.userDb)
    try rejectInvalidLocalFile(paths.settingsFile)
    if try itemType(at: paths.settingsFile) != nil {
      try FileManager.default.setAttributes(
        [.posixPermissions: 0o600],
        ofItemAtPath: paths.settingsFile.path
      )
    }
  }

  private func productPaths() throws -> ProductPaths {
    let fileManager = FileManager.default
    guard
      let applicationSupport = fileManager.urls(
        for: .applicationSupportDirectory,
        in: .userDomainMask
      ).first,
      let frameworks = Bundle.main.privateFrameworksURL
    else {
      throw RuntimeBridgeFailure(
        code: "platform_paths_unavailable",
        message: "macOS product paths are unavailable"
      )
    }
    let supportDirectory = applicationSupport
      .appendingPathComponent("RadishLex", isDirectory: true)
    return ProductPaths(
      supportDirectory: supportDirectory,
      userDb: supportDirectory.appendingPathComponent("userdb.sqlite3"),
      settingsFile: supportDirectory.appendingPathComponent("manager-settings.json"),
      nativeLibrary: frameworks.appendingPathComponent(Self.nativeLibraryName)
    )
  }

  private func secureSupportDirectory(_ directory: URL) throws {
    let fileManager = FileManager.default
    if let type = try itemType(at: directory) {
      guard type == S_IFDIR else {
        throw RuntimeBridgeFailure(
          code: "local_file_permissions_failed",
          message: "RadishLex Application Support path is invalid"
        )
      }
    } else {
      try fileManager.createDirectory(
        at: directory,
        withIntermediateDirectories: true,
        attributes: [.posixPermissions: 0o700]
      )
    }
    try fileManager.setAttributes(
      [.posixPermissions: 0o700],
      ofItemAtPath: directory.path
    )
  }

  private func rejectInvalidLocalFile(_ url: URL) throws {
    guard let type = try itemType(at: url) else {
      return
    }
    if type != S_IFREG {
      throw RuntimeBridgeFailure(
        code: "local_file_permissions_failed",
        message: "manager local data path must be a regular file"
      )
    }
  }

  private func itemType(at url: URL) throws -> mode_t? {
    var metadata = stat()
    let status = url.path.withCString { path in
      lstat(path, &metadata)
    }
    if status == 0 {
      return metadata.st_mode & S_IFMT
    }
    if errno == ENOENT {
      return nil
    }
    throw RuntimeBridgeFailure(
      code: "local_file_permissions_failed",
      message: "manager local file type could not be inspected"
    )
  }

  private func readPrivacyModeState() throws -> PrivacyModeState {
    guard let value = copyPrivacyValue() else {
      return PrivacyModeState(present: false, enabled: false)
    }
    guard CFGetTypeID(value) == CFBooleanGetTypeID(), let number = value as? NSNumber else {
      throw RuntimeBridgeFailure(
        code: "privacy_read_failed",
        message: "macOS privacy preference has an invalid type"
      )
    }
    return PrivacyModeState(present: true, enabled: number.boolValue)
  }

  private func writePrivacyMode(_ enabled: Bool) throws {
    let previousValue = copyPrivacyValue()
    let value: CFBoolean = enabled ? kCFBooleanTrue : kCFBooleanFalse
    CFPreferencesSetValue(
      Self.privacyKey,
      value,
      Self.privacyDomain,
      kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost
    )
    let synchronized = CFPreferencesSynchronize(
      Self.privacyDomain,
      kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost
    )
    if !synchronized || (try? readPrivacyModeState()) != PrivacyModeState(
      present: true,
      enabled: enabled
    ) {
      CFPreferencesSetValue(
        Self.privacyKey,
        previousValue,
        Self.privacyDomain,
        kCFPreferencesCurrentUser,
        kCFPreferencesAnyHost
      )
      _ = CFPreferencesSynchronize(
        Self.privacyDomain,
        kCFPreferencesCurrentUser,
        kCFPreferencesAnyHost
      )
      throw RuntimeBridgeFailure(
        code: "privacy_write_failed",
        message: "macOS privacy preference read-back did not match"
      )
    }
  }

  private func restorePrivacyModeState(_ state: PrivacyModeState) throws {
    let value: CFPropertyList? = state.present
      ? (state.enabled ? kCFBooleanTrue : kCFBooleanFalse)
      : nil
    CFPreferencesSetValue(
      Self.privacyKey,
      value,
      Self.privacyDomain,
      kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost
    )
    let synchronized = CFPreferencesSynchronize(
      Self.privacyDomain,
      kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost
    )
    if !synchronized || (try? readPrivacyModeState()) != state {
      throw RuntimeBridgeFailure(
        code: "privacy_rollback_failed",
        message: "macOS privacy preference rollback did not match"
      )
    }
  }

  private func privacyModeResult(_ state: PrivacyModeState) -> [String: Bool] {
    ["present": state.present, "enabled": state.enabled]
  }

  private func copyPrivacyValue() -> CFPropertyList? {
    CFPreferencesCopyValue(
      Self.privacyKey,
      Self.privacyDomain,
      kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost
    )
  }
}

private struct ProductPaths {
  let supportDirectory: URL
  let userDb: URL
  let settingsFile: URL
  let nativeLibrary: URL
}

private struct PrivacyModeState: Equatable {
  let present: Bool
  let enabled: Bool
}

private struct RuntimeBridgeFailure: Error {
  let code: String
  let message: String
}
