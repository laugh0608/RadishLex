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

        NSArray<NSString *> *maintenancePhases = @[@"preparing_source", @"cancellation_requested",
            @"cancellation_finishing_source", @"cancellation_preserving",
            @"cancellation_awaiting_qualification", @"cancellation_completed"];
        for (NSString *phase in maintenancePhases) {
            NSDictionary *value = Snapshot(phase, @"refresh", @"none", @"none", @"none", @"upgrade", 0, @"none");
            RLXInstallerPresentation *view = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:value];
            Require([view.phaseCode isEqualToString:phase], @"maintenance phase must survive mapping");
            Require([view isActionEnabled:@"refresh"] && ![view requiresConfirmationForAction:@"refresh"], @"refresh stays read-only");
            Require(![view isActionEnabled:@"retry_operation"] && ![view isActionEnabled:@"remove_programs"], @"maintenance must suppress old outer actions");
            for (NSString *action in @[@"begin_source_preparation", @"resume_source_preparation", @"preserve_preparation_cancellation", @"finish_preparation_cancellation"]) {
                Require(![view isActionEnabled:action], @"read-only maintenance cannot execute preparation or cancellation");
            }
            Require(!view.showsReceiptProgress && view.progressStep == 0, @"old terminal progress is not maintenance progress");
            if ([phase isEqualToString:@"cancellation_awaiting_qualification"]) {
                Require([view.statusTitle isEqualToString:@"取消仍被阻断"], @"proof and index cannot imply success");
            }
            for (NSString *key in @[RLXInstallerDriverPrimaryActionKey, RLXInstallerDriverStateKey,
                RLXInstallerDriverProgressKey, RLXInstallerDriverManualPromptKey]) {
                NSMutableDictionary *bad = [value mutableCopy];
                if ([key isEqualToString:RLXInstallerDriverPrimaryActionKey]) { bad[key] = @"retry_operation"; }
                if ([key isEqualToString:RLXInstallerDriverStateKey]) { bad[key] = @"completed"; }
                if ([key isEqualToString:RLXInstallerDriverProgressKey]) { bad[key] = @10; }
                if ([key isEqualToString:RLXInstallerDriverManualPromptKey]) { bad[key] = @"select_neutral_input_source_and_close_manager"; }
                RLXInstallerPresentation *rejected = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad];
                Require(rejected.failedClosed && [rejected.errorCode isEqualToString:@"unknown_driver_result"], @"contradictory maintenance view must fail closed");
            }
        }

        NSString *manualPrompt = @"select_neutral_input_source_and_close_manager";
        NSArray<NSDictionary *> *preparationValues = @[
            Snapshot(@"preparing_source", @"begin_source_preparation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"preparing_source", @"resume_source_preparation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"preparing_source", @"resume_source_preparation", @"preserve_preparation_cancellation", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"preparing_source", @"preserve_preparation_cancellation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"cancellation_requested", @"preserve_preparation_cancellation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"cancellation_finishing_source", @"preserve_preparation_cancellation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
            Snapshot(@"cancellation_preserving", @"preserve_preparation_cancellation", @"none", @"none", @"none", @"upgrade", 0, manualPrompt),
        ];
        NSArray<NSString *> *allPhases = @[@"ready", @"awaiting_user_action", @"in_progress",
            @"completed", @"recovery_available", @"blocked", @"preparing_source",
            @"cancellation_requested", @"cancellation_finishing_source", @"cancellation_preserving",
            @"cancellation_awaiting_qualification", @"cancellation_completed"];
        NSDictionary *contradictions = @{
            RLXInstallerDriverStateKey: @"prepared",
            RLXInstallerDriverOperationKey: @"repair",
            RLXInstallerDriverProgressKey: @1,
            RLXInstallerDriverManualPromptKey: @"none",
            RLXInstallerDriverSecondaryActionKey: @"refresh",
            RLXInstallerDriverErrorKey: @"io",
        };
        for (NSDictionary *value in preparationValues) {
            NSString *action = value[RLXInstallerDriverPrimaryActionKey];
            RLXInstallerPresentation *view = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:value];
            Require(!view.failedClosed && [view isActionEnabled:action], @"exact preparation projection must enable the offered action");
            Require([view requiresConfirmationForAction:action] && view.requiresManualQuiescence, @"preparation mutations require fresh explicit and manual confirmation");
            Require(!view.showsReceiptProgress && view.progressStep == 0, @"preparation cannot display an old receipt's progress");
            Require(![view.statusDetail containsString:@"只能刷新"] && ![view.statusDetail containsString:@"只提供状态查看"], @"offered preparation actions must replace read-only detail");
            Require(![view isActionEnabled:@"finish_preparation_cancellation"], @"final cancellation release must remain unavailable");
            if ([action isEqualToString:@"preserve_preparation_cancellation"]) {
                NSString *confirmation = [view confirmationTextForAction:action];
                for (NSString *text in @[@"不可回到本次升级", @"安全的源词库收尾", @"所有数据与事务材料都会保留",
                    @"旧版本兼容性资格", @"暂不能启动旧版本", @"保存材料不等于完成取消", @"关闭管理器"]) {
                    Require([confirmation containsString:text], @"cancellation confirmation must explain its full scope and limits");
                }
                Require([[view titleForAction:action] isEqualToString:@"保存取消请求与材料"], @"preservation must not imply completed cancellation");
            } else {
                Require([[view confirmationTextForAction:action] containsString:@"继续升级仍需新的确认"], @"successful preparation requires a separate upgrade confirmation");
                Require([[view titleForAction:action] isEqualToString:
                    [action isEqualToString:@"begin_source_preparation"] ? @"准备升级词库" : @"继续词库准备"], @"preparation action title must identify the offered work");
            }
            if ([value[RLXInstallerDriverSecondaryActionKey] isEqualToString:@"preserve_preparation_cancellation"]) {
                Require([view isActionEnabled:@"preserve_preparation_cancellation"] &&
                    [view requiresConfirmationForAction:@"preserve_preparation_cancellation"], @"offered secondary preservation also requires confirmation");
            }
            for (NSString *key in contradictions) {
                NSMutableDictionary *bad = [value mutableCopy];
                bad[key] = contradictions[key];
                RLXInstallerPresentation *rejected = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad];
                Require(rejected.failedClosed && [rejected.errorCode isEqualToString:@"unknown_driver_result"], @"contradictory preparation field must fail closed");
                Require(![rejected isActionEnabled:action] && [rejected isActionEnabled:@"refresh"], @"rejected preparation may only refresh");
            }
            for (NSString *phase in allPhases) {
                BOOL legal = [phase isEqualToString:@"preparing_source"] ||
                    ([action isEqualToString:@"preserve_preparation_cancellation"] &&
                     [@[@"cancellation_requested", @"cancellation_finishing_source", @"cancellation_preserving"] containsObject:phase]);
                if (legal) { continue; }
                NSMutableDictionary *bad = [value mutableCopy];
                bad[RLXInstallerDriverPhaseKey] = phase;
                Require([[[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad] failedClosed], @"preparation action in any other phase must fail closed");
            }
        }
        for (NSString *primary in @[@"refresh", @"begin_source_preparation", @"preserve_preparation_cancellation"]) {
            NSDictionary *bad = Snapshot(@"preparing_source", primary, @"preserve_preparation_cancellation",
                @"none", @"none", @"upgrade", 0, manualPrompt);
            Require([[[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad] failedClosed], @"preservation secondary is only valid beside resume");
        }
        for (NSString *secondary in @[@"begin_source_preparation", @"resume_source_preparation"]) {
            NSDictionary *bad = Snapshot(@"preparing_source", @"resume_source_preparation", secondary,
                @"none", @"none", @"upgrade", 0, manualPrompt);
            Require([[[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad] failedClosed], @"begin and resume cannot be secondary actions");
        }

        NSDictionary *continuationValue = Snapshot(@"preparing_source",@"continue_prepared_upgrade",@"none",@"none",@"data_coordinating",@"upgrade",7,@"select_neutral_input_source_and_close_manager");
        RLXInstallerPresentation *continuation = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:continuationValue];
        Require([continuation isActionEnabled:@"continue_prepared_upgrade"] && continuation.requiresManualQuiescence,@"continuation requires fresh manual confirmation");
        Require([[continuation confirmationTextForAction:@"continue_prepared_upgrade"] containsString:@"保留用户数据"],@"confirmation includes data retention");
        for (NSString *key in @[RLXInstallerDriverPhaseKey,RLXInstallerDriverStateKey,RLXInstallerDriverManualPromptKey]) {
            NSMutableDictionary *bad=[continuationValue mutableCopy];
            bad[key]=[key isEqualToString:RLXInstallerDriverPhaseKey]?@"cancellation_awaiting_qualification":@"none";
            Require([[[RLXInstallerPresentation alloc] initWithDriverSnapshot:bad] failedClosed],@"contradictory continuation must fail closed");
        }
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
