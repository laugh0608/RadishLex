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
        @"recovery_available", @"blocked",
        @"preparing_source", @"cancellation_requested", @"cancellation_finishing_source", @"cancellation_preserving", @"cancellation_awaiting_qualification", @"cancellation_completed"
    ]];
    NSSet<NSString *> *actions = [NSSet setWithArray:@[
        RLXNoAction, RLXRefreshAction, @"begin_first_install", @"begin_upgrade",
        @"begin_repair", @"confirm_quiescence", @"resume_operation",
        @"retry_operation", @"remove_programs", @"abort_pre_switch_upgrade", @"continue_prepared_upgrade"
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

    NSSet<NSString *> *maintenancePhases = [NSSet setWithArray:@[
        @"preparing_source", @"cancellation_requested", @"cancellation_finishing_source",
        @"cancellation_preserving", @"cancellation_awaiting_qualification", @"cancellation_completed"
    ]];
    if (valid && ([primary isEqualToString:@"continue_prepared_upgrade"] ||
                  [secondary isEqualToString:@"continue_prepared_upgrade"])) {
        valid = [phase isEqualToString:@"preparing_source"] &&
            [primary isEqualToString:@"continue_prepared_upgrade"] &&
            [secondary isEqualToString:RLXNoAction];
    }
    if (valid && [maintenancePhases containsObject:phase]) {
        BOOL continuation = [phase isEqualToString:@"preparing_source"] &&
            [primary isEqualToString:@"continue_prepared_upgrade"];
        valid = [secondary isEqualToString:RLXNoAction] && [error isEqualToString:@"none"] &&
            [operation isEqualToString:@"upgrade"] && (continuation ?
                (![state isEqualToString:@"none"] && ![state isEqualToString:@"aborted_preserved"] &&
                 [manualPrompt isEqualToString:@"select_neutral_input_source_and_close_manager"]) :
            ([primary isEqualToString:RLXRefreshAction] &&
            [secondary isEqualToString:RLXNoAction] && [error isEqualToString:@"none"] &&
            [state isEqualToString:@"none"] && [operation isEqualToString:@"upgrade"] &&
            progress.integerValue == 0 && [manualPrompt isEqualToString:@"none"]));
    }

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

- (BOOL)showsReceiptProgress {
    return ![self.stateCode isEqualToString:@"none"];
}

- (NSString *)statusTitle {
    if ([self.primaryActionCode isEqualToString:@"continue_prepared_upgrade"]) { return @"等待继续升级的确认"; }
    NSDictionary<NSString *, NSString *> *titles = @{
        @"ready": @"可以开始",
        @"awaiting_user_action": @"等待你的确认",
        @"in_progress": @"安装事务进行中",
        @"completed": @"操作已完成",
        @"recovery_available": @"可以继续恢复",
        @"blocked": @"当前不能继续",
        @"preparing_source": @"源词库准备尚未完成",
        @"cancellation_requested": @"已记录取消请求",
        @"cancellation_finishing_source": @"取消前的词库收尾尚未完成",
        @"cancellation_preserving": @"取消材料尚未完成封存",
        @"cancellation_awaiting_qualification": @"取消仍被阻断",
        @"cancellation_completed": @"本次升级准备已取消",
    };
    return titles[self.phaseCode] ?: @"当前不能继续";
}

- (NSString *)statusDetail {
    if ([self.primaryActionCode isEqualToString:@"continue_prepared_upgrade"]) {
        return @"词库准备已完成交接，升级尚未完成。继续前请切换到其他输入源并关闭管理器；安装器会核验当前进度与程序身份，完成升级或继续已记录的恢复。";
    }
    NSDictionary<NSString *, NSString *> *maintenanceDetails = @{
        @"preparing_source": @"准备记录尚未完成。当前只提供状态查看，刷新不会继续维护或开始升级；已有词库及事务材料保持保留。",
        @"cancellation_requested": @"取消请求已保存，但取消尚未完成。源词库收尾及材料保留仍需核验，当前只能刷新状态。",
        @"cancellation_finishing_source": @"取消所需的源词库收尾尚未完成。请保留现场；刷新只重新检查状态，不会继续处理词库。",
        @"cancellation_preserving": @"源词库收尾已有记录，取消材料的封存与最终释放尚未完成。请保留现场，当前不能重新升级或移除程序。",
        @"cancellation_awaiting_qualification": @"取消材料已保留，仍需完成旧版本兼容性验证，当前尚未解除启动阻断。刷新不会执行验证或完成取消。",
        @"cancellation_completed": @"已核验本次准备取消的完整历史记录及最终释放状态，原版本安装回执保留。此页只查看结果，不会启动程序或重新升级。",
    };
    NSString *maintenanceDetail = maintenanceDetails[self.phaseCode];
    if (maintenanceDetail != nil) { return maintenanceDetail; }
    if ([self.primaryActionCode isEqualToString:@"abort_pre_switch_upgrade"] ||
        [self.secondaryActionCode isEqualToString:@"abort_pre_switch_upgrade"]) {
        return @"可以中止尚未切换数据的升级并恢复原版本程序。原词库及备份全部保留；已开始的恢复会继续同一事务。请先切换到其他输入源并关闭管理器。";
    }
    if (self.requiresManualQuiescence) {
        return @"请先手动切换到其他输入源并关闭萝卜词核管理器。确认后安装器仍会通过公开平台接口重新检查，勾选状态不作为静止证明。";
    }
    if (self.failedClosed) {
        if ([self.errorCode isEqualToString:RLXUnknownDriverResult] &&
            ![self.stateCode isEqualToString:@"none"]) {
            return @"上次操作返回错误，当前进度只表示已保存的阶段。请保留现场并按诊断码排查；刷新只重新检查状态。";
        }
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
        @"abort_pre_switch_upgrade": @"中止升级并恢复源程序",
        @"continue_prepared_upgrade": @"继续升级或恢复",
    };
    return titles[actionCode] ?: @"";
}

- (BOOL)isActionEnabled:(NSString *)actionCode {
    return [actionCode isKindOfClass:NSString.class] &&
        ![actionCode isEqualToString:RLXNoAction] &&
        ([actionCode isEqualToString:self.primaryActionCode] ||
         [actionCode isEqualToString:self.secondaryActionCode]) &&
        (!self.failedClosed || [actionCode isEqualToString:RLXRefreshAction]) &&
        [[self titleForAction:actionCode] length] > 0;
}

- (BOOL)requiresConfirmationForAction:(NSString *)actionCode {
    return [self isActionEnabled:actionCode] &&
        ![actionCode isEqualToString:RLXRefreshAction];
}

- (NSString *)confirmationTextForAction:(NSString *)actionCode {
    if ([actionCode isEqualToString:@"continue_prepared_upgrade"]) {
        return @"确认继续本次已完成词库准备的升级，并保留用户数据。请确认已手动切换到其他输入源并关闭管理器。安装器会重新读取当前状态、检查静止和程序身份；若事务已进入回滚，将继续同一事务的恢复。只有程序、数据与终态封存全部完成后才报告完成。";
    }
    if ([actionCode isEqualToString:@"abort_pre_switch_upgrade"]) {
        return @"确认中止本次尚未切换数据的升级，并从已验证备份恢复原版本 Manager 和 InputMethod。原词库、WAL、候选与事务证据均保留。若恢复已开始，将继续同一事务。请确认已手动切换到其他输入源并关闭管理器；安装器会重新验证静止、程序身份及数据保留条件。";
    }
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
