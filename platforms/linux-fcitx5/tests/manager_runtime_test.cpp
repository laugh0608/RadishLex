#include "radishlex/linux/manager_runtime.h"
#include "radishlex/linux/privacy_mode.h"
#include "radishlex/linux/xdg_paths.h"

#include <sys/stat.h>
#include <unistd.h>

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

using radishlex::linux_platform::ManagerRuntimeError;
using radishlex::linux_platform::ManagerRuntimeException;
using radishlex::linux_platform::PrivacyModeError;
using radishlex::linux_platform::PrivacyModeException;
using radishlex::linux_platform::PrivacyModeRuntime;
using radishlex::linux_platform::PrivacyModeRuntimeStatus;
using radishlex::linux_platform::PrivacyModeState;
using radishlex::linux_platform::XdgPaths;
using radishlex::linux_platform::XdgTestEnvironment;

class TemporaryDirectory final {
 public:
  TemporaryDirectory() {
    const std::filesystem::path base =
        std::filesystem::canonical(std::filesystem::temp_directory_path());
    std::string pattern = (base / "radishlex-manager-runtime.XXXXXX").string();
    storage_.assign(pattern.begin(), pattern.end());
    storage_.push_back('\0');
    char *created = mkdtemp(storage_.data());
    if (created == nullptr) {
      throw std::runtime_error("mkdtemp failed");
    }
    path_ = created;
  }

  ~TemporaryDirectory() {
    std::error_code ignored;
    std::filesystem::remove_all(path_, ignored);
  }

  const std::filesystem::path &path() const { return path_; }

 private:
  std::vector<char> storage_;
  std::filesystem::path path_;
};

void require(bool condition, const char *message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}

mode_t permissions(const std::filesystem::path &path) {
  struct stat status {};
  require(lstat(path.c_str(), &status) == 0, "lstat must succeed");
  return status.st_mode & 0777;
}

void writeFile(const std::filesystem::path &path, const std::string &contents,
               mode_t mode = 0600) {
  std::filesystem::create_directories(path.parent_path());
  std::ofstream stream(path, std::ios::binary | std::ios::trunc);
  if (!stream) {
    throw std::runtime_error("test file could not be created");
  }
  stream << contents;
  stream.close();
  if (chmod(path.c_str(), mode) != 0) {
    throw std::runtime_error("test file mode could not be set");
  }
}

XdgPaths testPaths(const std::filesystem::path &home) {
  std::filesystem::create_directory(home);
  chmod(home.c_str(), 0700);
  return radishlex::linux_platform::resolveXdgPathsForTesting(
      XdgTestEnvironment{
          home,
          std::nullopt,
          std::nullopt,
          std::nullopt,
          std::nullopt,
          static_cast<std::uint32_t>(geteuid()),
      });
}

template <typename Callback>
void requirePrivacyError(PrivacyModeError expected, Callback callback,
                         const char *message) {
  try {
    callback();
  } catch (const PrivacyModeException &error) {
    require(error.code() == expected, message);
    return;
  }
  require(false, message);
}

template <typename Callback>
void requireRuntimeError(ManagerRuntimeError expected, Callback callback,
                         const char *message) {
  try {
    callback();
  } catch (const ManagerRuntimeException &error) {
    require(error.code() == expected, message);
    return;
  }
  require(false, message);
}

void testPrivacyRoundTripAndRollback() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  radishlex::linux_platform::preparePrivateProductPaths(paths);

  require(radishlex::linux_platform::readPrivacyMode(paths) ==
              PrivacyModeState{},
          "missing privacy file must use the absent default");

  radishlex::linux_platform::writePrivacyMode(paths, true);
  require(radishlex::linux_platform::readPrivacyMode(paths) ==
              PrivacyModeState{true, true},
          "privacy true must round trip");
  require(permissions(paths.privacy_path) == 0600,
          "privacy file must be private from creation");

  radishlex::linux_platform::writePrivacyMode(paths, false);
  require(radishlex::linux_platform::readPrivacyMode(paths) ==
              PrivacyModeState{true, false},
          "privacy false must round trip");

  radishlex::linux_platform::restorePrivacyMode(paths, PrivacyModeState{});
  require(radishlex::linux_platform::readPrivacyMode(paths) ==
              PrivacyModeState{},
          "rollback to absent must remove only the privacy truth source");
  for (const auto &entry : std::filesystem::directory_iterator(paths.config_root)) {
    require(entry.path().filename().string().rfind(
                ".privacy-mode.json.tmp.", 0) != 0,
            "privacy writes must not leave temporary files behind");
  }
}

