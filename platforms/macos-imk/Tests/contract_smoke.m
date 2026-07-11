#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

#import "RadishLexBridge.h"
#import "RadishLexRuntime.h"

static void Require(BOOL condition, NSString *message) {
  if (!condition) {
    NSLog(@"contract smoke failed: %@", message);
    exit(1);
  }
}

static RadishLexKeyEvent Character(unichar character) {
  return (RadishLexKeyEvent){RADISHLEX_KEY_KIND_CHAR, character, 0, 0,
                             RADISHLEX_KEY_PHASE_PRESS};
}

int main(void) {
  @autoreleasepool {
    RadishLexKeyEvent normalized = {0};
    NSEvent *keyDown = [NSEvent keyEventWithType:NSEventTypeKeyDown
                                        location:NSZeroPoint
                                   modifierFlags:NSEventModifierFlagShift
                                       timestamp:0
                                    windowNumber:0
                                         context:nil
                                      characters:@"L"
                     charactersIgnoringModifiers:@"l"
                                       isARepeat:NO
                                         keyCode:kVK_ANSI_L];
    Require(RLXNormalizeKeyEvent(keyDown, &normalized) &&
                normalized.key_kind == RADISHLEX_KEY_KIND_CHAR &&
                normalized.codepoint == 'l' &&
                normalized.modifiers == RADISHLEX_KEY_MOD_SHIFT &&
                normalized.phase == RADISHLEX_KEY_PHASE_PRESS,
            @"character key normalization");
    NSEvent *keyUp = [NSEvent keyEventWithType:NSEventTypeKeyUp
                                      location:NSZeroPoint
                                 modifierFlags:0
                                     timestamp:0
                                  windowNumber:0
                                       context:nil
                                    characters:@""
                   charactersIgnoringModifiers:@""
                                     isARepeat:NO
                                       keyCode:kVK_Delete];
    Require(RLXNormalizeKeyEvent(keyUp, &normalized) &&
                normalized.named_key == RADISHLEX_NAMED_KEY_BACKSPACE &&
                normalized.phase == RADISHLEX_KEY_PHASE_RELEASE,
            @"named key release normalization");
    NSEvent *shiftDown = [NSEvent keyEventWithType:NSEventTypeFlagsChanged
                                          location:NSZeroPoint
                                     modifierFlags:NSEventModifierFlagShift
                                         timestamp:0
                                      windowNumber:0
                                           context:nil
                                        characters:@""
                       charactersIgnoringModifiers:@""
                                         isARepeat:NO
                                           keyCode:kVK_Shift];
    Require(RLXNormalizeKeyEvent(shiftDown, &normalized) &&
                normalized.named_key == RADISHLEX_NAMED_KEY_SHIFT &&
                normalized.phase == RADISHLEX_KEY_PHASE_PRESS,
            @"modifier-only normalization");
    NSEvent *unknownFunction = [NSEvent keyEventWithType:NSEventTypeKeyDown
                                                location:NSZeroPoint
                                           modifierFlags:0
                                               timestamp:0
                                            windowNumber:0
                                                 context:nil
                                              characters:[NSString stringWithFormat:@"%C", (unichar)NSF1FunctionKey]
                             charactersIgnoringModifiers:[NSString stringWithFormat:@"%C", (unichar)NSF1FunctionKey]
                                               isARepeat:NO
                                                 keyCode:kVK_F1];
    Require(!RLXNormalizeKeyEvent(unknownFunction, &normalized),
            @"unknown function key is returned to the host");

    NSError *error = nil;
    Require([RLXSessionBridge validateFFIContract:&error], @"ABI v2 contract");
    RLXSessionBridge *session =
        [[RLXProcessRuntime sharedRuntime] createSessionWithError:&error];
    Require(session != nil, @"create owner-thread session");

    RLXKeyHandlingResult *ignored = [session handleEvent:Character('!') error:&error];
    Require(ignored != nil && !ignored.isConsumed && ignored.commit == nil,
            @"unconsumed key is returned to the host");

    for (NSNumber *codepoint in @[@'l', @'u', @'o', @'b', @'o']) {
      RLXKeyHandlingResult *result =
          [session handleEvent:Character(codepoint.unsignedShortValue) error:&error];
      Require(result != nil && result.isConsumed, @"composition key is consumed");
    }
    RLXSnapshot *snapshot = [session snapshotWithError:&error];
    Require([snapshot.preedit isEqualToString:@"luobo"] && snapshot.cursor == 5,
            @"snapshot preedit and cursor");
    Require(snapshot.candidates.count == 2, @"snapshot candidates");
    NSAttributedString *candidate = RLXAttributedCandidate(snapshot.candidates[1]);
    Require([RLXCandidateIndexFromAttributedString(candidate) isEqualToNumber:@1],
            @"candidate display keeps the Rust index");
    RLXCandidateCommitResult *candidateCommit =
        [session commitCandidateAtIndex:1 error:&error];
    Require([candidateCommit.commit isEqualToString:@"萝卜词核"] &&
                candidateCommit.snapshot.preedit.length == 0,
            @"candidate commit and post-commit snapshot");

    Require([session setSchema:@"contract.schema" error:&error], @"schema switch");
    for (NSNumber *codepoint in @[@'c', @'i', @'h', @'e']) {
      Require([[session handleEvent:Character(codepoint.unsignedShortValue) error:&error]
                  isConsumed],
              @"second composition");
    }
    RadishLexKeyEvent enter = {RADISHLEX_KEY_KIND_NAMED, 0, RADISHLEX_NAMED_KEY_ENTER, 0,
                                RADISHLEX_KEY_PHASE_PRESS};
    RLXKeyHandlingResult *immediate = [session handleEvent:enter error:&error];
    Require(immediate.isConsumed && [immediate.commit isEqualToString:@"cihe"] &&
                immediate.snapshot.preedit.length == 0,
            @"same-event immediate commit and snapshot");

    Require([session handleEvent:Character('l') error:&error].isConsumed,
            @"reset setup");
    Require([session resetWithError:&error] &&
                [session snapshotWithError:&error].preedit.length == 0,
            @"reset clears composition");

    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    __block RLXKeyHandlingResult *crossThreadResult = nil;
    __block NSError *crossThreadError = nil;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      crossThreadResult = [session handleEvent:Character('x') error:&crossThreadError];
      dispatch_semaphore_signal(done);
    });
    dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
    Require(crossThreadResult == nil &&
                crossThreadError.code == RADISHLEX_STATUS_INVALID_STATE,
            @"owner-thread violation is rejected");

    Require([[RLXProcessRuntime sharedRuntime] shutdownWithError:&error],
            @"process teardown");
    Require(!session.isValid, @"process teardown releases sessions before shutdown");
    NSLog(@"macOS InputMethodKit wrapper contract smoke passed");
  }
  return 0;
}
