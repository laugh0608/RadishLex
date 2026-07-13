#import "RadishLexInputController.h"

#import <Carbon/Carbon.h>

#import "RadishLexBridge.h"
#import "RadishLexCandidatePanel.h"
#import "RadishLexRuntime.h"

@interface RadishLexInputController () <RLXCandidatePanelOwner>
@property(nonatomic, strong) RLXSessionBridge *session;
@property(nonatomic, strong) RLXSnapshot *snapshot;
@property(nonatomic, strong) RLXCandidatePanel *candidatePanel;
@property(nonatomic) NSInteger candidatePanelIndex;
- (BOOL)moveCandidateSelectionByOffset:(NSInteger)offset;
- (BOOL)selectCandidateAtIndex:(NSUInteger)index client:(id)sender;
@end

@implementation RadishLexInputController

- (instancetype)initWithServer:(IMKServer *)server
                      delegate:(id)delegate
                        client:(id)inputClient {
  if (![NSThread isMainThread]) {
    return nil;
  }
  self = [super initWithServer:server delegate:delegate client:inputClient];
  if (self == nil) {
    return nil;
  }
  NSError *error = nil;
  _session = [[RLXProcessRuntime sharedRuntime] createSessionWithError:&error];
  if (_session == nil) {
    NSLog(@"RadishLex session creation failed (%@:%ld)", error.domain,
          (long)error.code);
    return nil;
  }
  _snapshot = [_session snapshotWithError:&error];
  if (_snapshot == nil) {
    NSLog(@"RadishLex initial snapshot failed (%@:%ld)", error.domain,
          (long)error.code);
    [_session invalidate];
    return nil;
  }
  _candidatePanel = [RLXCandidatePanel sharedPanel];
  if (_candidatePanel == nil) {
    NSLog(@"RadishLex candidate panel creation failed");
    [_session invalidate];
    return nil;
  }
  _candidatePanelIndex = NSNotFound;
  return self;
}

- (NSUInteger)recognizedEvents:(id)sender {
  (void)sender;
  return NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged;
}

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
  if (![NSThread isMainThread] || self.session == nil)
    return NO;
  RadishLexKeyEvent normalized = {0};
  if (!RLXNormalizeKeyEvent(event, &normalized))
    return NO;

  if (self.snapshot.preedit.length > 0 && self.snapshot.candidates.count > 0 &&
      normalized.phase == RADISHLEX_KEY_PHASE_PRESS &&
      normalized.modifiers == 0 &&
      normalized.key_kind == RADISHLEX_KEY_KIND_NAMED) {
    switch (normalized.named_key) {
    case RADISHLEX_NAMED_KEY_ARROW_UP:
    case RADISHLEX_NAMED_KEY_ARROW_LEFT:
      return [self moveCandidateSelectionByOffset:-1];
    case RADISHLEX_NAMED_KEY_ARROW_DOWN:
    case RADISHLEX_NAMED_KEY_ARROW_RIGHT:
      return [self moveCandidateSelectionByOffset:1];
    default:
      break;
    }
  }

  if (self.snapshot.preedit.length > 0 && self.snapshot.candidates.count > 0 &&
      normalized.phase == RADISHLEX_KEY_PHASE_PRESS &&
      normalized.modifiers == 0 &&
      normalized.key_kind == RADISHLEX_KEY_KIND_NAMED &&
      normalized.named_key == RADISHLEX_NAMED_KEY_SPACE) {
    if (self.candidatePanelIndex != NSNotFound &&
        self.candidatePanelIndex >= 0 &&
        (NSUInteger)self.candidatePanelIndex < self.snapshot.candidates.count) {
      return [self selectCandidateAtIndex:(NSUInteger)self.candidatePanelIndex
                                   client:sender];
    }
  }

  if (normalized.phase == RADISHLEX_KEY_PHASE_PRESS &&
      normalized.key_kind == RADISHLEX_KEY_KIND_NAMED &&
      normalized.named_key == RADISHLEX_NAMED_KEY_ESCAPE &&
      self.snapshot.preedit.length > 0) {
    return [self cancelCompositionForClient:sender];
  }

  NSError *error = nil;
  RLXKeyHandlingResult *result = [self.session handleEvent:normalized
                                                     error:&error];
  if (result == nil) {
    [self recoverFromError:error client:sender];
    return NO;
  }
  if (result.commit != nil) {
    [(id<IMKTextInput>)sender insertText:result.commit
                        replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  }
  if (result.snapshot != nil) {
    [self applySnapshot:result.snapshot client:sender];
  } else {
    [self recoverFromError:error client:sender];
  }
  return result.isConsumed;
}

