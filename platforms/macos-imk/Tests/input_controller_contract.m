#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

#import "RadishLexCandidatePanelTesting.h"
#import "RadishLexInputController.h"
#import "RadishLexInputControllerTesting.h"
#import "RadishLexRuntime.h"

static void Require(BOOL condition, NSString *message) {
  if (!condition) {
    NSLog(@"input controller contract failed: %@", message);
    exit(1);
  }
}

static NSEvent *KeyEventWithType(NSEventType type, unsigned short keyCode,
                         NSEventModifierFlags modifiers,
                         NSString *characters) {
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

static NSEvent *KeyEvent(unsigned short keyCode,
                         NSEventModifierFlags modifiers,
                         NSString *characters) {
  return KeyEventWithType(NSEventTypeKeyDown, keyCode, modifiers, characters);
}

static unsigned short KeyCodeForCharacter(unichar character) {
  switch (character) {
  case 'b':
    return kVK_ANSI_B;
  case 'c':
    return kVK_ANSI_C;
  case 'e':
    return kVK_ANSI_E;
  case 'h':
    return kVK_ANSI_H;
  case 'i':
    return kVK_ANSI_I;
  case 'l':
    return kVK_ANSI_L;
  case 'o':
    return kVK_ANSI_O;
  case 'u':
    return kVK_ANSI_U;
  default:
    return kVK_ANSI_A;
  }
}

@interface RLXContractInputClient : NSObject
@property(nonatomic, copy) NSString *markedText;
@property(nonatomic, strong) NSMutableArray<NSString *> *committedTexts;
@property(nonatomic) NSRange markedSelection;
@property(nonatomic) NSRect lineRect;
@property(nonatomic) NSInteger contractWindowLevel;
@end

@implementation RLXContractInputClient
- (instancetype)init {
  self = [super init];
  if (self != nil) {
    _markedText = @"";
    _committedTexts = [NSMutableArray array];
    _markedSelection = NSMakeRange(0, 0);
    _lineRect = NSMakeRect(300, 500, 2, 22);
    _contractWindowLevel = NSNormalWindowLevel;
  }
  return self;
}
- (void)insertText:(id)string replacementRange:(NSRange)replacementRange {
  (void)replacementRange;
  NSString *text = [string isKindOfClass:NSAttributedString.class]
                       ? [string string]
                       : [string description];
  [self.committedTexts addObject:text ?: @""];
}
- (void)setMarkedText:(id)string
        selectionRange:(NSRange)selectionRange
      replacementRange:(NSRange)replacementRange {
  (void)replacementRange;
  self.markedText = [string isKindOfClass:NSAttributedString.class]
                        ? [string string]
                        : [string description];
  self.markedSelection = selectionRange;
}
- (NSRange)markedRange {
  return self.markedText.length > 0 ? NSMakeRange(0, self.markedText.length)
                                    : NSMakeRange(NSNotFound, 0);
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index
                          lineHeightRectangle:(NSRectPointer)lineRect {
  (void)index;
  if (lineRect != NULL)
    *lineRect = self.lineRect;
  return @{};
}
- (NSRect)firstRectForCharacterRange:(NSRange)range
                         actualRange:(NSRangePointer)actualRange {
  (void)range;
  if (actualRange != NULL)
    *actualRange = self.markedRange;
  return self.lineRect;
}
- (NSInteger)windowLevel {
  return self.contractWindowLevel;
}
@end

static BOOL TypeASCII(RadishLexInputController *controller,
                      RLXContractInputClient *client, NSString *text) {
  for (NSUInteger index = 0; index < text.length; ++index) {
    unichar character = [text characterAtIndex:index];
    NSString *characters = [NSString stringWithCharacters:&character length:1];
    if (![controller handleEvent:KeyEvent(KeyCodeForCharacter(character), 0,
                                          characters)
                          client:client]) {
      return NO;
    }
  }
  return YES;
}

static RadishLexInputController *Controller(RLXContractInputClient *client) {
  return [[RadishLexInputController alloc] initForContractWithClient:client];
}

int main(void) {
  @autoreleasepool {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];

    RLXContractInputClient *client = [[RLXContractInputClient alloc] init];
    RadishLexInputController *controller = Controller(client);
    Require(controller != nil, @"controller accepts a contract client");
    RLXCandidatePanel *panel = [RLXCandidatePanel sharedPanel];
    Require(([controller recognizedEvents:client] &
             (NSEventMaskKeyDown | NSEventMaskKeyUp |
              NSEventMaskFlagsChanged)) ==
                (NSEventMaskKeyDown | NSEventMaskKeyUp |
                 NSEventMaskFlagsChanged) &&
                controller.menu == nil,
            @"controller declares its full key contract without fake commands");

    Require(TypeASCII(controller, client, @"luobo") &&
                [client.markedText isEqualToString:@"luobo"] &&
                panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractSelectedIndex == 0,
            @"composition displays candidates with index zero selected");
    Require([controller handleEvent:KeyEvent(kVK_RightArrow, 0, @"")
                              client:client] &&
                panel.rlx_contractSelectedIndex == 1 &&
                [panel.rlx_contractCandidateButtons[1].accessibilityValue
                    length] > 0,
            @"right arrow consumes the event and updates the visible index");
    Require(![controller
                handleEvent:KeyEventWithType(NSEventTypeKeyUp, kVK_RightArrow,
                                             0, @"")
                     client:client] &&
                panel.rlx_contractSelectedIndex == 1,
            @"direction key release preserves the current display index");
    Require(![controller
                handleEvent:KeyEventWithType(NSEventTypeFlagsChanged,
                                             kVK_Shift,
                                             NSEventModifierFlagShift, @"")
                     client:client] &&
                panel.rlx_contractSelectedIndex == 1,
            @"modifier-only events preserve the current display index");
    Require(client.committedTexts.count == 0,
            @"candidate movement does not commit early");
    Require([controller handleEvent:KeyEvent(kVK_ANSI_A, 0, @"a")
                              client:client] &&
                !panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractSelectedIndex == NSNotFound,
            @"a changed candidate presentation clears the stale selection");
    Require([controller handleEvent:KeyEvent(kVK_Delete, 0, @"")
                              client:client] &&
                panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractSelectedIndex == 0,
            @"a rebuilt candidate presentation starts from index zero");
    Require([controller handleEvent:KeyEvent(kVK_RightArrow, 0, @"")
                              client:client] &&
                panel.rlx_contractSelectedIndex == 1,
            @"selection can move again after candidate rebuilding");
    Require([controller handleEvent:KeyEvent(kVK_Space, 0, @" ")
                              client:client] &&
                [client.committedTexts.lastObject isEqualToString:@"萝卜词核"] &&
                client.markedText.length == 0 &&
                !panel.rlx_contractWindow.isVisible,
            @"Space commits the candidate at the same display index");

    Require(TypeASCII(controller, client, @"luobo"),
            @"second composition for pointer selection");
    [panel.rlx_contractCandidateButtons[1] performClick:nil];
    Require([client.committedTexts.lastObject isEqualToString:@"萝卜词核"] &&
                client.markedText.length == 0 &&
                !panel.rlx_contractWindow.isVisible,
            @"pointer action enters the same Rust candidate selection path");

    Require(TypeASCII(controller, client, @"luobo"),
            @"third composition for accessibility selection");
    NSButton *accessibleCandidate = panel.rlx_contractCandidateButtons[1];
    Require([(id<NSAccessibilityButton>)accessibleCandidate
                accessibilityPerformPress] &&
                [client.committedTexts.lastObject isEqualToString:@"萝卜词核"] &&
                client.markedText.length == 0 &&
                !panel.rlx_contractWindow.isVisible,
            @"accessibility press enters the same Rust selection path");

    Require(TypeASCII(controller, client, @"cihe") &&
                [controller handleEvent:KeyEvent(kVK_Return, 0, @"\r")
                                  client:client] &&
                [client.committedTexts.lastObject isEqualToString:@"cihe"] &&
                client.markedText.length == 0,
            @"Enter commits the original composition");
    Require(TypeASCII(controller, client, @"l") &&
                [controller handleEvent:KeyEvent(kVK_Escape, 0, @"\e")
                                  client:client] &&
                client.markedText.length == 0 &&
                !panel.rlx_contractWindow.isVisible,
            @"Escape resets composition and visible candidate state");
    NSUInteger commitsBeforeCommand = client.committedTexts.count;
    Require(![controller handleEvent:KeyEvent(kVK_ANSI_N,
                                              NSEventModifierFlagCommand, @"n")
                                   client:client] &&
                client.committedTexts.count == commitsBeforeCommand,
            @"Command-modified input is returned to the host");

    RLXContractInputClient *clientB = [[RLXContractInputClient alloc] init];
    clientB.lineRect = NSMakeRect(600, 400, 2, 22);
    RadishLexInputController *controllerB = Controller(clientB);
    Require(controllerB != nil && TypeASCII(controller, client, @"luobo") &&
                TypeASCII(controllerB, clientB, @"luobo"),
            @"two controllers can own independent engine sessions");
    Require(panel.rlx_contractOwner == (id)controllerB,
            @"the newest active controller owns the process panel");
    [controller deactivateServer:client];
    Require(panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractOwner == (id)controllerB,
            @"an old controller deactivation cannot hide the current panel");
    [controller inputControllerWillClose];
    Require(panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractOwner == (id)controllerB,
            @"an old controller close cannot hide the current panel");
    [controllerB inputControllerWillClose];
    Require(!panel.rlx_contractWindow.isVisible &&
                panel.rlx_contractOwner == nil &&
                panel.rlx_contractCandidateButtons.count == 0,
            @"the current controller close clears process panel state");

    NSError *error = nil;
    Require([[RLXProcessRuntime sharedRuntime] shutdownWithError:&error],
            @"controller contracts release sessions before runtime shutdown");
    NSLog(@"macOS InputMethodKit controller integration contract passed");
  }
  return 0;
}
