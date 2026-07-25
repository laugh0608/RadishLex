#import <Cocoa/Cocoa.h>

#import "RLXInstallerBridge.h"
#import "RLXInstallerPresentation.h"

static NSString *const RLXManagerTarget = @"Applications/RadishLex Manager.app";
static NSString *const RLXInputMethodTarget = @"Library/Input Methods/RadishLexInputMethod.app";
static NSString *const RLXDataRoot = @"Library/Application Support/RadishLex";

@interface RLXInstallerAppDelegate : NSObject <NSApplicationDelegate>

@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) NSTextField *statusTitle;
@property(nonatomic, strong) NSTextField *statusDetail;
@property(nonatomic, strong) NSTextField *operation;
@property(nonatomic, strong) NSTextField *diagnostic;
@property(nonatomic, strong) NSProgressIndicator *progress;
@property(nonatomic, strong) NSButton *primaryButton;
@property(nonatomic, strong) NSButton *secondaryButton;
@property(nonatomic, strong) RLXInstallerPresentation *presentation;

@end

@implementation RLXInstallerAppDelegate

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    (void)notification;
    self.presentation =
        [[RLXInstallerPresentation alloc] initWithDriverSnapshot:RLXInstallerBridgeSnapshot()];

    NSRect frame = NSMakeRect(0, 0, 680, 520);
    self.window = [[NSWindow alloc]
        initWithContentRect:frame
                  styleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                             NSWindowStyleMaskMiniaturizable)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    self.window.title = @"萝卜词核安装器";
    [self.window center];

    NSStackView *content = [NSStackView stackViewWithViews:@[]];
    content.orientation = NSUserInterfaceLayoutOrientationVertical;
    content.alignment = NSLayoutAttributeLeading;
    content.spacing = 14;
    content.edgeInsets = NSEdgeInsetsMake(28, 32, 28, 32);
    content.translatesAutoresizingMaskIntoConstraints = NO;
    [self.window.contentView addSubview:content];
    [NSLayoutConstraint activateConstraints:@[
        [content.leadingAnchor constraintEqualToAnchor:self.window.contentView.leadingAnchor],
        [content.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor],
        [content.topAnchor constraintEqualToAnchor:self.window.contentView.topAnchor],
        [content.bottomAnchor constraintEqualToAnchor:self.window.contentView.bottomAnchor],
    ]];

    NSTextField *heading = [self label:@"安装与恢复 RadishLex" size:26 weight:NSFontWeightSemibold];
    [content addArrangedSubview:heading];
    self.statusTitle = [self label:@"" size:17 weight:NSFontWeightSemibold];
    [content addArrangedSubview:self.statusTitle];
    self.statusDetail = [self wrappingLabel:@""];
    [content addArrangedSubview:self.statusDetail];
    self.operation = [self label:@"" size:13 weight:NSFontWeightMedium];
    [content addArrangedSubview:self.operation];

    NSBox *separator = [[NSBox alloc] init];
    separator.boxType = NSBoxSeparator;
    [content addArrangedSubview:separator];
    [separator.widthAnchor constraintEqualToAnchor:content.widthAnchor].active = YES;

    [content addArrangedSubview:[self label:@"固定目标" size:13 weight:NSFontWeightSemibold]];
    [content addArrangedSubview:[self wrappingLabel:
        [NSString stringWithFormat:@"Manager：%@\nInputMethod：%@\n数据（默认保留）：%@",
                                   RLXManagerTarget, RLXInputMethodTarget, RLXDataRoot]]];

    self.progress = [[NSProgressIndicator alloc] init];
    self.progress.minValue = 0;
    self.progress.maxValue = 10;
    self.progress.indeterminate = NO;
    [content addArrangedSubview:self.progress];
    [self.progress.widthAnchor constraintEqualToAnchor:content.widthAnchor].active = YES;

    self.diagnostic = [self wrappingLabel:@""];
    self.diagnostic.font = [NSFont monospacedSystemFontOfSize:12 weight:NSFontWeightRegular];
    [content addArrangedSubview:self.diagnostic];

    NSStackView *actions = [NSStackView stackViewWithViews:@[]];
    actions.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    actions.spacing = 10;
    [content addArrangedSubview:actions];
    self.secondaryButton = [NSButton buttonWithTitle:@"" target:self action:@selector(performSecondary:)];
    self.primaryButton = [NSButton buttonWithTitle:@"" target:self action:@selector(performPrimary:)];
    self.primaryButton.keyEquivalent = @"\r";
    [actions addArrangedSubview:self.secondaryButton];
    [actions addArrangedSubview:self.primaryButton];

    [self renderPresentation];
    [self.window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    (void)sender;
    return YES;
}

- (NSTextField *)label:(NSString *)value size:(CGFloat)size weight:(NSFontWeight)weight {
    NSTextField *label = [NSTextField labelWithString:value];
    label.font = [NSFont systemFontOfSize:size weight:weight];
    return label;
}

- (NSTextField *)wrappingLabel:(NSString *)value {
    NSTextField *label = [NSTextField wrappingLabelWithString:value];
    label.maximumNumberOfLines = 0;
    return label;
}

- (void)renderPresentation {
    self.statusTitle.stringValue = self.presentation.statusTitle;
    self.statusDetail.stringValue = self.presentation.statusDetail;
    self.operation.stringValue =
        [NSString stringWithFormat:@"当前操作：%@", self.presentation.operationTitle];
    self.diagnostic.stringValue = self.presentation.stableDiagnosticSummary;
    self.progress.doubleValue = self.presentation.progressStep;
    self.primaryButton.title =
        [self.presentation titleForAction:self.presentation.primaryActionCode];
    self.primaryButton.enabled =
        [self.presentation isActionEnabled:self.presentation.primaryActionCode];
    self.secondaryButton.title =
        [self.presentation titleForAction:self.presentation.secondaryActionCode];
    self.secondaryButton.enabled =
        [self.presentation isActionEnabled:self.presentation.secondaryActionCode];
    self.secondaryButton.hidden = !self.secondaryButton.enabled;
}

- (void)performPrimary:(id)sender {
    (void)sender;
    [self confirmAction:self.presentation.primaryActionCode];
}

- (void)performSecondary:(id)sender {
    (void)sender;
    [self confirmAction:self.presentation.secondaryActionCode];
}

- (void)confirmAction:(NSString *)actionCode {
    if (![self.presentation isActionEnabled:actionCode]) {
        return;
    }
    if (![self.presentation requiresConfirmationForAction:actionCode]) {
        self.presentation = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:RLXInstallerBridgePerformAction(actionCode)];
        [self renderPresentation];
        return;
    }
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = [self.presentation titleForAction:actionCode];
    alert.informativeText = [self.presentation confirmationTextForAction:actionCode];
    [alert addButtonWithTitle:@"确认"];
    [alert addButtonWithTitle:@"取消"];
    [alert beginSheetModalForWindow:self.window completionHandler:^(NSModalResponse response) {
        if (response == NSAlertFirstButtonReturn) {
            self.presentation = [[RLXInstallerPresentation alloc]
                initWithDriverSnapshot:RLXInstallerBridgePerformAction(actionCode)];
            [self renderPresentation];
        }
    }];
}

@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        (void)argv;
        if (argc != 1) {
            return 2;
        }
        NSApplication *application = NSApplication.sharedApplication;
        RLXInstallerAppDelegate *delegate = [[RLXInstallerAppDelegate alloc] init];
        application.delegate = delegate;
        [application run];
    }
    return 0;
}
