#import "RadishLexCandidatePanel.h"

static const CGFloat RLXCandidatePanelGap = 6.0;

static BOOL RLXCandidateAnchorIsUsable(NSRect rect) {
  NSRect standardized = CGRectStandardize(rect);
  return !CGRectIsNull(standardized) && !CGRectIsInfinite(standardized) &&
         isfinite(NSMinX(standardized)) && isfinite(NSMinY(standardized)) &&
         isfinite(NSWidth(standardized)) && isfinite(NSHeight(standardized)) &&
         NSHeight(standardized) > 0;
}

NSInteger RLXCandidateIndexByMoving(NSInteger currentIndex, NSInteger offset,
                                    NSUInteger candidateCount) {
  if (candidateCount == 0)
    return NSNotFound;
  NSInteger normalized = currentIndex;
  if (normalized == NSNotFound || normalized < 0 ||
      (NSUInteger)normalized >= candidateCount) {
    normalized = 0;
  }
  NSInteger target = normalized + offset;
  if (target < 0 || (NSUInteger)target >= candidateCount)
    return normalized;
  return target;
}

NSRect RLXCandidatePanelFrame(NSSize panelSize, NSRect anchorRect,
                              NSRect visibleFrame, CGFloat gap) {
  NSRect anchor = CGRectStandardize(anchorRect);
  NSRect visible = CGRectStandardize(visibleFrame);
  CGFloat width = MIN(panelSize.width, NSWidth(visible));
  CGFloat height = MIN(panelSize.height, NSHeight(visible));
  CGFloat x = NSMinX(anchor);
  CGFloat below = NSMinY(anchor) - gap - height;
  CGFloat above = NSMaxY(anchor) + gap;
  CGFloat y = below >= NSMinY(visible) ? below : above;
  x = MIN(MAX(x, NSMinX(visible)), NSMaxX(visible) - width);
  y = MIN(MAX(y, NSMinY(visible)), NSMaxY(visible) - height);
  return NSMakeRect(x, y, width, height);
}

@interface RLXNonactivatingCandidatePanel : NSPanel
@end

@implementation RLXNonactivatingCandidatePanel
- (BOOL)canBecomeKeyWindow {
  return NO;
}
- (BOOL)canBecomeMainWindow {
  return NO;
}
@end

@interface RLXCandidateButton : NSButton
@property(nonatomic, getter=isCandidateSelected) BOOL candidateSelected;
@end

@implementation RLXCandidateButton

- (void)setCandidateSelected:(BOOL)candidateSelected {
  _candidateSelected = candidateSelected;
  [self applyCandidateAppearance];
}

- (void)viewDidChangeEffectiveAppearance {
  [super viewDidChangeEffectiveAppearance];
  [self applyCandidateAppearance];
}

- (void)applyCandidateAppearance {
  if (self.layer == nil)
    return;
  NSColor *background = self.isCandidateSelected
                            ? NSColor.selectedContentBackgroundColor
                            : NSColor.clearColor;
  [self.effectiveAppearance performAsCurrentDrawingAppearance:^{
    self.layer.backgroundColor = background.CGColor;
  }];
  self.contentTintColor = self.isCandidateSelected
                              ? NSColor.selectedMenuItemTextColor
                              : NSColor.labelColor;
}

- (BOOL)accessibilityPerformPress {
  [self performClick:nil];
  return YES;
}

@end

@interface RLXCandidatePanel ()
@property(nonatomic, strong) RLXNonactivatingCandidatePanel *window;
@property(nonatomic, strong) NSVisualEffectView *backgroundView;
@property(nonatomic, strong) NSStackView *candidateStack;
@property(nonatomic, copy) NSArray<RLXCandidateButton *> *candidateButtons;
@property(nonatomic, weak) id<RLXCandidatePanelOwner> owner;
@property(nonatomic) NSInteger selectedIndex;
- (void)clearCandidateViews;
@end

@implementation RLXCandidatePanel

