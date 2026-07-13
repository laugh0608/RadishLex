#import "RadishLexInputController.h"

#import <Carbon/Carbon.h>

#import "RadishLexBridge.h"
#import "RadishLexCandidatePanel.h"
#import "RadishLexRuntime.h"

static BOOL RLXNullableStringsEqual(NSString *left, NSString *right) {
  return left == right || [left isEqualToString:right];
}

static BOOL RLXSnapshotsHaveSameCandidatePresentation(RLXSnapshot *left,
                                                       RLXSnapshot *right) {
  if (left == right)
    return YES;
  if (left == nil || right == nil || left.cursor != right.cursor ||
      ![left.schema isEqualToString:right.schema] ||
      ![left.preedit isEqualToString:right.preedit] ||
      left.candidates.count != right.candidates.count) {
    return NO;
  }
  for (NSUInteger index = 0; index < left.candidates.count; ++index) {
    RLXCandidate *leftCandidate = left.candidates[index];
    RLXCandidate *rightCandidate = right.candidates[index];
    if (leftCandidate.index != rightCandidate.index ||
        leftCandidate.source != rightCandidate.source ||
        ![leftCandidate.text isEqualToString:rightCandidate.text] ||
        !RLXNullableStringsEqual(leftCandidate.reading,
                                 rightCandidate.reading) ||
        !RLXNullableStringsEqual(leftCandidate.annotation,
                                 rightCandidate.annotation)) {
      return NO;
    }
  }
  return YES;
}

@interface RadishLexInputController () <RLXCandidatePanelOwner>
@property(nonatomic, strong) RLXSessionBridge *session;
@property(nonatomic, strong) RLXSnapshot *snapshot;
@property(nonatomic, strong) RLXCandidatePanel *candidatePanel;
@property(nonatomic) NSInteger candidatePanelIndex;
#if RADISHLEX_CONTRACT_SMOKE
@property(nonatomic, strong) id contractClient;
#endif
- (BOOL)prepareSessionAndCandidatePanel;
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
  if (![self prepareSessionAndCandidatePanel])
    return nil;
  return self;
}

- (BOOL)prepareSessionAndCandidatePanel {
  NSError *error = nil;
  self.session = [[RLXProcessRuntime sharedRuntime] createSessionWithError:&error];
  if (self.session == nil) {
    NSLog(@"RadishLex session creation failed (%@:%ld)", error.domain,
          (long)error.code);
    return NO;
  }
  self.snapshot = [self.session snapshotWithError:&error];
  if (self.snapshot == nil) {
    NSLog(@"RadishLex initial snapshot failed (%@:%ld)", error.domain,
          (long)error.code);
    [self.session invalidate];
    self.session = nil;
    return NO;
  }
  self.candidatePanel = [RLXCandidatePanel sharedPanel];
  if (self.candidatePanel == nil) {
    NSLog(@"RadishLex candidate panel creation failed");
    [self.session invalidate];
    self.session = nil;
    return NO;
  }
  self.candidatePanelIndex = NSNotFound;
  return YES;
}

#if RADISHLEX_CONTRACT_SMOKE
- (instancetype)initForContractWithClient:(id)inputClient {
  self = [super init];
  if (self == nil)
    return nil;
  self.contractClient = inputClient;
  if (![self prepareSessionAndCandidatePanel])
    return nil;
  return self;
}

- (id<IMKTextInput, NSObject>)client {
  if (self.contractClient != nil)
    return (id<IMKTextInput, NSObject>)self.contractClient;
  return [super client];
}
#endif

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
  BOOL preserveCandidateSelection =
      RLXSnapshotsHaveSameCandidatePresentation(self.snapshot, snapshot);
  NSInteger nextCandidateIndex = 0;
  if (preserveCandidateSelection && self.candidatePanelIndex != NSNotFound &&
      self.candidatePanelIndex >= 0 &&
      (NSUInteger)self.candidatePanelIndex < snapshot.candidates.count) {
    nextCandidateIndex = self.candidatePanelIndex;
  }
  self.snapshot = snapshot;
  [(id<IMKTextInput>)sender setMarkedText:snapshot.preedit
                           selectionRange:NSMakeRange(snapshot.cursor, 0)
                         replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  NSMutableArray<NSAttributedString *> *candidateData = [NSMutableArray array];
  for (RLXCandidate *candidate in snapshot.candidates) {
    [candidateData addObject:RLXAttributedCandidate(candidate)];
  }
  self.candidatePanelIndex =
      candidateData.count > 0 ? nextCandidateIndex : NSNotFound;
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
#if RADISHLEX_CONTRACT_SMOKE
  if (self.contractClient != nil)
    return;
#endif
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
#if RADISHLEX_CONTRACT_SMOKE
  if (self.contractClient != nil) {
    self.contractClient = nil;
    return;
  }
#endif
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
