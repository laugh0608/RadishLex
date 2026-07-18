#define _DARWIN_C_SOURCE

#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <pwd.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#ifndef RLX_TEST_DATA_STATE_DIR
#error "RLX_TEST_DATA_STATE_DIR must be a fixed build-time directory"
#endif

#ifndef O_CLOEXEC
#define O_CLOEXEC 0
#endif

#ifndef RLX_TEST_DATA_CONTRACT
#define RLX_TEST_DATA_CONTRACT 0
#endif

#ifndef RLX_TEST_DATA_PROFILE_MANAGER
#define RLX_TEST_DATA_PROFILE_MANAGER 0
#endif

#if RLX_TEST_DATA_PROFILE_MANAGER
#define RLX_RECEIPT_HEADER "radishlex-m2-manager-test-data-baseline-v1"
#define RLX_TEST_DATA_ENTRY_COUNT 6
static const char *const kReceiptName =
    "m2-manager-test-data-baseline.receipt";
static const char *const kAuthorizedDeleteAction =
    "--authorized-delete-m2-manager-test-data";
#else
#define RLX_RECEIPT_HEADER "radishlex-r01b-test-userdb-baseline-v1"
#define RLX_TEST_DATA_ENTRY_COUNT 4
static const char *const kReceiptName =
    "r01b-test-userdb-baseline.receipt";
static const char *const kAuthorizedDeleteAction =
    "--authorized-delete-r01b-test-userdb";
#endif

static const char *const kReceiptHeader = RLX_RECEIPT_HEADER;
static const char *const kTestDataNames[] = {
    "userdb.sqlite3",
    "userdb.sqlite3-wal",
    "userdb.sqlite3-shm",
    "userdb.sqlite3-journal",
#if RLX_TEST_DATA_PROFILE_MANAGER
    "manager-settings.json",
    "manager-settings.json.tmp",
#endif
};
static const size_t kTestDataNameCount =
    sizeof(kTestDataNames) / sizeof(kTestDataNames[0]);
_Static_assert(RLX_TEST_DATA_ENTRY_COUNT ==
                   sizeof(kTestDataNames) / sizeof(kTestDataNames[0]),
               "test data entry count must match the fixed allowlist");

typedef struct {
  dev_t device;
  ino_t inode;
  uid_t owner;
  mode_t mode;
  dev_t receipt_device;
  ino_t receipt_inode;
} RLXParentBaseline;

typedef struct {
  bool present;
  dev_t device;
  ino_t inode;
} RLXTestDataEntry;

#if RLX_TEST_DATA_CONTRACT
static bool RLXContractFailureIs(const char *phase) {
  const char *requested =
      getenv("RADISHLEX_TEST_DATA_CONTRACT_FAIL_PHASE");
  return requested != NULL && strcmp(requested, phase) == 0;
}
#endif

static int RLXFail(const char *message) {
  fprintf(stderr, "%s\n", message);
  return 1;
}

static bool RLXIsExactMode(mode_t actual, mode_t expected) {
  return (actual & 07777) == expected;
}

static bool RLXValidateOwnedDirectory(int descriptor, mode_t expected_mode,
                                      struct stat *metadata) {
  if (fstat(descriptor, metadata) != 0 ||
      !S_ISDIR(metadata->st_mode) || metadata->st_uid != getuid() ||
      !RLXIsExactMode(metadata->st_mode, expected_mode)) {
    return false;
  }
  return true;
}

