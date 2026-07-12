#import <AppKit/AppKit.h>

#import "radishlex_input.h"

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSErrorDomain const RLXBridgeErrorDomain;

@interface RLXCandidate : NSObject
@property(nonatomic, readonly) NSUInteger index;
@property(nonatomic, copy, readonly) NSString *text;
@property(nonatomic, copy, readonly, nullable) NSString *reading;
@property(nonatomic, copy, readonly, nullable) NSString *annotation;
@property(nonatomic, readonly) uint32_t source;
@end

@interface RLXSnapshot : NSObject
@property(nonatomic, copy, readonly) NSString *schema;
@property(nonatomic, copy, readonly) NSString *preedit;
@property(nonatomic, readonly) NSUInteger cursor;
@property(nonatomic, copy, readonly) NSArray<RLXCandidate *> *candidates;
@end

@interface RLXKeyHandlingResult : NSObject
@property(nonatomic, readonly, getter=isConsumed) BOOL consumed;
@property(nonatomic, copy, readonly, nullable) NSString *commit;
@property(nonatomic, strong, readonly, nullable) RLXSnapshot *snapshot;
@end

@interface RLXSessionBridge : NSObject

@property(nonatomic, strong, readonly) NSThread *ownerThread;
@property(nonatomic, readonly, getter=isValid) BOOL valid;

+ (BOOL)validateFFIContract:(NSError **)error;

- (nullable instancetype)initDemoWithError:(NSError **)error;
- (nullable instancetype)initRimeWithSharedDataDirectory:(NSString *)sharedDataDirectory
                                       userDataDirectory:(NSString *)userDataDirectory
                                                  schema:(NSString *)schema
                                            logDirectory:(nullable NSString *)logDirectory
                                           deployOnStart:(BOOL)deployOnStart
                                                   error:(NSError **)error;

- (nullable RLXKeyHandlingResult *)handleEvent:(RadishLexKeyEvent)event
                                          error:(NSError **)error;
- (nullable RLXSnapshot *)snapshotWithError:(NSError **)error;
- (nullable RLXKeyHandlingResult *)selectCandidateAtIndex:(NSUInteger)index
                                                      error:(NSError **)error;
- (BOOL)resetWithError:(NSError **)error;
- (BOOL)setSchema:(NSString *)schema error:(NSError **)error;
- (void)invalidate;

@end

FOUNDATION_EXPORT BOOL RLXNormalizeKeyEvent(NSEvent *event, RadishLexKeyEvent *eventOut);
FOUNDATION_EXPORT NSUInteger RLXUTF16CursorForUTF8Offset(NSString *value,
                                                         size_t utf8Offset,
                                                         NSError **error);
FOUNDATION_EXPORT NSAttributedString *RLXAttributedCandidate(RLXCandidate *candidate);

NS_ASSUME_NONNULL_END
