#import <Foundation/Foundation.h>

#include <CoreFoundation/CoreFoundation.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#ifndef RADISHLEX_PRIVACY_CONTRACT
#define RADISHLEX_PRIVACY_CONTRACT 0
#endif

#ifndef RLX_PRIVACY_STATE_DIR
#error "RLX_PRIVACY_STATE_DIR must be fixed at compile time"
#endif

#define RLX_PRIVACY_RECEIPT_NAME "privacy-mode-baseline"

typedef NS_ENUM(NSInteger, RLXPrivacyModeState) {
  RLXPrivacyModeStateAbsent = 0,
  RLXPrivacyModeStateDisabled = 1,
  RLXPrivacyModeStateEnabled = 2,
  RLXPrivacyModeStateInvalid = 3,
};

typedef struct {
  int directoryDescriptor;
  int descriptor;
  struct stat metadata;
  RLXPrivacyModeState state;
} RLXPrivacyReceipt;

static NSString *const RLXPrivacyDomain = @"org.radishlex.inputmethod.macos";
static NSString *const RLXPrivacyKey = @"RadishLexPrivacyMode";
static NSString *const RLXReceiptHeader = @"radishlex-privacy-baseline-v2";
static const char *const RLXStateDirectoryPath = RLX_PRIVACY_STATE_DIR;
static const char *const RLXReceiptName = RLX_PRIVACY_RECEIPT_NAME;

#if RADISHLEX_PRIVACY_CONTRACT
static BOOL RLXContractDidWrite = NO;
#endif

static NSString *RLXStateName(RLXPrivacyModeState state) {
  switch (state) {
  case RLXPrivacyModeStateAbsent:
    return @"absent";
  case RLXPrivacyModeStateDisabled:
    return @"false";
  case RLXPrivacyModeStateEnabled:
    return @"true";
  case RLXPrivacyModeStateInvalid:
    return @"invalid";
  }
  return @"invalid";
}

static BOOL RLXParseState(NSString *value, RLXPrivacyModeState *state) {
  if ([value isEqualToString:@"absent"]) {
    *state = RLXPrivacyModeStateAbsent;
    return YES;
  }
  if ([value isEqualToString:@"false"]) {
    *state = RLXPrivacyModeStateDisabled;
    return YES;
  }
  if ([value isEqualToString:@"true"]) {
    *state = RLXPrivacyModeStateEnabled;
    return YES;
  }
  return NO;
}

static void RLXSetError(NSString **error, NSString *message) {
  if (error != NULL)
    *error = message;
}

static BOOL RLXSameIdentity(const struct stat *left,
                            const struct stat *right) {
  return left->st_dev == right->st_dev && left->st_ino == right->st_ino;
}

static BOOL RLXParseUnsignedInteger(NSString *value,
                                    unsigned long long *result) {
  const char *characters = value.UTF8String;
  if (characters == NULL || characters[0] == '\0')
    return NO;
  for (const char *cursor = characters; *cursor != '\0'; ++cursor) {
    if (*cursor < '0' || *cursor > '9')
      return NO;
  }
  errno = 0;
  char *end = NULL;
  unsigned long long parsed = strtoull(characters, &end, 10);
  if (errno != 0 || end == characters || *end != '\0')
    return NO;
  *result = parsed;
  return YES;
}

static BOOL RLXWriteAll(int descriptor, NSData *data) {
  const uint8_t *bytes = data.bytes;
  size_t remaining = data.length;
  while (remaining > 0) {
    ssize_t written = write(descriptor, bytes, remaining);
    if (written < 0) {
      if (errno == EINTR)
        continue;
      return NO;
    }
    bytes += written;
    remaining -= (size_t)written;
  }
  return YES;
}

