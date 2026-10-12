#import <Foundation/Foundation.h>
#import "RLXInstallerBridge.h"
#import "RLXInstallerPresentation.h"
#import "radishlex_installer_bridge.h"

static uint32_t receivedAction;
static uint32_t receivedFlags;
static uint32_t responsePhase = RADISHLEX_INSTALLER_PHASE_RECOVERY_AVAILABLE;
static uint32_t responseAction = RADISHLEX_INSTALLER_ACTION_ABORT_PRE_SWITCH_UPGRADE;

uint32_t radishlex_installer_bridge_contract_version(void) {
    return RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION;
}

RadishLexInstallerBridgeSnapshotV1 radishlex_installer_bridge_snapshot_v1(void) {
    BOOL maintenance = responsePhase >= 7 && responsePhase <= 12 &&
        responseAction != RADISHLEX_INSTALLER_ACTION_CONTINUE_PREPARED_UPGRADE;
    return (RadishLexInstallerBridgeSnapshotV1){
        .contract_version = RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION,
        .phase = responsePhase,
        .primary_action = responseAction,
        .secondary_action = RADISHLEX_INSTALLER_ACTION_NONE,
        .stable_error = RADISHLEX_INSTALLER_ERROR_NONE,
        .operation_kind = RADISHLEX_INSTALLER_OPERATION_UPGRADE,
        .receipt_state = maintenance ? RADISHLEX_INSTALLER_STATE_NONE : RADISHLEX_INSTALLER_STATE_DATA_COORDINATING,
        .progress_step = maintenance ? 0 : 7,
        .manual_prompt = maintenance ? RADISHLEX_INSTALLER_PROMPT_NONE : RADISHLEX_INSTALLER_PROMPT_SELECT_NEUTRAL_AND_CLOSE_MANAGER,
    };
}

RadishLexInstallerBridgeSnapshotV1 radishlex_installer_bridge_perform_v1(
    uint32_t action, uint32_t authorization_flags) {
    receivedAction = action;
    receivedFlags = authorization_flags;
    return radishlex_installer_bridge_snapshot_v1();
}

static void Require(BOOL value) {
    if (!value) { fprintf(stderr, "Installer bridge recovery mapping failed\n"); exit(1); }
}

int main(void) {
    @autoreleasepool {
        NSDictionary *result = RLXInstallerBridgePerformAction(@"abort_pre_switch_upgrade");
        Require(receivedAction == RADISHLEX_INSTALLER_ACTION_ABORT_PRE_SWITCH_UPGRADE);
        Require(receivedFlags == (RADISHLEX_INSTALLER_AUTH_EXPLICIT_CONFIRMATION |
            RADISHLEX_INSTALLER_AUTH_DATA_RETENTION | RADISHLEX_INSTALLER_AUTH_NEUTRAL_INPUT_SOURCE |
            RADISHLEX_INSTALLER_AUTH_MANAGER_CLOSED));
        Require([result[RLXInstallerDriverPrimaryActionKey] isEqualToString:@"abort_pre_switch_upgrade"]);
        responseAction = RADISHLEX_INSTALLER_ACTION_REFRESH;
        NSArray<NSString *> *phases = @[@"preparing_source", @"cancellation_requested",
            @"cancellation_finishing_source", @"cancellation_preserving",
            @"cancellation_awaiting_qualification", @"cancellation_completed"];
        for (NSUInteger index = 0; index < phases.count; ++index) {
            responsePhase = (uint32_t)index + 7;
            result = RLXInstallerBridgePerformAction(@"refresh");
            Require(receivedAction == RADISHLEX_INSTALLER_ACTION_REFRESH && receivedFlags == 0);
            Require([result[RLXInstallerDriverPhaseKey] isEqualToString:phases[index]]);
            RLXInstallerPresentation *view = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:result];
            Require([view isActionEnabled:@"refresh"] && ![view isActionEnabled:@"retry_operation"]);
            Require(!view.showsReceiptProgress && !view.requiresManualQuiescence);
        }
        responsePhase = RADISHLEX_INSTALLER_PHASE_PREPARING_SOURCE;
        responseAction = RADISHLEX_INSTALLER_ACTION_CONTINUE_PREPARED_UPGRADE;
        result = RLXInstallerBridgePerformAction(@"continue_prepared_upgrade");
        Require(receivedAction == 10);
        Require(receivedFlags == (RADISHLEX_INSTALLER_AUTH_EXPLICIT_CONFIRMATION |
            RADISHLEX_INSTALLER_AUTH_DATA_RETENTION | RADISHLEX_INSTALLER_AUTH_NEUTRAL_INPUT_SOURCE |
            RADISHLEX_INSTALLER_AUTH_MANAGER_CLOSED));
        Require([result[RLXInstallerDriverPrimaryActionKey] isEqualToString:@"continue_prepared_upgrade"]);
        RLXInstallerPresentation *continuation = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:result];
        Require([continuation isActionEnabled:@"continue_prepared_upgrade"] && continuation.requiresManualQuiescence);
        responseAction = 11;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverErrorKey] isEqualToString:@"unknown_driver_result"]);
        responsePhase = 99;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverErrorKey] isEqualToString:@"unknown_driver_result"]);
        responsePhase = RADISHLEX_INSTALLER_PHASE_RECOVERY_AVAILABLE;
        responseAction = 999;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverPhaseKey] isEqualToString:@"blocked"]);
        Require([result[RLXInstallerDriverErrorKey] isEqualToString:@"unknown_driver_result"]);
    }
    return 0;
}
