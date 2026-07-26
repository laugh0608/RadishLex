#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSString *const RLXInstallerDriverContractVersionKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverPhaseKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverPrimaryActionKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverSecondaryActionKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverErrorKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverStateKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverOperationKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverProgressKey;
FOUNDATION_EXPORT NSString *const RLXInstallerDriverManualPromptKey;

@interface RLXInstallerPresentation : NSObject

@property(nonatomic, readonly, copy) NSString *phaseCode;
@property(nonatomic, readonly, copy) NSString *primaryActionCode;
@property(nonatomic, readonly, copy) NSString *secondaryActionCode;
@property(nonatomic, readonly, copy) NSString *errorCode;
@property(nonatomic, readonly, copy) NSString *stateCode;
@property(nonatomic, readonly, copy) NSString *operationCode;
@property(nonatomic, readonly) NSInteger progressStep;
@property(nonatomic, readonly) BOOL requiresManualQuiescence;
@property(nonatomic, readonly) BOOL failedClosed;

- (instancetype)initWithDriverSnapshot:(NSDictionary<NSString *, id> *)snapshot;
- (NSString *)statusTitle;
- (NSString *)statusDetail;
- (NSString *)operationTitle;
- (NSString *)titleForAction:(NSString *)actionCode;
- (BOOL)isActionEnabled:(NSString *)actionCode;
- (BOOL)requiresConfirmationForAction:(NSString *)actionCode;
- (NSString *)confirmationTextForAction:(NSString *)actionCode;
- (NSString *)stableDiagnosticSummary;

@end

NS_ASSUME_NONNULL_END
