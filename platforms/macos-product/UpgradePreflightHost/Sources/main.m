#import <Foundation/Foundation.h>

#import "RLXUpgradePreflight.h"

#ifndef RLX_MANAGER_BUNDLE_ID
#error "RLX_MANAGER_BUNDLE_ID must be defined by the product build"
#endif

#ifndef RLX_INPUT_METHOD_BUNDLE_ID
#error "RLX_INPUT_METHOD_BUNDLE_ID must be defined by the product build"
#endif

#define RLX_STRINGIFY_INNER(value) #value
#define RLX_STRINGIFY(value) RLX_STRINGIFY_INNER(value)

static int RLXWriteJSON(NSDictionary<NSString *, id> *object);

int main(int argc, const char *argv[]) {
  (void)argv;
  @autoreleasepool {
    if (argc != 1) {
      return 2;
    }
    NSArray<NSURL *> *applicationSupport =
        [[NSFileManager defaultManager]
            URLsForDirectory:NSApplicationSupportDirectory
                   inDomains:NSUserDomainMask];
    NSURL *dataRootURL = [[applicationSupport firstObject]
        URLByAppendingPathComponent:@"RadishLex"
                        isDirectory:YES];
    if (dataRootURL == nil) {
      return RLXWriteJSON(@{
        @"format" : @"radishlex-upgrade-preflight-v1",
        @"result" : @"failed",
        @"error" : @"data_root_unavailable"
      });
    }

    NSError *error = nil;
    RLXUpgradePreflightResult *result = RLXInspectUpgradePreflight(
        dataRootURL, @RLX_STRINGIFY(RLX_MANAGER_BUNDLE_ID),
        @RLX_STRINGIFY(RLX_INPUT_METHOD_BUNDLE_ID), &error);
    if (result == nil) {
      return RLXWriteJSON(@{
        @"format" : @"radishlex-upgrade-preflight-v1",
        @"result" : @"failed",
        @"error" : @"preflight_inspection_failed",
        @"error_code" : @(error.code)
      });
    }
    NSMutableDictionary<NSString *, id> *output = [@{
      @"format" : @"radishlex-upgrade-preflight-v1",
      @"result" : result.isQuiescent ? @"ready" : @"blocked",
      @"available_bytes" : @(result.availableBytes),
      @"quiescent" : @(result.isQuiescent)
    } mutableCopy];
    if (result.blocker != nil) {
      output[@"blocker"] = result.blocker;
    }
    return RLXWriteJSON(output);
  }
}

static int RLXWriteJSON(NSDictionary<NSString *, id> *object) {
  NSError *error = nil;
  NSData *data = [NSJSONSerialization dataWithJSONObject:object
                                                 options:NSJSONWritingSortedKeys
                                                   error:&error];
  if (data == nil || error != nil) {
    return 4;
  }
  NSFileHandle *standardOutput = [NSFileHandle fileHandleWithStandardOutput];
  [standardOutput writeData:data];
  [standardOutput writeData:[@"\n" dataUsingEncoding:NSUTF8StringEncoding]];
  return [object[@"result"] isEqual:@"ready"] ? 0 : 3;
}
