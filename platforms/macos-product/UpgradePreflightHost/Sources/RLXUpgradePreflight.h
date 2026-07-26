#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSErrorDomain const RLXUpgradePreflightErrorDomain;

typedef NS_ERROR_ENUM(RLXUpgradePreflightErrorDomain, RLXUpgradePreflightErrorCode) {
  RLXUpgradePreflightErrorUnsafeDataRoot = 1,
  RLXUpgradePreflightErrorAvailableSpaceUnavailable = 2,
  RLXUpgradePreflightErrorUnsafeDataFile = 3,
  RLXUpgradePreflightErrorProcessInspectionFailed = 4,
  RLXUpgradePreflightErrorOpenHandleInspectionFailed = 5,
};

@interface RLXUpgradePreflightResult : NSObject

@property(nonatomic, readonly) unsigned long long availableBytes;
@property(nonatomic, readonly, getter=isQuiescent) BOOL quiescent;
@property(nonatomic, copy, readonly, nullable) NSString *blocker;

- (instancetype)init NS_UNAVAILABLE;

@end

FOUNDATION_EXPORT NSString *_Nullable RLXUpgradeQuiescenceBlocker(
    NSUInteger managerProcessCount, NSUInteger inputMethodProcessCount,
    BOOL hasOpenDataHandle);

FOUNDATION_EXPORT RLXUpgradePreflightResult *_Nullable
RLXInspectUpgradePreflight(NSURL *dataRootURL, NSString *managerBundleIdentifier,
                           NSString *inputMethodBundleIdentifier,
                           NSError **error);

NS_ASSUME_NONNULL_END
