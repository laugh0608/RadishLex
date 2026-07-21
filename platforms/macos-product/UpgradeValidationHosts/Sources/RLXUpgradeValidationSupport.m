#import "RLXUpgradeValidationSupport.h"

#import <errno.h>
#import <sys/stat.h>

NSURL *_Nullable RLXFixedUpgradeDataRoot(void) {
  NSArray<NSURL *> *roots =
      [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory
                                             inDomains:NSUserDomainMask];
  return [[roots firstObject] URLByAppendingPathComponent:@"RadishLex"
                                               isDirectory:YES];
}

BOOL RLXValidateCandidateUnchanged(NSURL *candidateURL, NSData *beforeBytes,
                                   NSError **error) {
  NSData *afterBytes = [NSData dataWithContentsOfURL:candidateURL
                                            options:NSDataReadingMappedIfSafe
                                              error:error];
  return afterBytes != nil && [afterBytes isEqualToData:beforeBytes];
}

BOOL RLXRequireNoCandidateSidecars(NSURL *candidateURL, NSError **error) {
  for (NSString *suffix in @[@"-wal", @"-shm", @"-journal"]) {
    NSString *path = [candidateURL.path stringByAppendingString:suffix];
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
