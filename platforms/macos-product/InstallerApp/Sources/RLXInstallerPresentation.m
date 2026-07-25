#import "RLXInstallerPresentation.h"

NSString *const RLXInstallerDriverContractVersionKey = @"contract_version";
NSString *const RLXInstallerDriverPhaseKey = @"phase";
NSString *const RLXInstallerDriverPrimaryActionKey = @"primary_action";
NSString *const RLXInstallerDriverSecondaryActionKey = @"secondary_action";
NSString *const RLXInstallerDriverErrorKey = @"error";
NSString *const RLXInstallerDriverStateKey = @"state";
NSString *const RLXInstallerDriverOperationKey = @"operation";
NSString *const RLXInstallerDriverProgressKey = @"progress_step";
NSString *const RLXInstallerDriverManualPromptKey = @"manual_prompt";

static NSString *const RLXNoAction = @"none";
static NSString *const RLXRefreshAction = @"refresh";
static NSString *const RLXUnknownDriverResult = @"unknown_driver_result";

@interface RLXInstallerPresentation ()

@property(nonatomic, readwrite, copy) NSString *phaseCode;
@property(nonatomic, readwrite, copy) NSString *primaryActionCode;
@property(nonatomic, readwrite, copy) NSString *secondaryActionCode;
@property(nonatomic, readwrite, copy) NSString *errorCode;
@property(nonatomic, readwrite, copy) NSString *stateCode;
@property(nonatomic, readwrite, copy) NSString *operationCode;
@property(nonatomic, readwrite) NSInteger progressStep;
@property(nonatomic, readwrite) BOOL requiresManualQuiescence;
@property(nonatomic, readwrite) BOOL failedClosed;

@end

@implementation RLXInstallerPresentation

- (instancetype)initWithDriverSnapshot:(NSDictionary<NSString *, id> *)snapshot {
    self = [super init];
    if (self == nil) {
        return nil;
    }

    NSSet<NSString *> *phases = [NSSet setWithArray:@[
        @"ready", @"awaiting_user_action", @"in_progress", @"completed",
        @"recovery_available", @"blocked"
    ]];
    NSSet<NSString *> *actions = [NSSet setWithArray:@[
        RLXNoAction, RLXRefreshAction, @"begin_first_install", @"begin_upgrade",
        @"begin_repair", @"confirm_quiescence", @"resume_operation",
        @"retry_operation", @"remove_programs"
    ]];
    NSSet<NSString *> *errors = [NSSet setWithArray:@[
        @"none", @"operation_active", @"unsafe_data_root",
        @"unsafe_state_directory", @"interrupted_receipt", @"invalid_receipt",
        @"unexpected_state_object", @"root_identity_changed", @"io",
        @"installed_release_is_newer", @"product_identity_unavailable",
        @"manual_recovery_required", @"driver_unavailable",
        RLXUnknownDriverResult
    ]];
    NSSet<NSString *> *states = [NSSet setWithArray:@[
        @"none", @"prepared", @"quiesced", @"target_staged", @"source_preserved",
        @"manager_committed", @"programs_committed", @"data_coordinating",
        @"data_settled", @"final_verified", @"completed", @"aborted_preserved",
        @"rollback_required", @"programs_restored", @"rolled_back"
    ]];
    NSSet<NSString *> *operations = [NSSet setWithArray:@[
        @"none", @"first_install", @"upgrade", @"repair", @"remove_programs"
    ]];

    NSNumber *version = snapshot[RLXInstallerDriverContractVersionKey];
    NSString *phase = snapshot[RLXInstallerDriverPhaseKey];
    NSString *primary = snapshot[RLXInstallerDriverPrimaryActionKey];
    NSString *secondary = snapshot[RLXInstallerDriverSecondaryActionKey];
    NSString *error = snapshot[RLXInstallerDriverErrorKey];
    NSString *state = snapshot[RLXInstallerDriverStateKey];
    NSString *operation = snapshot[RLXInstallerDriverOperationKey];
    NSNumber *progress = snapshot[RLXInstallerDriverProgressKey];
    NSString *manualPrompt = snapshot[RLXInstallerDriverManualPromptKey];

    BOOL valid = [version isKindOfClass:NSNumber.class] &&
        version.integerValue == 1 &&
        [phase isKindOfClass:NSString.class] && [phases containsObject:phase] &&
        [primary isKindOfClass:NSString.class] && [actions containsObject:primary] &&
        [secondary isKindOfClass:NSString.class] && [actions containsObject:secondary] &&
        [error isKindOfClass:NSString.class] && [errors containsObject:error] &&
        [state isKindOfClass:NSString.class] && [states containsObject:state] &&
        [operation isKindOfClass:NSString.class] && [operations containsObject:operation] &&
        [progress isKindOfClass:NSNumber.class] &&
        progress.integerValue >= 0 && progress.integerValue <= 10 &&
        [manualPrompt isKindOfClass:NSString.class] &&
        ([manualPrompt isEqualToString:@"none"] ||
         [manualPrompt isEqualToString:@"select_neutral_input_source_and_close_manager"]);

    if (!valid) {
        self.phaseCode = @"blocked";
        self.primaryActionCode = RLXRefreshAction;
        self.secondaryActionCode = RLXNoAction;
        self.errorCode = RLXUnknownDriverResult;
        self.stateCode = @"none";
        self.operationCode = @"none";
        self.progressStep = 0;
        self.requiresManualQuiescence = NO;
        self.failedClosed = YES;
        return self;
    }

    self.phaseCode = phase;
    self.primaryActionCode = primary;
    self.secondaryActionCode = secondary;
    self.errorCode = error;
    self.stateCode = state;
    self.operationCode = operation;
    self.progressStep = progress.integerValue;
    self.requiresManualQuiescence =
        [manualPrompt isEqualToString:@"select_neutral_input_source_and_close_manager"];
    self.failedClosed = [phase isEqualToString:@"blocked"];
    return self;
}

