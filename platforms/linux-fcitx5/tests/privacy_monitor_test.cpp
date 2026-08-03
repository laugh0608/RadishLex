#include "radishlex/linux/privacy_monitor.h"
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

using radishlex::linux_platform::PrivacyModeRuntimeStatus;
using radishlex::linux_platform::XdgPaths;
using radishlex::linux_platform::XdgTestEnvironment;

class TemporaryDirectory final {
 public:
  TemporaryDirectory() {
    const std::filesystem::path base =
        std::filesystem::canonical(std::filesystem::temp_directory_path());
    std::string pattern = (base / "radishlex-privacy-monitor.XXXXXX").string();
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

void writeFile(const std::filesystem::path &path, const std::string &contents,
               mode_t mode) {
  std::ofstream stream(path, std::ios::binary | std::ios::trunc);
  require(static_cast<bool>(stream), "privacy fixture must open");
  stream << contents;
  stream.close();
  require(chmod(path.c_str(), mode) == 0,
          "privacy fixture permissions must be set");
}

void replacePrivacyFile(const XdgPaths &paths, const std::string &contents,
                        mode_t mode = 0600) {
  const std::filesystem::path temporary =
      paths.config_root / ".privacy-monitor-test.tmp";
  writeFile(temporary, contents, mode);
  require(rename(temporary.c_str(), paths.privacy_path.c_str()) == 0,
          "privacy fixture must be atomically replaced");
}

XdgPaths testPaths(const std::filesystem::path &home) {
  std::filesystem::create_directory(home);
  require(chmod(home.c_str(), 0700) == 0, "test home must be private");
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

void testAtomicChangesAreSerializedAndFailClosed() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  radishlex::linux_platform::preparePrivateProductPaths(paths);
  radishlex::linux_platform::PrivacyModeMonitor monitor(paths);

  require(monitor.active() && monitor.descriptor() >= 0,
          "privacy monitor must watch the prepared config directory");
  require(!monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "missing privacy file must start in the valid disabled state");

  radishlex::linux_platform::writePrivacyMode(paths, true);
  require(monitor.consumeEvents(), "true replacement must keep monitor active");
  require(monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "atomic true replacement must apply before the next consumer");

  radishlex::linux_platform::writePrivacyMode(paths, false);
  require(monitor.consumeEvents(),
          "false replacement must keep monitor active");
  require(!monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "atomic false replacement must apply before the next consumer");

  replacePrivacyFile(paths,
                     "{\"format_version\":1,\"privacy_mode\":true,}\n");
  require(monitor.consumeEvents(),
          "malformed replacement must not disable directory monitoring");
  require(monitor.snapshot().enabled &&
              monitor.snapshot().status ==
                  PrivacyModeRuntimeStatus::InvalidFormat,
          "malformed replacement must fail closed with a stable category");

  replacePrivacyFile(
      paths, "{\"format_version\":1,\"privacy_mode\":false}\n", 0644);
  require(monitor.consumeEvents(),
          "unsafe replacement must not disable directory monitoring");
  require(monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::UnsafeFile,
          "unsafe replacement must fail closed instead of trusting false");

  replacePrivacyFile(
      paths, "{\"format_version\":1,\"privacy_mode\":false}\n");
  require(monitor.consumeEvents(),
          "valid recovery must keep directory monitoring active");
  require(!monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "valid replacement must recover from fail-closed state");

  require(unlink(paths.privacy_path.c_str()) == 0,
          "privacy fixture must be removable");
  require(monitor.consumeEvents(), "deletion must keep monitor active");
  require(!monitor.snapshot().enabled &&
              monitor.snapshot().status == PrivacyModeRuntimeStatus::Ready,
          "deletion must restore the valid absent default");
}

void testDirectoryWatchFailureStaysClosed() {
  TemporaryDirectory temporary;
  const XdgPaths paths = testPaths(temporary.path() / "home");
  radishlex::linux_platform::preparePrivateProductPaths(paths);
  radishlex::linux_platform::PrivacyModeMonitor monitor(paths);
  require(chmod(paths.config_root.c_str(), 0755) == 0,
          "test must broaden the watched directory permissions");
  require(!monitor.consumeEvents(),
          "watched directory metadata change must disable the monitor");
  require(!monitor.active() && monitor.snapshot().enabled &&
              monitor.snapshot().status ==
                  PrivacyModeRuntimeStatus::MonitorUnavailable,
          "monitor failure must permanently fail closed for this addon run");
  require(std::string(radishlex::linux_platform::privacyModeRuntimeStatusCode(
                          monitor.snapshot().status)) ==
              "monitor_unavailable",
          "monitor failure must expose only its stable category");
  require(chmod(paths.config_root.c_str(), 0700) == 0,
          "test cleanup must restore private directory permissions");
}

}  // namespace

int main() {
  testAtomicChangesAreSerializedAndFailClosed();
  testDirectoryWatchFailureStaysClosed();
  return 0;
}
