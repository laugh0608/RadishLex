#import <Foundation/Foundation.h>

#import "radishlex_input.h"

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSURL *_Nullable RLXFixedUpgradeDataRoot(void);
FOUNDATION_EXPORT BOOL RLXValidateCandidateUnchanged(
    NSURL *candidateURL, NSData *beforeBytes, NSError **error);
FOUNDATION_EXPORT BOOL RLXRequireNoCandidateSidecars(NSURL *candidateURL,
                                                     NSError **error);

NS_ASSUME_NONNULL_END