static int RLXOpenDirectoryAt(int parent, const char *name) {
  return openat(parent, name,
                O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
}

static int RLXOpenFixedParent(void) {
  const char *home = getenv("HOME");
  if (home == NULL || home[0] != '/') {
    errno = EINVAL;
    return -1;
  }
#if !RLX_TEST_DATA_CONTRACT
  struct passwd *account = getpwuid(getuid());
  if (account == NULL || account->pw_dir == NULL ||
      strcmp(home, account->pw_dir) != 0) {
    errno = EPERM;
    return -1;
  }
#endif

  int home_fd = open(home, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (home_fd < 0)
    return -1;
  int library_fd = RLXOpenDirectoryAt(home_fd, "Library");
  close(home_fd);
  if (library_fd < 0)
    return -1;
  int support_fd = RLXOpenDirectoryAt(library_fd, "Application Support");
  close(library_fd);
  if (support_fd < 0)
    return -1;
  int parent_fd = RLXOpenDirectoryAt(support_fd, "RadishLex");
  close(support_fd);
  return parent_fd;
}

static int RLXOpenStateDirectory(void) {
  return open(RLX_TEST_DATA_STATE_DIR,
              O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
}

static bool RLXValidateStateDirectory(int descriptor) {
  struct stat metadata;
  return fstat(descriptor, &metadata) == 0 &&
         S_ISDIR(metadata.st_mode) && metadata.st_uid == getuid() &&
         (metadata.st_mode & 0022) == 0;
}

static bool RLXReceiptMetadataIsSafe(const struct stat *metadata) {
  return S_ISREG(metadata->st_mode) && metadata->st_uid == getuid() &&
         metadata->st_nlink == 1 &&
         RLXIsExactMode(metadata->st_mode, 0600) && metadata->st_size > 0 &&
         metadata->st_size < 384;
}

static bool RLXDirectoryIsEmpty(int descriptor) {
  int iteration_fd = RLXOpenDirectoryAt(descriptor, ".");
  if (iteration_fd < 0)
    return false;
  DIR *directory = fdopendir(iteration_fd);
  if (directory == NULL) {
    close(iteration_fd);
    return false;
  }

  bool empty = true;
  errno = 0;
  for (struct dirent *entry = readdir(directory); entry != NULL;
       entry = readdir(directory)) {
    if (strcmp(entry->d_name, ".") != 0 &&
        strcmp(entry->d_name, "..") != 0) {
      empty = false;
      break;
    }
    errno = 0;
  }
  if (errno != 0)
    empty = false;
  if (closedir(directory) != 0)
    empty = false;
  return empty;
}

static bool RLXWriteAll(int descriptor, const char *bytes, size_t length) {
  while (length > 0) {
    ssize_t written = write(descriptor, bytes, length);
    if (written < 0) {
      if (errno == EINTR)
        continue;
      return false;
    }
    bytes += written;
    length -= (size_t)written;
  }
  return true;
}

static int RLXFormatReceipt(char *buffer, size_t capacity,
                            const RLXParentBaseline *baseline) {
  return snprintf(buffer, capacity,
                  "%s\nparent_dev=%" PRIuMAX "\nparent_ino=%" PRIuMAX
                  "\nparent_uid=%" PRIuMAX "\nparent_mode=%04o"
                  "\nreceipt_dev=%" PRIuMAX "\nreceipt_ino=%" PRIuMAX
                  "\n",
                  kReceiptHeader, (uintmax_t)baseline->device,
                  (uintmax_t)baseline->inode, (uintmax_t)baseline->owner,
                  (unsigned int)baseline->mode,
                  (uintmax_t)baseline->receipt_device,
                  (uintmax_t)baseline->receipt_inode);
}

static bool RLXCreateReceipt(int state_fd,
                             const RLXParentBaseline *baseline) {
  int receipt_fd = openat(state_fd, kReceiptName,
                          O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW |
                              O_CLOEXEC,
                          0600);
  if (receipt_fd < 0)
    return false;

  struct stat metadata = {0};
  bool valid = fstat(receipt_fd, &metadata) == 0 &&
               S_ISREG(metadata.st_mode) && metadata.st_uid == getuid() &&
               metadata.st_nlink == 1 &&
               RLXIsExactMode(metadata.st_mode, 0600);
  RLXParentBaseline receipt_baseline = *baseline;
  receipt_baseline.receipt_device = metadata.st_dev;
  receipt_baseline.receipt_inode = metadata.st_ino;
  char contents[384];
  int length = valid ? RLXFormatReceipt(contents, sizeof(contents),
                                        &receipt_baseline)
                     : -1;
  bool wrote = valid && length > 0 && (size_t)length < sizeof(contents) &&
               RLXWriteAll(receipt_fd, contents, (size_t)length) &&
               fsync(receipt_fd) == 0;
  bool closed = close(receipt_fd) == 0;
  if (!wrote || !closed) {
    struct stat current;
    if (valid &&
        fstatat(state_fd, kReceiptName, &current, AT_SYMLINK_NOFOLLOW) == 0 &&
        current.st_dev == metadata.st_dev &&
        current.st_ino == metadata.st_ino) {
      unlinkat(state_fd, kReceiptName, 0);
    }
    return false;
  }
  return true;
}

static bool RLXParseReceipt(const char *contents, size_t length,
                            RLXParentBaseline *baseline) {
  if (length == 0 || length >= 384 || contents[length - 1] != '\n')
    return false;

  uintmax_t device = 0;
  uintmax_t inode = 0;
  uintmax_t owner = 0;
  uintmax_t receipt_device = 0;
  uintmax_t receipt_inode = 0;
  unsigned int mode = 0;
  int consumed = 0;
  int matched = sscanf(contents,
                       RLX_RECEIPT_HEADER "\n"
                       "parent_dev=%" SCNuMAX "\n"
                       "parent_ino=%" SCNuMAX "\n"
                       "parent_uid=%" SCNuMAX "\n"
                       "parent_mode=%o\n"
                       "receipt_dev=%" SCNuMAX "\n"
                       "receipt_ino=%" SCNuMAX "\n%n",
                       &device, &inode, &owner, &mode, &receipt_device,
                       &receipt_inode, &consumed);
  if (matched != 6 || consumed <= 0 || (size_t)consumed != length ||
      mode != 0755 || owner != (uintmax_t)getuid()) {
    return false;
  }

  baseline->device = (dev_t)device;
  baseline->inode = (ino_t)inode;
  baseline->owner = (uid_t)owner;
  baseline->mode = (mode_t)mode;
  baseline->receipt_device = (dev_t)receipt_device;
  baseline->receipt_inode = (ino_t)receipt_inode;

  char canonical[384];
  int canonical_length =
      RLXFormatReceipt(canonical, sizeof(canonical), baseline);
  return canonical_length == consumed &&
         memcmp(canonical, contents, length) == 0;
}

static bool RLXReadReceipt(int state_fd, RLXParentBaseline *baseline) {
  struct stat entry_metadata;
  if (fstatat(state_fd, kReceiptName, &entry_metadata,
              AT_SYMLINK_NOFOLLOW) != 0 ||
      !RLXReceiptMetadataIsSafe(&entry_metadata)) {
    return false;
  }

  int receipt_fd =
      openat(state_fd, kReceiptName, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
  if (receipt_fd < 0)
    return false;
  struct stat opened_metadata;
  if (fstat(receipt_fd, &opened_metadata) != 0 ||
      !RLXReceiptMetadataIsSafe(&opened_metadata) ||
      opened_metadata.st_dev != entry_metadata.st_dev ||
      opened_metadata.st_ino != entry_metadata.st_ino ||
      opened_metadata.st_size != entry_metadata.st_size) {
    close(receipt_fd);
    return false;
  }

  char contents[384] = {0};
  size_t total = 0;
  bool valid = true;
  while (total < sizeof(contents) - 1) {
    ssize_t count = read(receipt_fd, contents + total,
                         sizeof(contents) - 1 - total);
    if (count < 0) {
      if (errno == EINTR)
        continue;
      valid = false;
      break;
    }
    if (count == 0)
      break;
    total += (size_t)count;
  }
  if (total != (size_t)entry_metadata.st_size)
    valid = false;
  if (close(receipt_fd) != 0)
    valid = false;
  if (!valid || !RLXParseReceipt(contents, total, baseline))
    return false;
  return baseline->receipt_device == opened_metadata.st_dev &&
         baseline->receipt_inode == opened_metadata.st_ino;
}

static bool RLXReceiptStillMatches(int state_fd,
                                   const RLXParentBaseline *baseline) {
  struct stat metadata;
  return fstatat(state_fd, kReceiptName, &metadata,
                 AT_SYMLINK_NOFOLLOW) == 0 &&
         RLXReceiptMetadataIsSafe(&metadata) &&
         metadata.st_dev == baseline->receipt_device &&
         metadata.st_ino == baseline->receipt_inode;
}

static ssize_t RLXTestDataNameIndex(const char *name) {
  for (size_t index = 0; index < kTestDataNameCount; index++) {
    if (strcmp(name, kTestDataNames[index]) == 0)
      return (ssize_t)index;
  }
  return -1;
}

static bool RLXInspectTestDataEntries(int parent_fd,
                                      RLXTestDataEntry *entries) {
  memset(entries, 0, sizeof(*entries) * kTestDataNameCount);
  int iteration_fd = RLXOpenDirectoryAt(parent_fd, ".");
  if (iteration_fd < 0)
    return false;
  DIR *directory = fdopendir(iteration_fd);
  if (directory == NULL) {
    close(iteration_fd);
    return false;
  }

  bool valid = true;
  errno = 0;
  for (struct dirent *entry = readdir(directory); entry != NULL;
       entry = readdir(directory)) {
    if (strcmp(entry->d_name, ".") == 0 ||
        strcmp(entry->d_name, "..") == 0) {
      errno = 0;
      continue;
    }
    ssize_t index = RLXTestDataNameIndex(entry->d_name);
    if (index < 0 || entries[index].present) {
      valid = false;
      break;
    }

    struct stat metadata;
    if (fstatat(parent_fd, entry->d_name, &metadata,
                AT_SYMLINK_NOFOLLOW) != 0 ||
        !S_ISREG(metadata.st_mode) || metadata.st_uid != getuid() ||
        metadata.st_nlink != 1 || !RLXIsExactMode(metadata.st_mode, 0600)) {
      valid = false;
      break;
    }
    entries[index].present = true;
    entries[index].device = metadata.st_dev;
    entries[index].inode = metadata.st_ino;
    errno = 0;
  }
  if (errno != 0)
    valid = false;
  if (closedir(directory) != 0)
    valid = false;
  return valid;
}

static bool RLXAnyTestDataEntryIsPresent(const RLXTestDataEntry *entries) {
  for (size_t index = 0; index < kTestDataNameCount; index++) {
    if (entries[index].present)
      return true;
  }
  return false;
}

static bool RLXTestDataEntryStillMatches(int parent_fd, size_t index,
                                         const RLXTestDataEntry *entry) {
  struct stat metadata;
  return fstatat(parent_fd, kTestDataNames[index], &metadata,
                 AT_SYMLINK_NOFOLLOW) == 0 &&
         S_ISREG(metadata.st_mode) && metadata.st_uid == getuid() &&
         metadata.st_nlink == 1 && RLXIsExactMode(metadata.st_mode, 0600) &&
         metadata.st_dev == entry->device && metadata.st_ino == entry->inode;
}

static int RLXCaptureBaseline(void) {
  int parent_fd = RLXOpenFixedParent();
  if (parent_fd < 0)
    return RLXFail(
        "Test-data parent cannot be opened through fixed ordinary directories");

  struct stat parent_metadata;
  if (!RLXValidateOwnedDirectory(parent_fd, 0755, &parent_metadata) ||
      !RLXDirectoryIsEmpty(parent_fd)) {
    close(parent_fd);
    return RLXFail(
        "Test-data baseline requires an owned empty parent with mode 0755");
  }

  int state_fd = RLXOpenStateDirectory();
  if (state_fd < 0) {
    close(parent_fd);
    return RLXFail(
        "Test-data baseline state directory is unavailable or unsafe");
  }
  if (!RLXValidateStateDirectory(state_fd)) {
    close(state_fd);
    close(parent_fd);
    return RLXFail(
        "Test-data baseline state directory permissions are unsafe");
  }

  RLXParentBaseline baseline = {
      .device = parent_metadata.st_dev,
      .inode = parent_metadata.st_ino,
      .owner = parent_metadata.st_uid,
      .mode = 0755,
      .receipt_device = 0,
      .receipt_inode = 0,
  };
  bool created = RLXCreateReceipt(state_fd, &baseline);
  close(state_fd);
  close(parent_fd);
  if (!created)
    return RLXFail(
        "Test-data baseline receipt already exists or cannot be created safely");

#if RLX_TEST_DATA_PROFILE_MANAGER
  printf("m2_manager_test_data_baseline=captured\n");
#else
  printf("r01b_userdb_baseline=captured\n");
#endif
  return 0;
}

static bool RLXDeleteTestDataEntry(int parent_fd, size_t index,
                                   const RLXTestDataEntry *entry) {
  if (!entry->present)
    return true;
  if (!RLXTestDataEntryStillMatches(parent_fd, index, entry))
    return false;
#if RLX_TEST_DATA_CONTRACT
  if ((index == 2 && RLXContractFailureIs("unlink-sidecar")) ||
      (index == 0 && RLXContractFailureIs("unlink-main")) ||
      (index == 4 && RLXContractFailureIs("unlink-settings"))) {
    errno = EIO;
    return false;
  }
#endif
  return unlinkat(parent_fd, kTestDataNames[index], 0) == 0;
}

static int RLXDeleteTestData(void) {
  int state_fd = RLXOpenStateDirectory();
  if (state_fd < 0)
    return RLXFail(
        "Test-data baseline state directory is unavailable or unsafe");
  if (!RLXValidateStateDirectory(state_fd)) {
    close(state_fd);
    return RLXFail(
        "Test-data baseline state directory permissions drifted");
  }
  RLXParentBaseline baseline;
  if (!RLXReadReceipt(state_fd, &baseline)) {
    close(state_fd);
    return RLXFail(
        "Test-data baseline receipt is missing, unsafe or malformed");
  }

  int parent_fd = RLXOpenFixedParent();
  if (parent_fd < 0) {
    close(state_fd);
    return RLXFail(
        "Test-data parent cannot be opened through fixed ordinary directories");
  }
  struct stat parent_metadata;
  if (fstat(parent_fd, &parent_metadata) != 0 ||
      !S_ISDIR(parent_metadata.st_mode) ||
      parent_metadata.st_uid != getuid() ||
      parent_metadata.st_dev != baseline.device ||
      parent_metadata.st_ino != baseline.inode ||
      parent_metadata.st_uid != baseline.owner || baseline.mode != 0755) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data parent identity or permissions drifted from the baseline");
  }

  RLXTestDataEntry entries[RLX_TEST_DATA_ENTRY_COUNT];
  if (!RLXInspectTestDataEntries(parent_fd, entries)) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data parent contains an unsafe or unknown fixed-path entry");
  }

  mode_t parent_mode = parent_metadata.st_mode & 07777;
  bool any_entry_present = RLXAnyTestDataEntryIsPresent(entries);
  bool database_sidecar_present =
      entries[1].present || entries[2].present || entries[3].present;
  if (parent_mode == 0700 && !entries[0].present &&
      database_sidecar_present) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data recovery refuses database sidecars without the main userdb");
  }
  if (parent_mode == 0700) {
    for (size_t index = 1; index < kTestDataNameCount; index++) {
      if (!RLXDeleteTestDataEntry(parent_fd, index, &entries[index])) {
        close(parent_fd);
        close(state_fd);
        return RLXFail(
            "Test-data entry identity changed or unlink failed during exact deletion");
      }
    }
    if (!RLXDeleteTestDataEntry(parent_fd, 0, &entries[0])) {
      close(parent_fd);
      close(state_fd);
      return RLXFail(
          "Test-data main userdb identity changed or unlink failed during exact deletion");
    }
  } else if (parent_mode == 0755 && any_entry_present) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data restored parent must be empty before receipt removal");
  } else if (parent_mode != 0700 && parent_mode != 0755) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data parent mode is outside normal and recovery states");
  }

  if (!RLXDirectoryIsEmpty(parent_fd)) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data parent is not empty after exact fixed-path deletion");
  }
  if (parent_mode == 0700) {
#if RLX_TEST_DATA_CONTRACT
    if (RLXContractFailureIs("fchmod")) {
      close(parent_fd);
      close(state_fd);
      return RLXFail(
          "Test-data contract injected a parent mode restoration failure");
    }
#endif
    if (fchmod(parent_fd, baseline.mode) != 0) {
      close(parent_fd);
      close(state_fd);
      return RLXFail(
          "Test-data parent mode could not be restored to the baseline");
    }
  }
  if (fsync(parent_fd) != 0 ||
      !RLXValidateOwnedDirectory(parent_fd, 0755, &parent_metadata) ||
      !RLXDirectoryIsEmpty(parent_fd)) {
    close(parent_fd);
    close(state_fd);
    return RLXFail(
        "Test-data restored parent could not be verified and synchronized");
  }
  close(parent_fd);

  if (!RLXValidateStateDirectory(state_fd) ||
      !RLXReceiptStillMatches(state_fd, &baseline)) {
    close(state_fd);
    return RLXFail(
        "Test data was deleted but baseline state safety drifted");
  }