- (NSString *)operationTitle {
    NSDictionary<NSString *, NSString *> *titles = @{
        @"none": @"无已确认操作",
        @"first_install": @"首次安装",
        @"upgrade": @"升级",
        @"repair": @"程序修复",
        @"remove_programs": @"程序移除",
    };
    return titles[self.operationCode] ?: @"无已确认操作";
}

- (NSString *)statusTitle {
    NSDictionary<NSString *, NSString *> *titles = @{
        @"ready": @"可以开始",
        @"awaiting_user_action": @"等待你的确认",
        @"in_progress": @"安装事务进行中",
        @"completed": @"操作已完成",
        @"recovery_available": @"可以继续恢复",
        @"blocked": @"当前不能继续",
    };
    return titles[self.phaseCode] ?: @"当前不能继续";
}

- (NSString *)statusDetail {
    if (self.requiresManualQuiescence) {
        return @"请先手动切换到其他输入源并关闭萝卜词核管理器。确认后安装器仍会通过公开平台接口重新检查，勾选状态不作为静止证明。";
    }
    if (self.failedClosed) {
        return @"安装器无法安全确认产品或事务状态。请保留现场并使用下方稳定诊断码排查。";
    }
    if ([self.phaseCode isEqualToString:@"completed"]) {
        return @"持久化事务已到达终态。默认移除只删除两个程序，Application Support 中的数据继续保留。";
    }
    return @"目标固定为当前用户的 Applications 与 Library/Input Methods；安装器不接受自定义路径。";
}

- (NSString *)titleForAction:(NSString *)actionCode {
    NSDictionary<NSString *, NSString *> *titles = @{
        RLXNoAction: @"",
        RLXRefreshAction: @"刷新状态",
        @"begin_first_install": @"开始安装",
        @"begin_upgrade": @"开始升级",
        @"begin_repair": @"修复程序",
        @"confirm_quiescence": @"已完成手动操作，继续",
        @"resume_operation": @"继续未完成操作",
        @"retry_operation": @"重新执行",
        @"remove_programs": @"移除两个程序",
    };
    return titles[actionCode] ?: @"";
}

- (BOOL)isActionEnabled:(NSString *)actionCode {
    return [actionCode isKindOfClass:NSString.class] &&
        ![actionCode isEqualToString:RLXNoAction] &&
        [[self titleForAction:actionCode] length] > 0;
}

- (BOOL)requiresConfirmationForAction:(NSString *)actionCode {
    return [self isActionEnabled:actionCode] &&
        ![actionCode isEqualToString:RLXRefreshAction];
}

- (NSString *)confirmationTextForAction:(NSString *)actionCode {
    if ([actionCode isEqualToString:@"remove_programs"]) {
        return @"确认只移除 RadishLex Manager 与 InputMethod 两个程序。Application Support、用户词库、备份和历史事务材料都会保留。";
    }
    if ([actionCode isEqualToString:@"confirm_quiescence"]) {
        return @"确认你已手动切换到其他输入源并关闭萝卜词核管理器。安装器会重新执行只读平台预检。";
    }
    return @"确认执行此操作。安装器会重新读取持久化状态并执行必要的平台预检。";
}

- (NSString *)stableDiagnosticSummary {
    return [NSString stringWithFormat:@"phase=%@ action=%@ error=%@ state=%@",
                                      self.phaseCode, self.primaryActionCode,
                                      self.errorCode, self.stateCode];
}

@end
