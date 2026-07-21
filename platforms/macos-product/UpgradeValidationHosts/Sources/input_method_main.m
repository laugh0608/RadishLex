#import <Foundation/Foundation.h>

#import <mach-o/dyld.h>
#import <unistd.h>

#import "RLXUpgradeValidationSupport.h"

int main(int argc, const char *argv[]) {
  (void)argv;
  @autoreleasepool {
    if (argc != 1) return 2;
    NSURL *root = RLXFixedUpgradeDataRoot();
    NSURL *state = [root URLByAppendingPathComponent:@".radishlex-upgrade-v1"
                                         isDirectory:YES];
    NSURL *candidate = [state URLByAppendingPathComponent:@"migration-candidate.sqlite3"];
    uint32_t executableLength = 0;
    _NSGetExecutablePath(NULL, &executableLength);
    NSMutableData *buffer = [NSMutableData dataWithLength:executableLength];
    if (_NSGetExecutablePath(buffer.mutableBytes, &executableLength) != 0) return 3;
    NSString *executable = [NSString stringWithUTF8String:buffer.bytes];
    NSURL *contents = [[[NSURL fileURLWithPath:executable] URLByDeletingLastPathComponent]
        URLByDeletingLastPathComponent];
    NSURL *sharedData = [[contents URLByAppendingPathComponent:@"Resources" isDirectory:YES]
        URLByAppendingPathComponent:@"RimeData" isDirectory:YES];
    NSString *schema = [[[NSBundle mainBundle] objectForInfoDictionaryKey:@"RadishLexRimeSchema"]
        description];
    if (schema.length == 0) schema = @"radishlex_pinyin";
    NSError *error = nil;
    NSData *before = [NSData dataWithContentsOfURL:candidate
                                           options:NSDataReadingMappedIfSafe
                                             error:&error];
    if (root == nil || before == nil ||
        !RLXRequireNoCandidateSidecars(candidate, &error)) return 3;
    NSURL *validationUserData = [NSURL fileURLWithPath:[NSTemporaryDirectory()
        stringByAppendingPathComponent:[NSString stringWithFormat:
            @"radishlex-input-upgrade-validation-%d-%@", getpid(), NSUUID.UUID.UUIDString]]
                                      isDirectory:YES];
    if (![[NSFileManager defaultManager] createDirectoryAtURL:validationUserData
                                  withIntermediateDirectories:NO
                                                   attributes:@{NSFilePosixPermissions : @0700}
                                                        error:&error]) return 3;
    RadishLexInputMethodUpgradeValidationRequest request = {
        .version = RADISHLEX_INPUT_METHOD_UPGRADE_VALIDATION_REQUEST_VERSION,
        .candidate_path = candidate.fileSystemRepresentation,
        .shared_data_path = sharedData.fileSystemRepresentation,
        .validation_user_data_path = validationUserData.fileSystemRepresentation,
        .schema = schema.UTF8String,
    };
    RadishLexUpgradeValidationSummary summary = {0};
    RadishLexError *ffiError = NULL;
    RadishLexStatusCode status = radishlex_input_method_upgrade_validate_candidate(
        &request, &summary, &ffiError);
    if (ffiError != NULL) radishlex_error_free(ffiError);
    BOOL cleanup = [[NSFileManager defaultManager] removeItemAtURL:validationUserData error:&error];
    if (status != RADISHLEX_STATUS_OK || summary.version != 1 ||
        summary.personalized_runtime_checked != 1 || summary.candidate_signals_read != 1 ||
        !cleanup || !RLXValidateCandidateUnchanged(candidate, before, &error) ||
        !RLXRequireNoCandidateSidecars(candidate, &error)) return 4;
  }
  return 0;
}
