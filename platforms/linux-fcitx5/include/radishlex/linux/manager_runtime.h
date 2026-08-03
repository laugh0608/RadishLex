#ifndef RADISHLEX_LINUX_MANAGER_RUNTIME_H
#define RADISHLEX_LINUX_MANAGER_RUNTIME_H

#include <filesystem>
#include <stdexcept>
#include <string>

#include "radishlex/linux/xdg_paths.h"

namespace radishlex::linux_platform {

enum class ManagerRuntimeError {
  ExecutableLookupFailed,
  UnsafeBundle,
  NativeLibraryMissing,
};

class ManagerRuntimeException final : public std::runtime_error {
 public:
  ManagerRuntimeException(ManagerRuntimeError code,
                          const std::string &message);

  ManagerRuntimeError code() const noexcept;

 private:
  ManagerRuntimeError code_;
};

struct ManagerRuntimePaths {
  XdgPaths xdg;
  std::filesystem::path native_library_path;
};

ManagerRuntimePaths resolveManagerRuntimePaths();
void secureManagerLocalFiles(const XdgPaths &paths);

#if defined(RADISHLEX_MANAGER_RUNTIME_TESTING)
ManagerRuntimePaths resolveManagerRuntimePathsForTesting(
    const std::filesystem::path &executable_path, const XdgPaths &xdg_paths);
#endif

}  // namespace radishlex::linux_platform

#endif
