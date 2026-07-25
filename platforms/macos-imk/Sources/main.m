#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#import <unistd.h>

#import "RadishLexRuntime.h"
#import "radishlex_input.h"

int main(int argc, const char *argv[]) {
  (void)argc;
  (void)argv;
  @autoreleasepool {
    NSArray<NSURL *> *applicationSupport =
        [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory
                                                inDomains:NSUserDomainMask];
    NSURL *dataRoot = [[applicationSupport firstObject]
        URLByAppendingPathComponent:@"RadishLex" isDirectory:YES];
    if (dataRoot == nil) {
      return 2;
    }
    RadishLexProductInstallStartupGateRequest installGateRequest = {
        .version = RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_REQUEST_VERSION,
        .data_root_path = dataRoot.fileSystemRepresentation,
        .expected_owner_id = geteuid(),
    };
    RadishLexProductInstallStartupGateResult installGateResult = {0};
    RadishLexError *installGateError = NULL;
    RadishLexStatusCode installGateStatus = radishlex_product_install_startup_gate(
        &installGateRequest, &installGateResult, &installGateError);
    if (installGateError != NULL) {
      radishlex_error_free(installGateError);
    }
    BOOL installGateAllowed =
        installGateStatus == RADISHLEX_STATUS_OK &&
        installGateResult.version ==
            RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_RESULT_VERSION &&
        installGateResult.error_code == RADISHLEX_STARTUP_GATE_ERROR_NONE &&
        (((installGateResult.decision ==
               RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH ||
           installGateResult.decision ==
               RADISHLEX_INSTALL_GATE_ALLOWED_NO_INSTALL_STATE) &&
          installGateResult.receipt_state == 0) ||
         (installGateResult.decision ==
              RADISHLEX_INSTALL_GATE_ALLOWED_TERMINAL_RECEIPT &&
          (installGateResult.receipt_state ==
               RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED ||
           installGateResult.receipt_state ==
               RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED ||
           installGateResult.receipt_state ==
               RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK)));
    if (!installGateAllowed) {
      NSLog(@"RadishLex install startup gate decision=%u error=%u state=%u",
            installGateResult.decision, installGateResult.error_code,
            installGateResult.receipt_state);
      return 5;
    }
    RadishLexProductUpgradeStartupGateRequest dataGateRequest = {
        .version = RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION,
        .data_root_path = dataRoot.fileSystemRepresentation,
        .expected_owner_id = geteuid(),
    };
    RadishLexProductUpgradeStartupGateResult dataGateResult = {0};
    RadishLexError *dataGateError = NULL;
    RadishLexStatusCode dataGateStatus = radishlex_product_upgrade_startup_gate(
        &dataGateRequest, &dataGateResult, &dataGateError);
    if (dataGateError != NULL) {
      radishlex_error_free(dataGateError);
    }
    BOOL dataGateAllowed =
        dataGateStatus == RADISHLEX_STATUS_OK &&
        dataGateResult.version ==
            RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_RESULT_VERSION &&
        dataGateResult.error_code == RADISHLEX_STARTUP_GATE_ERROR_NONE &&
        (((dataGateResult.decision ==
               RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH ||
           dataGateResult.decision ==
               RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE) &&
          dataGateResult.receipt_state == 0) ||
         (dataGateResult.decision ==
              RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT &&
          (dataGateResult.receipt_state ==
               RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED ||
           dataGateResult.receipt_state ==
               RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED ||
           dataGateResult.receipt_state ==
               RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK)));
    if (!dataGateAllowed) {
      NSLog(@"RadishLex data startup gate decision=%u error=%u state=%u",
            dataGateResult.decision, dataGateResult.error_code,
            dataGateResult.receipt_state);
      return 5;
    }
    NSBundle *bundle = [NSBundle mainBundle];
    NSString *connectionName = [bundle objectForInfoDictionaryKey:@"InputMethodConnectionName"];
    if (connectionName.length == 0 || bundle.bundleIdentifier.length == 0) {
      return 2;
    }
    IMKServer *server = [[IMKServer alloc] initWithName:connectionName
                                      bundleIdentifier:bundle.bundleIdentifier];
    if (server == nil) {
      return 3;
    }
    [[NSApplication sharedApplication] run];

    NSError *shutdownError = nil;
    if (![[RLXProcessRuntime sharedRuntime] shutdownWithError:&shutdownError]) {
      NSLog(@"RadishLex runtime shutdown failed (%@:%ld)", shutdownError.domain,
            (long)shutdownError.code);
      return 4;
    }
  }
  return 0;
}
