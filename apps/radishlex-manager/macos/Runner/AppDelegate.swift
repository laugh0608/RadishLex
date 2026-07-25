import Cocoa
import Darwin
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationWillFinishLaunching(_ notification: Notification) {
    if RadishLexAppleSecureEnclaveKeyAgreementProductSmoke.isRequested(
      arguments: CommandLine.arguments
    ) {
      guard
        let request = RadishLexAppleSecureEnclaveKeyAgreementProductSmoke.request(
          arguments: CommandLine.arguments
        )
      else {
        fputs("RadishLex key-agreement product smoke result=3 scenario=invalid\n", stderr)
        fflush(stderr)
        exit(EXIT_FAILURE)
      }
      let result = RadishLexAppleSecureEnclaveKeyAgreementProductSmoke.run(
        request: request
      )
      fputs(result.safeLogLine + "\n", stderr)
      fflush(stderr)
      exit(result.passed ? EXIT_SUCCESS : EXIT_FAILURE)
    }
    guard RadishLexAppleP256ProductSmoke.isRequested(arguments: CommandLine.arguments) else {
      let installGate = RadishLexProductInstallStartupGate.inspect()
      guard installGate.installAllowed else {
        fputs("RadishLex install startup gate \(installGate.safeFields)\n", stderr)
        fflush(stderr)
        exit(EXIT_FAILURE)
      }
      let dataGate = RadishLexProductUpgradeStartupGate.inspect()
      guard dataGate.dataAllowed else {
        fputs("RadishLex data startup gate \(dataGate.safeFields)\n", stderr)
        fflush(stderr)
        exit(EXIT_FAILURE)
      }
      super.applicationWillFinishLaunching(notification)
      return
    }
    guard
      let request = RadishLexAppleP256ProductSmoke.request(
        arguments: CommandLine.arguments
      )
    else {
      fputs("RadishLex Apple P-256 product smoke result=1 scenario=invalid\n", stderr)
      fflush(stderr)
      exit(EXIT_FAILURE)
    }
    let result = RadishLexAppleP256ProductSmoke.run(request: request)
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

private struct RadishLexStartupGateRequest {
  let version: UInt32
  let dataRootPath: UnsafePointer<CChar>?
  let expectedOwnerID: UInt32
}

private struct RadishLexStartupGateResult {
  var version: UInt32 = 0
  var decision: UInt32 = 0
  var errorCode: UInt32 = 0
  var receiptState: UInt32 = 0

  var installAllowed: Bool {
    guard version == 1 && errorCode == 0 else { return false }
    switch decision {
    case 1, 2:
      return receiptState == 0
    case 3:
      return [10, 11, 14].contains(receiptState)
    default:
      return false
    }
  }

  var dataAllowed: Bool {
    guard version == 1 && errorCode == 0 else { return false }
    switch decision {
    case 1, 2:
      return receiptState == 0
    case 3:
      return [9, 10, 12].contains(receiptState)
    default:
      return false
    }
  }

  var safeFields: String {
    "decision=\(decision) error=\(errorCode) state=\(receiptState)"
  }
}

private enum RadishLexProductInstallStartupGate {
  static func inspect() -> RadishLexStartupGateResult {
    inspectStartupGate(symbolName: "radishlex_product_install_startup_gate")
  }
}

private enum RadishLexProductUpgradeStartupGate {
  static func inspect() -> RadishLexStartupGateResult {
    inspectStartupGate(symbolName: "radishlex_product_upgrade_startup_gate")
  }
}

private typealias RadishLexStartupGateFunction = @convention(c) (
    UnsafeRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeMutablePointer<UnsafeMutableRawPointer?>?
  ) -> UInt32

private func inspectStartupGate(symbolName: String) -> RadishLexStartupGateResult {
  var result = RadishLexStartupGateResult()
  guard
    let applicationSupport = FileManager.default.urls(
      for: .applicationSupportDirectory,
      in: .userDomainMask
    ).first,
    let frameworks = Bundle.main.privateFrameworksURL
  else { return result }
  let dataRoot = applicationSupport.appendingPathComponent("RadishLex", isDirectory: true)
  let library = frameworks.appendingPathComponent("libradishlex_ime_ffi.dylib")
  guard let handle = dlopen(library.path, RTLD_NOW | RTLD_LOCAL) else { return result }
  defer { dlclose(handle) }
  guard let symbol = symbolName.withCString({ dlsym(handle, $0) }) else { return result }
  let gate = unsafeBitCast(symbol, to: RadishLexStartupGateFunction.self)
  let status = dataRoot.path.withCString { path in
    var request = RadishLexStartupGateRequest(
      version: 1,
      dataRootPath: path,
      expectedOwnerID: geteuid()
    )
    return withUnsafePointer(to: &request) { requestPointer in
      withUnsafeMutablePointer(to: &result) { resultPointer in
        gate(
          UnsafeRawPointer(requestPointer),
          UnsafeMutableRawPointer(resultPointer),
          nil
        )
      }
    }
  }
  if status != 0 {
    result = RadishLexStartupGateResult()
  }
  return result
}

private struct RadishLexAppleKeyAgreementSmokeSummary {
  var version: UInt32 = 0
  var result: UInt32 = 255
  var scenario: UInt32 = 0
  var errorCategory: UInt32 = 0
  var errorDetail: UInt32 = 0
  var platformStatus: Int32 = 0
  var compiled: UInt32 = 0
  var runtimeQualified: UInt32 = 0
  var productQualified: UInt32 = 0
  var userSyncEnabled: UInt32 = 0
  var exportable: UInt32 = 0
  var hardwareBacked: UInt32 = 0
  var userPresenceRequired: UInt32 = 0
  var backupMigratable: UInt32 = 0
  var created: UInt32 = 0
  var reloaded: UInt32 = 0
  var publicKeyMatched: UInt32 = 0
  var sharedSecretDerived: UInt32 = 0
  var wrappedEpochVerified: UInt32 = 0
  var deleted: UInt32 = 0
  var missingConfirmed: UInt32 = 0
  var failClosed: UInt32 = 0
  var expectedFailureConfirmed: UInt32 = 0
  var cleanupRequired: UInt32 = 0
  var cleanupAttempted: UInt32 = 0

  init() {}

  init(words: [UInt32]) {
    precondition(words.count == 25)
    version = words[0]
    result = words[1]
    scenario = words[2]
    errorCategory = words[3]
    errorDetail = words[4]
    platformStatus = Int32(bitPattern: words[5])
    compiled = words[6]
    runtimeQualified = words[7]
    productQualified = words[8]
    userSyncEnabled = words[9]
    exportable = words[10]
    hardwareBacked = words[11]
    userPresenceRequired = words[12]
    backupMigratable = words[13]
    created = words[14]
    reloaded = words[15]
    publicKeyMatched = words[16]
    sharedSecretDerived = words[17]
    wrappedEpochVerified = words[18]
    deleted = words[19]
    missingConfirmed = words[20]
    failClosed = words[21]
    expectedFailureConfirmed = words[22]
    cleanupRequired = words[23]
    cleanupAttempted = words[24]
  }
}

private struct RadishLexAppleKeyAgreementSmokeRequest {
  let scenario: UInt32
}

private struct RadishLexAppleKeyAgreementSmokeResult {
  let summary: RadishLexAppleKeyAgreementSmokeSummary

  var passed: Bool { summary.result == 0 }

  var safeLogLine: String {
    "RadishLex Apple Secure Enclave key-agreement product smoke" +
      " result=\(summary.result)" +
      " scenario=\(summary.scenario)" +
      " error_category=\(summary.errorCategory)" +
      " error_detail=\(summary.errorDetail)" +
      " platform_status=\(summary.platformStatus)" +
      " compiled=\(summary.compiled)" +
      " runtime_qualified=\(summary.runtimeQualified)" +
      " product_qualified=\(summary.productQualified)" +
      " user_sync_enabled=\(summary.userSyncEnabled)" +
      " created=\(summary.created)" +
      " reloaded=\(summary.reloaded)" +
      " public_key_matched=\(summary.publicKeyMatched)" +
      " shared_secret_derived=\(summary.sharedSecretDerived)" +
      " wrapped_epoch_verified=\(summary.wrappedEpochVerified)" +
      " deleted=\(summary.deleted)" +
      " missing=\(summary.missingConfirmed)" +
      " fail_closed=\(summary.failClosed)" +
      " expected_failure=\(summary.expectedFailureConfirmed)" +
      " cleanup_required=\(summary.cleanupRequired)" +
      " cleanup_attempted=\(summary.cleanupAttempted)"
  }
}

private enum RadishLexAppleSecureEnclaveKeyAgreementProductSmoke {
  private static let prefix = "--radishlex-apple-secure-enclave-key-agreement-"
  private static let requests: [String: RadishLexAppleKeyAgreementSmokeRequest] = [
    "--radishlex-apple-secure-enclave-key-agreement-product-smoke": request(0),
    "--radishlex-apple-secure-enclave-key-agreement-denied-probe": request(1),
    "--radishlex-apple-secure-enclave-key-agreement-locked-prepare": request(2),
    "--radishlex-apple-secure-enclave-key-agreement-locked-probe": request(3),
    "--radishlex-apple-secure-enclave-key-agreement-locked-cleanup": request(4),
    "--radishlex-apple-secure-enclave-key-agreement-unsupported-probe": request(5),
  ]

  private typealias SmokeFunction = @convention(c) (
    UInt32,
    UnsafeMutableRawPointer?
  ) -> UInt32

  static func isRequested(arguments: [String]) -> Bool {
    arguments.contains { $0.hasPrefix(prefix) }
  }

  static func request(arguments: [String]) -> RadishLexAppleKeyAgreementSmokeRequest? {
    let requested = arguments.filter { $0.hasPrefix(prefix) }
    guard requested.count == 1 else { return nil }
    return requests[requested[0]]
  }

  static func run(request: RadishLexAppleKeyAgreementSmokeRequest)
    -> RadishLexAppleKeyAgreementSmokeResult
  {
    var summary = RadishLexAppleKeyAgreementSmokeSummary()
    summary.scenario = request.scenario
    guard
      ProcessInfo.processInfo.environment[
        "RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SMOKE"
      ] == "1",
      let frameworks = Bundle.main.privateFrameworksURL
    else {
      summary.result = 1
      return RadishLexAppleKeyAgreementSmokeResult(summary: summary)
    }

    let library = frameworks.appendingPathComponent("libradishlex_ime_ffi.dylib")
    guard let handle = dlopen(library.path, RTLD_NOW | RTLD_LOCAL) else {
      summary.result = 2
      return RadishLexAppleKeyAgreementSmokeResult(summary: summary)
    }
    defer { dlclose(handle) }
    guard
      let symbol = dlsym(
        handle,
        "radishlex_apple_secure_enclave_key_agreement_product_smoke"
      )
    else {
      summary.result = 2
      return RadishLexAppleKeyAgreementSmokeResult(summary: summary)
    }
    let smoke = unsafeBitCast(symbol, to: SmokeFunction.self)
    var words = [UInt32](repeating: 0, count: 25)
    let result = words.withUnsafeMutableBufferPointer { buffer in
      smoke(request.scenario, UnsafeMutableRawPointer(buffer.baseAddress))
    }
    summary = RadishLexAppleKeyAgreementSmokeSummary(words: words)
    if result != summary.result { summary.result = 255 }
    return RadishLexAppleKeyAgreementSmokeResult(summary: summary)
  }

  private static func request(_ scenario: UInt32) -> RadishLexAppleKeyAgreementSmokeRequest {
    RadishLexAppleKeyAgreementSmokeRequest(scenario: scenario)
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
  let label: String
  let summary: RadishLexAppleP256SmokeSummary

  var passed: Bool {
    summary.result == 0
  }

  var safeLogLine: String {
    label +
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

private struct RadishLexAppleP256SmokeRequest {
  let scenario: UInt32
  let environmentGate: String
  let symbol: String
  let label: String
}

private enum RadishLexAppleP256ProductSmoke {
  private static let requests: [String: RadishLexAppleP256SmokeRequest] = [
    "--radishlex-apple-p256-product-smoke": softwareDPKRequest(scenario: 0),
    "--radishlex-apple-p256-denied-probe": softwareDPKRequest(scenario: 1),
    "--radishlex-apple-p256-locked-prepare": softwareDPKRequest(scenario: 2),
    "--radishlex-apple-p256-locked-probe": softwareDPKRequest(scenario: 3),
    "--radishlex-apple-p256-locked-cleanup": softwareDPKRequest(scenario: 4),
    "--radishlex-apple-secure-enclave-p256-product-smoke":
      secureEnclaveRequest(scenario: 0),
    "--radishlex-apple-secure-enclave-p256-denied-probe":
      secureEnclaveRequest(scenario: 1),
    "--radishlex-apple-secure-enclave-p256-locked-prepare":
      secureEnclaveRequest(scenario: 2),
    "--radishlex-apple-secure-enclave-p256-locked-probe":
      secureEnclaveRequest(scenario: 3),
    "--radishlex-apple-secure-enclave-p256-locked-cleanup":
      secureEnclaveRequest(scenario: 4),
    "--radishlex-apple-secure-enclave-p256-unsupported-probe":
      secureEnclaveRequest(scenario: 5),
  ]

  private typealias SmokeFunction = @convention(c) (
    UInt32,
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
  ) -> UInt32

  static func isRequested(arguments: [String]) -> Bool {
    arguments.contains {
      $0.hasPrefix("--radishlex-apple-p256-")
        || $0.hasPrefix("--radishlex-apple-secure-enclave-p256-")
    }
  }

  static func request(arguments: [String]) -> RadishLexAppleP256SmokeRequest? {
    let requested = arguments.filter {
      $0.hasPrefix("--radishlex-apple-p256-")
        || $0.hasPrefix("--radishlex-apple-secure-enclave-p256-")
    }
    guard requested.count == 1 else {
      return nil
    }
    return requests[requested[0]]
  }

  static func run(request: RadishLexAppleP256SmokeRequest)
    -> RadishLexAppleP256ProductSmokeResult
  {
    var summary = RadishLexAppleP256SmokeSummary()
    summary.scenario = request.scenario
    guard
      ProcessInfo.processInfo.environment[request.environmentGate] == "1",
      let goServerDirectory = ProcessInfo.processInfo.environment[
        "RADISHLEX_MANAGER_APPLE_P256_GO_SERVER_DIR"
      ],
      !goServerDirectory.isEmpty,
      let frameworks = Bundle.main.privateFrameworksURL
    else {
      summary.result = 1
      return RadishLexAppleP256ProductSmokeResult(label: request.label, summary: summary)
    }

    let library = frameworks.appendingPathComponent(
      "libradishlex_ime_ffi.dylib"
    )
    guard let handle = dlopen(library.path, RTLD_NOW | RTLD_LOCAL) else {
      summary.result = 2
      return RadishLexAppleP256ProductSmokeResult(label: request.label, summary: summary)
    }
    defer { dlclose(handle) }

    guard let symbol = dlsym(handle, request.symbol) else {
      summary.result = 2
      return RadishLexAppleP256ProductSmokeResult(label: request.label, summary: summary)
    }
    let smoke = unsafeBitCast(symbol, to: SmokeFunction.self)
    var words = [UInt32](repeating: 0, count: 26)
    let result = goServerDirectory.withCString { directory in
      words.withUnsafeMutableBufferPointer { buffer in
        smoke(request.scenario, directory, UnsafeMutableRawPointer(buffer.baseAddress))
      }
    }
    summary = RadishLexAppleP256SmokeSummary(words: words)
    if result != summary.result {
      summary.result = 255
    }
    return RadishLexAppleP256ProductSmokeResult(label: request.label, summary: summary)
  }

  private static func softwareDPKRequest(scenario: UInt32)
    -> RadishLexAppleP256SmokeRequest
  {
    RadishLexAppleP256SmokeRequest(
      scenario: scenario,
      environmentGate: "RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE",
      symbol: "radishlex_apple_p256_product_smoke",
      label: "RadishLex Apple P-256 product smoke"
    )
  }

  private static func secureEnclaveRequest(scenario: UInt32)
    -> RadishLexAppleP256SmokeRequest
  {
    RadishLexAppleP256SmokeRequest(
      scenario: scenario,
      environmentGate: "RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_P256_SMOKE",
      symbol: "radishlex_apple_secure_enclave_p256_product_smoke",
      label: "RadishLex Apple Secure Enclave P-256 product smoke"
    )
  }
}
