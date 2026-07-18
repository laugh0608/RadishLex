import Cocoa
import Darwin
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationWillFinishLaunching(_ notification: Notification) {
    super.applicationWillFinishLaunching(notification)
    guard CommandLine.arguments.contains("--radishlex-apple-p256-product-smoke") else {
      return
    }
    let result = RadishLexAppleP256ProductSmoke.run()
    fputs(result.safeLogLine + "\n", stderr)
    fflush(stderr)
    exit(result.passed ? EXIT_SUCCESS : EXIT_FAILURE)
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}

private struct RadishLexAppleP256SmokeSummary {
  var version: UInt32 = 0
  var result: UInt32 = 255
  var compiled: UInt32 = 0
  var runtimeAvailable: UInt32 = 0
  var canCreateSigningKeys: UInt32 = 0
  var canSign: UInt32 = 0
  var productQualified: UInt32 = 0
  var userSyncEnabled: UInt32 = 0
  var exportable: UInt32 = 0
  var hardwareBacked: UInt32 = 0
  var userPresenceRequired: UInt32 = 0
  var backupMigratable: UInt32 = 0
  var created: UInt32 = 0
  var reloaded: UInt32 = 0
  var rustVerified: UInt32 = 0
  var goVerified: UInt32 = 0
  var deleted: UInt32 = 0
  var missingConfirmed: UInt32 = 0
  var failClosed: UInt32 = 0
  var cleanupAttempted: UInt32 = 0

  init() {}

  init(words: [UInt32]) {
    precondition(words.count == 20)
    version = words[0]
    result = words[1]
    compiled = words[2]
    runtimeAvailable = words[3]
    canCreateSigningKeys = words[4]
    canSign = words[5]
    productQualified = words[6]
    userSyncEnabled = words[7]
    exportable = words[8]
    hardwareBacked = words[9]
    userPresenceRequired = words[10]
    backupMigratable = words[11]
    created = words[12]
    reloaded = words[13]
    rustVerified = words[14]
    goVerified = words[15]
    deleted = words[16]
    missingConfirmed = words[17]
    failClosed = words[18]
    cleanupAttempted = words[19]
  }
}

private struct RadishLexAppleP256ProductSmokeResult {
  let summary: RadishLexAppleP256SmokeSummary

  var passed: Bool {
    summary.result == 0
  }

  var safeLogLine: String {
    "RadishLex Apple P-256 product smoke" +
      " result=\(summary.result)" +
      " compiled=\(summary.compiled)" +
      " runtime=\(summary.runtimeAvailable)" +
      " product_qualified=\(summary.productQualified)" +
      " user_sync_enabled=\(summary.userSyncEnabled)" +
      " created=\(summary.created)" +
      " reloaded=\(summary.reloaded)" +
      " rust_verified=\(summary.rustVerified)" +
      " go_verified=\(summary.goVerified)" +
      " deleted=\(summary.deleted)" +
      " missing=\(summary.missingConfirmed)" +
      " fail_closed=\(summary.failClosed)" +
      " cleanup_attempted=\(summary.cleanupAttempted)"
  }
}

private enum RadishLexAppleP256ProductSmoke {
  private typealias SmokeFunction = @convention(c) (
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
  ) -> UInt32

  static func run() -> RadishLexAppleP256ProductSmokeResult {
    var summary = RadishLexAppleP256SmokeSummary()
    guard
      ProcessInfo.processInfo.environment[
        "RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE"
      ] == "1",
      let goServerDirectory = ProcessInfo.processInfo.environment[
        "RADISHLEX_MANAGER_APPLE_P256_GO_SERVER_DIR"
      ],
      !goServerDirectory.isEmpty,
      let frameworks = Bundle.main.privateFrameworksURL
    else {
      summary.result = 1
      return RadishLexAppleP256ProductSmokeResult(summary: summary)
    }

    let library = frameworks.appendingPathComponent(
      "libradishlex_ime_ffi.dylib"
    )
    guard let handle = dlopen(library.path, RTLD_NOW | RTLD_LOCAL) else {
      summary.result = 2
      return RadishLexAppleP256ProductSmokeResult(summary: summary)
    }
    defer { dlclose(handle) }

    guard let symbol = dlsym(handle, "radishlex_apple_p256_product_smoke") else {
      summary.result = 2
      return RadishLexAppleP256ProductSmokeResult(summary: summary)
    }
    let smoke = unsafeBitCast(symbol, to: SmokeFunction.self)
    var words = [UInt32](repeating: 0, count: 20)
    let result = goServerDirectory.withCString { directory in
      words.withUnsafeMutableBufferPointer { buffer in
        smoke(directory, UnsafeMutableRawPointer(buffer.baseAddress))
      }
    }
    summary = RadishLexAppleP256SmokeSummary(words: words)
    if result != summary.result {
      summary.result = 255
    }
    return RadishLexAppleP256ProductSmokeResult(summary: summary)
  }
}
