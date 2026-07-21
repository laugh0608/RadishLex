#import "RLXUpgradePreflight.h"

#import <AppKit/AppKit.h>
#import <errno.h>
#import <sys/stat.h>
#import <unistd.h>

NSErrorDomain const RLXUpgradePreflightErrorDomain =
    @"org.radishlex.product-upgrade.preflight";

static NSString *const RLXProcessBlocker = @"process_not_quiescent";
static NSString *const RLXOpenHandleBlocker = @"data_handle_open";
static NSArray<NSString *> *RLXControlledDataFileNames(void);
static BOOL RLXValidateDataRoot(NSURL *dataRootURL, NSError **error);
static NSArray<NSString *> *_Nullable RLXExistingControlledDataPaths(
    NSURL *dataRootURL, NSError **error);
static NSNumber *_Nullable RLXAvailableCapacity(NSURL *dataRootURL,
                                                 NSError **error);
static BOOL RLXHasOpenDataHandle(NSArray<NSString *> *paths,
                                 BOOL *hasOpenDataHandle, NSError **error);
static NSError *RLXError(RLXUpgradePreflightErrorCode code);

@interface RLXUpgradePreflightResult ()

- (instancetype)initWithAvailableBytes:(unsigned long long)availableBytes
                              blocker:(nullable NSString *)blocker;

@end


@implementation RLXUpgradePreflightResult

- (instancetype)initWithAvailableBytes:(unsigned long long)availableBytes
                              blocker:(nullable NSString *)blocker {
  self = [super init];
  if (self != nil) {
    _availableBytes = availableBytes;
    _blocker = [blocker copy];
    _quiescent = blocker == nil;
  }
  return self;
}

@end

NSString *_Nullable RLXUpgradeQuiescenceBlocker(
    NSUInteger managerProcessCount, NSUInteger inputMethodProcessCount,
    BOOL hasOpenDataHandle) {
  if (managerProcessCount > 0 || inputMethodProcessCount > 0) {
    return RLXProcessBlocker;
  }
  if (hasOpenDataHandle) {
    return RLXOpenHandleBlocker;
  }
  return nil;
}

RLXUpgradePreflightResult *_Nullable RLXInspectUpgradePreflight(
    NSURL *dataRootURL, NSString *managerBundleIdentifier,
    NSString *inputMethodBundleIdentifier, NSError **error) {
  if (!RLXValidateDataRoot(dataRootURL, error)) {
    return nil;
  }
  NSNumber *availableCapacity = RLXAvailableCapacity(dataRootURL, error);
  if (availableCapacity == nil) {
    return nil;
  }
  NSArray<NSString *> *paths = RLXExistingControlledDataPaths(dataRootURL, error);
  if (paths == nil) {
    return nil;
  }

  NSArray<NSRunningApplication *> *managerProcesses =
      [NSRunningApplication
          runningApplicationsWithBundleIdentifier:managerBundleIdentifier];
  NSArray<NSRunningApplication *> *inputMethodProcesses =
      [NSRunningApplication
          runningApplicationsWithBundleIdentifier:inputMethodBundleIdentifier];
  if (managerProcesses == nil || inputMethodProcesses == nil) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorProcessInspectionFailed);
    }
    return nil;
  }

  BOOL hasOpenDataHandle = NO;
  if (!RLXHasOpenDataHandle(paths, &hasOpenDataHandle, error)) {
    return nil;
  }
  NSString *blocker = RLXUpgradeQuiescenceBlocker(
      managerProcesses.count, inputMethodProcesses.count, hasOpenDataHandle);
  return [[RLXUpgradePreflightResult alloc]
      initWithAvailableBytes:availableCapacity.unsignedLongLongValue
                     blocker:blocker];
}

static NSArray<NSString *> *RLXControlledDataFileNames(void) {
  return @[
    @"userdb.sqlite3", @"userdb.sqlite3-wal", @"userdb.sqlite3-shm",
    @"userdb.sqlite3-journal", @"manager-settings.json",
    @"manager-settings.json.tmp"
  ];
}

static BOOL RLXValidateDataRoot(NSURL *dataRootURL, NSError **error) {
  if (!dataRootURL.isFileURL || dataRootURL.path.length == 0 ||
      ![dataRootURL.URLByStandardizingPath.path isEqualToString:dataRootURL.path] ||
      ![dataRootURL.URLByResolvingSymlinksInPath.path
          isEqualToString:dataRootURL.path]) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorUnsafeDataRoot);
    }
    return NO;
  }
  struct stat metadata;
  if (lstat(dataRootURL.fileSystemRepresentation, &metadata) != 0 ||
      !S_ISDIR(metadata.st_mode) || metadata.st_uid != geteuid() ||
      (metadata.st_mode & 07777) != 0700 || metadata.st_nlink == 0) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorUnsafeDataRoot);
    }
    return NO;
  }
  return YES;
}