+ (instancetype)sharedPanel {
  static RLXCandidatePanel *panel;
  static dispatch_once_t onceToken;
  dispatch_once(&onceToken, ^{
    panel = [[RLXCandidatePanel alloc] initPrivate];
  });
  return panel;
}

- (instancetype)initPrivate {
  self = [super init];
  if (self == nil)
    return nil;

  _window = [[RLXNonactivatingCandidatePanel alloc]
      initWithContentRect:NSMakeRect(0, 0, 1, 1)
                styleMask:NSWindowStyleMaskBorderless |
                          NSWindowStyleMaskNonactivatingPanel
                  backing:NSBackingStoreBuffered
                    defer:YES];
  _window.opaque = NO;
  _window.backgroundColor = NSColor.clearColor;
  _window.hasShadow = YES;
  _window.floatingPanel = YES;
  _window.becomesKeyOnlyIfNeeded = YES;
  _window.hidesOnDeactivate = NO;
  _window.releasedWhenClosed = NO;
  _window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                               NSWindowCollectionBehaviorTransient |
                               NSWindowCollectionBehaviorFullScreenAuxiliary |
                               NSWindowCollectionBehaviorIgnoresCycle;

  _backgroundView =
      [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, 1, 1)];
  _backgroundView.material = NSVisualEffectMaterialPopover;
  _backgroundView.blendingMode = NSVisualEffectBlendingModeBehindWindow;
  _backgroundView.state = NSVisualEffectStateActive;
  _backgroundView.wantsLayer = YES;
  _backgroundView.layer.cornerRadius = 7.0;
  _backgroundView.layer.masksToBounds = YES;
  _backgroundView.accessibilityElement = NO;

  _candidateStack = [[NSStackView alloc] initWithFrame:NSMakeRect(0, 0, 1, 1)];
  _candidateStack.orientation = NSUserInterfaceLayoutOrientationHorizontal;
  _candidateStack.alignment = NSLayoutAttributeCenterY;
  _candidateStack.spacing = 2.0;
  _candidateStack.edgeInsets = NSEdgeInsetsMake(4.0, 4.0, 4.0, 4.0);
  _candidateStack.translatesAutoresizingMaskIntoConstraints = NO;
  _candidateStack.accessibilityElement = YES;
  _candidateStack.accessibilityRole = NSAccessibilityListRole;
  _candidateStack.accessibilityLabel =
      NSLocalizedString(@"candidate_list", nil);
  [_backgroundView addSubview:_candidateStack];
  [NSLayoutConstraint activateConstraints:@[
    [_candidateStack.leadingAnchor
        constraintEqualToAnchor:_backgroundView.leadingAnchor],
    [_candidateStack.trailingAnchor
        constraintEqualToAnchor:_backgroundView.trailingAnchor],
    [_candidateStack.topAnchor
        constraintEqualToAnchor:_backgroundView.topAnchor],
    [_candidateStack.bottomAnchor
        constraintEqualToAnchor:_backgroundView.bottomAnchor],
  ]];
  _window.contentView = _backgroundView;
  _candidateButtons = @[];
  _selectedIndex = NSNotFound;
  return self;
}

