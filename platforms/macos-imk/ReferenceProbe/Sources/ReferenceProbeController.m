#import "ReferenceProbeController.h"

#import <Carbon/Carbon.h>

#import "ReferenceProbeState.h"

static IMKCandidates *RLXReferenceProbeCandidatePanel(IMKServer *server) {
  static IMKCandidates *candidatePanel;
  static dispatch_once_t onceToken;
  dispatch_once(&onceToken, ^{
    candidatePanel = [[IMKCandidates alloc]
        initWithServer:server
             panelType:kIMKSingleRowSteppingCandidatePanel];
    NSMutableDictionary *attributes =
        [NSMutableDictionary dictionaryWithDictionary:candidatePanel.attributes ?: @{}];
    attributes[IMKCandidatesSendServerKeyEventFirst] = @YES;
    [candidatePanel setAttributes:attributes];
    [candidatePanel setSelectionKeys:@[@18, @19, @20, @21, @23]];
    [candidatePanel setDismissesAutomatically:NO];
  });
  return candidatePanel;
}

static NSArray<NSAttributedString *> *RLXReferenceProbeCandidates(void) {
  return @[
    [[NSAttributedString alloc] initWithString:@"候选甲"],
    [[NSAttributedString alloc] initWithString:@"候选乙"],
    [[NSAttributedString alloc] initWithString:@"候选丙"],
    [[NSAttributedString alloc] initWithString:@"候选丁"],
    [[NSAttributedString alloc] initWithString:@"候选戊"],
  ];
}

@interface RLXReferenceProbeController ()
@property(nonatomic, strong) IMKCandidates *candidatePanel;
@property(nonatomic, strong) RLXReferenceProbeState *probeState;
@property(nonatomic, copy) NSString *rawComposition;
- (void)refreshCompositionForClient:(id<IMKTextInput>)client;
- (void)clearCompositionForClient:(id<IMKTextInput>)client;
- (BOOL)commitSelectedCandidateToClient:(id<IMKTextInput>)client;
- (BOOL)commitRawCompositionToClient:(id<IMKTextInput>)client;
@end

@implementation RLXReferenceProbeController

- (instancetype)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)inputClient {
  if (![NSThread isMainThread]) return nil;
  self = [super initWithServer:server delegate:delegate client:inputClient];
  if (self == nil) return nil;
  _candidatePanel = RLXReferenceProbeCandidatePanel(server);
  if (_candidatePanel == nil) return nil;
  _probeState = [[RLXReferenceProbeState alloc]
      initWithCandidates:RLXReferenceProbeCandidates()];
  _rawComposition = @"";
  return self;
}

- (NSUInteger)recognizedEvents:(id)sender {
  (void)sender;
  return NSEventMaskKeyDown;
}

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
  if (![NSThread isMainThread] || event.type != NSEventTypeKeyDown) return NO;
  id<IMKTextInput> client = (id<IMKTextInput>)sender;
  switch (RLXReferenceProbeRouteForEvent(event)) {
    case RLXReferenceProbeEventRouteHostApplication:
    case RLXReferenceProbeEventRouteCandidatePanel:
      return NO;
    case RLXReferenceProbeEventRouteCommitCandidate:
      return [self commitSelectedCandidateToClient:client];
    case RLXReferenceProbeEventRouteCommitRaw:
      return [self commitRawCompositionToClient:client];
    case RLXReferenceProbeEventRouteControllerInput:
      break;
  }

  if (event.keyCode == kVK_Escape && self.rawComposition.length > 0) {
    [self clearCompositionForClient:client];
    return YES;
  }
  if (event.keyCode == kVK_Delete && self.rawComposition.length > 0) {
    NSRange finalCharacter =
        [self.rawComposition rangeOfComposedCharacterSequenceAtIndex:self.rawComposition.length - 1];
    self.rawComposition = [self.rawComposition stringByReplacingCharactersInRange:finalCharacter
                                                                        withString:@""];
    [self refreshCompositionForClient:client];
    return YES;
  }

  NSString *characters = event.characters;
  NSCharacterSet *letters = NSCharacterSet.letterCharacterSet;
  if (characters.length == 0 ||
      [characters rangeOfCharacterFromSet:letters.invertedSet].location != NSNotFound) {
    return NO;
  }
  self.rawComposition = [self.rawComposition stringByAppendingString:characters.lowercaseString];
  [self refreshCompositionForClient:client];
  return YES;
}

- (NSArray *)candidates:(id)sender {
  (void)sender;
  return self.probeState.candidates;
}

- (void)candidateSelectionChanged:(NSAttributedString *)candidateString {
  if (![NSThread isMainThread]) return;
  if (![self.probeState updateSelectionFromCandidate:candidateString]) {
    NSLog(@"RadishLex IMK reference probe rejected an ambiguous candidate callback");
    return;
  }
  NSLog(@"RadishLex IMK reference probe selection index=%ld",
        (long)self.probeState.selectedIndex);
}

- (void)candidateSelected:(NSAttributedString *)candidateString {
  if (![NSThread isMainThread]) return;
  if (![self.probeState updateSelectionFromCandidate:candidateString]) {
    NSLog(@"RadishLex IMK reference probe rejected an ambiguous final callback");
    return;
  }
  [self commitSelectedCandidateToClient:(id<IMKTextInput>)self.client];
}

- (void)refreshCompositionForClient:(id<IMKTextInput>)client {
  if (self.rawComposition.length == 0) {
    [self clearCompositionForClient:client];
    return;
  }
  [client setMarkedText:self.rawComposition
         selectionRange:NSMakeRange(self.rawComposition.length, 0)
       replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  [self.candidatePanel updateCandidates];
  [self.candidatePanel show:kIMKLocateCandidatesBelowHint];
}

- (void)clearCompositionForClient:(id<IMKTextInput>)client {
  self.rawComposition = @"";
  [self.probeState resetSelection];
  [client setMarkedText:@""
         selectionRange:NSMakeRange(0, 0)
       replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  [self.candidatePanel hide];
}

- (BOOL)commitSelectedCandidateToClient:(id<IMKTextInput>)client {
  if (self.rawComposition.length == 0) return NO;
  NSAttributedString *candidate = self.probeState.selectedCandidate;
  if (candidate == nil) return NO;
  [client insertText:candidate.string replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  NSLog(@"RadishLex IMK reference probe committed candidate index=%ld",
        (long)self.probeState.selectedIndex);
  [self clearCompositionForClient:client];
  return YES;
}

- (BOOL)commitRawCompositionToClient:(id<IMKTextInput>)client {
  if (self.rawComposition.length == 0) return NO;
  [client insertText:self.rawComposition replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
  [self clearCompositionForClient:client];
  return YES;
}

- (void)commitComposition:(id)sender {
  [self commitRawCompositionToClient:(id<IMKTextInput>)sender];
}

- (void)deactivateServer:(id)sender {
  [self clearCompositionForClient:(id<IMKTextInput>)sender];
  [super deactivateServer:sender];
}

- (NSMenu *)menu {
  return nil;
}

- (void)inputControllerWillClose {
  [self.candidatePanel hide];
  [super inputControllerWillClose];
}

@end
