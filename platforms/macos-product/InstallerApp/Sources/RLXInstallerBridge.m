#import "RLXInstallerBridge.h"

#import "RLXInstallerPresentation.h"
#import "radishlex_installer_bridge.h"

static NSDictionary<NSString *, id> *RLXUnknownSnapshot(void) {
    return @{
        RLXInstallerDriverContractVersionKey: @1,
        RLXInstallerDriverPhaseKey: @"blocked",
        RLXInstallerDriverPrimaryActionKey: @"refresh",
        RLXInstallerDriverSecondaryActionKey: @"none",
        RLXInstallerDriverErrorKey: @"unknown_driver_result",
        RLXInstallerDriverStateKey: @"none",
        RLXInstallerDriverOperationKey: @"none",
        RLXInstallerDriverProgressKey: @0,
        RLXInstallerDriverManualPromptKey: @"none",
    };
}

static NSString *_Nullable RLXPhaseCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_PHASE_READY: return @"ready";
    case RADISHLEX_INSTALLER_PHASE_AWAITING_USER_ACTION: return @"awaiting_user_action";
    case RADISHLEX_INSTALLER_PHASE_IN_PROGRESS: return @"in_progress";
    case RADISHLEX_INSTALLER_PHASE_COMPLETED: return @"completed";
    case RADISHLEX_INSTALLER_PHASE_RECOVERY_AVAILABLE: return @"recovery_available";
    case RADISHLEX_INSTALLER_PHASE_BLOCKED: return @"blocked";
    default: return nil;
    }
}

static NSString *_Nullable RLXActionCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_ACTION_NONE: return @"none";
    case RADISHLEX_INSTALLER_ACTION_REFRESH: return @"refresh";
    case RADISHLEX_INSTALLER_ACTION_BEGIN_FIRST_INSTALL: return @"begin_first_install";
    case RADISHLEX_INSTALLER_ACTION_BEGIN_UPGRADE: return @"begin_upgrade";
    case RADISHLEX_INSTALLER_ACTION_BEGIN_REPAIR: return @"begin_repair";
    case RADISHLEX_INSTALLER_ACTION_CONFIRM_QUIESCENCE: return @"confirm_quiescence";
    case RADISHLEX_INSTALLER_ACTION_RESUME_OPERATION: return @"resume_operation";
    case RADISHLEX_INSTALLER_ACTION_RETRY_OPERATION: return @"retry_operation";
    case RADISHLEX_INSTALLER_ACTION_REMOVE_PROGRAMS: return @"remove_programs";
    default: return nil;
    }
}

static NSString *_Nullable RLXErrorCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_ERROR_NONE: return @"none";
    case RADISHLEX_INSTALLER_ERROR_OPERATION_ACTIVE: return @"operation_active";
    case RADISHLEX_INSTALLER_ERROR_UNSAFE_DATA_ROOT: return @"unsafe_data_root";
    case RADISHLEX_INSTALLER_ERROR_UNSAFE_STATE_DIRECTORY: return @"unsafe_state_directory";
    case RADISHLEX_INSTALLER_ERROR_INTERRUPTED_RECEIPT: return @"interrupted_receipt";
    case RADISHLEX_INSTALLER_ERROR_INVALID_RECEIPT: return @"invalid_receipt";
    case RADISHLEX_INSTALLER_ERROR_UNEXPECTED_STATE_OBJECT: return @"unexpected_state_object";
    case RADISHLEX_INSTALLER_ERROR_ROOT_IDENTITY_CHANGED: return @"root_identity_changed";
    case RADISHLEX_INSTALLER_ERROR_IO: return @"io";
    case RADISHLEX_INSTALLER_ERROR_INSTALLED_RELEASE_IS_NEWER:
        return @"installed_release_is_newer";
    case RADISHLEX_INSTALLER_ERROR_PRODUCT_IDENTITY_UNAVAILABLE:
        return @"product_identity_unavailable";
    case RADISHLEX_INSTALLER_ERROR_MANUAL_RECOVERY_REQUIRED:
        return @"manual_recovery_required";
    case RADISHLEX_INSTALLER_ERROR_DRIVER_UNAVAILABLE: return @"driver_unavailable";
    case RADISHLEX_INSTALLER_ERROR_UNKNOWN_DRIVER_RESULT: return @"unknown_driver_result";
    default: return nil;
    }
}

static NSString *_Nullable RLXOperationCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_OPERATION_NONE: return @"none";
    case RADISHLEX_INSTALLER_OPERATION_FIRST_INSTALL: return @"first_install";
    case RADISHLEX_INSTALLER_OPERATION_UPGRADE: return @"upgrade";
    case RADISHLEX_INSTALLER_OPERATION_REPAIR: return @"repair";
    case RADISHLEX_INSTALLER_OPERATION_REMOVE_PROGRAMS: return @"remove_programs";
    default: return nil;
    }
}

