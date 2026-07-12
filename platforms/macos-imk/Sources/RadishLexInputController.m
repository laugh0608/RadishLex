#import "RadishLexInputController.h"

#import <Carbon/Carbon.h>

#import "RadishLexBridge.h"
#import "RadishLexRuntime.h"

static IMKCandidates *RLXProcessCandidatePanel(IMKServer *server) {
  static IMKCandidates *candidatePanel;
  static dispatch_once_t onceToken;
  dispatch_once(&onceToken, ^{
    candidatePanel = [[IMKCandidates alloc] initWithServer:server
                                                 panelType:kIMKScrollingGridCandidatePanel];
    NSMutableDictionary *candidateAttributes =
        [NSMutableDictionary dictionaryWithDictionary:candidatePanel.attributes ?: @{}];
    candidateAttributes[IMKCandidatesSendServerKeyEventFirst] = @YES;
    [candidatePanel setAttributes:candidateAttributes];
    [candidatePanel setDismissesAutomatically:NO];
  });
  return candidatePanel;
}

@interface RadishLexInputController ()
@property(nonatomic, strong) RLXSessionBridge *session;
@property(nonatomic, strong) RLXSnapshot *snapshot;
@property(nonatomic, strong) IMKCandidates *candidatePanel;
@property(nonatomic, copy) NSArray<NSAttributedString *> *candidatePanelCandidates;
@property(nonatomic) NSInteger candidatePanelIndex;
- (NSInteger)candidateIndexForAttributedString:(NSAttributedString *)candidateString;
- (BOOL)moveCandidateSelectionByOffset:(NSInteger)offset;
- (BOOL)selectCandidateAtIndex:(NSUInteger)index client:(id)sender;
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
  _candidatePanel = RLXProcessCandidatePanel(server);
  if (_candidatePanel == nil) {
    NSLog(@"RadishLex candidate panel creation failed");
    [_session invalidate];
    return nil;
  }
  _candidatePanelCandidates = @[];
  _candidatePanelIndex = NSNotFound;
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

  if (self.snapshot.preedit.length > 0 && self.snapshot.candidates.count > 0 &&
      normalized.phase == RADISHLEX_KEY_PHASE_PRESS && normalized.modifiers == 0 &&
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
      normalized.phase == RADISHLEX_KEY_PHASE_PRESS && normalized.modifiers == 0 &&
      normalized.key_kind == RADISHLEX_KEY_KIND_NAMED &&
      normalized.named_key == RADISHLEX_NAMED_KEY_SPACE) {
    if (self.candidatePanelIndex != NSNotFound && self.candidatePanelIndex >= 0 &&
        (NSUInteger)self.candidatePanelIndex < self.snapshot.candidates.count) {
      return [self selectCandidateAtIndex:(NSUInteger)self.candidatePanelIndex client:sender];
    }
  }

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
  self.candidatePanelCandidates = [candidateData copy];
  self.candidatePanelIndex = candidateData.count > 0 ? 0 : NSNotFound;
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
  NSInteger index = [self candidateIndexForAttributedString:candidateString];
  if (index == NSNotFound) {
    [self recoverFromError:nil client:self.client];
    return;
  }
  [self selectCandidateAtIndex:(NSUInteger)index client:self.client];
}

- (void)candidateSelectionChanged:(NSAttributedString *)candidateString {
  if (![NSThread isMainThread] || self.session == nil) return;
  NSInteger index = [self candidateIndexForAttributedString:candidateString];
  if (index == NSNotFound) {
    self.candidatePanelIndex = NSNotFound;
    NSLog(@"RadishLex candidate highlight index is unavailable");
    return;
  }
  self.candidatePanelIndex = index;
}

- (NSInteger)candidateIndexForAttributedString:(NSAttributedString *)candidateString {
  if (candidateString.length == 0 || self.snapshot.candidates.count == 0) {
    return NSNotFound;
  }
  NSInteger identifier = [self.candidatePanel candidateStringIdentifier:candidateString];
  if (identifier == NSNotFound) return NSNotFound;
  for (NSUInteger index = 0; index < self.candidatePanelCandidates.count; index++) {
    NSInteger visibleIdentifier = [self.candidatePanel
        candidateStringIdentifier:self.candidatePanelCandidates[index]];
    if (visibleIdentifier == identifier) return (NSInteger)index;
  }
  return NSNotFound;
}

- (BOOL)moveCandidateSelectionByOffset:(NSInteger)offset {
  NSUInteger candidateCount = self.candidatePanelCandidates.count;
  if (candidateCount == 0 || candidateCount != self.snapshot.candidates.count) return NO;
  NSInteger current = self.candidatePanelIndex;
  if (current == NSNotFound || current < 0 || (NSUInteger)current >= candidateCount) {
    current = 0;
  }
  NSInteger target = current + offset;
  if (target < 0 || (NSUInteger)target >= candidateCount) {
    return YES;
  }
  NSInteger identifier = [self.candidatePanel
      candidateStringIdentifier:self.candidatePanelCandidates[(NSUInteger)target]];
  if (identifier == NSNotFound) {
    NSLog(@"RadishLex candidate panel rejected a visible selection");
    return YES;
  }
  if (![self.candidatePanel selectCandidateWithIdentifier:identifier]) {
    NSLog(@"RadishLex candidate panel rejected a visible selection");
    return YES;
  }
  self.candidatePanelIndex = target;
  NSLog(@"RadishLex candidate highlight moved to index %ld", (long)target);
  return YES;
}

- (BOOL)selectCandidateAtIndex:(NSUInteger)index client:(id)sender {
  if (![NSThread isMainThread] || self.session == nil) return NO;
  NSError *error = nil;
  RLXKeyHandlingResult *result =
      [self.session selectCandidateAtIndex:index error:&error];
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

- (NSMenu *)menu {
  return nil;
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
