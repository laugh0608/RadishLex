import Cocoa
import Darwin
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationWillFinishLaunching(_ notification: Notification) {
    super.applicationWillFinishLaunching(notification)
    guard RadishLexAppleP256ProductSmoke.isRequested(arguments: CommandLine.arguments) else {
      return
    }
    guard
      let scenario = RadishLexAppleP256ProductSmoke.scenario(
        arguments: CommandLine.arguments
      )
    else {
      fputs("RadishLex Apple P-256 product smoke result=1 scenario=invalid\n", stderr)
      fflush(stderr)
      exit(EXIT_FAILURE)
    }
    let result = RadishLexAppleP256ProductSmoke.run(scenario: scenario)
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
  var scenario: UInt32 = 0
  var errorCategory: UInt32 = 0
  var errorDetail: UInt32 = 0
  var platformStatus: Int32 = 0
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
  var expectedFailureConfirmed: UInt32 = 0
  var cleanupRequired: UInt32 = 0
  var cleanupAttempted: UInt32 = 0

  init() {}

  init(words: [UInt32]) {
    precondition(words.count == 26)
    version = words[0]
    result = words[1]
    scenario = words[2]
    errorCategory = words[3]
    errorDetail = words[4]
    platformStatus = Int32(bitPattern: words[5])
    compiled = words[6]
    runtimeAvailable = words[7]
    canCreateSigningKeys = words[8]
    canSign = words[9]
    productQualified = words[10]
    userSyncEnabled = words[11]
    exportable = words[12]
    hardwareBacked = words[13]
    userPresenceRequired = words[14]
    backupMigratable = words[15]
    created = words[16]
    reloaded = words[17]
    rustVerified = words[18]
    goVerified = words[19]
    deleted = words[20]
    missingConfirmed = words[21]
    failClosed = words[22]
    expectedFailureConfirmed = words[23]
    cleanupRequired = words[24]
    cleanupAttempted = words[25]
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
      " scenario=\(summary.scenario)" +
      " error_category=\(summary.errorCategory)" +
      " error_detail=\(summary.errorDetail)" +
      " platform_status=\(summary.platformStatus)" +
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
      " expected_failure=\(summary.expectedFailureConfirmed)" +
      " cleanup_required=\(summary.cleanupRequired)" +
      " cleanup_attempted=\(summary.cleanupAttempted)"
  }
}

private enum RadishLexAppleP256ProductSmoke {
  private static let scenarios: [String: UInt32] = [
    "--radishlex-apple-p256-product-smoke": 0,
    "--radishlex-apple-p256-denied-probe": 1,
    "--radishlex-apple-p256-locked-prepare": 2,
    "--radishlex-apple-p256-locked-probe": 3,
    "--radishlex-apple-p256-locked-cleanup": 4,
  ]

  private typealias SmokeFunction = @convention(c) (
    UInt32,
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
  ) -> UInt32

  static func isRequested(arguments: [String]) -> Bool {
    arguments.contains { $0.hasPrefix("--radishlex-apple-p256-") }
  }

  static func scenario(arguments: [String]) -> UInt32? {
    let requested = arguments.filter { $0.hasPrefix("--radishlex-apple-p256-") }
    guard requested.count == 1 else {
      return nil
    }
    return scenarios[requested[0]]
  }

  static func run(scenario: UInt32) -> RadishLexAppleP256ProductSmokeResult {
    var summary = RadishLexAppleP256SmokeSummary()
    summary.scenario = scenario
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
    var words = [UInt32](repeating: 0, count: 26)
    let result = goServerDirectory.withCString { directory in
      words.withUnsafeMutableBufferPointer { buffer in
        smoke(scenario, directory, UnsafeMutableRawPointer(buffer.baseAddress))
      }
    }
    summary = RadishLexAppleP256SmokeSummary(words: words)
    if result != summary.result {
      summary.result = 255
    }
    return RadishLexAppleP256ProductSmokeResult(summary: summary)
  }
}
