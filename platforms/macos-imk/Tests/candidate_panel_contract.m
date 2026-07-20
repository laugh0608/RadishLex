#import <AppKit/AppKit.h>

#import "RadishLexCandidatePanel.h"
#import "RadishLexCandidatePanelTesting.h"

static void Require(BOOL condition, NSString *message) {
  if (!condition) {
    NSLog(@"candidate panel contract failed: %@", message);
    exit(1);
  }
}

@interface RLXContractCandidateOwner : NSObject <RLXCandidatePanelOwner>
@property(nonatomic) NSInteger requestedIndex;
@property(nonatomic) NSUInteger requestCount;
@end

@implementation RLXContractCandidateOwner
- (instancetype)init {
  self = [super init];
  if (self != nil)
    _requestedIndex = NSNotFound;
  return self;
}
- (void)candidatePanelDidRequestSelectionAtIndex:(NSUInteger)index {
  self.requestedIndex = (NSInteger)index;
  self.requestCount += 1;
}
@end

@interface RLXContractTextClient : NSObject
@property(nonatomic) NSRect lineRect;
@property(nonatomic) NSRect fallbackRect;
@property(nonatomic) NSRange contractMarkedRange;
@property(nonatomic) NSInteger contractWindowLevel;
@property(nonatomic) BOOL provideLineRect;
@property(nonatomic) NSUInteger lineRectRequestCount;
@property(nonatomic) NSUInteger fallbackRequestCount;
@property(nonatomic) NSUInteger lastCharacterIndex;
@property(nonatomic) NSRange lastFallbackRange;
@end

