#import <Cocoa/Cocoa.h>

#import "RLXInstallerApplicationMenu.h"
#import "RLXInstallerBridge.h"
#import "RLXInstallerPresentation.h"

static void Require(BOOL condition, NSString *message) {
    if (!condition) {
        NSLog(@"contract failure: %@", message);
        exit(1);
    }
}

static NSDictionary<NSString *, id> *Snapshot(NSString *phase,
                                               NSString *primary,
                                               NSString *secondary,
                                               NSString *error,
                                               NSString *state,
                                               NSString *operation,
                                               NSInteger progress,
                                               NSString *manualPrompt) {
    return @{
        RLXInstallerDriverContractVersionKey: @1,
        RLXInstallerDriverPhaseKey: phase,
        RLXInstallerDriverPrimaryActionKey: primary,
        RLXInstallerDriverSecondaryActionKey: secondary,
        RLXInstallerDriverErrorKey: error,
        RLXInstallerDriverStateKey: state,
        RLXInstallerDriverOperationKey: operation,
        RLXInstallerDriverProgressKey: @(progress),
        RLXInstallerDriverManualPromptKey: manualPrompt,
    };
}

int main(void) {
    @autoreleasepool {
        NSMenu *mainMenu = RLXCreateInstallerMainMenu(@"RadishLex Installer");
        Require(mainMenu.numberOfItems == 1,
                @"main menu must expose one standard application menu");
        NSMenu *applicationMenu = mainMenu.itemArray.firstObject.submenu;
        Require(applicationMenu != nil, @"application menu must have a submenu");
        NSMenuItem *quitItem = nil;
        for (NSMenuItem *item in applicationMenu.itemArray) {
            if (item.action == @selector(terminate:)) {
                quitItem = item;
                break;
            }
        }
        Require(quitItem != nil, @"application menu must expose a Quit action");
        Require([quitItem.keyEquivalent isEqualToString:@"q"],
                @"Quit action must use the q key equivalent");
        Require((quitItem.keyEquivalentModifierMask & NSEventModifierFlagCommand) != 0,
                @"Quit action must require the Command modifier");
        Require([quitItem.title isEqualToString:@"退出 RadishLex Installer"],
                @"Quit action must name the Installer");

        RLXInstallerPresentation *bridge = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:RLXInstallerBridgeSnapshot()];
        Require(bridge.failedClosed, @"production bridge must fail closed");
        Require([bridge.errorCode isEqualToString:@"product_identity_unavailable"],
                @"unsigned development build must expose missing release identity");
        RLXInstallerPresentation *unknownBridge = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:RLXInstallerBridgePerformAction(@"not_an_action")];
        Require([unknownBridge.errorCode isEqualToString:@"unknown_driver_result"],
                @"unknown native action must fail closed");

        RLXInstallerPresentation *prepared = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:Snapshot(
                @"awaiting_user_action", @"confirm_quiescence", @"none",
                @"none", @"prepared", @"first_install", 1,
                @"select_neutral_input_source_and_close_manager")];
        Require(prepared.requiresManualQuiescence, @"prepared must show manual prompt");
        Require([prepared.operationTitle isEqualToString:@"首次安装"],
                @"operation must be displayed from the stable code");
        Require([prepared.statusDetail containsString:@"公开平台接口"],
                @"manual acknowledgement must not become platform proof");
        Require([prepared.stableDiagnosticSummary
                    isEqualToString:@"phase=awaiting_user_action "
                                    "action=confirm_quiescence error=none state=prepared"],
                @"diagnostics must contain stable codes only");

        RLXInstallerPresentation *resumed = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:Snapshot(
                @"in_progress", @"resume_operation", @"none", @"none",
                @"programs_committed", @"upgrade", 6, @"none")];
        Require(resumed.progressStep == 6, @"restart progress must use receipt step");
        Require([[resumed titleForAction:resumed.primaryActionCode]
                    isEqualToString:@"继续未完成操作"],
                @"restart action must be explicit");

        RLXInstallerPresentation *failedUpgrade = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:Snapshot(
                @"blocked", @"refresh", @"none", @"unknown_driver_result",
                @"data_coordinating", @"upgrade", 7, @"none")];
        Require(failedUpgrade.failedClosed && failedUpgrade.progressStep == 7,
                @"failed execution must retain durable progress while blocked");
        Require([failedUpgrade.statusDetail containsString:@"上次操作返回错误"],
                @"an execution error must not look like normal progress");
        Require([failedUpgrade isActionEnabled:@"refresh"] &&
                ![failedUpgrade isActionEnabled:@"resume_operation"] &&
                ![failedUpgrade isActionEnabled:@"retry_operation"],
                @"failed execution may only refresh; it cannot offer another mutation");
        Require([failedUpgrade.stableDiagnosticSummary
                    isEqualToString:@"phase=blocked action=refresh "
                                    "error=unknown_driver_result state=data_coordinating"],
                @"error diagnostics must retain the actual receipt stage");

        RLXInstallerPresentation *removal = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:Snapshot(
                @"recovery_available", @"abort_pre_switch_upgrade", @"none", @"none",
                @"data_coordinating", @"upgrade", 7,
                @"select_neutral_input_source_and_close_manager")];
        Require([removal isActionEnabled:@"abort_pre_switch_upgrade"] &&
                [removal requiresConfirmationForAction:@"abort_pre_switch_upgrade"] &&
                removal.requiresManualQuiescence,
                @"recovery must be offered explicitly and require confirmation");
        Require([[removal confirmationTextForAction:@"abort_pre_switch_upgrade"]
                    containsString:@"WAL"] &&
                ![removal isActionEnabled:@"resume_operation"],
                @"recovery must explain preservation and reject stale resume");

        removal = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:Snapshot(
                @"ready", @"begin_repair", @"remove_programs", @"none",
                @"none", @"repair", 0, @"none")];
        NSString *removalConfirmation =
            [removal confirmationTextForAction:@"remove_programs"];
        Require([removalConfirmation containsString:@"Application Support"],
                @"removal must name retained data root");
        Require([removalConfirmation containsString:@"都会保留"],
                @"removal must state retention semantics");

        RLXInstallerPresentation *unknown = [[RLXInstallerPresentation alloc]
            initWithDriverSnapshot:@{
                RLXInstallerDriverContractVersionKey: @99,
                RLXInstallerDriverPhaseKey: @"invented",
            }];
        Require(unknown.failedClosed, @"unknown driver result must fail closed");
        Require([unknown.errorCode isEqualToString:@"unknown_driver_result"],
                @"unknown result must use stable error");
        Require([unknown.primaryActionCode isEqualToString:@"refresh"],
                @"unknown result may only refresh");
        Require(![unknown isActionEnabled:@"invented"],
                @"unknown action must remain disabled");
    }
    return 0;
}
