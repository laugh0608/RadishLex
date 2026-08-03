#include "radishlex/linux/manager_runtime.h"

#include <cerrno>
#include <climits>
#include <cstring>
#include <sys/stat.h>
#include <unistd.h>

#include <array>
#include <filesystem>
#include <system_error>

#include "radishlex/linux/privacy_mode.h"

namespace radishlex::linux_platform {
namespace {

constexpr const char *kManagerNativeLibrary = "libradishlex_ime_ffi.so";

std::filesystem::path canonicalExecutable(
    const std::filesystem::path &executable_path) {
  if (executable_path.empty() || !executable_path.is_absolute()) {
    throw ManagerRuntimeException(
        ManagerRuntimeError::ExecutableLookupFailed,
        "manager executable path must be absolute");
  }
  std::error_code error;
  const std::filesystem::path canonical =
      std::filesystem::canonical(executable_path, error);
  if (error || canonical.filename() != "radishlex_manager") {
    throw ManagerRuntimeException(
        ManagerRuntimeError::ExecutableLookupFailed,
        "manager executable identity is invalid");
  }
  error.clear();
  if (!std::filesystem::is_regular_file(canonical, error) || error) {
    throw ManagerRuntimeException(
        ManagerRuntimeError::ExecutableLookupFailed,
        "manager executable identity is invalid");
  }
  return canonical;
}

void validateNativeLibrary(const std::filesystem::path &path) {
  struct stat status {};
  if (lstat(path.c_str(), &status) != 0) {
    if (errno == ENOENT) {
      throw ManagerRuntimeException(ManagerRuntimeError::NativeLibraryMissing,
                                    "manager native library is missing");
    }
    throw ManagerRuntimeException(ManagerRuntimeError::UnsafeBundle,
                                  "manager native library is unavailable");
  }
  if (!S_ISREG(status.st_mode) || S_ISLNK(status.st_mode) ||
      (status.st_mode & 0022) != 0) {
    throw ManagerRuntimeException(ManagerRuntimeError::UnsafeBundle,
                                  "manager native library identity is unsafe");
  }
}

ManagerRuntimePaths resolve(const std::filesystem::path &executable_path,
                            const XdgPaths &xdg_paths) {
  const std::filesystem::path executable =
      canonicalExecutable(executable_path);
  const std::filesystem::path bundle_root = executable.parent_path();
  const std::filesystem::path native_library =
      bundle_root / "lib" / kManagerNativeLibrary;
  if (native_library.parent_path().parent_path() != bundle_root) {
    throw ManagerRuntimeException(ManagerRuntimeError::UnsafeBundle,
                                  "manager bundle layout is inconsistent");
  }
  validateNativeLibrary(native_library);
  secureManagerLocalFiles(xdg_paths);
  return ManagerRuntimePaths{xdg_paths, native_library};
}

#if defined(__linux__)
std::filesystem::path currentExecutable() {
  std::array<char, PATH_MAX + 1> buffer{};
  const ssize_t length =
      readlink("/proc/self/exe", buffer.data(), buffer.size() - 1);
  if (length <= 0 || static_cast<std::size_t>(length) >= buffer.size() - 1) {
    throw ManagerRuntimeException(
        ManagerRuntimeError::ExecutableLookupFailed,
        "manager executable could not be resolved");
  }
  buffer[static_cast<std::size_t>(length)] = '\0';
  return std::filesystem::path(buffer.data());
}
#endif

}  // namespace

ManagerRuntimeException::ManagerRuntimeException(ManagerRuntimeError code,
                                                 const std::string &message)
    : std::runtime_error(message), code_(code) {}

ManagerRuntimeError ManagerRuntimeException::code() const noexcept {
  return code_;
}

ManagerRuntimePaths resolveManagerRuntimePaths() {
#if defined(__linux__)
  return resolve(currentExecutable(), resolveProductionXdgPaths());
#else
  throw ManagerRuntimeException(
      ManagerRuntimeError::ExecutableLookupFailed,
      "Linux manager runtime resolution is unavailable on this platform");
#endif
}

void secureManagerLocalFiles(const XdgPaths &paths) {
  preparePrivateProductPaths(paths);
  validatePrivateRegularFileIfPresent(paths.settings_path, paths.owner_id);
  static_cast<void>(readPrivacyMode(paths));
}

#if defined(RADISHLEX_MANAGER_RUNTIME_TESTING)
ManagerRuntimePaths resolveManagerRuntimePathsForTesting(
    const std::filesystem::path &executable_path, const XdgPaths &xdg_paths) {
  return resolve(executable_path, xdg_paths);
}
#endif

}  // namespace radishlex::linux_platform
