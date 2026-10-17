#import <Foundation/Foundation.h>
#include <stdio.h>

// Foundation path resolution only: never open product files or load product code.
static NSString *Append(NSString *base, NSString *relative) {
  return base ? [base stringByAppendingPathComponent:relative] : nil;
}

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    if (argc != 2) return 2;
    NSString *expected = [NSString stringWithUTF8String:argv[1]];
    if (![expected isAbsolutePath]) return 2;
    NSString *resolvedHome = NSHomeDirectory();
    NSArray<NSURL *> *supportURLs = [[NSFileManager defaultManager]
        URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask];
    NSString *support = [supportURLs firstObject].path;
    NSString *data = Append(support, @"RadishLex");
    NSString *gateHome = data;
    for (int i = 0; i < 3; ++i) gateHome = [gateHome stringByDeletingLastPathComponent];
    BOOL gateShape = data && [Append(gateHome, @"Library/Application Support/RadishLex")
        isEqualToString:data];
    NSArray<NSString *> *keys = @[@"home", @"application_support", @"runtime_data",
        @"userdb", @"settings", @"rime", @"manager_bundle", @"input_method_bundle"];
    NSArray<NSString *> *relative = @[@"", @"Library/Application Support",
        @"Library/Application Support/RadishLex",
        @"Library/Application Support/RadishLex/userdb.sqlite3",
        @"Library/Application Support/RadishLex/manager-settings.json",
        @"Library/Application Support/RadishLex/Rime",
        @"Applications/RadishLex Manager.app",
        @"Library/Input Methods/RadishLexInputMethod.app"];
    NSArray *paths = @[resolvedHome ?: [NSNull null], support ?: [NSNull null],
        data ?: [NSNull null], Append(data, @"userdb.sqlite3") ?: [NSNull null],
        Append(data, @"manager-settings.json") ?: [NSNull null],
        Append(data, @"Rime") ?: [NSNull null],
        (gateShape ? Append(gateHome, relative[6]) : nil) ?: [NSNull null],
        (gateShape ? Append(gateHome, relative[7]) : nil) ?: [NSNull null]];
    NSMutableDictionary *rows = [NSMutableDictionary dictionary];
    for (NSUInteger i = 0; i < keys.count; ++i) {
      NSString *path = paths[i] == [NSNull null] ? nil : paths[i];
      NSString *target = relative[i].length ? Append(expected, relative[i]) : expected;
      NSString *homeTarget = relative[i].length ? Append(resolvedHome, relative[i]) : resolvedHome;
      BOOL matches = path && [path isEqualToString:target];
      BOOL homeShape = path && [path isEqualToString:homeTarget];
      BOOL inside = path && ([path isEqualToString:expected] ||
          [path hasPrefix:[expected stringByAppendingString:@"/"]]);
      // Only approved synthetic relative paths can ever appear in output.
      rows[keys[i]] = @{@"matches_expected": @(matches), @"in_synthetic_home": @(inside),
          @"same_user_home_shape": @(homeShape),
          @"classification": matches ? @"synthetic_home" :
              (homeShape && !inside ? @"real_user_domain" : @"unexpected"),
          @"relative_path": matches ? relative[i] : (id)[NSNull null]};
    }
    NSDictionary *result = @{@"schema": @1, @"support_url_count": @(supportURLs.count),
        @"gate_home_shape": @(gateShape), @"paths": rows};
    NSData *json = [NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingSortedKeys
        error:NULL];
    if (!json) return 3;
    if (fwrite(json.bytes, 1, json.length, stdout) != json.length) return 4;
    return fputc('\n', stdout) == EOF ? 4 : 0;
  }
}
