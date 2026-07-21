#import <Foundation/Foundation.h>

#import "RLXUpgradeValidationSupport.h"

int main(int argc, const char *argv[]) {
  (void)argv;
  @autoreleasepool {
    if (argc != 1) return 2;
    NSURL *root = RLXFixedUpgradeDataRoot();
    NSURL *state = [root URLByAppendingPathComponent:@".radishlex-upgrade-v1"
                                         isDirectory:YES];
    NSURL *candidate = [state URLByAppendingPathComponent:@"migration-candidate.sqlite3"];
    NSURL *settings = [state URLByAppendingPathComponent:@"source-settings.json"];
    NSError *error = nil;
    NSData *before = [NSData dataWithContentsOfURL:candidate
                                           options:NSDataReadingMappedIfSafe
                                             error:&error];
    if (root == nil || before == nil ||
        !RLXRequireNoCandidateSidecars(candidate, &error)) return 3;
    RadishLexManagerUpgradeValidationRequest request = {
        .version = RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION,
        .candidate_path = candidate.fileSystemRepresentation,
        .settings_path = settings.fileSystemRepresentation,
    };
    RadishLexUpgradeValidationSummary summary = {0};
    RadishLexError *ffiError = NULL;
    RadishLexStatusCode status = radishlex_manager_upgrade_validate_candidate(
        &request, &summary, &ffiError);
    if (ffiError != NULL) radishlex_error_free(ffiError);
    if (status != RADISHLEX_STATUS_OK || summary.version != 1 ||
        summary.management_queries_checked != 1 || summary.settings_checked != 1 ||
        !RLXValidateCandidateUnchanged(candidate, before, &error) ||
        !RLXRequireNoCandidateSidecars(candidate, &error)) return 4;
  }
  return 0;
}
