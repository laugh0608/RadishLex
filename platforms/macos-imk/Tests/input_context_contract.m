#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <objc/runtime.h>

#import "RadishLexCandidatePanelTesting.h"
#import "RadishLexInputControllerTesting.h"
#import "RadishLexLearningContext.h"

static void Require(BOOL condition, NSString *message) {
  if (!condition) {
    NSLog(@"input context contract failed: %@", message);
    exit(1);
  }
}

// A demo engine keeps these routing tests independent of Rime and user data.
// The spy records the production controller's calls at the Rust bridge boundary;
// runtime privacy/learning behavior is covered by the native policy regressions.
@interface RLXContextSession : RLXSessionBridge
@property(nonatomic, strong) NSMutableArray<NSDictionary *> *contexts;
@property(nonatomic, strong) NSMutableArray<NSString *> *calls;
@property(nonatomic) BOOL rejectContext;
@end

@implementation RLXContextSession
- (instancetype)initDemoWithError:(NSError **)error {
  self = [super initDemoWithError:error];
  if (self != nil) {
    _contexts = [NSMutableArray array];
    _calls = [NSMutableArray array];
  }
  return self;
}
- (BOOL)setLearningContextSecureInput:(BOOL)secureInput
                 sensitiveApplication:(BOOL)sensitiveApplication
                          privacyMode:(BOOL)privacyMode
                         contextKnown:(BOOL)contextKnown
                           contextKind:(NSString *)contextKind
                                 error:(NSError **)error {
  [self.calls addObject:@"context"];
  if (self.rejectContext) {
    if (error != NULL)
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                  code:RADISHLEX_STATUS_INVALID_STATE
                              userInfo:nil];
    return NO;
  }
  [self.contexts addObject:@{
    @"secure" : @(secureInput), @"sensitive" : @(sensitiveApplication),
    @"private" : @(privacyMode), @"known" : @(contextKnown),
    @"kind" : contextKind
  }];
  return YES;
}
- (RLXSnapshot *)snapshotWithError:(NSError **)error {
  [self.calls addObject:@"snapshot"];
  return [super snapshotWithError:error];
}
- (RLXKeyHandlingResult *)handleEvent:(RadishLexKeyEvent)event
                              error:(NSError **)error {
  [self.calls addObject:@"key"];
  return [super handleEvent:event error:error];
}
- (RLXKeyHandlingResult *)selectCandidateAtIndex:(NSUInteger)index
                                          error:(NSError **)error {
  [self.calls addObject:@"select"];
  return [super selectCandidateAtIndex:index error:error];
}
@end

@interface RLXContextClient : NSObject
@property(nonatomic, strong) id identifier;
@property(nonatomic) BOOL missingSelector;
@property(nonatomic) BOOL throwsOnIdentity;
@property(nonatomic) NSUInteger identityReads;
@property(nonatomic, copy) NSString *markedText;
@property(nonatomic, strong) NSMutableArray<NSString *> *commits;
@end

