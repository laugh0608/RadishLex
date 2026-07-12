#import "RadishLexInputController.h"

#import <Carbon/Carbon.h>

#import "RadishLexBridge.h"
#import "RadishLexRuntime.h"

@interface RadishLexInputController ()
@property(nonatomic, strong) RLXSessionBridge *session;
@property(nonatomic, strong) RLXSnapshot *snapshot;
@property(nonatomic, strong) IMKCandidates *candidatePanel;
@end

@implementation RadishLexInputController

- (instancetype)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)inputClient {
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
    NSLog(@"RadishLex session creation failed (%@:%ld)", error.domain, (long)error.code);
    return nil;
  }
  _snapshot = [_session snapshotWithError:&error];
  if (_snapshot == nil) {
    NSLog(@"RadishLex initial snapshot failed (%@:%ld)", error.domain, (long)error.code);
    [_session invalidate];
    return nil;
  }
  _candidatePanel = [[IMKCandidates alloc] initWithServer:server
                                                panelType:kIMKScrollingGridCandidatePanel];
  NSMutableDictionary *candidateAttributes =
      [NSMutableDictionary dictionaryWithDictionary:_candidatePanel.attributes ?: @{}];
  candidateAttributes[IMKCandidatesSendServerKeyEventFirst] = @YES;
  [_candidatePanel setAttributes:candidateAttributes];
  [_candidatePanel setDismissesAutomatically:NO];
  return self;
}

- (NSUInteger)recognizedEvents:(id)sender {
  (void)sender;
  return NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged;
}

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
  if (![NSThread isMainThread] || self.session == nil) return NO;
  RadishLexKeyEvent normalized = {0};
  if (!RLXNormalizeKeyEvent(event, &normalized)) return NO;

  if (normalized.phase == RADISHLEX_KEY_PHASE_PRESS &&
      normalized.key_kind == RADISHLEX_KEY_KIND_NAMED &&
      normalized.named_key == RADISHLEX_NAMED_KEY_ESCAPE &&
      self.snapshot.preedit.length > 0) {
    return [self cancelCompositionForClient:sender];
  }

  NSError *error = nil;
  RLXKeyHandlingResult *result = [self.session handleEvent:normalized error:&error];
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
  [self.candidatePanel setCandidateData:candidateData];
  if (snapshot.preedit.length > 0 && candidateData.count > 0) {
    [self.candidatePanel show:kIMKLocateCandidatesBelowHint];
  } else {
    [self.candidatePanel hide];
  }
}

- (NSArray *)candidates:(id)sender {
  (void)sender;
  NSMutableArray<NSAttributedString *> *values = [NSMutableArray array];
  for (RLXCandidate *candidate in self.snapshot.candidates) {
    [values addObject:RLXAttributedCandidate(candidate)];
  }
  return values;
}

- (id)composedString:(id)sender {
  (void)sender;
  return self.snapshot.preedit ?: @"";
}

- (NSRange)selectionRange {
  return NSMakeRange(self.snapshot.cursor, 0);
}

- (void)candidateSelected:(NSAttributedString *)candidateString {
  if (![NSThread isMainThread] || self.session == nil) return;
  NSNumber *index = RLXCandidateIndexFromAttributedString(candidateString);
  if (index == nil) {
    [self recoverFromError:nil client:self.client];
    return;
  }
  NSError *error = nil;
  RLXCandidateCommitResult *result =
      [self.session commitCandidateAtIndex:index.unsignedIntegerValue error:&error];
  if (result == nil) {
    [self recoverFromError:error client:self.client];
    return;
  }
  [self.client insertText:result.commit replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  if (result.snapshot != nil) {
    [self applySnapshot:result.snapshot client:self.client];
  } else {
    [self recoverFromError:error client:self.client];
  }
}

- (void)commitComposition:(id)sender {
  if (self.snapshot.preedit.length == 0) return;
  RadishLexKeyEvent enter = {RADISHLEX_KEY_KIND_NAMED, 0, RADISHLEX_NAMED_KEY_ENTER, 0,
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
  [self.candidatePanel hide];
  [super deactivateServer:sender];
}

- (void)inputControllerWillClose {
  [self.candidatePanel hide];
  [self.session invalidate];
  [[RLXProcessRuntime sharedRuntime] forgetSession:self.session];
  self.session = nil;
  [super inputControllerWillClose];
}

- (void)recoverFromError:(NSError *)error client:(id)sender {
  NSLog(@"RadishLex input call failed (%@:%ld)", error.domain ?: RLXBridgeErrorDomain,
        (long)error.code);
  NSError *resetError = nil;
  if ([self.session resetWithError:&resetError]) {
    RLXSnapshot *snapshot = [self.session snapshotWithError:&resetError];
    if (snapshot != nil) [self applySnapshot:snapshot client:sender];
  }
  [self.candidatePanel hide];
}

@end