- (void)showCandidates:(NSArray<NSAttributedString *> *)candidates
         selectedIndex:(NSInteger)selectedIndex
           cursorIndex:(NSUInteger)cursorIndex
                client:(id<IMKTextInput>)client
                 owner:(id<RLXCandidatePanelOwner>)owner {
  NSAssert([NSThread isMainThread],
           @"candidate panel must stay on the main thread");
  if (owner == nil)
    return;
  if (candidates.count == 0 || client == nil) {
    [self hideForOwner:owner];
    return;
  }

  self.owner = owner;
  [self clearCandidateViews];

  NSMutableArray<RLXCandidateButton *> *buttons =
      [NSMutableArray arrayWithCapacity:candidates.count];
  [candidates enumerateObjectsUsingBlock:^(NSAttributedString *candidate,
                                           NSUInteger index, BOOL *stop) {
    (void)stop;
    RLXCandidateButton *button = [[RLXCandidateButton alloc] initWithFrame:NSZeroRect];
    button.title = [NSString stringWithFormat:@"%lu %@",
                                              (unsigned long)(index + 1),
                                              candidate.string];
    button.target = self;
    button.action = @selector(candidatePressed:);
    button.tag = (NSInteger)index;
    button.bordered = NO;
    button.refusesFirstResponder = YES;
    button.font = [NSFont systemFontOfSize:15.0 weight:NSFontWeightRegular];
    button.wantsLayer = YES;
    button.layer.cornerRadius = 5.0;
    button.lineBreakMode = NSLineBreakByTruncatingTail;
    button.toolTip = candidate.string;
    [button setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                     forOrientation:NSLayoutConstraintOrientationHorizontal];
    button.accessibilityElement = YES;
    button.accessibilityLabel = candidate.string;
    button.accessibilityIdentifier = [NSString
        stringWithFormat:@"radishlex.candidate.%lu", (unsigned long)index];
    [button.widthAnchor constraintGreaterThanOrEqualToConstant:36.0].active =
        YES;
    [button.heightAnchor constraintGreaterThanOrEqualToConstant:30.0].active =
        YES;
    [self.candidateStack addArrangedSubview:button];
    [buttons addObject:button];
  }];
  self.candidateButtons = buttons;
  self.selectedIndex = NSNotFound;
  if (![self applySelectedIndex:selectedIndex announce:NO]) {
    NSLog(@"RadishLex candidate panel rejected its initial selection");
    [self hideForOwner:owner];
    return;
  }

  [self.backgroundView layoutSubtreeIfNeeded];
  NSSize contentSize = self.candidateStack.fittingSize;
  contentSize.width = MAX(contentSize.width, 1.0);
  contentSize.height = MAX(contentSize.height, 1.0);

  NSRange markedRange = [client markedRange];
  // IMKTextInput expects a character index inside the inline session, not the
  // insertion position immediately after its final character.
  NSUInteger inlineCharacterIndex = 0;
  if (markedRange.location != NSNotFound && markedRange.length > 0) {
    inlineCharacterIndex = MIN(cursorIndex, markedRange.length - 1);
  }
  NSRect lineRect = NSZeroRect;
  [client attributesForCharacterIndex:inlineCharacterIndex
                  lineHeightRectangle:&lineRect];
  if (!RLXCandidateAnchorIsUsable(lineRect)) {
    // This fallback uses an absolute document insertion range and may point at
    // the end of the marked range.
    NSUInteger location =
        markedRange.location == NSNotFound
            ? 0
            : markedRange.location + MIN(cursorIndex, markedRange.length);
    lineRect = [client firstRectForCharacterRange:NSMakeRange(location, 0)
                                      actualRange:NULL];
  }
  if (!RLXCandidateAnchorIsUsable(lineRect)) {
    NSLog(@"RadishLex candidate panel could not obtain a client anchor "
          @"rectangle");
    [self hideForOwner:owner];
    return;
  }
  NSScreen *screen = [self screenForAnchorRect:lineRect];
  NSRect visibleFrame =
      screen != nil ? screen.visibleFrame : NSScreen.mainScreen.visibleFrame;
  if (NSIsEmptyRect(visibleFrame)) {
    visibleFrame = NSMakeRect(
        NSMinX(lineRect),
        NSMinY(lineRect) - RLXCandidatePanelGap - contentSize.height,
        MAX(contentSize.width, NSWidth(lineRect)),
        contentSize.height + RLXCandidatePanelGap + NSHeight(lineRect));
  }
  NSRect panelFrame = RLXCandidatePanelFrame(
      contentSize, lineRect, visibleFrame, RLXCandidatePanelGap);
  self.window.level = (NSWindowLevel)[client windowLevel] + 1;
  [self.window setFrame:panelFrame display:YES];
  [self.window orderFrontRegardless];
}