@implementation RLXContextClient
- (instancetype)init {
  self = [super init];
  if (self != nil) {
    _identifier = @"com.apple.TextEdit";
    _markedText = @"";
    _commits = [NSMutableArray array];
  }
  return self;
}
- (BOOL)respondsToSelector:(SEL)selector {
  if (self.missingSelector && selector == @selector(bundleIdentifier))
    return NO;
  return [super respondsToSelector:selector];
}
- (NSString *)bundleIdentifier {
  self.identityReads += 1;
  if (self.throwsOnIdentity)
    [NSException raise:NSGenericException format:@"synthetic client failure"];
  return self.identifier;
}
- (void)insertText:(NSString *)text replacementRange:(NSRange)range {
  (void)range;
  [self.commits addObject:text];
}
- (void)setMarkedText:(NSString *)text selectionRange:(NSRange)selection
      replacementRange:(NSRange)replacement {
  (void)selection;
  (void)replacement;
  self.markedText = text;
}
- (NSRange)markedRange {
  return NSMakeRange(0, self.markedText.length);
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index
                          lineHeightRectangle:(NSRectPointer)rect {
  (void)index;
  *rect = NSMakeRect(300, 500, 2, 22);
  return @{};
}
- (NSInteger)windowLevel {
  return NSNormalWindowLevel;
}
@end

// Replace only the query inside this short-lived test process. No app activation
// or system input event is performed, and no real foreground identity is read.
@interface RLXContextForeground : NSObject
@property(nonatomic, copy) NSString *bundleIdentifier;
@end
@implementation RLXContextForeground
@end
static RLXContextForeground *foreground;
static NSUInteger foregroundReads;
static id TestFrontmostApplication(id workspace, SEL selector) {
  (void)workspace;
  (void)selector;
  foregroundReads += 1;
  return foreground;
}

static NSEvent *Key(unsigned short code, NSString *characters) {
  return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
                   modifierFlags:0 timestamp:0 windowNumber:0 context:nil
                      characters:characters
     charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
}

static void TypeWord(RadishLexInputController *controller,
                     RLXContextClient *client) {
  for (NSString *letter in @[@"l", @"u", @"o", @"b", @"o"])
    Require([controller handleEvent:Key(kVK_ANSI_A, letter) client:client],
            @"synthetic letters reach the demo engine");
  Require([client.markedText isEqualToString:@"luobo"],
          @"composition remains attached to its input client");
}

static void RequireContext(RLXContextSession *session, BOOL known,
                           BOOL sensitive, BOOL privacy, BOOL secure,
                           NSString *kind) {
  Require([session.contexts.lastObject isEqualToDictionary:@{
            @"secure" : @(secure), @"sensitive" : @(sensitive),
            @"private" : @(privacy), @"known" : @(known), @"kind" : kind
          }], @"all policy signals and client category reach the bridge");
}

static RadishLexInputController *Controller(RLXContextClient *client,
                                           RLXContextSession **session) {
  NSError *error = nil;
  *session = [[RLXContextSession alloc] initDemoWithError:&error];
  Require(*session != nil, @"isolated demo bridge is available");
  RadishLexInputController *controller = [[RadishLexInputController alloc]
      initForContractWithClient:client session:*session];
  Require(controller != nil, @"controller accepts an isolated context session");
  return controller;
}

static void CheckIdentity(id identifier, BOOL missing, BOOL throws,
                          BOOL known, BOOL sensitive, NSString *kind) {
  RLXContextClient *client = [[RLXContextClient alloc] init];
  client.identifier = identifier;
  client.missingSelector = missing;
  client.throwsOnIdentity = throws;
  RLXContextSession *session = nil;
  RadishLexInputController *controller = Controller(client, &session);
  TypeWord(controller, client);
  RequireContext(session, known, sensitive, NO, NO, kind);
  Require(session.contexts.count == 1,
          @"unchanged policy avoids redundant bridge updates");
  Require(client.identityReads == (missing ? 0 : 5),
          @"identity is revalidated on every input event");
  Require([controller handleEvent:Key(kVK_Space, @" ") client:client] &&
              client.commits.count == 1 && client.markedText.length == 0,
          @"client identity restrictions preserve the commit path");
  [controller inputControllerWillClose];
}

static void Select(RadishLexInputController *controller,
                   RLXContextClient *client, NSUInteger route) {
  RLXCandidatePanel *panel = [RLXCandidatePanel sharedPanel];
  switch (route) {
  case 0:
    Require([controller handleEvent:Key(kVK_Space, @" ") client:client],
            @"Space is consumed");
    break;
  case 1:
    Require([controller handleEvent:Key(kVK_ANSI_1, @"1") client:client],
            @"candidate number is consumed");
    break;
  case 2:
    [panel.rlx_contractCandidateButtons[0] performClick:nil];
    break;
  case 3:
    Require([(id<NSAccessibilityButton>)panel.rlx_contractCandidateButtons[0]
                accessibilityPerformPress], @"accessibility selection succeeds");
    break;
  default:
    [controller commitComposition:client];
    break;
  }
}

static void CheckSelectionRoutes(void) {
  for (NSUInteger route = 0; route < 5; ++route) {
    RLXContextClient *client = [[RLXContextClient alloc] init];
    RLXContextSession *session = nil;
    RadishLexInputController *controller = Controller(client, &session);
    TypeWord(controller, client);
    // Identity may become unavailable/restricted after typing and before any
    // selection route. Refresh policy and candidates before handling selection.
    client.identifier = RLXValidationP0BundleIdentifier;
    [session.calls removeAllObjects];
    Select(controller, client, route);
    RequireContext(session, YES, YES, NO, NO, @"other");
    Require(session.calls.count >= 3 &&
                [session.calls[0] isEqualToString:@"context"] &&
                [session.calls[1] isEqualToString:@"snapshot"] &&
                [session.calls[2] isEqualToString:route < 4 ? @"select" : @"key"],
            @"each commit route refreshes restricted policy before selection");
    Require(client.commits.count == 1 && client.markedText.length == 0,
            @"each route commits once to the same input client");
    client.identifier = @"com.apple.TextEdit";
    TypeWord(controller, client);
    RequireContext(session, YES, NO, NO, NO, @"editor");
    Select(controller, client, 0);
    Require(client.commits.count == 2,
            @"a fresh ordinary composition still reaches the bridge");
    [controller inputControllerWillClose];
  }
}

static void CheckPolicyChangesAndFailure(void) {
  RLXContextClient *client = [[RLXContextClient alloc] init];
  RLXContextSession *session = nil;
  RadishLexInputController *controller = Controller(client, &session);
  TypeWord(controller, client);
  controller.contractPrivacyMode = YES;
  Require([controller handleEvent:Key(kVK_LeftArrow, @"") client:client],
          @"privacy-only change is observed in an ordinary client");
  RequireContext(session, YES, NO, YES, NO, @"editor");
  controller.contractPrivacyMode = NO;
  controller.contractSecureInput = YES;
  Require([controller handleEvent:Key(kVK_LeftArrow, @"") client:client],
          @"secure-only change is observed in an ordinary client");
  RequireContext(session, YES, NO, NO, YES, @"editor");
  controller.contractPrivacyMode = YES;
  client.identifier = nil;
  Require([controller handleEvent:Key(kVK_LeftArrow, @"") client:client],
          @"policy changes can be observed without committing");
  RequireContext(session, NO, NO, YES, YES, @"other");
  Require([client.markedText isEqualToString:@"luobo"],
          @"policy refresh preserves pending composition");
  client.identifier = @"com.apple.TextEdit";
  controller.contractPrivacyMode = NO;
  controller.contractSecureInput = NO;
  Select(controller, client, 0);
  RequireContext(session, YES, NO, NO, NO, @"editor");
  Require(session.contexts.count == 5 && client.commits.count == 1,
          @"restriction and return are both forwarded to the sticky runtime");

  TypeWord(controller, client);
  client.identifier = RLXValidationP0BundleIdentifier;
  session.rejectContext = YES;
  [session.calls removeAllObjects];
  Require(![controller handleEvent:Key(kVK_Space, @" ") client:client] &&
              [session.calls isEqualToArray:@[@"context"]] &&
              client.commits.count == 1 &&
              [client.markedText isEqualToString:@"luobo"],
          @"failed policy update cannot reach selection or discard composition");
  session.rejectContext = NO;
  Select(controller, client, 0);
  RequireContext(session, YES, YES, NO, NO, @"other");
  Require(client.commits.count == 2,
          @"failed update is not cached as successful on the next event");
  [controller inputControllerWillClose];
}

int main(void) {
  @autoreleasepool {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
    foreground = [[RLXContextForeground alloc] init];
    Method method = class_getInstanceMethod(NSWorkspace.class,
                                            @selector(frontmostApplication));
    IMP original = method_setImplementation(method, (IMP)TestFrontmostApplication);

    foreground.bundleIdentifier = @"com.openai.codex";
    CheckIdentity(@"com.apple.TextEdit", NO, NO, YES, NO, @"editor");
    foreground.bundleIdentifier = @"com.apple.TextEdit";
    CheckIdentity(@"com.openai.codex", NO, NO, YES, NO, @"code");
    CheckIdentity(RLXValidationP0BundleIdentifier, NO, NO, YES, YES, @"other");
    CheckIdentity(@"org.radishlex.validation.macos.unknown", NO, NO, NO, NO, @"other");
    CheckIdentity(nil, NO, NO, NO, NO, @"other");
    CheckIdentity(@"", NO, NO, NO, NO, @"other");
    CheckIdentity(@42, NO, NO, NO, NO, @"other");
    CheckIdentity(@"com.apple.TextEdit", YES, NO, NO, NO, @"other");
    CheckIdentity(@"com.apple.TextEdit", NO, YES, NO, NO, @"other");
    CheckSelectionRoutes();
    CheckPolicyChangesAndFailure();

    method_setImplementation(method, original);
    Require(foregroundReads == 0,
            @"foreground application is never used to authorize client learning");
    NSLog(@"macOS input client identity and policy routing contract passed");
  }
  return 0;
}
