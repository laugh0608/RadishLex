#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>

#import "RadishLexRuntime.h"

int main(int argc, const char *argv[]) {
  (void)argc;
  (void)argv;
  @autoreleasepool {
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
