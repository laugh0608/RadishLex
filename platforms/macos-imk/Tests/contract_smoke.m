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

static NSEvent *KeyEvent(NSEventType type, unsigned short keyCode,
                         NSEventModifierFlags modifiers, NSString *characters) {
  return [NSEvent keyEventWithType:type
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

    NSArray<NSNumber *> *namedKeyCodes = @[
      @(kVK_Space), @(kVK_Return), @(kVK_ANSI_KeypadEnter), @(kVK_Delete),
      @(kVK_Escape), @(kVK_Tab), @(kVK_UpArrow), @(kVK_DownArrow),
      @(kVK_LeftArrow), @(kVK_RightArrow), @(kVK_PageUp), @(kVK_PageDown),
      @(kVK_Shift), @(kVK_RightShift), @(kVK_Control), @(kVK_RightControl),
      @(kVK_Option), @(kVK_RightOption), @(kVK_Command), @(kVK_RightCommand)
    ];
    NSArray<NSNumber *> *namedKeys = @[
      @(RADISHLEX_NAMED_KEY_SPACE), @(RADISHLEX_NAMED_KEY_ENTER),
      @(RADISHLEX_NAMED_KEY_ENTER), @(RADISHLEX_NAMED_KEY_BACKSPACE),
      @(RADISHLEX_NAMED_KEY_ESCAPE), @(RADISHLEX_NAMED_KEY_TAB),
      @(RADISHLEX_NAMED_KEY_ARROW_UP), @(RADISHLEX_NAMED_KEY_ARROW_DOWN),
      @(RADISHLEX_NAMED_KEY_ARROW_LEFT), @(RADISHLEX_NAMED_KEY_ARROW_RIGHT),
      @(RADISHLEX_NAMED_KEY_PAGE_UP), @(RADISHLEX_NAMED_KEY_PAGE_DOWN),
      @(RADISHLEX_NAMED_KEY_SHIFT), @(RADISHLEX_NAMED_KEY_SHIFT),
      @(RADISHLEX_NAMED_KEY_CONTROL), @(RADISHLEX_NAMED_KEY_CONTROL),
      @(RADISHLEX_NAMED_KEY_ALT), @(RADISHLEX_NAMED_KEY_ALT),
      @(RADISHLEX_NAMED_KEY_META), @(RADISHLEX_NAMED_KEY_META)
    ];
    for (NSUInteger index = 0; index < namedKeyCodes.count; ++index) {
      NSEvent *namedEvent = KeyEvent(NSEventTypeKeyDown,
                                      namedKeyCodes[index].unsignedShortValue, 0, @"");
      Require(RLXNormalizeKeyEvent(namedEvent, &normalized) &&
                  normalized.key_kind == RADISHLEX_KEY_KIND_NAMED &&
                  normalized.named_key == namedKeys[index].unsignedIntValue,
              @"complete named key mapping");
    }

    NSEventModifierFlags allCocoaModifiers = NSEventModifierFlagShift |
        NSEventModifierFlagControl | NSEventModifierFlagOption |
        NSEventModifierFlagCommand;
    NSEvent *allModifiers = KeyEvent(NSEventTypeKeyDown, kVK_ANSI_A,
                                      allCocoaModifiers, @"a");
    Require(RLXNormalizeKeyEvent(allModifiers, &normalized) &&
                normalized.modifiers == (RADISHLEX_KEY_MOD_SHIFT |
                                         RADISHLEX_KEY_MOD_CONTROL |
                                         RADISHLEX_KEY_MOD_ALT |
                                         RADISHLEX_KEY_MOD_META),
            @"complete modifier mapping");
    NSEvent *shiftUp = KeyEvent(NSEventTypeFlagsChanged, kVK_Shift, 0, @"");
    Require(RLXNormalizeKeyEvent(shiftUp, &normalized) &&
                normalized.phase == RADISHLEX_KEY_PHASE_RELEASE,
            @"modifier-only release normalization");
    NSEvent *emoji = KeyEvent(NSEventTypeKeyDown, kVK_ANSI_A, 0, @"😀");
    Require(RLXNormalizeKeyEvent(emoji, &normalized) && normalized.codepoint == 0x1F600,
            @"supplementary Unicode scalar normalization");
    Require(!RLXNormalizeKeyEvent(KeyEvent(NSEventTypeKeyDown, kVK_ANSI_A, 0, @"ab"),
                                  &normalized),
            @"multiple scalars are returned to the host");

    NSString *mixed = @"a萝卜😀z";
    NSError *cursorError = nil;
    Require(RLXUTF16CursorForUTF8Offset(mixed, 0, &cursorError) == 0 &&
                RLXUTF16CursorForUTF8Offset(mixed, 1, &cursorError) == 1 &&
                RLXUTF16CursorForUTF8Offset(mixed, 4, &cursorError) == 2 &&
                RLXUTF16CursorForUTF8Offset(mixed, 7, &cursorError) == 3 &&
                RLXUTF16CursorForUTF8Offset(mixed, 11, &cursorError) == 5 &&
                RLXUTF16CursorForUTF8Offset(mixed, 12, &cursorError) == 6,
            @"UTF-8 byte cursor converts to Cocoa UTF-16 units");
    cursorError = nil;
    Require(RLXUTF16CursorForUTF8Offset(mixed, 2, &cursorError) == NSNotFound &&
                cursorError.code == RADISHLEX_STATUS_INTERNAL_ERROR,
            @"cursor inside a UTF-8 scalar is rejected");
    Require(RLXCandidateIndexFromAttributedString(
                [[NSAttributedString alloc] initWithString:@"unindexed"]) == nil,
            @"candidate without a Rust index is rejected");

    NSError *error = nil;
    Require([RLXSessionBridge validateFFIContract:&error], @"ABI v2 contract");
    RLXSessionBridge *session =
        [[RLXProcessRuntime sharedRuntime] createSessionWithError:&error];
    Require(session != nil, @"create owner-thread session");

    RLXKeyHandlingResult *ignored = [session handleEvent:Character('!') error:&error];
    Require(ignored != nil && !ignored.isConsumed && ignored.commit == nil,
            @"unconsumed key is returned to the host");

    RadishLexKeyEvent commandC = {RADISHLEX_KEY_KIND_CHAR, 'c', 0,
                                   RADISHLEX_KEY_MOD_META,
                                   RADISHLEX_KEY_PHASE_PRESS};
    ignored = [session handleEvent:commandC error:&error];
    Require(ignored != nil && !ignored.isConsumed && ignored.commit == nil &&
                ignored.snapshot.preedit.length == 0,
            @"Command-modified character is returned to the host");

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