@implementation RLXContractTextClient
- (instancetype)init {
  self = [super init];
  if (self != nil) {
    _contractMarkedRange = NSMakeRange(10, 5);
    _contractWindowLevel = NSNormalWindowLevel;
    _provideLineRect = YES;
    _lastCharacterIndex = NSNotFound;
    _lastFallbackRange = NSMakeRange(NSNotFound, 0);
  }
  return self;
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index
                          lineHeightRectangle:(NSRectPointer)lineRect {
  self.lineRectRequestCount += 1;
  self.lastCharacterIndex = index;
  BOOL hasInlineSession = self.contractMarkedRange.location != NSNotFound &&
                          self.contractMarkedRange.length > 0;
  BOOL indexIsValid = hasInlineSession
                          ? index < self.contractMarkedRange.length
                          : index == 0;
  if (lineRect != NULL) {
    if (!indexIsValid) {
      *lineRect = NSMakeRect(0, 0, 2, 22);
    } else {
      *lineRect = self.provideLineRect ? self.lineRect : NSZeroRect;
    }
  }
  return @{};
}
- (NSRange)markedRange {
  return self.contractMarkedRange;
}
- (NSRect)firstRectForCharacterRange:(NSRange)range
                         actualRange:(NSRangePointer)actualRange {
  self.fallbackRequestCount += 1;
  self.lastFallbackRange = range;
  if (actualRange != NULL)
    *actualRange = NSMakeRange(NSNotFound, 0);
  return self.fallbackRect;
}
- (NSInteger)windowLevel {
  return self.contractWindowLevel;
}
@end

static NSArray<NSAttributedString *> *Candidates(void) {
  return @[
    [[NSAttributedString alloc] initWithString:@"候选甲"],
    [[NSAttributedString alloc] initWithString:@"候选乙"],
    [[NSAttributedString alloc] initWithString:@"这是一个用于验证压缩和截断行为的较长候选"],
    [[NSAttributedString alloc] initWithString:@"候选丁"],
    [[NSAttributedString alloc] initWithString:@"候选戊"],
  ];
}

static void RequirePanelFollowsLineRect(RLXCandidatePanel *panel,
                                        NSRect lineRect, NSString *message) {
  NSRect panelFrame = panel.rlx_contractWindow.frame;
  NSScreen *screen = NSScreen.mainScreen ?: NSScreen.screens.firstObject;
  NSRect visibleFrame = screen != nil ? screen.visibleFrame : NSZeroRect;
  if (NSIsEmptyRect(visibleFrame)) {
    visibleFrame = NSMakeRect(
        NSMinX(lineRect), NSMinY(lineRect) - 6.0 - NSHeight(panelFrame),
        MAX(NSWidth(panelFrame), NSWidth(lineRect)),
        NSHeight(panelFrame) + 6.0 + NSHeight(lineRect));
  }
  NSRect expectedFrame = RLXCandidatePanelFrame(
      panelFrame.size, lineRect, visibleFrame, 6.0);
  NSString *failure = [NSString
      stringWithFormat:@"%@ (panel=%@ expected=%@ line=%@)", message,
                       NSStringFromRect(panelFrame),
                       NSStringFromRect(expectedFrame),
                       NSStringFromRect(lineRect)];
  Require(NSEqualRects(panelFrame, expectedFrame), failure);
}

int main(void) {
  @autoreleasepool {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];

    NSScreen *screen = NSScreen.mainScreen ?: NSScreen.screens.firstObject;
    NSRect visible =
        screen != nil ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900);
    RLXContractTextClient *client = [[RLXContractTextClient alloc] init];
    client.lineRect = NSMakeRect(NSMidX(visible), NSMidY(visible), 2, 22);
    client.fallbackRect = NSMakeRect(NSMidX(visible) + 20,
                                     NSMidY(visible) + 20, 2, 22);
    client.contractWindowLevel = NSStatusWindowLevel;
    RLXContractCandidateOwner *ownerA =
        [[RLXContractCandidateOwner alloc] init];
    RLXContractCandidateOwner *ownerB =
        [[RLXContractCandidateOwner alloc] init];
    RLXCandidatePanel *panel = [RLXCandidatePanel sharedPanel];

    [panel showCandidates:Candidates()
             selectedIndex:0
               cursorIndex:5
                    client:(id)client
                     owner:ownerA];
    NSPanel *window = panel.rlx_contractWindow;
    Require(window.isVisible && panel.rlx_contractOwner == ownerA,
            @"show establishes the visible owner");
    Require(client.lastCharacterIndex == 4,
            @"an insertion cursor at marked length is clamped to the last "
             "inline character");
    RequirePanelFollowsLineRect(
        panel, client.lineRect,
        @"an end insertion cursor follows the valid client line rectangle");
    Require(!window.canBecomeKeyWindow && !window.canBecomeMainWindow &&
                !window.isKeyWindow && !window.isMainWindow,
            @"candidate panel cannot take host focus");
    Require((window.styleMask & NSWindowStyleMaskNonactivatingPanel) != 0,
            @"candidate panel uses the nonactivating style");
    NSWindowCollectionBehavior requiredBehavior =
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorTransient |
        NSWindowCollectionBehaviorFullScreenAuxiliary |
        NSWindowCollectionBehaviorIgnoresCycle;
    Require((window.collectionBehavior & requiredBehavior) == requiredBehavior,
            @"candidate panel joins Spaces without entering window cycling");
    Require(window.level == client.contractWindowLevel + 1,
            @"candidate panel follows the public client window level contract");
    Require(panel.rlx_contractCandidateButtons.count == 5 &&
                panel.rlx_contractSelectedIndex == 0,
            @"five candidates share one initial display index");
    NSButton *first = panel.rlx_contractCandidateButtons[0];
    NSButton *second = panel.rlx_contractCandidateButtons[1];
    Require(first.refusesFirstResponder && second.refusesFirstResponder,
            @"candidate controls refuse keyboard focus");
    Require([first.accessibilityLabel isEqualToString:@"候选甲"] &&
                [first.accessibilityValue length] > 0 &&
                [second.accessibilityValue length] == 0,
            @"initial visual selection matches accessibility values");
    Require([panel setSelectedIndex:1 owner:ownerA] &&
                panel.rlx_contractSelectedIndex == 1 &&
                [first.accessibilityValue length] == 0 &&
                [second.accessibilityValue length] > 0 &&
                [panel.rlx_contractCandidateStack.accessibilitySelectedChildren
                    isEqualToArray:@[ second ]],
            @"selection updates the visible and accessibility state together");
    Require([panel.rlx_contractCandidateButtons[2].toolTip
                containsString:@"较长候选"],
            @"truncated candidates preserve the complete tooltip");

    NSButton *staleButton = second;
    [panel showCandidates:Candidates()
             selectedIndex:0
               cursorIndex:3
                    client:(id)client
                     owner:ownerB];
    Require(client.lastCharacterIndex == 3,
            @"a middle insertion cursor remains the inline character index");
    RequirePanelFollowsLineRect(
        panel, client.lineRect,
        @"a middle insertion cursor follows the valid client line rectangle");
    [panel hideForOwner:ownerA];
    Require(window.isVisible && panel.rlx_contractOwner == ownerB,
            @"an old owner cannot hide the current session");
    [staleButton performClick:nil];
    Require(ownerA.requestCount == 0 && ownerB.requestCount == 0,
            @"a stale candidate control cannot call either owner");
    NSButton *currentSecond = panel.rlx_contractCandidateButtons[1];
    [currentSecond performClick:nil];
    Require(ownerB.requestCount == 1 && ownerB.requestedIndex == 1,
            @"the current candidate control reports its display index");
    Require(![panel setSelectedIndex:2 owner:ownerA] &&
                panel.rlx_contractSelectedIndex == 0,
            @"a stale owner cannot mutate the current selection");

    client.contractMarkedRange = NSMakeRange(NSNotFound, 0);
    [panel showCandidates:Candidates()
             selectedIndex:0
               cursorIndex:4
                    client:(id)client
                     owner:ownerB];
    Require(client.lastCharacterIndex == 0,
            @"a client without an inline session receives character index zero");
    RequirePanelFollowsLineRect(
        panel, client.lineRect,
        @"a client without an inline session follows its valid line rectangle");

    client.contractMarkedRange = NSMakeRange(10, 5);
    client.provideLineRect = NO;
    [panel showCandidates:Candidates()
             selectedIndex:0
               cursorIndex:5
                    client:(id)client
                     owner:ownerB];
    Require(window.isVisible && client.lastCharacterIndex == 4 &&
                client.fallbackRequestCount == 1 &&
                NSEqualRanges(client.lastFallbackRange, NSMakeRange(15, 0)),
            @"firstRectForCharacterRange keeps the absolute end insertion "
             "position as the public anchor fallback");
    client.fallbackRect = NSZeroRect;
    [panel showCandidates:Candidates()
             selectedIndex:0
               cursorIndex:4
                    client:(id)client
                     owner:ownerB];
    Require(!window.isVisible && panel.rlx_contractOwner == nil &&
                panel.rlx_contractCandidateButtons.count == 0,
            @"missing anchors clear the complete panel state");

    client.provideLineRect = YES;
    [panel showCandidates:Candidates()
             selectedIndex:99
               cursorIndex:0
                    client:(id)client
                     owner:ownerB];
    Require(!window.isVisible && panel.rlx_contractSelectedIndex == NSNotFound,
            @"an invalid initial selection cannot leave partial UI visible");
    [panel showCandidates:@[]
             selectedIndex:0
               cursorIndex:0
                    client:(id)client
                     owner:ownerB];
    Require(!window.isVisible && panel.rlx_contractCandidateButtons.count == 0,
            @"an empty snapshot remains hidden");

    window.appearance =
        [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    [panel showCandidates:Candidates()
             selectedIndex:1
               cursorIndex:0
                    client:(id)client
                     owner:ownerB];
    Require(panel.rlx_contractCandidateButtons[1].layer.backgroundColor != nil &&
                CGColorGetAlpha(panel.rlx_contractCandidateButtons[1]
                                    .layer.backgroundColor) > 0,
            @"selected layer color is resolved for the effective appearance");
    [panel hideForOwner:ownerB];
    Require(!window.isVisible && panel.rlx_contractOwner == nil &&
                panel.rlx_contractCandidateButtons.count == 0 &&
                panel.rlx_contractSelectedIndex == NSNotFound,
            @"owner hide releases all transient panel state");

    NSLog(@"macOS AppKit candidate panel contract passed");
  }
  return 0;
}
