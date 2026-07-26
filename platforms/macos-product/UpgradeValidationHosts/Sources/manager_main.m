#import <Foundation/Foundation.h>

#import "RLXUpgradeValidationSupport.h"

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    RLXUpgradeValidationTarget target;
    if (!RLXParseUpgradeValidationTarget(argc, argv, &target)) return 2;
    NSURL *root = RLXFixedUpgradeDataRoot();
    if (root == nil) return 3;
    NSURL *database = RLXUpgradeDatabaseURL(root, target);
    NSURL *settings = RLXUpgradeSettingsURL(root, target);
    NSError *error = nil;
    NSData *before = [NSData dataWithContentsOfURL:database
                                           options:NSDataReadingMappedIfSafe
                                             error:&error];
    if (before == nil || !RLXRequireNoDatabaseSidecars(database, &error))
      return 3;
    RadishLexManagerUpgradeValidationRequest request = {
        .version = RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION,
        .candidate_path = database.fileSystemRepresentation,
        .settings_path = settings.fileSystemRepresentation,
    };
    RadishLexUpgradeValidationSummary summary = {0};
    RadishLexError *ffiError = NULL;
    RadishLexStatusCode status = radishlex_manager_upgrade_validate_candidate(
        &request, &summary, &ffiError);
    if (ffiError != NULL) radishlex_error_free(ffiError);
    if (status != RADISHLEX_STATUS_OK || summary.version != 1 ||
        summary.management_queries_checked != 1 || summary.settings_checked != 1 ||
        !RLXValidateDatabaseUnchanged(database, before, &error) ||
        !RLXRequireNoDatabaseSidecars(database, &error))
      return 4;
  }
  return 0;
}