static NSArray<NSString *> *_Nullable RLXExistingControlledDataPaths(
    NSURL *dataRootURL, NSError **error) {
  NSMutableArray<NSString *> *paths = [NSMutableArray array];
  for (NSString *fileName in RLXControlledDataFileNames()) {
    NSURL *fileURL = [dataRootURL URLByAppendingPathComponent:fileName
                                                  isDirectory:NO];
    struct stat metadata;
    if (lstat(fileURL.fileSystemRepresentation, &metadata) != 0) {
      if (errno == ENOENT) {
        continue;
      }
      if (error != NULL) {
        *error = RLXError(RLXUpgradePreflightErrorUnsafeDataFile);
      }
      return nil;
    }
    if (!S_ISREG(metadata.st_mode) || metadata.st_uid != geteuid() ||
        (metadata.st_mode & 07777) != 0600 || metadata.st_nlink != 1) {
      if (error != NULL) {
        *error = RLXError(RLXUpgradePreflightErrorUnsafeDataFile);
      }
      return nil;
    }
    [paths addObject:fileURL.path];
  }
  return paths;
}

static NSNumber *_Nullable RLXAvailableCapacity(NSURL *dataRootURL,
                                                 NSError **error) {
  NSError *capacityError = nil;
  NSDictionary<NSURLResourceKey, id> *values = [dataRootURL
      resourceValuesForKeys:@[
        NSURLVolumeAvailableCapacityForImportantUsageKey,
        NSURLVolumeAvailableCapacityKey
      ]
                       error:&capacityError];
  NSNumber *importantCapacity =
      values[NSURLVolumeAvailableCapacityForImportantUsageKey];
  NSNumber *generalCapacity = values[NSURLVolumeAvailableCapacityKey];
  unsigned long long importantBytes = importantCapacity.unsignedLongLongValue;
  unsigned long long generalBytes = generalCapacity.unsignedLongLongValue;
  unsigned long long availableBytes = 0;
  if (importantBytes > 0 && generalBytes > 0) {
    availableBytes = MIN(importantBytes, generalBytes);
  } else {
    availableBytes = MAX(importantBytes, generalBytes);
  }
  if (values == nil || availableBytes == 0) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorAvailableSpaceUnavailable);
    }
    return nil;
  }
  return @(availableBytes);
}

static BOOL RLXHasOpenDataHandle(NSArray<NSString *> *paths,
                                 BOOL *hasOpenDataHandle, NSError **error) {
  if (hasOpenDataHandle == NULL) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorOpenHandleInspectionFailed);
    }
    return NO;
  }
  *hasOpenDataHandle = NO;
  if (paths.count == 0) {
    return YES;
  }

  NSMutableArray<NSString *> *arguments =
      [NSMutableArray arrayWithObjects:@"-n", @"-P", @"-F", @"p", @"--", nil];
  [arguments addObjectsFromArray:paths];
  NSTask *task = [[NSTask alloc] init];
  task.executableURL = [NSURL fileURLWithPath:@"/usr/sbin/lsof"];
  task.arguments = arguments;
  NSPipe *standardOutput = [NSPipe pipe];
  NSPipe *standardError = [NSPipe pipe];
  task.standardOutput = standardOutput;
  task.standardError = standardError;
  NSError *launchError = nil;
  if (![task launchAndReturnError:&launchError]) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorOpenHandleInspectionFailed);
    }
    return NO;
  }
  [task waitUntilExit];
  NSData *output = [standardOutput.fileHandleForReading readDataToEndOfFile];
  NSData *errorOutput = [standardError.fileHandleForReading readDataToEndOfFile];
  if (task.terminationReason != NSTaskTerminationReasonExit ||
      errorOutput.length != 0 ||
      (task.terminationStatus != 0 && task.terminationStatus != 1) ||
      (task.terminationStatus == 1 && output.length != 0)) {
    if (error != NULL) {
      *error = RLXError(RLXUpgradePreflightErrorOpenHandleInspectionFailed);
    }
    return NO;
  }
  *hasOpenDataHandle = task.terminationStatus == 0 && output.length > 0;
  return YES;
}

static NSError *RLXError(RLXUpgradePreflightErrorCode code) {
  return [NSError errorWithDomain:RLXUpgradePreflightErrorDomain
                             code:code
                         userInfo:nil];
}
