#include "radishlex/linux/runtime_layout.h"

#include <dlfcn.h>
#include <sys/stat.h>

#include <array>
#include <cerrno>
#include <cstring>

namespace radishlex::linux_platform {
namespace {

constexpr const char *kAddonLibrary = "radishlex.so";
constexpr const char *kFfiLibrary = "libradishlex_ime_ffi.so";
#ifdef RADISHLEX_SYSTEM_RIME_DATA_DIR
constexpr const char *kSystemRimeDataDirectory =
    RADISHLEX_SYSTEM_RIME_DATA_DIR;
#else
constexpr const char *kRimeDataDirectory = "radishlex-rime";
#endif
constexpr std::array<const char *, 3> kRimeAssets{
    "default.yaml",
    "radishlex_pinyin.schema.yaml",
    "pinyin_simp.dict.yaml",
};
const int kRuntimeLayoutAnchor = 0;

struct FileStatus {
  struct stat value;
  bool exists;
};

void validateAbsoluteNormalized(const std::filesystem::path &path,
                                const char *field) {
  if (path.empty() || !path.is_absolute() ||
      path.lexically_normal() != path) {
    throw RuntimeLayoutException(
        RuntimeLayoutError::InvalidAddonPath,
        std::string(field) + " must be an absolute normalized path");
  }
}

FileStatus noFollowStatus(const std::filesystem::path &path) {
  struct stat value {};
  if (lstat(path.c_str(), &value) == 0) {
    return FileStatus{value, true};
  }
  if (errno == ENOENT) {
    return FileStatus{value, false};
  }
  throw RuntimeLayoutException(
      RuntimeLayoutError::UnsafeResource,
      "unable to inspect an installed runtime resource");
}

void requireSafeMode(const struct stat &status) {
  if ((status.st_mode & (S_IWGRP | S_IWOTH)) != 0) {
    throw RuntimeLayoutException(
        RuntimeLayoutError::UnsafePermissions,
        "installed runtime resource is writable by group or other");
  }
}

void requireRegularFile(const std::filesystem::path &path) {
  const FileStatus status = noFollowStatus(path);
  if (!status.exists) {
    throw RuntimeLayoutException(RuntimeLayoutError::MissingResource,
                                 "installed runtime file is missing");
  }
  if (S_ISLNK(status.value.st_mode) || !S_ISREG(status.value.st_mode)) {
    throw RuntimeLayoutException(RuntimeLayoutError::UnsafeResource,
                                 "installed runtime file is not regular");
  }
  requireSafeMode(status.value);
}

void requireDirectory(const std::filesystem::path &path) {
  const FileStatus status = noFollowStatus(path);
  if (!status.exists) {
    throw RuntimeLayoutException(RuntimeLayoutError::MissingResource,
                                 "installed runtime directory is missing");
  }
  if (S_ISLNK(status.value.st_mode) || !S_ISDIR(status.value.st_mode)) {
    throw RuntimeLayoutException(RuntimeLayoutError::UnsafeResource,
                                 "installed runtime directory is unsafe");
  }
  requireSafeMode(status.value);
}

}  // namespace

RuntimeLayoutException::RuntimeLayoutException(RuntimeLayoutError code,
                                               const std::string &message)
    : std::runtime_error(message), code_(code) {}

RuntimeLayoutError RuntimeLayoutException::code() const noexcept {
  return code_;
}

const char *runtimeLayoutErrorName(RuntimeLayoutError error) noexcept {
  switch (error) {
    case RuntimeLayoutError::InvalidAddonPath:
      return "invalid_addon_path";
    case RuntimeLayoutError::MissingResource:
      return "missing_resource";
    case RuntimeLayoutError::UnsafeResource:
      return "unsafe_resource";
    case RuntimeLayoutError::UnsafePermissions:
      return "unsafe_permissions";
    case RuntimeLayoutError::LoaderFailure:
      return "loader_failure";
  }
  return "unknown";
}

RuntimeLayout runtimeLayoutFromAddonLibrary(
    const std::filesystem::path &addon_library) {
  validateAbsoluteNormalized(addon_library, "addon library");
  if (addon_library.filename() != kAddonLibrary) {
    throw RuntimeLayoutException(RuntimeLayoutError::InvalidAddonPath,
                                 "addon library name is not fixed");
  }
  const std::filesystem::path addon_directory = addon_library.parent_path();
#ifdef RADISHLEX_SYSTEM_RIME_DATA_DIR
  const std::filesystem::path rime_data_directory =
      std::filesystem::path(kSystemRimeDataDirectory);
  validateAbsoluteNormalized(rime_data_directory, "RimeData directory");
#else
  const std::filesystem::path rime_data_directory =
      addon_directory / kRimeDataDirectory;
#endif
  return RuntimeLayout{
      addon_library,
      addon_directory / kFfiLibrary,
      rime_data_directory,
  };
}

RuntimeLayout resolveLoadedRuntimeLayout() {
  Dl_info info{};
  if (dladdr(static_cast<const void *>(&kRuntimeLayoutAnchor), &info) == 0 ||
      info.dli_fname == nullptr || info.dli_fname[0] == '\0') {
    throw RuntimeLayoutException(RuntimeLayoutError::LoaderFailure,
                                 "unable to resolve loaded addon library");
  }
  const RuntimeLayout layout =
      runtimeLayoutFromAddonLibrary(std::filesystem::path(info.dli_fname));
  validateRuntimeLayout(layout);
  return layout;
}

void validateRuntimeLayout(const RuntimeLayout &layout) {
  const RuntimeLayout expected =
      runtimeLayoutFromAddonLibrary(layout.addon_library);
  if (layout.ffi_library != expected.ffi_library ||
      layout.rime_shared_data_dir != expected.rime_shared_data_dir) {
    throw RuntimeLayoutException(RuntimeLayoutError::InvalidAddonPath,
                                 "runtime resources do not match the build profile");
  }
  requireDirectory(layout.addon_library.parent_path());
  requireRegularFile(layout.addon_library);
  requireRegularFile(layout.ffi_library);
  requireDirectory(layout.rime_shared_data_dir);
  for (const char *asset : kRimeAssets) {
    requireRegularFile(layout.rime_shared_data_dir / asset);
  }
}

}  // namespace radishlex::linux_platform