void testPrivacyRejectsMalformedAndUnsafeFiles() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  radishlex::linux_platform::preparePrivateProductPaths(paths);

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":true,}\n");
  requirePrivacyError(
      PrivacyModeError::InvalidFormat,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "trailing comma must be rejected");

  writeFile(paths.privacy_path,
            "{\"format_version\":2,\"privacy_mode\":true}\n");
  requirePrivacyError(
      PrivacyModeError::InvalidFormat,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "unsupported privacy format version must be rejected");

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":1}\n");
  requirePrivacyError(
      PrivacyModeError::InvalidFormat,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "non-boolean privacy value must be rejected");

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":true,\"extra\":false}\n");
  requirePrivacyError(
      PrivacyModeError::InvalidFormat,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "unknown privacy field must be rejected");

  writeFile(paths.privacy_path, std::string(513, ' '));
  requirePrivacyError(
      PrivacyModeError::InvalidFormat,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "oversized privacy file must be rejected");

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":true}\n", 0644);
  requirePrivacyError(
      PrivacyModeError::UnsafeFile,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "broad privacy permissions must be rejected");

  std::filesystem::remove(paths.privacy_path);
  const std::filesystem::path target = temporary.path() / "privacy-target";
  writeFile(target, "{\"format_version\":1,\"privacy_mode\":true}\n");
  std::filesystem::create_hard_link(target, paths.privacy_path);
  requirePrivacyError(
      PrivacyModeError::UnsafeFile,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "privacy hardlink must be rejected");

  std::filesystem::remove(paths.privacy_path);
  std::filesystem::create_symlink(target, paths.privacy_path);
  requirePrivacyError(
      PrivacyModeError::UnsafeFile,
      [&] { radishlex::linux_platform::readPrivacyMode(paths); },
      "privacy symlink must be rejected");
  requirePrivacyError(
      PrivacyModeError::UnsafeFile,
      [&] { radishlex::linux_platform::writePrivacyMode(paths, false); },
      "privacy write must not replace an unsafe existing file");
}

void testPrivacyRuntimeFailsClosedAndRecovers() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  radishlex::linux_platform::preparePrivateProductPaths(paths);
  PrivacyModeRuntime runtime(paths);

  runtime.refresh();
  require(!runtime.snapshot().enabled &&
              runtime.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "runtime must treat an absent privacy file as valid and disabled");

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":true,}\n");
  runtime.refresh();
  require(runtime.snapshot().enabled &&
              runtime.snapshot().status ==
                  PrivacyModeRuntimeStatus::InvalidFormat,
          "runtime must fail closed on invalid privacy JSON");
  require(std::string(radishlex::linux_platform::privacyModeRuntimeStatusCode(
                          runtime.snapshot().status)) == "invalid_format",
          "runtime errors must expose only a stable category");

  writeFile(paths.privacy_path,
            "{\"format_version\":1,\"privacy_mode\":false}\n");
  runtime.refresh();
  require(!runtime.snapshot().enabled &&
              runtime.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "runtime must recover only after a valid strict read");

  runtime.markMonitorUnavailable();
  require(runtime.snapshot().enabled &&
              runtime.snapshot().status ==
                  PrivacyModeRuntimeStatus::MonitorUnavailable,
          "unavailable change monitoring must fail closed");
}

void testManagerBundleResolution() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  const std::filesystem::path bundle = temporary.path() / "bundle";
  const std::filesystem::path executable = bundle / "radishlex_manager";
  const std::filesystem::path native_library =
      bundle / "lib/libradishlex_ime_ffi.so";
  writeFile(executable, "runner", 0700);
  writeFile(native_library, "ffi", 0644);

  const auto resolved =
      radishlex::linux_platform::resolveManagerRuntimePathsForTesting(
          executable, paths);
  require(resolved.native_library_path == native_library,
          "manager library must come from the fixed bundle lib directory");
  require(resolved.xdg.userdb_path == paths.userdb_path,
          "manager runtime must retain the shared XDG userdb");
  require(std::filesystem::exists(paths.userdb_path),
          "manager runtime must prepare the shared private userdb");

  chmod(native_library.c_str(), 0664);
  requireRuntimeError(
      ManagerRuntimeError::UnsafeBundle,
      [&] {
        radishlex::linux_platform::resolveManagerRuntimePathsForTesting(
            executable, paths);
      },
      "group-writable manager library must be rejected");

  std::filesystem::remove(native_library);
  const std::filesystem::path target = temporary.path() / "ffi-target";
  writeFile(target, "ffi", 0644);
  std::filesystem::create_symlink(target, native_library);
  requireRuntimeError(
      ManagerRuntimeError::UnsafeBundle,
      [&] {
        radishlex::linux_platform::resolveManagerRuntimePathsForTesting(
            executable, paths);
      },
      "manager library symlink must be rejected");

  std::filesystem::remove(native_library);
  requireRuntimeError(
      ManagerRuntimeError::NativeLibraryMissing,
      [&] {
        radishlex::linux_platform::resolveManagerRuntimePathsForTesting(
            executable, paths);
      },
      "missing manager library must retain a stable error category");

  const std::filesystem::path wrong_executable = bundle / "manager";
  writeFile(wrong_executable, "runner", 0700);
  requireRuntimeError(
      ManagerRuntimeError::ExecutableLookupFailed,
      [&] {
        radishlex::linux_platform::resolveManagerRuntimePathsForTesting(
            wrong_executable, paths);
      },
      "manager executable basename must remain fixed");
}

}  // namespace

int main() {
  testPrivacyRoundTripAndRollback();
  testPrivacyRejectsMalformedAndUnsafeFiles();
  testPrivacyRuntimeFailsClosedAndRecovers();
  testManagerBundleResolution();
  std::cout << "Linux Manager runtime contract passed.\n";
  return 0;
}