#if RLX_TEST_DATA_CONTRACT
  if (RLXContractFailureIs("receipt-unlink")) {
    close(state_fd);
    return RLXFail(
        "Test-data contract injected a baseline receipt unlink failure");
  }
#endif
  if (unlinkat(state_fd, kReceiptName, 0) != 0) {
    close(state_fd);
    return RLXFail(
        "Test data was deleted but baseline receipt removal failed");
  }

  bool state_synced = false;
#if RLX_TEST_DATA_CONTRACT
  if (RLXContractFailureIs("receipt-fsync")) {
    errno = EIO;
  } else
#endif
  {
    state_synced = fsync(state_fd) == 0;
  }
  if (!state_synced) {
    if (RLXCreateReceipt(state_fd, &baseline)) {
      (void)fsync(state_fd);
      close(state_fd);
      return RLXFail(
          "Test-data receipt removal was not durable; a retry receipt was restored");
    }

    struct stat receipt_metadata;
    if (fstatat(state_fd, kReceiptName, &receipt_metadata,
                AT_SYMLINK_NOFOLLOW) == 0 || errno != ENOENT) {
      close(state_fd);
      return RLXFail(
          "Test-data receipt durability failed and recovery state is unsafe");
    }
    fprintf(stderr,
            "Test-data warning: receipt directory synchronization failed after "
            "terminal removal\n");
  }
  close(state_fd);
#if RLX_TEST_DATA_PROFILE_MANAGER
  printf("m2_manager_test_data=deleted\n"
         "application_support_parent=empty\n"
         "application_support_parent_mode=0755\n"
         "m2_manager_test_data_baseline=removed\n");
#else
  printf("r01b_test_userdb=deleted\n"
         "application_support_parent=empty\n"
         "application_support_parent_mode=0755\n"
         "r01b_userdb_baseline=removed\n");
#endif
  return 0;
}

static void RLXPrintUsage(const char *program) {
  fprintf(stderr, "usage: %s --capture-baseline | %s\n", program,
          kAuthorizedDeleteAction);
}

int main(int argc, const char *argv[]) {
  if (argc != 2) {
    RLXPrintUsage(argv[0]);
    return 2;
  }
  if (strcmp(argv[1], "--capture-baseline") == 0)
    return RLXCaptureBaseline();
  if (strcmp(argv[1], kAuthorizedDeleteAction) == 0)
    return RLXDeleteTestData();
  RLXPrintUsage(argv[0]);
  return 2;
}