static BOOL RLXReadExactAtStart(int descriptor, size_t length,
                                NSMutableData **data, NSString **error) {
  NSMutableData *contents = [NSMutableData dataWithLength:length];
  uint8_t *bytes = contents.mutableBytes;
  size_t offset = 0;
  while (offset < length) {
    ssize_t count = pread(descriptor, bytes + offset, length - offset,
                          (off_t)offset);
    if (count < 0) {
      if (errno == EINTR)
        continue;
      RLXSetError(error, @"privacy baseline cannot be read completely");
      return NO;
    }
    if (count == 0) {
      RLXSetError(error, @"privacy baseline ended unexpectedly");
      return NO;
    }
    offset += (size_t)count;
  }
  uint8_t trailing = 0;
  ssize_t trailingCount;
  do {
    trailingCount = pread(descriptor, &trailing, 1, (off_t)length);
  } while (trailingCount < 0 && errno == EINTR);
  if (trailingCount != 0) {
    RLXSetError(error, @"privacy baseline changed while it was read");
    return NO;
  }
  *data = contents;
  return YES;
}

static BOOL RLXValidateReceiptMetadata(const struct stat *metadata,
                                       NSString **error) {
  if (!S_ISREG(metadata->st_mode) || metadata->st_uid != getuid() ||
      (metadata->st_mode & 07777) != 0600 || metadata->st_nlink != 1 ||
      metadata->st_size <= 0 || metadata->st_size > 256) {
    RLXSetError(error,
                @"privacy baseline is missing, unsafe or malformed");
    return NO;
  }
  return YES;
}

static BOOL RLXParseReceipt(int descriptor, const struct stat *metadata,
                            RLXPrivacyModeState *state, NSString **error) {
  NSMutableData *data = nil;
  if (!RLXReadExactAtStart(descriptor, (size_t)metadata->st_size, &data,
                          error))
    return NO;
  NSString *contents = [[NSString alloc] initWithData:data
                                              encoding:NSUTF8StringEncoding];
  NSArray<NSString *> *lines = [contents componentsSeparatedByString:@"\n"];
  unsigned long long recordedDevice = 0;
  unsigned long long recordedInode = 0;
  if (contents == nil || lines.count != 5 ||
      ![lines[0] isEqualToString:RLXReceiptHeader] ||
      ![lines[1] hasPrefix:@"state="] ||
      !RLXParseState([lines[1] substringFromIndex:6], state) ||
      *state == RLXPrivacyModeStateInvalid ||
      ![lines[2] hasPrefix:@"device="] ||
      !RLXParseUnsignedInteger([lines[2] substringFromIndex:7],
                               &recordedDevice) ||
      ![lines[3] hasPrefix:@"inode="] ||
      !RLXParseUnsignedInteger([lines[3] substringFromIndex:6],
                               &recordedInode) ||
      ![lines[4] isEqualToString:@""] ||
      recordedDevice != (unsigned long long)metadata->st_dev ||
      recordedInode != (unsigned long long)metadata->st_ino) {
    RLXSetError(error, @"privacy baseline has an invalid identity or format");
    return NO;
  }
  return YES;
}

static uid_t RLXObservedStateDirectoryOwner(uid_t owner) {
#if RADISHLEX_PRIVACY_CONTRACT
  if (getenv("RADISHLEX_PRIVACY_CONTRACT_FOREIGN_STATE_OWNER") != NULL)
    return owner == UINT32_MAX ? owner - 1 : owner + 1;
#endif
  return owner;
}

static BOOL RLXSynchronizeStateParent(int descriptor) {
#if RADISHLEX_PRIVACY_CONTRACT
  if (getenv("RADISHLEX_PRIVACY_CONTRACT_FAIL_PARENT_FSYNC") != NULL) {
    errno = EIO;
    return NO;
  }
#endif
  return fsync(descriptor) == 0;
}

