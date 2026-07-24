#import <Foundation/Foundation.h>

#import "radishlex_input.h"

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSUInteger, RLXUpgradeValidationTarget) {
  RLXUpgradeValidationTargetCandidate = 0,
  RLXUpgradeValidationTargetPostSwitch = 1,
};

FOUNDATION_EXPORT NSURL *_Nullable RLXFixedUpgradeDataRoot(void);
FOUNDATION_EXPORT BOOL RLXParseUpgradeValidationTarget(
    int argc, const char *_Nonnull const *_Nonnull argv,
    RLXUpgradeValidationTarget *target);
FOUNDATION_EXPORT NSURL *RLXUpgradeDatabaseURL(
    NSURL *root, RLXUpgradeValidationTarget target);
FOUNDATION_EXPORT NSURL *RLXUpgradeSettingsURL(
    NSURL *root, RLXUpgradeValidationTarget target);
FOUNDATION_EXPORT BOOL RLXValidateDatabaseUnchanged(
    NSURL *databaseURL, NSData *beforeBytes, NSError **error);
FOUNDATION_EXPORT BOOL RLXRequireNoDatabaseSidecars(NSURL *databaseURL,
                                                    NSError **error);

NS_ASSUME_NONNULL_END
