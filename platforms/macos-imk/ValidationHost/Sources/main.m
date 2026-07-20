#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

@interface RLXValidationHostDelegate
    : NSObject <NSApplicationDelegate, NSTextFieldDelegate>
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) NSTextField *normalField;
@property(nonatomic, strong) NSSecureTextField *secureField;
@property(nonatomic, strong) NSTextField *secureStatusLabel;
@property(nonatomic, strong) NSTimer *secureStatusTimer;
@end

@implementation RLXValidationHostDelegate

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    (void)notification;

    NSRect windowFrame = NSMakeRect(0, 0, 600, 330);
    NSWindowStyleMask style =
        NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
        NSWindowStyleMaskMiniaturizable;
    self.window = [[NSWindow alloc] initWithContentRect:windowFrame
                                              styleMask:style
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
    self.window.title = [[NSBundle mainBundle]
        objectForInfoDictionaryKey:@"CFBundleDisplayName"];
    self.window.restorable = NO;
    [self.window center];

    NSView *contentView = self.window.contentView;
    NSTextField *intro = [NSTextField labelWithString:
        @"仅使用仓库约定的合成测试文本；宿主不会读取、记录或持久化正文。"];
    intro.frame = NSMakeRect(24, 278, 552, 24);
    [contentView addSubview:intro];

    NSTextField *normalLabel =
        [NSTextField labelWithString:@"普通合成输入框"];
    normalLabel.frame = NSMakeRect(24, 226, 552, 22);
    [contentView addSubview:normalLabel];

    self.normalField = [[NSTextField alloc] initWithFrame:
        NSMakeRect(24, 188, 552, 30)];
    self.normalField.placeholderString = @"输入固定合成 case";
    self.normalField.delegate = self;
    [contentView addSubview:self.normalField];

    NSTextField *secureLabel =
        [NSTextField labelWithString:@"Secure 合成输入框"];
    secureLabel.frame = NSMakeRect(24, 136, 552, 22);
    [contentView addSubview:secureLabel];

    self.secureField = [[NSSecureTextField alloc] initWithFrame:
        NSMakeRect(24, 98, 552, 30)];
    self.secureField.placeholderString = @"输入同一固定合成 case";
    self.secureField.delegate = self;
    [contentView addSubview:self.secureField];

    self.secureStatusLabel =
        [NSTextField labelWithString:@"Secure Event Input：等待控件聚焦"];
    self.secureStatusLabel.frame = NSMakeRect(24, 50, 552, 24);
    [contentView addSubview:self.secureStatusLabel];

    self.secureStatusTimer =
        [NSTimer scheduledTimerWithTimeInterval:0.2
                                         target:self
                                       selector:@selector(refreshSecureStatus:)
                                       userInfo:nil
                                        repeats:YES];
    [self.window makeKeyAndOrderFront:nil];
    [self.window makeFirstResponder:self.normalField];
    [NSApp activateIgnoringOtherApps:YES];
}

- (void)refreshSecureStatus:(NSTimer *)timer {
    (void)timer;
    if (!NSApp.isActive) {
        self.secureStatusLabel.stringValue =
            @"Secure Event Input：宿主未激活，不读取全局状态";
        return;
    }
    self.secureStatusLabel.stringValue = IsSecureEventInputEnabled()
        ? @"Secure Event Input：已启用"
        : @"Secure Event Input：未启用";
}

- (void)controlTextDidEndEditing:(NSNotification *)notification {
    if (notification.object == self.normalField) {
        self.normalField.stringValue = @"";
    } else if (notification.object == self.secureField) {
        self.secureField.stringValue = @"";
    }
}

- (void)clearSyntheticFields {
    self.normalField.stringValue = @"";
    self.secureField.stringValue = @"";
}

- (void)applicationWillResignActive:(NSNotification *)notification {
    (void)notification;
    [self clearSyntheticFields];
}

- (void)applicationWillTerminate:(NSNotification *)notification {
    (void)notification;
    [self clearSyntheticFields];
    [self.secureStatusTimer invalidate];
    self.secureStatusTimer = nil;
}

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:
    (NSApplication *)application {
    (void)application;
    return YES;
}

- (BOOL)applicationShouldSaveApplicationState:(NSApplication *)application {
    (void)application;
    return NO;
}

- (BOOL)applicationShouldRestoreApplicationState:(NSApplication *)application {
    (void)application;
    return NO;
}

- (BOOL)applicationSupportsSecureRestorableState:(NSApplication *)application {
    (void)application;
    return NO;
}

@end

int main(void) {
    @autoreleasepool {
        NSApplication *application = [NSApplication sharedApplication];
        RLXValidationHostDelegate *delegate =
            [[RLXValidationHostDelegate alloc] init];
        application.delegate = delegate;
        [application setActivationPolicy:NSApplicationActivationPolicyRegular];
        [application run];
    }
    return 0;
}