static BOOL RLXOpenStateDirectory(BOOL createIfMissing, int *descriptor,
                                  NSString **error) {
  if (RLXStateDirectoryPath == NULL || RLXStateDirectoryPath[0] != '/' ||
      RLXStateDirectoryPath[1] == '\0') {
    RLXSetError(error, @"privacy state directory is not a fixed absolute path");
    return NO;
  }

  int current = open("/", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (current < 0) {
    RLXSetError(error, @"privacy state directory root cannot be opened");
    return NO;
  }

  const char *cursor = RLXStateDirectoryPath + 1;
  BOOL sawComponent = NO;
  while (*cursor != '\0') {
    const char *separator = strchr(cursor, '/');
    size_t length = separator == NULL ? strlen(cursor)
                                      : (size_t)(separator - cursor);
    if (length == 0 || length > NAME_MAX ||
        (length == 1 && cursor[0] == '.') ||
        (length == 2 && cursor[0] == '.' && cursor[1] == '.')) {
      close(current);
      RLXSetError(error, @"privacy state directory path is not canonical");
      return NO;
    }
    char component[NAME_MAX + 1];
    memcpy(component, cursor, length);
    component[length] = '\0';
    BOOL isFinal = separator == NULL;

    int next = openat(current, component,
                      O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (next < 0 && isFinal && createIfMissing && errno == ENOENT) {
      if (mkdirat(current, component, 0700) != 0 && errno != EEXIST) {
        close(current);
        RLXSetError(error, @"privacy state directory cannot be created safely");
        return NO;
      }
      next = openat(current, component,
                    O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    }
    if (next < 0) {
      close(current);
      RLXSetError(error,
                  @"privacy state directory has a missing or unsafe component");
      return NO;
    }

    struct stat openedMetadata;
    struct stat pathMetadata;
    if (fstat(next, &openedMetadata) != 0 ||
        fstatat(current, component, &pathMetadata, AT_SYMLINK_NOFOLLOW) != 0 ||
        !S_ISDIR(openedMetadata.st_mode) ||
        !S_ISDIR(pathMetadata.st_mode) ||
        !RLXSameIdentity(&openedMetadata, &pathMetadata)) {
      close(next);
      close(current);
      RLXSetError(error,
                  @"privacy state directory component changed while opening");
      return NO;
    }

    if (!isFinal) {
      mode_t ancestorMode = openedMetadata.st_mode & 07777;
      uid_t observedAncestorOwner =
          RLXObservedStateDirectoryOwner(openedMetadata.st_uid);
      BOOL isRootOwnedStickyDirectory =
          observedAncestorOwner == 0 && (ancestorMode & S_ISVTX) != 0;
      if ((observedAncestorOwner != 0 &&
           observedAncestorOwner != getuid()) ||
          ((ancestorMode & 0022) != 0 && !isRootOwnedStickyDirectory)) {
        close(next);
        close(current);
        RLXSetError(
            error,
            @"privacy state directory has a replaceable unsafe ancestor");
        return NO;
      }
    }

    if (isFinal) {
      mode_t mode = openedMetadata.st_mode & 07777;
      uid_t observedOwner =
          RLXObservedStateDirectoryOwner(openedMetadata.st_uid);
      if (observedOwner != getuid() || (mode & 07000) != 0 ||
          (mode & 0022) != 0) {
        close(next);
        close(current);
        RLXSetError(error,
                    @"privacy state directory owner or mode is unsafe");
        return NO;
      }
      if (mode != 0700) {
        if (fchmod(next, 0700) != 0 || fstat(next, &openedMetadata) != 0 ||
            (openedMetadata.st_mode & 07777) != 0700 ||
            openedMetadata.st_uid != getuid()) {
          close(next);
          close(current);
          RLXSetError(error,
                      @"privacy state directory cannot be restricted to 0700");
          return NO;
        }
      }
      if (fstatat(current, component, &pathMetadata,
                  AT_SYMLINK_NOFOLLOW) != 0 ||
          !S_ISDIR(pathMetadata.st_mode) ||
          !RLXSameIdentity(&openedMetadata, &pathMetadata)) {
        close(next);
        close(current);
        RLXSetError(error,
                    @"privacy state directory changed while securing it");
        return NO;
      }
      if (!RLXSynchronizeStateParent(current)) {
        close(next);
        close(current);
        RLXSetError(error,
                    @"privacy state directory parent could not be synced");
        return NO;
      }
    }

    close(current);
    current = next;
    sawComponent = YES;
    if (separator == NULL)
      break;
    cursor = separator + 1;
    if (*cursor == '\0') {
      close(current);
      RLXSetError(error, @"privacy state directory path is not canonical");
      return NO;
    }
  }

  if (!sawComponent) {
    close(current);
    RLXSetError(error, @"privacy state directory path has no component");
    return NO;
  }
  *descriptor = current;
  return YES;
}

static void RLXCloseReceipt(RLXPrivacyReceipt *receipt) {
  if (receipt->descriptor >= 0)
    close(receipt->descriptor);
  if (receipt->directoryDescriptor >= 0)
    close(receipt->directoryDescriptor);
  receipt->descriptor = -1;
  receipt->directoryDescriptor = -1;
}

static BOOL RLXRevalidateReceipt(RLXPrivacyReceipt *receipt,
                                 NSString **error) {
  struct stat openedMetadata;
  struct stat pathMetadata;
  RLXPrivacyModeState state;
  if (fstat(receipt->descriptor, &openedMetadata) != 0 ||
      !RLXValidateReceiptMetadata(&openedMetadata, error) ||
      !RLXSameIdentity(&openedMetadata, &receipt->metadata) ||
      fstatat(receipt->directoryDescriptor, RLXReceiptName, &pathMetadata,
              AT_SYMLINK_NOFOLLOW) != 0 ||
      !RLXValidateReceiptMetadata(&pathMetadata, error) ||
      !RLXSameIdentity(&openedMetadata, &pathMetadata) ||
      !RLXParseReceipt(receipt->descriptor, &openedMetadata, &state, error) ||
      state != receipt->state) {
    if (error != NULL && *error == nil)
      *error = @"privacy baseline identity drifted";
    return NO;
  }
  return YES;
}

static BOOL RLXOpenReceipt(RLXPrivacyReceipt *receipt, NSString **error) {
  receipt->directoryDescriptor = -1;
  receipt->descriptor = -1;
  if (!RLXOpenStateDirectory(NO, &receipt->directoryDescriptor, error))
    return NO;

  struct stat pathMetadata;
  if (fstatat(receipt->directoryDescriptor, RLXReceiptName, &pathMetadata,
              AT_SYMLINK_NOFOLLOW) != 0 ||
      !RLXValidateReceiptMetadata(&pathMetadata, error)) {
    if (error != NULL && *error == nil)
      *error = @"privacy baseline is missing, unsafe or malformed";
    RLXCloseReceipt(receipt);
    return NO;
  }

  receipt->descriptor =
      openat(receipt->directoryDescriptor, RLXReceiptName,
             O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
  if (receipt->descriptor < 0 ||
      fstat(receipt->descriptor, &receipt->metadata) != 0 ||
      !RLXValidateReceiptMetadata(&receipt->metadata, error) ||
      !RLXSameIdentity(&receipt->metadata, &pathMetadata) ||
      !RLXParseReceipt(receipt->descriptor, &receipt->metadata,
                       &receipt->state, error) ||
      !RLXRevalidateReceipt(receipt, error)) {
    if (error != NULL && *error == nil)
      *error = @"privacy baseline cannot be opened safely";
    RLXCloseReceipt(receipt);
    return NO;
  }
  return YES;
}

static void RLXRemoveCreatedReceiptIfSame(int directoryDescriptor,
                                          int descriptor) {
  struct stat openedMetadata;
  struct stat pathMetadata;
  if (fstat(descriptor, &openedMetadata) == 0 &&
      fstatat(directoryDescriptor, RLXReceiptName, &pathMetadata,
              AT_SYMLINK_NOFOLLOW) == 0 &&
      RLXSameIdentity(&openedMetadata, &pathMetadata) &&
      openedMetadata.st_nlink == 1)
    (void)unlinkat(directoryDescriptor, RLXReceiptName, 0);
}

static BOOL RLXCreateReceipt(RLXPrivacyModeState state, NSString **error) {
  int directoryDescriptor = -1;
  if (!RLXOpenStateDirectory(YES, &directoryDescriptor, error))
    return NO;

  int descriptor =
      openat(directoryDescriptor, RLXReceiptName,
             O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  if (descriptor < 0) {
    close(directoryDescriptor);
    RLXSetError(error, @"privacy baseline already exists or is unsafe");
    return NO;
  }
  if (fchmod(descriptor, 0600) != 0) {
    RLXRemoveCreatedReceiptIfSame(directoryDescriptor, descriptor);
    close(descriptor);
    close(directoryDescriptor);
    RLXSetError(error, @"privacy baseline mode could not be secured");
    return NO;
  }

  struct stat metadata;
  if (fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) ||
      metadata.st_uid != getuid() || (metadata.st_mode & 07777) != 0600 ||
      metadata.st_nlink != 1) {
    RLXRemoveCreatedReceiptIfSame(directoryDescriptor, descriptor);
    close(descriptor);
    close(directoryDescriptor);
    RLXSetError(error, @"privacy baseline inode is unsafe");
    return NO;
  }

  NSString *contents = [NSString
      stringWithFormat:@"%@\nstate=%@\ndevice=%llu\ninode=%llu\n",
                       RLXReceiptHeader, RLXStateName(state),
                       (unsigned long long)metadata.st_dev,
                       (unsigned long long)metadata.st_ino];
  NSData *data = [contents dataUsingEncoding:NSUTF8StringEncoding];
  BOOL wrote = RLXWriteAll(descriptor, data) && fsync(descriptor) == 0;
  RLXPrivacyReceipt receipt = {
      .directoryDescriptor = directoryDescriptor,
      .descriptor = descriptor,
      .metadata = metadata,
      .state = state,
  };
  if (!wrote || !RLXRevalidateReceipt(&receipt, error)) {
    RLXRemoveCreatedReceiptIfSame(directoryDescriptor, descriptor);
    RLXCloseReceipt(&receipt);
    if (error != NULL && *error == nil)
      *error = @"privacy baseline could not be written completely";
    return NO;
  }
  if (fsync(directoryDescriptor) != 0) {
    RLXCloseReceipt(&receipt);
    RLXSetError(error,
                @"privacy baseline is retained but directory sync failed; "
                 "inspect the fixed receipt before retrying");
    return NO;
  }
  RLXCloseReceipt(&receipt);
  return YES;
}

#if RADISHLEX_PRIVACY_CONTRACT
static void RLXContractReplaceReceiptIfRequested(const char *environmentName,
                                                  RLXPrivacyReceipt *receipt) {
  if (getenv(environmentName) == NULL)
    return;
  if (unlinkat(receipt->directoryDescriptor, RLXReceiptName, 0) != 0)
    return;
  int replacement =
      openat(receipt->directoryDescriptor, RLXReceiptName,
             O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  if (replacement < 0)
    return;
  static const char replacementContents[] = "replacement\n";
  (void)write(replacement, replacementContents,
              sizeof(replacementContents) - 1);
  (void)close(replacement);
}
#endif

#if RADISHLEX_PRIVACY_CONTRACT
static NSString *RLXContractStorePath(NSString **error) {
  const char *path = getenv("RADISHLEX_PRIVACY_CONTRACT_STORE");
  if (path == NULL || path[0] == '\0') {
    RLXSetError(error, @"contract store path is unavailable");
    return nil;
  }
  return [NSString stringWithUTF8String:path];
}

static BOOL RLXReadPrivacyState(RLXPrivacyModeState *state,
                                NSString **error) {
  if (RLXContractDidWrite) {
    const char *override = getenv("RADISHLEX_PRIVACY_CONTRACT_VERIFY_STATE");
    if (override != NULL && override[0] != '\0') {
      NSString *value = [NSString stringWithUTF8String:override];
      if (!RLXParseState(value, state))
        *state = RLXPrivacyModeStateInvalid;
      return YES;
    }
  }

  NSString *path = RLXContractStorePath(error);
  if (path == nil)
    return NO;
  if (access(path.fileSystemRepresentation, F_OK) != 0) {
    if (errno == ENOENT) {
      *state = RLXPrivacyModeStateAbsent;
      return YES;
    }
    RLXSetError(error, @"contract store cannot be inspected");
    return NO;
  }
  NSError *readError = nil;
  NSString *value = [NSString stringWithContentsOfFile:path
                                              encoding:NSUTF8StringEncoding
                                                 error:&readError];
  if (value == nil) {
    RLXSetError(error, @"contract store cannot be read");
    return NO;
  }
  value = [value stringByTrimmingCharactersInSet:
                     NSCharacterSet.whitespaceAndNewlineCharacterSet];
  if (!RLXParseState(value, state))
    *state = RLXPrivacyModeStateInvalid;
  return YES;
}

static BOOL RLXWritePrivacyState(RLXPrivacyModeState state,
                                 NSString **error) {
  if (getenv("RADISHLEX_PRIVACY_CONTRACT_FAIL_WRITE") != NULL) {
    RLXSetError(error, @"contract write failed");
    return NO;
  }
  NSString *path = RLXContractStorePath(error);
  if (path == nil)
    return NO;

  if (state == RLXPrivacyModeStateAbsent) {
    if (unlink(path.fileSystemRepresentation) != 0 && errno != ENOENT) {
      RLXSetError(error, @"contract store cannot be removed");
      return NO;
    }
  } else {
    NSString *value = [RLXStateName(state) stringByAppendingString:@"\n"];
    NSError *writeError = nil;
    if (![value writeToFile:path
                 atomically:YES
                   encoding:NSUTF8StringEncoding
                      error:&writeError]) {
      RLXSetError(error, @"contract store cannot be written");
      return NO;
    }
  }
  RLXContractDidWrite = YES;
  return YES;
}
#else
static BOOL RLXReadPrivacyState(RLXPrivacyModeState *state,
                                NSString **error) {
  CFPropertyListRef value = CFPreferencesCopyValue(
      (__bridge CFStringRef)RLXPrivacyKey,
      (__bridge CFStringRef)RLXPrivacyDomain, kCFPreferencesCurrentUser,
      kCFPreferencesAnyHost);
  if (value == NULL) {
    *state = RLXPrivacyModeStateAbsent;
    return YES;
  }
  if (CFGetTypeID(value) != CFBooleanGetTypeID()) {
    CFRelease(value);
    *state = RLXPrivacyModeStateInvalid;
    return YES;
  }
  *state = CFBooleanGetValue((CFBooleanRef)value)
               ? RLXPrivacyModeStateEnabled
               : RLXPrivacyModeStateDisabled;
  CFRelease(value);
  (void)error;
  return YES;
}

static BOOL RLXWritePrivacyState(RLXPrivacyModeState state,
                                 NSString **error) {
  CFPropertyListRef value = NULL;
  if (state == RLXPrivacyModeStateEnabled)
    value = kCFBooleanTrue;
  else if (state == RLXPrivacyModeStateDisabled)
    value = kCFBooleanFalse;
  else if (state != RLXPrivacyModeStateAbsent) {
    RLXSetError(error, @"invalid privacy state cannot be written");
    return NO;
  }

  CFPreferencesSetValue((__bridge CFStringRef)RLXPrivacyKey, value,
                        (__bridge CFStringRef)RLXPrivacyDomain,
                        kCFPreferencesCurrentUser, kCFPreferencesAnyHost);
  if (!CFPreferencesSynchronize((__bridge CFStringRef)RLXPrivacyDomain,
                                kCFPreferencesCurrentUser,
                                kCFPreferencesAnyHost)) {
    RLXSetError(error, @"privacy preferences could not be synchronized");
    return NO;
  }
  return YES;
}
#endif

static int RLXPrintStatus(void) {
  RLXPrivacyModeState state;
  NSString *error = nil;
  if (!RLXReadPrivacyState(&state, &error)) {
    fprintf(stderr, "%s\n", error.UTF8String);
    return 5;
  }
  printf("privacy_mode=%s\n", RLXStateName(state).UTF8String);
  return state == RLXPrivacyModeStateInvalid ? 3 : 0;
}

static int RLXCaptureBaseline(void) {
  RLXPrivacyModeState state;
  NSString *error = nil;
  if (!RLXReadPrivacyState(&state, &error)) {
    fprintf(stderr, "%s\n", error.UTF8String);
    return 5;
  }
  if (state == RLXPrivacyModeStateInvalid) {
    fprintf(stderr, "privacy mode is not stored as a boolean\n");
    return 3;
  }
  if (state == RLXPrivacyModeStateEnabled) {
    fprintf(stderr,
            "privacy mode is already enabled; refusing a normal-mode baseline\n");
    return 3;
  }
  if (!RLXCreateReceipt(state, &error)) {
    fprintf(stderr, "%s\n", error.UTF8String);
    return 4;
  }
  printf("privacy_baseline=%s\n", RLXStateName(state).UTF8String);
  return 0;
}

static int RLXEnableFromBaseline(void) {
  RLXPrivacyReceipt receipt;
  RLXPrivacyModeState current;
  NSString *error = nil;
  if (!RLXOpenReceipt(&receipt, &error)) {
    fprintf(stderr, "%s\n", error.UTF8String);
    return 4;
  }
  if (receipt.state == RLXPrivacyModeStateEnabled ||
      !RLXReadPrivacyState(&current, &error) || current != receipt.state) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr, "privacy state drifted from the captured baseline\n");
    return 5;
  }
#if RADISHLEX_PRIVACY_CONTRACT
  RLXContractReplaceReceiptIfRequested(
      "RADISHLEX_PRIVACY_CONTRACT_REPLACE_BEFORE_WRITE", &receipt);
#endif
  if (!RLXRevalidateReceipt(&receipt, &error) ||
      !RLXWritePrivacyState(RLXPrivacyModeStateEnabled, &error) ||
      !RLXReadPrivacyState(&current, &error) ||
      current != RLXPrivacyModeStateEnabled) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr, "%s\n",
            (error ?: @"privacy mode enable verification failed").UTF8String);
    return 5;
  }
  printf("privacy_mode=true\nprivacy_baseline=%s\n",
         RLXStateName(receipt.state).UTF8String);
  RLXCloseReceipt(&receipt);
  return 0;
}

static int RLXRestoreBaseline(void) {
  RLXPrivacyReceipt receipt;
  RLXPrivacyModeState current;
  NSString *error = nil;
  if (!RLXOpenReceipt(&receipt, &error)) {
    fprintf(stderr, "%s\n", error.UTF8String);
    return 4;
  }
  if (!RLXReadPrivacyState(&current, &error) ||
      current != RLXPrivacyModeStateEnabled) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr,
            "privacy mode is not enabled; refusing to mask state drift\n");
    return 5;
  }
  if (!RLXRevalidateReceipt(&receipt, &error)) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr, "%s\n", error.UTF8String);
    return 5;
  }
  if (!RLXWritePrivacyState(receipt.state, &error) ||
      !RLXReadPrivacyState(&current, &error) || current != receipt.state) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr, "%s\n",
            (error ?: @"privacy baseline restore verification failed")
                .UTF8String);
    return 5;
  }
