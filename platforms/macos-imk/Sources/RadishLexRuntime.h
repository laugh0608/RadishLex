#import <Foundation/Foundation.h>

@class RLXSessionBridge;

NS_ASSUME_NONNULL_BEGIN

@interface RLXProcessRuntime : NSObject

+ (instancetype)sharedRuntime;
- (nullable RLXSessionBridge *)createSessionWithError:(NSError **)error;
- (void)forgetSession:(RLXSessionBridge *)session;
- (BOOL)shutdownWithError:(NSError **)error;

@end

NS_ASSUME_NONNULL_END
