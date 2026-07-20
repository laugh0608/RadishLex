#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

#import "ReferenceProbeState.h"

static void Require(BOOL condition, NSString *message) {
  if (!condition) {
    NSLog(@"reference probe contract failed: %@", message);
    exit(1);
  }
}

static NSEvent *KeyEvent(unsigned short keyCode, NSEventModifierFlags modifiers,
                         NSString *characters) {
  return [NSEvent keyEventWithType:NSEventTypeKeyDown
                          location:NSZeroPoint
                     modifierFlags:modifiers
                         timestamp:0
                      windowNumber:0
                           context:nil
                        characters:characters
       charactersIgnoringModifiers:characters
                         isARepeat:NO
                           keyCode:keyCode];
}

int main(void) {
  @autoreleasepool {
    for (NSNumber *keyCode in @[@(kVK_LeftArrow), @(kVK_RightArrow), @(kVK_UpArrow),
                                @(kVK_DownArrow)]) {
      Require(RLXReferenceProbeRouteForEvent(KeyEvent(keyCode.unsignedShortValue, 0, @"")) ==
                  RLXReferenceProbeEventRouteCandidatePanel,
              @"four direction keys are returned to IMKCandidates");
    }
    Require(RLXReferenceProbeRouteForEvent(KeyEvent(kVK_Space, 0, @" ")) ==
                RLXReferenceProbeEventRouteCommitCandidate,
            @"Space remains controller-owned");
    Require(RLXReferenceProbeRouteForEvent(KeyEvent(kVK_Return, 0, @"\r")) ==
                RLXReferenceProbeEventRouteCommitRaw,
            @"Enter remains controller-owned");
    Require(RLXReferenceProbeRouteForEvent(
                KeyEvent(kVK_ANSI_A, NSEventModifierFlagCommand, @"a")) ==
                RLXReferenceProbeEventRouteHostApplication,
            @"Command shortcuts return to the host application");
    Require(RLXReferenceProbeRouteForEvent(
                KeyEvent(kVK_LeftArrow, NSEventModifierFlagShift, @"")) ==
                RLXReferenceProbeEventRouteHostApplication,
            @"modified direction keys return to the host application");

    NSArray<NSAttributedString *> *candidates = @[
      [[NSAttributedString alloc] initWithString:@"候选甲"],
      [[NSAttributedString alloc] initWithString:@"候选乙"],
      [[NSAttributedString alloc] initWithString:@"候选丙"],
    ];
    RLXReferenceProbeState *state =
        [[RLXReferenceProbeState alloc] initWithCandidates:candidates];
    Require(state.selectedIndex == 0, @"first candidate is the initial selection");
    Require([state updateSelectionFromCandidate:candidates[2]],
            @"selection callback maps to a stable index");
    Require(state.selectedIndex == 2 &&
                [state.selectedCandidate.string isEqualToString:@"候选丙"],
            @"callback index and Space commit candidate stay aligned");
    [state resetSelection];
    Require(state.selectedIndex == 0, @"new composition resets to the first candidate");
    Require(![state updateSelectionFromCandidate:
                  [[NSAttributedString alloc] initWithString:@"不存在"]],
            @"unknown callback candidates are rejected");

    RLXReferenceProbeState *ambiguous = [[RLXReferenceProbeState alloc]
        initWithCandidates:@[
          [[NSAttributedString alloc] initWithString:@"重复"],
          [[NSAttributedString alloc] initWithString:@"重复"],
        ]];
    Require(![ambiguous updateSelectionFromCandidate:ambiguous.candidates[0]],
            @"ambiguous callback candidates are rejected");
  }
  return 0;
}
