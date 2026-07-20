#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSString *const RLXValidationP0BundleIdentifier;

void RLXClassifyApplicationBundleIdentifier(
    NSString *_Nullable bundleIdentifier,
    BOOL *sensitiveApplication,
    BOOL *contextKnown,
    NSString * _Nonnull __autoreleasing * _Nonnull contextKind);

NS_ASSUME_NONNULL_END
