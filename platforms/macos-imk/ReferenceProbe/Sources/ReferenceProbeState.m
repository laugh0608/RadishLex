#import "ReferenceProbeState.h"

#import <Carbon/Carbon.h>

@interface RLXReferenceProbeState ()
@property(nonatomic, copy) NSArray<NSAttributedString *> *candidates;
@property(nonatomic) NSInteger selectedIndex;
@end

RLXReferenceProbeEventRoute RLXReferenceProbeRouteForEvent(NSEvent *event) {
  NSEventModifierFlags modifiers = event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
  if ((modifiers & (NSEventModifierFlagCommand | NSEventModifierFlagControl |
                    NSEventModifierFlagOption | NSEventModifierFlagShift)) != 0) {
    return RLXReferenceProbeEventRouteHostApplication;
  }

  switch (event.keyCode) {
    case kVK_LeftArrow:
    case kVK_RightArrow:
    case kVK_UpArrow:
    case kVK_DownArrow:
      return RLXReferenceProbeEventRouteCandidatePanel;
    case kVK_Space:
      return RLXReferenceProbeEventRouteCommitCandidate;
    case kVK_Return:
    case kVK_ANSI_KeypadEnter:
      return RLXReferenceProbeEventRouteCommitRaw;
    default:
      return RLXReferenceProbeEventRouteControllerInput;
  }
}

@implementation RLXReferenceProbeState

- (instancetype)initWithCandidates:(NSArray<NSAttributedString *> *)candidates {
  self = [super init];
  if (self == nil) return nil;
  _candidates = [candidates copy];
  _selectedIndex = candidates.count > 0 ? 0 : NSNotFound;
  return self;
}

- (BOOL)updateSelectionFromCandidate:(NSAttributedString *)candidate {
  NSInteger matchedIndex = NSNotFound;
  for (NSUInteger index = 0; index < self.candidates.count; index++) {
    if ([self.candidates[index].string isEqualToString:candidate.string]) {
      if (matchedIndex != NSNotFound) return NO;
      matchedIndex = (NSInteger)index;
    }
  }
  if (matchedIndex == NSNotFound) return NO;
  self.selectedIndex = matchedIndex;
  return YES;
}

- (void)resetSelection {
  self.selectedIndex = self.candidates.count > 0 ? 0 : NSNotFound;
}

- (NSAttributedString *)selectedCandidate {
  if (self.selectedIndex == NSNotFound || self.selectedIndex < 0 ||
      (NSUInteger)self.selectedIndex >= self.candidates.count) {
    return nil;
  }
  return self.candidates[(NSUInteger)self.selectedIndex];
}

@end
