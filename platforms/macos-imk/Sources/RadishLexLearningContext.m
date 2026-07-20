#import "RadishLexLearningContext.h"

NSString *const RLXValidationP0BundleIdentifier =
    @"org.radishlex.validation.macos.p0";

void RLXClassifyApplicationBundleIdentifier(
    NSString *bundleIdentifier,
    BOOL *sensitiveApplication,
    BOOL *contextKnown,
    NSString *__autoreleasing *contextKind) {
    NSCParameterAssert(sensitiveApplication != NULL);
    NSCParameterAssert(contextKnown != NULL);
    NSCParameterAssert(contextKind != NULL);

    *sensitiveApplication = NO;
    *contextKnown = NO;
    *contextKind = @"other";

    if ([bundleIdentifier isEqualToString:@"com.apple.TextEdit"]) {
        *contextKnown = YES;
        *contextKind = @"editor";
        return;
    }
    if ([bundleIdentifier isEqualToString:@"com.openai.codex"]) {
        *contextKnown = YES;
        *contextKind = @"code";
        return;
    }

    if ([bundleIdentifier isEqualToString:@"com.apple.Passwords"] ||
        [bundleIdentifier isEqualToString:@"com.apple.keychainaccess"] ||
        [bundleIdentifier isEqualToString:@"com.1password.1password"] ||
        [bundleIdentifier isEqualToString:@"com.agilebits.onepassword7"] ||
        [bundleIdentifier isEqualToString:RLXValidationP0BundleIdentifier]) {
        *sensitiveApplication = YES;
        *contextKnown = YES;
    }
}
