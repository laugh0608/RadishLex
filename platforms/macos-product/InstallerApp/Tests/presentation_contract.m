#import <Foundation/Foundation.h>

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

        RLXInstallerPresentation *removal = [[RLXInstallerPresentation alloc]
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