static NSString *_Nullable RLXStateCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_STATE_NONE: return @"none";
    case RADISHLEX_INSTALLER_STATE_PREPARED: return @"prepared";
    case RADISHLEX_INSTALLER_STATE_QUIESCED: return @"quiesced";
    case RADISHLEX_INSTALLER_STATE_TARGET_STAGED: return @"target_staged";
    case RADISHLEX_INSTALLER_STATE_SOURCE_PRESERVED: return @"source_preserved";
    case RADISHLEX_INSTALLER_STATE_MANAGER_COMMITTED: return @"manager_committed";
    case RADISHLEX_INSTALLER_STATE_PROGRAMS_COMMITTED: return @"programs_committed";
    case RADISHLEX_INSTALLER_STATE_DATA_COORDINATING: return @"data_coordinating";
    case RADISHLEX_INSTALLER_STATE_DATA_SETTLED: return @"data_settled";
    case RADISHLEX_INSTALLER_STATE_FINAL_VERIFIED: return @"final_verified";
    case RADISHLEX_INSTALLER_STATE_COMPLETED: return @"completed";
    case RADISHLEX_INSTALLER_STATE_ABORTED_PRESERVED: return @"aborted_preserved";
    case RADISHLEX_INSTALLER_STATE_ROLLBACK_REQUIRED: return @"rollback_required";
    case RADISHLEX_INSTALLER_STATE_PROGRAMS_RESTORED: return @"programs_restored";
    case RADISHLEX_INSTALLER_STATE_ROLLED_BACK: return @"rolled_back";
    default: return nil;
    }
}

static NSString *_Nullable RLXPromptCode(uint32_t value) {
    switch (value) {
    case RADISHLEX_INSTALLER_PROMPT_NONE: return @"none";
    case RADISHLEX_INSTALLER_PROMPT_SELECT_NEUTRAL_AND_CLOSE_MANAGER:
        return @"select_neutral_input_source_and_close_manager";
    default: return nil;
    }
}

static NSDictionary<NSString *, id> *
RLXSnapshotDictionary(RadishLexInstallerBridgeSnapshotV1 snapshot) {
    NSString *phase = RLXPhaseCode(snapshot.phase);
    NSString *primary = RLXActionCode(snapshot.primary_action);
    NSString *secondary = RLXActionCode(snapshot.secondary_action);
    NSString *error = RLXErrorCode(snapshot.stable_error);
    NSString *operation = RLXOperationCode(snapshot.operation_kind);
    NSString *state = RLXStateCode(snapshot.receipt_state);
    NSString *prompt = RLXPromptCode(snapshot.manual_prompt);
    if (snapshot.contract_version != RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION ||
        phase == nil || primary == nil || secondary == nil || error == nil ||
        operation == nil || state == nil || prompt == nil || snapshot.progress_step > 10) {
        return RLXUnknownSnapshot();
    }
    return @{
        RLXInstallerDriverContractVersionKey: @(snapshot.contract_version),
        RLXInstallerDriverPhaseKey: phase,
        RLXInstallerDriverPrimaryActionKey: primary,
        RLXInstallerDriverSecondaryActionKey: secondary,
        RLXInstallerDriverErrorKey: error,
        RLXInstallerDriverStateKey: state,
        RLXInstallerDriverOperationKey: operation,
        RLXInstallerDriverProgressKey: @(snapshot.progress_step),
        RLXInstallerDriverManualPromptKey: prompt,
    };
}

static uint32_t RLXActionValue(NSString *actionCode) {
    NSDictionary<NSString *, NSNumber *> *actions = @{
        @"refresh": @(RADISHLEX_INSTALLER_ACTION_REFRESH),
        @"begin_first_install": @(RADISHLEX_INSTALLER_ACTION_BEGIN_FIRST_INSTALL),
        @"begin_upgrade": @(RADISHLEX_INSTALLER_ACTION_BEGIN_UPGRADE),
        @"begin_repair": @(RADISHLEX_INSTALLER_ACTION_BEGIN_REPAIR),
        @"confirm_quiescence": @(RADISHLEX_INSTALLER_ACTION_CONFIRM_QUIESCENCE),
        @"resume_operation": @(RADISHLEX_INSTALLER_ACTION_RESUME_OPERATION),
        @"retry_operation": @(RADISHLEX_INSTALLER_ACTION_RETRY_OPERATION),
        @"remove_programs": @(RADISHLEX_INSTALLER_ACTION_REMOVE_PROGRAMS),
    };
    return actions[actionCode].unsignedIntValue ?: UINT32_MAX;
}

static uint32_t RLXAuthorizationFlags(NSString *actionCode) {
    if ([actionCode isEqualToString:@"refresh"]) {
        return 0;
    }
    uint32_t flags = RADISHLEX_INSTALLER_AUTH_EXPLICIT_CONFIRMATION;
    if ([actionCode isEqualToString:@"remove_programs"]) {
        flags |= RADISHLEX_INSTALLER_AUTH_DATA_RETENTION;
    }
    if ([actionCode isEqualToString:@"begin_upgrade"] ||
        [actionCode isEqualToString:@"begin_repair"] ||
        [actionCode isEqualToString:@"confirm_quiescence"] ||
        [actionCode isEqualToString:@"retry_operation"] ||
        [actionCode isEqualToString:@"remove_programs"]) {
        flags |= RADISHLEX_INSTALLER_AUTH_NEUTRAL_INPUT_SOURCE;
        flags |= RADISHLEX_INSTALLER_AUTH_MANAGER_CLOSED;
    }
    return flags;
}

NSDictionary<NSString *, id> *RLXInstallerBridgeSnapshot(void) {
    if (radishlex_installer_bridge_contract_version() !=
        RADISHLEX_INSTALLER_BRIDGE_CONTRACT_VERSION) {
        return RLXUnknownSnapshot();
    }
    return RLXSnapshotDictionary(radishlex_installer_bridge_snapshot_v1());
}

NSDictionary<NSString *, id> *RLXInstallerBridgePerformAction(NSString *actionCode) {
    return RLXSnapshotDictionary(radishlex_installer_bridge_perform_v1(
        RLXActionValue(actionCode), RLXAuthorizationFlags(actionCode)));
}
