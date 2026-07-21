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
    RadishLexProductUpgradeStartupGateRequest gateRequest = {
        .version = RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION,
        .data_root_path = dataRoot.fileSystemRepresentation,
        .expected_owner_id = geteuid(),
    };
    RadishLexProductUpgradeStartupGateResult gateResult = {0};
    RadishLexError *gateError = NULL;
    RadishLexStatusCode gateStatus = radishlex_product_upgrade_startup_gate(
        &gateRequest, &gateResult, &gateError);
    if (gateError != NULL) {
      radishlex_error_free(gateError);
    }
    BOOL gateAllowed =
        gateStatus == RADISHLEX_STATUS_OK &&
        gateResult.version == RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_RESULT_VERSION &&
        (gateResult.decision == RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH ||
         gateResult.decision == RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE ||
         gateResult.decision == RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT);
    if (!gateAllowed) {
      NSLog(@"RadishLex startup gate blocked decision=%u error=%u state=%u",
            gateResult.decision, gateResult.error_code, gateResult.receipt_state);
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
