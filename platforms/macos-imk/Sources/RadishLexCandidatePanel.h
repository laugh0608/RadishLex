#import <AppKit/AppKit.h>
#import <InputMethodKit/InputMethodKit.h>

@protocol RLXCandidatePanelOwner <NSObject>
- (void)candidatePanelDidRequestSelectionAtIndex:(NSUInteger)index;
@end

FOUNDATION_EXPORT NSInteger RLXCandidateIndexByMoving(
    NSInteger currentIndex, NSInteger offset, NSUInteger candidateCount);
FOUNDATION_EXPORT NSRect RLXCandidatePanelFrame(NSSize panelSize,
                                                NSRect anchorRect,
                                                NSRect visibleFrame,
                                                CGFloat gap);

@interface RLXCandidatePanel : NSObject

+ (instancetype)sharedPanel;
- (instancetype)init NS_UNAVAILABLE;

- (void)showCandidates:(NSArray<NSAttributedString *> *)candidates
         selectedIndex:(NSInteger)selectedIndex
           cursorIndex:(NSUInteger)cursorIndex
                client:(id<IMKTextInput>)client
                 owner:(id<RLXCandidatePanelOwner>)owner;
- (BOOL)setSelectedIndex:(NSInteger)selectedIndex
                   owner:(id<RLXCandidatePanelOwner>)owner;
- (void)hideForOwner:(id<RLXCandidatePanelOwner>)owner;

@end
