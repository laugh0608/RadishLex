#import "RLXUpgradeValidationSupport.h"

#import <errno.h>
#import <string.h>
#import <sys/stat.h>

NSURL *_Nullable RLXFixedUpgradeDataRoot(void) {
  NSArray<NSURL *> *roots =
      [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory
                                             inDomains:NSUserDomainMask];
  return [[roots firstObject] URLByAppendingPathComponent:@"RadishLex"
                                               isDirectory:YES];
}

BOOL RLXParseUpgradeValidationTarget(int argc, const char *const *argv,
                                     RLXUpgradeValidationTarget *target) {
  if (argc == 1) {
    *target = RLXUpgradeValidationTargetCandidate;
    return YES;
  }
  if (argc == 2 && strcmp(argv[1], "--post-switch") == 0) {
    *target = RLXUpgradeValidationTargetPostSwitch;
    return YES;
  }
  return NO;
}

NSURL *RLXUpgradeDatabaseURL(NSURL *root,
                             RLXUpgradeValidationTarget target) {
  if (target == RLXUpgradeValidationTargetPostSwitch) {
    return [root URLByAppendingPathComponent:@"userdb.sqlite3"];
  }
  NSURL *state = [root URLByAppendingPathComponent:@".radishlex-upgrade-v1"
                                       isDirectory:YES];
  return [state URLByAppendingPathComponent:@"migration-candidate.sqlite3"];
}

NSURL *RLXUpgradeSettingsURL(NSURL *root,
                             RLXUpgradeValidationTarget target) {
  if (target == RLXUpgradeValidationTargetPostSwitch) {
    return [root URLByAppendingPathComponent:@"manager-settings.json"];
  }
  NSURL *state = [root URLByAppendingPathComponent:@".radishlex-upgrade-v1"
                                       isDirectory:YES];
  return [state URLByAppendingPathComponent:@"source-settings.json"];
}

BOOL RLXValidateDatabaseUnchanged(NSURL *databaseURL, NSData *beforeBytes,
                                  NSError **error) {
  NSData *afterBytes = [NSData dataWithContentsOfURL:databaseURL
                                            options:NSDataReadingMappedIfSafe
                                              error:error];
  return afterBytes != nil && [afterBytes isEqualToData:beforeBytes];
}

BOOL RLXRequireNoDatabaseSidecars(NSURL *databaseURL, NSError **error) {
  for (NSString *suffix in @[@"-wal", @"-shm", @"-journal"]) {
    NSString *path = [databaseURL.path stringByAppendingString:suffix];
    struct stat metadata;
    if (lstat(path.fileSystemRepresentation, &metadata) == 0 || errno != ENOENT) {
      if (error != NULL) {
        *error = [NSError errorWithDomain:@"org.radishlex.upgrade-validation"
                                     code:1
                                 userInfo:nil];
      }
      return NO;
    }
  }
  return YES;
}