#if RADISHLEX_PRIVACY_CONTRACT
  RLXContractReplaceReceiptIfRequested(
      "RADISHLEX_PRIVACY_CONTRACT_REPLACE_BEFORE_UNLINK", &receipt);
#endif
  if (!RLXRevalidateReceipt(&receipt, &error)) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr,
            "privacy baseline was restored but receipt identity drifted\n");
    return 4;
  }
  if (fsync(receipt.directoryDescriptor) != 0) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr,
            "privacy baseline was restored but receipt directory cannot be "
            "synced; receipt retained for retry\n");
    return 4;
  }
  if (unlinkat(receipt.directoryDescriptor, RLXReceiptName, 0) != 0) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr,
            "privacy baseline was restored but snapshot removal failed\n");
    return 4;
  }
  if (fsync(receipt.directoryDescriptor) != 0) {
    RLXCloseReceipt(&receipt);
    fprintf(stderr,
            "privacy baseline was restored and receipt deletion is visible, "
            "but deletion durability could not be confirmed; stop and "
            "inspect the fixed state directory\n");
    return 4;
  }
  printf("privacy_mode=%s\nprivacy_baseline=restored\n",
         RLXStateName(receipt.state).UTF8String);
  RLXCloseReceipt(&receipt);
  return 0;
}

static void RLXPrintUsage(const char *program) {
  fprintf(stderr,
          "usage: %s --status|--capture-baseline|--authorized-enable|"
          "--authorized-restore\n",
          program);
}

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    if (argc != 2) {
      RLXPrintUsage(argv[0]);
      return 2;
    }
    if (strcmp(argv[1], "--status") == 0)
      return RLXPrintStatus();
    if (strcmp(argv[1], "--capture-baseline") == 0)
      return RLXCaptureBaseline();
    if (strcmp(argv[1], "--authorized-enable") == 0)
      return RLXEnableFromBaseline();
    if (strcmp(argv[1], "--authorized-restore") == 0)
      return RLXRestoreBaseline();
    RLXPrintUsage(argv[0]);
    return 2;
  }
}
