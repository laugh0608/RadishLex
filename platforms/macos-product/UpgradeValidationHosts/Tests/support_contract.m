#import <Foundation/Foundation.h>

#import "RLXUpgradeValidationSupport.h"

int main(void) {
  @autoreleasepool {
    RLXUpgradeValidationTarget target = RLXUpgradeValidationTargetPostSwitch;
    const char *candidateArguments[] = {"validation-host"};
    if (!RLXParseUpgradeValidationTarget(1, candidateArguments, &target) ||
        target != RLXUpgradeValidationTargetCandidate) {
      return 1;
    }

    const char *postSwitchArguments[] = {"validation-host", "--post-switch"};
    if (!RLXParseUpgradeValidationTarget(2, postSwitchArguments, &target) ||
        target != RLXUpgradeValidationTargetPostSwitch) {
      return 2;
    }

    const char *pathArguments[] = {"validation-host", "/tmp/userdb.sqlite3"};
    const char *extraArguments[] = {"validation-host", "--post-switch", "extra"};
    if (RLXParseUpgradeValidationTarget(2, pathArguments, &target) ||
        RLXParseUpgradeValidationTarget(3, extraArguments, &target)) {
      return 3;
    }

    NSURL *root = [NSURL fileURLWithPath:@"/private/tmp/radishlex-fixed-root"
                            isDirectory:YES];
    NSURL *candidate =
        RLXUpgradeDatabaseURL(root, RLXUpgradeValidationTargetCandidate);
    NSURL *candidateSettings =
        RLXUpgradeSettingsURL(root, RLXUpgradeValidationTargetCandidate);
    NSURL *active =
        RLXUpgradeDatabaseURL(root, RLXUpgradeValidationTargetPostSwitch);
    NSURL *activeSettings =
        RLXUpgradeSettingsURL(root, RLXUpgradeValidationTargetPostSwitch);
    if (![candidate.path
            isEqualToString:@"/private/tmp/radishlex-fixed-root/"
                             ".radishlex-upgrade-v1/"
                             "migration-candidate.sqlite3"] ||
        ![candidateSettings.path
            isEqualToString:@"/private/tmp/radishlex-fixed-root/"
                             ".radishlex-upgrade-v1/source-settings.json"] ||
        ![active.path
            isEqualToString:@"/private/tmp/radishlex-fixed-root/userdb.sqlite3"] ||
        ![activeSettings.path
            isEqualToString:@"/private/tmp/radishlex-fixed-root/"
                             "manager-settings.json"]) {
      return 4;
    }
  }
  return 0;
}