- (void)applySnapshot:(RLXSnapshot *)snapshot client:(id)sender {
  self.snapshot = snapshot;
  [(id<IMKTextInput>)sender setMarkedText:snapshot.preedit
                           selectionRange:NSMakeRange(snapshot.cursor, 0)
                         replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  NSMutableArray<NSAttributedString *> *candidateData = [NSMutableArray array];
  for (RLXCandidate *candidate in snapshot.candidates) {
    [candidateData addObject:RLXAttributedCandidate(candidate)];
  }
  self.candidatePanelIndex = candidateData.count > 0 ? 0 : NSNotFound;
  if (snapshot.preedit.length > 0 && candidateData.count > 0) {
    [self.candidatePanel showCandidates:candidateData
                          selectedIndex:self.candidatePanelIndex
                            cursorIndex:snapshot.cursor
                                 client:(id<IMKTextInput>)sender
                                  owner:self];
  } else {
    [self.candidatePanel hideForOwner:self];
  }
}

- (id)composedString:(id)sender {
  (void)sender;
  return self.snapshot.preedit ?: @"";
}

- (NSRange)selectionRange {
  return NSMakeRange(self.snapshot.cursor, 0);
}

- (BOOL)moveCandidateSelectionByOffset:(NSInteger)offset {
  NSUInteger candidateCount = self.snapshot.candidates.count;
  if (candidateCount == 0)
    return NO;
  NSInteger target = RLXCandidateIndexByMoving(self.candidatePanelIndex, offset,
                                               candidateCount);
  if (target == NSNotFound)
    return NO;
  if (![self.candidatePanel setSelectedIndex:target owner:self]) {
    NSLog(@"RadishLex candidate panel rejected a visible selection");
    return YES;
  }
  self.candidatePanelIndex = target;
  return YES;
}

- (void)candidatePanelDidRequestSelectionAtIndex:(NSUInteger)index {
  if (![NSThread isMainThread] || self.session == nil ||
      index >= self.snapshot.candidates.count) {
    return;
  }
  [self selectCandidateAtIndex:index client:self.client];
}

- (BOOL)selectCandidateAtIndex:(NSUInteger)index client:(id)sender {
  if (![NSThread isMainThread] || self.session == nil)
    return NO;
  NSError *error = nil;
  RLXKeyHandlingResult *result = [self.session selectCandidateAtIndex:index
                                                                error:&error];
  if (result == nil) {
    [self recoverFromError:error client:sender];
    return NO;
  }
  if (result.commit != nil) {
    [(id<IMKTextInput>)sender insertText:result.commit
                        replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  }
  if (result.snapshot != nil) {
    [self applySnapshot:result.snapshot client:sender];
  } else {
    [self recoverFromError:error client:sender];
    return NO;
  }
  return result.isConsumed;
}

- (void)commitComposition:(id)sender {
  if (self.snapshot.preedit.length == 0)
    return;
  RadishLexKeyEvent enter = {RADISHLEX_KEY_KIND_NAMED, 0,
                             RADISHLEX_NAMED_KEY_ENTER, 0,
                             RADISHLEX_KEY_PHASE_PRESS};
  NSError *error = nil;
  RLXKeyHandlingResult *result = [self.session handleEvent:enter error:&error];
  if (result == nil || result.commit == nil) {
    [self recoverFromError:error client:sender];
    return;
  }
  [(id<IMKTextInput>)sender insertText:result.commit
                      replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  if (result.snapshot != nil) {
    [self applySnapshot:result.snapshot client:sender];
  } else {
    [self recoverFromError:error client:sender];
  }
}

- (BOOL)cancelCompositionForClient:(id)sender {
  NSError *error = nil;
  if (![self.session resetWithError:&error]) {
    [self recoverFromError:error client:sender];
    return NO;
  }
  RLXSnapshot *snapshot = [self.session snapshotWithError:&error];
  if (snapshot == nil) {
    [self recoverFromError:error client:sender];
    return NO;
  }
  [self applySnapshot:snapshot client:sender];
  return YES;
}

- (void)deactivateServer:(id)sender {
  if (self.snapshot.preedit.length > 0) {
    [self cancelCompositionForClient:sender];
  }
  [self.candidatePanel hideForOwner:self];
  [super deactivateServer:sender];
}

- (NSMenu *)menu {
  return nil;
}

- (void)inputControllerWillClose {
  [self.candidatePanel hideForOwner:self];
  [self.session invalidate];
  [[RLXProcessRuntime sharedRuntime] forgetSession:self.session];
  self.session = nil;
  [super inputControllerWillClose];
}

- (void)recoverFromError:(NSError *)error client:(id)sender {
  NSLog(@"RadishLex input call failed (%@:%ld)",
        error.domain ?: RLXBridgeErrorDomain, (long)error.code);
  NSError *resetError = nil;
  if ([self.session resetWithError:&resetError]) {
    RLXSnapshot *snapshot = [self.session snapshotWithError:&resetError];
    if (snapshot != nil)
      [self applySnapshot:snapshot client:sender];
  }
  [self.candidatePanel hideForOwner:self];
}

@end
