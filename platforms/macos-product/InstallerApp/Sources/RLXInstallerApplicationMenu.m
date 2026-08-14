#import "RLXInstallerApplicationMenu.h"

static NSMenuItem *RLXMenuItem(NSString *title,
                               SEL action,
                               NSString *keyEquivalent,
                               NSEventModifierFlags modifiers) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title
                                                 action:action
                                          keyEquivalent:keyEquivalent];
    item.keyEquivalentModifierMask = modifiers;
    return item;
}

NSMenu *RLXCreateInstallerMainMenu(NSString *applicationName) {
    NSMenu *mainMenu = [[NSMenu alloc] initWithTitle:@""];
    NSMenuItem *applicationMenuItem =
        [[NSMenuItem alloc] initWithTitle:applicationName action:nil keyEquivalent:@""];
    NSMenu *applicationMenu = [[NSMenu alloc] initWithTitle:applicationName];

    [applicationMenu
        addItem:RLXMenuItem([NSString stringWithFormat:@"关于 %@", applicationName],
                            @selector(orderFrontStandardAboutPanel:), @"", 0)];
    [applicationMenu addItem:NSMenuItem.separatorItem];
    [applicationMenu
        addItem:RLXMenuItem([NSString stringWithFormat:@"隐藏 %@", applicationName],
                            @selector(hide:), @"h", NSEventModifierFlagCommand)];
    [applicationMenu
        addItem:RLXMenuItem(@"隐藏其他", @selector(hideOtherApplications:), @"h",
                            NSEventModifierFlagCommand | NSEventModifierFlagOption)];
    [applicationMenu addItem:RLXMenuItem(@"全部显示", @selector(unhideAllApplications:), @"", 0)];
    [applicationMenu addItem:NSMenuItem.separatorItem];
    [applicationMenu
        addItem:RLXMenuItem([NSString stringWithFormat:@"退出 %@", applicationName],
                            @selector(terminate:), @"q", NSEventModifierFlagCommand)];

    applicationMenuItem.submenu = applicationMenu;
    [mainMenu addItem:applicationMenuItem];
    return mainMenu;
}