- (BOOL)setSelectedIndex:(NSInteger)selectedIndex
                   owner:(id<RLXCandidatePanelOwner>)owner {
  NSAssert([NSThread isMainThread],
           @"candidate panel must stay on the main thread");
  if (owner == nil || self.owner != owner)
    return NO;
  return [self applySelectedIndex:selectedIndex announce:YES];
}

- (void)hideForOwner:(id<RLXCandidatePanelOwner>)owner {
  NSAssert([NSThread isMainThread],
           @"candidate panel must stay on the main thread");
  if (owner != nil && self.owner != owner)
    return;
  [self.window orderOut:nil];
  self.owner = nil;
  [self clearCandidateViews];
}

- (BOOL)applySelectedIndex:(NSInteger)selectedIndex announce:(BOOL)announce {
  if (selectedIndex == NSNotFound || selectedIndex < 0 ||
      (NSUInteger)selectedIndex >= self.candidateButtons.count) {
    return NO;
  }
  NSInteger previousIndex = self.selectedIndex;
  self.selectedIndex = selectedIndex;
  [self.candidateButtons enumerateObjectsUsingBlock:^(
                             RLXCandidateButton *button, NSUInteger index,
                             BOOL *stop) {
    (void)stop;
    BOOL selected = index == (NSUInteger)selectedIndex;
    button.candidateSelected = selected;
    button.accessibilityValue =
        selected ? NSLocalizedString(@"candidate_selected", nil) : @"";
  }];
  NSButton *selectedButton = self.candidateButtons[(NSUInteger)selectedIndex];
  self.candidateStack.accessibilitySelectedChildren = @[ selectedButton ];
  if (announce && previousIndex != selectedIndex) {
    NSAccessibilityPostNotification(selectedButton,
                                    NSAccessibilityValueChangedNotification);
    NSAccessibilityPostNotification(
        self.candidateStack,
        NSAccessibilitySelectedChildrenChangedNotification);
  }
  return YES;
}

- (void)candidatePressed:(RLXCandidateButton *)sender {
  id<RLXCandidatePanelOwner> owner = self.owner;
  if (owner == nil || ![self.candidateButtons containsObject:sender] ||
      sender.tag < 0 || (NSUInteger)sender.tag >= self.candidateButtons.count) {
    return;
  }
  [owner candidatePanelDidRequestSelectionAtIndex:(NSUInteger)sender.tag];
}

- (void)clearCandidateViews {
  for (NSView *view in self.candidateStack.arrangedSubviews.copy) {
    [self.candidateStack removeArrangedSubview:view];
    [view removeFromSuperview];
  }
  self.candidateStack.accessibilitySelectedChildren = @[];
  self.candidateButtons = @[];
  self.selectedIndex = NSNotFound;
}

- (NSScreen *)screenForAnchorRect:(NSRect)anchorRect {
  NSRect anchor = CGRectStandardize(anchorRect);
  for (NSScreen *screen in NSScreen.screens) {
    if (NSPointInRect(anchor.origin, screen.frame))
      return screen;
  }
  NSScreen *bestScreen = nil;
  CGFloat bestArea = 0;
  for (NSScreen *screen in NSScreen.screens) {
    NSRect intersection = NSIntersectionRect(screen.frame, anchor);
    CGFloat area = NSWidth(intersection) * NSHeight(intersection);
    if (area > bestArea) {
      bestArea = area;
      bestScreen = screen;
    }
  }
  return bestScreen ?: NSScreen.mainScreen ?: NSScreen.screens.firstObject;
}

@end


#if RADISHLEX_CONTRACT_SMOKE
@implementation RLXCandidatePanel (ContractInspection)

- (NSPanel *)rlx_contractWindow {
  return self.window;
}

- (NSArray<NSButton *> *)rlx_contractCandidateButtons {
  return self.candidateButtons;
}

- (NSStackView *)rlx_contractCandidateStack {
  return self.candidateStack;
}

- (NSInteger)rlx_contractSelectedIndex {
  return self.selectedIndex;
}

- (id<RLXCandidatePanelOwner>)rlx_contractOwner {
  return self.owner;
}

@end
#endif
