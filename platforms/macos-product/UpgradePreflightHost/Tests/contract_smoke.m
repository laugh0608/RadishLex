#import <Foundation/Foundation.h>

#import <fcntl.h>
#import <sys/stat.h>
#import <unistd.h>

#import "RLXUpgradePreflight.h"

static void RLXRequire(BOOL condition, NSString *message);

int main(void) {
  @autoreleasepool {
    NSFileManager *fileManager = [NSFileManager defaultManager];
    NSURL *temporaryRoot =
        [NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES]
            .URLByResolvingSymlinksInPath;
    NSURL *container = [temporaryRoot
        URLByAppendingPathComponent:[NSString
                                        stringWithFormat:
                                            @"radishlex-upgrade-preflight-%d-%@",
                                            getpid(), NSUUID.UUID.UUIDString]
                        isDirectory:YES];
    NSURL *dataRoot = [container URLByAppendingPathComponent:@"RadishLex"
                                                 isDirectory:YES];
    NSError *error = nil;
    RLXRequire([fileManager createDirectoryAtURL:dataRoot
                      withIntermediateDirectories:YES
                                       attributes:@{NSFilePosixPermissions : @0700}
                                            error:&error],
               @"private synthetic data root is created");

    RLXRequire(RLXUpgradeQuiescenceBlocker(0, 0, NO) == nil,
               @"empty process and handle evidence is quiescent");
    RLXRequire([RLXUpgradeQuiescenceBlocker(1, 0, NO)
                   isEqualToString:@"process_not_quiescent"],
               @"manager process blocks quiescence");
    RLXRequire([RLXUpgradeQuiescenceBlocker(0, 1, NO)
                   isEqualToString:@"process_not_quiescent"],
               @"input method process blocks quiescence");
    RLXRequire([RLXUpgradeQuiescenceBlocker(0, 0, YES)
                   isEqualToString:@"data_handle_open"],
               @"open controlled file blocks quiescence");

    RLXUpgradePreflightResult *emptyResult = RLXInspectUpgradePreflight(
        dataRoot, @"org.radishlex.synthetic.manager",
        @"org.radishlex.synthetic.inputmethod", &error);
    RLXRequire(emptyResult != nil,
               [NSString stringWithFormat:@"empty root inspection succeeds (%ld)",
                                          (long)error.code]);
    RLXRequire(error == nil, @"empty root inspection has no error");
    RLXRequire(emptyResult.isQuiescent, @"empty root is quiescent");
    RLXRequire(emptyResult.availableBytes > 0,
               @"empty root reports available capacity");

    NSURL *database = [dataRoot URLByAppendingPathComponent:@"userdb.sqlite3"];
    RLXRequire([fileManager createFileAtPath:database.path
                                    contents:[@"synthetic\n"
                                                 dataUsingEncoding:NSUTF8StringEncoding]
                                  attributes:@{NSFilePosixPermissions : @0600}],
               @"synthetic database is created");
    int descriptor = open(database.fileSystemRepresentation, O_RDONLY | O_CLOEXEC);
    RLXRequire(descriptor >= 0, @"synthetic database descriptor opens");
    error = nil;
    RLXUpgradePreflightResult *busyResult = RLXInspectUpgradePreflight(
        dataRoot, @"org.radishlex.synthetic.manager",
        @"org.radishlex.synthetic.inputmethod", &error);
    RLXRequire(busyResult != nil && error == nil && !busyResult.isQuiescent &&
                   [busyResult.blocker isEqualToString:@"data_handle_open"],
               @"open synthetic database is blocked");
    RLXRequire(close(descriptor) == 0, @"synthetic database descriptor closes");

    error = nil;
    RLXUpgradePreflightResult *closedResult = RLXInspectUpgradePreflight(
        dataRoot, @"org.radishlex.synthetic.manager",
        @"org.radishlex.synthetic.inputmethod", &error);
    RLXRequire(closedResult != nil && error == nil && closedResult.isQuiescent,
               @"closed synthetic database is ready");

    NSURL *settings =
        [dataRoot URLByAppendingPathComponent:@"manager-settings.json"];
    RLXRequire(symlink(database.fileSystemRepresentation,
                       settings.fileSystemRepresentation) == 0,
               @"synthetic settings symlink is created");
    error = nil;
    RLXRequire(RLXInspectUpgradePreflight(
                   dataRoot, @"org.radishlex.synthetic.manager",
                   @"org.radishlex.synthetic.inputmethod", &error) == nil &&
                   error.code == RLXUpgradePreflightErrorUnsafeDataFile,
               @"symlinked controlled file is rejected");
    RLXRequire(unlink(settings.fileSystemRepresentation) == 0,
               @"synthetic settings symlink is removed");

    NSURL *rootAlias =
        [container URLByAppendingPathComponent:@"RadishLexAlias" isDirectory:YES];
    RLXRequire(symlink(dataRoot.fileSystemRepresentation,
                       rootAlias.fileSystemRepresentation) == 0,
               @"synthetic data root symlink is created");
    error = nil;
    RLXRequire(RLXInspectUpgradePreflight(
                   rootAlias, @"org.radishlex.synthetic.manager",
                   @"org.radishlex.synthetic.inputmethod", &error) == nil &&
                   error.code == RLXUpgradePreflightErrorUnsafeDataRoot,
               @"symlinked data root is rejected");

    error = nil;
    RLXRequire([fileManager removeItemAtURL:container error:&error],
               @"synthetic preflight directory is removed");
  }
  return 0;
}

static void RLXRequire(BOOL condition, NSString *message) {
  if (!condition) {
    fprintf(stderr, "Upgrade preflight contract failed: %s\n",
            message.UTF8String);
    exit(1);
  }
}
