#import <AppKit/AppKit.h>

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSUInteger, RLXReferenceProbeEventRoute) {
  RLXReferenceProbeEventRouteControllerInput = 0,
  RLXReferenceProbeEventRouteCandidatePanel = 1,
  RLXReferenceProbeEventRouteCommitCandidate = 2,
  RLXReferenceProbeEventRouteCommitRaw = 3,
  RLXReferenceProbeEventRouteHostApplication = 4,
};

FOUNDATION_EXPORT RLXReferenceProbeEventRoute RLXReferenceProbeRouteForEvent(
    NSEvent *event);

@interface RLXReferenceProbeState : NSObject
@property(nonatomic, copy, readonly) NSArray<NSAttributedString *> *candidates;
@property(nonatomic, readonly) NSInteger selectedIndex;

- (instancetype)initWithCandidates:(NSArray<NSAttributedString *> *)candidates
    NS_DESIGNATED_INITIALIZER;
- (instancetype)init NS_UNAVAILABLE;
- (void)resetSelection;
- (BOOL)updateSelectionFromCandidate:(NSAttributedString *)candidate;
- (nullable NSAttributedString *)selectedCandidate;
@end

NS_ASSUME_NONNULL_END
