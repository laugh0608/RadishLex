#import <Foundation/Foundation.h>
#import "RLXInstallerBridge.h"
#import "RLXInstallerPresentation.h"
#import "radishlex_installer_bridge.h"

static uint32_t receivedAction;
static uint32_t receivedFlags;
static uint32_t responsePhase = RADISHLEX_INSTALLER_PHASE_RECOVERY_AVAILABLE;
static uint32_t responseAction = RADISHLEX_INSTALLER_ACTION_ABORT_PRE_SWITCH_UPGRADE;
static uint32_t responseSecondary = RADISHLEX_INSTALLER_ACTION_NONE;

uint32_t radishlex_installer_bridge_contract_version(void) {
    return RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION;
}

RadishLexInstallerBridgeSnapshotV1 radishlex_installer_bridge_snapshot_v1(void) {
    BOOL maintenance = responsePhase >= 7 && responsePhase <= 12 &&
        responseAction != RADISHLEX_INSTALLER_ACTION_CONTINUE_PREPARED_UPGRADE;
    BOOL preparation = responseAction >= RADISHLEX_INSTALLER_ACTION_BEGIN_SOURCE_PREPARATION &&
        responseAction <= RADISHLEX_INSTALLER_ACTION_PRESERVE_PREPARATION_CANCELLATION;
    return (RadishLexInstallerBridgeSnapshotV1){
        .contract_version = RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION,
        .phase = responsePhase,
        .primary_action = responseAction,
        .secondary_action = responseSecondary,
        .stable_error = RADISHLEX_INSTALLER_ERROR_NONE,
        .operation_kind = RADISHLEX_INSTALLER_OPERATION_UPGRADE,
        .receipt_state = maintenance ? RADISHLEX_INSTALLER_STATE_NONE : RADISHLEX_INSTALLER_STATE_DATA_COORDINATING,
        .progress_step = maintenance ? 0 : 7,
        .manual_prompt = maintenance && !preparation ? RADISHLEX_INSTALLER_PROMPT_NONE : RADISHLEX_INSTALLER_PROMPT_SELECT_NEUTRAL_AND_CLOSE_MANAGER,
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
        NSArray<NSString *> *actions = @[@"none", @"refresh", @"begin_first_install",
            @"begin_upgrade", @"begin_repair", @"confirm_quiescence", @"resume_operation",
            @"retry_operation", @"remove_programs", @"abort_pre_switch_upgrade",
            @"continue_prepared_upgrade", @"begin_source_preparation",
            @"resume_source_preparation", @"preserve_preparation_cancellation"];
        Require(RADISHLEX_INSTALLER_ACTION_BEGIN_SOURCE_PREPARATION == 11 &&
            RADISHLEX_INSTALLER_ACTION_RESUME_SOURCE_PREPARATION == 12 &&
            RADISHLEX_INSTALLER_ACTION_PRESERVE_PREPARATION_CANCELLATION == 13);
        for (NSUInteger index = 0; index < actions.count; ++index) {
            responseAction = (uint32_t)index;
            NSDictionary *mapped = RLXInstallerBridgeSnapshot();
            Require([mapped[RLXInstallerDriverPrimaryActionKey] isEqualToString:actions[index]]);
            if (index != 0) {
                RLXInstallerBridgePerformAction(actions[index]);
                Require(receivedAction == index);
            }
        }
        responseAction = RADISHLEX_INSTALLER_ACTION_ABORT_PRE_SWITCH_UPGRADE;
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
        for (NSUInteger index = 11; index <= 13; ++index) {
            responseAction = (uint32_t)index;
            result = RLXInstallerBridgePerformAction(actions[index]);
            Require(receivedAction == index);
            Require(receivedFlags == (RADISHLEX_INSTALLER_AUTH_EXPLICIT_CONFIRMATION |
                RADISHLEX_INSTALLER_AUTH_DATA_RETENTION | RADISHLEX_INSTALLER_AUTH_NEUTRAL_INPUT_SOURCE |
                RADISHLEX_INSTALLER_AUTH_MANAGER_CLOSED));
            RLXInstallerPresentation *view = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:result];
            Require([view isActionEnabled:actions[index]] && view.requiresManualQuiescence);
            Require([view requiresConfirmationForAction:actions[index]] && !view.showsReceiptProgress);
        }
        responseAction = RADISHLEX_INSTALLER_ACTION_RESUME_SOURCE_PREPARATION;
        responseSecondary = RADISHLEX_INSTALLER_ACTION_PRESERVE_PREPARATION_CANCELLATION;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverSecondaryActionKey] isEqualToString:@"preserve_preparation_cancellation"]);
        RLXInstallerPresentation *preparation = [[RLXInstallerPresentation alloc] initWithDriverSnapshot:result];
        Require([preparation isActionEnabled:@"resume_source_preparation"] &&
            [preparation isActionEnabled:@"preserve_preparation_cancellation"]);
        responseSecondary = RADISHLEX_INSTALLER_ACTION_NONE;
        responseAction = 14;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverErrorKey] isEqualToString:@"unknown_driver_result"]);
        responseAction = RADISHLEX_INSTALLER_ACTION_REFRESH;
        responseSecondary = 14;
        result = RLXInstallerBridgeSnapshot();
        Require([result[RLXInstallerDriverErrorKey] isEqualToString:@"unknown_driver_result"]);
        responseSecondary = RADISHLEX_INSTALLER_ACTION_NONE;
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
