#import <Foundation/Foundation.h>

#import "RadishLexLearningContext.h"

static void Require(BOOL condition, NSString *message) {
    if (!condition) {
        NSLog(@"learning context contract failed: %@", message);
        exit(1);
    }
}

static void RequireContext(NSString *caseName,
                           NSString *bundleIdentifier,
                           BOOL expectedSensitive,
                           BOOL expectedKnown,
                           NSString *expectedKind) {
    BOOL sensitive = !expectedSensitive;
    BOOL known = !expectedKnown;
    NSString *kind = @"sentinel";
    RLXClassifyApplicationBundleIdentifier(
        bundleIdentifier, &sensitive, &known, &kind);
    Require(sensitive == expectedSensitive && known == expectedKnown &&
                [kind isEqualToString:expectedKind],
            caseName);
}

int main(void) {
    @autoreleasepool {
        RequireContext(@"TextEdit is a known editor", @"com.apple.TextEdit",
                       NO, YES, @"editor");
        RequireContext(@"Codex is a known code context", @"com.openai.codex",
                       NO, YES, @"code");

        for (NSString *bundleIdentifier in @[
                 @"com.apple.Passwords", @"com.apple.keychainaccess",
                 @"com.1password.1password", @"com.agilebits.onepassword7",
                 RLXValidationP0BundleIdentifier
             ]) {
            RequireContext(@"sensitive application is P0", bundleIdentifier,
                           YES, YES, @"other");
        }

        RequireContext(@"validation unknown remains fail-closed",
                       @"org.radishlex.validation.macos.unknown", NO, NO,
                       @"other");
        RequireContext(@"arbitrary application remains fail-closed",
                       @"org.example.unclassified", NO, NO, @"other");
        RequireContext(@"empty identifier remains fail-closed", @"", NO, NO,
                       @"other");
        RequireContext(@"nil identifier remains fail-closed", nil, NO, NO,
                       @"other");

        Require([RLXValidationP0BundleIdentifier
                    isEqualToString:@"org.radishlex.validation.macos.p0"],
                @"validation P0 identity is stable");
        NSLog(@"macOS learning context classification contract passed");
    }
    return 0;
}
