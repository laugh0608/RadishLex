#ifndef RADISHLEX_LINUX_RUNTIME_LAYOUT_H
#define RADISHLEX_LINUX_RUNTIME_LAYOUT_H

#include <filesystem>
#include <stdexcept>
#include <string>

namespace radishlex::linux_platform {

enum class RuntimeLayoutError {
  InvalidAddonPath,
  MissingResource,
  UnsafeResource,
  UnsafePermissions,
  LoaderFailure,
};

class RuntimeLayoutException final : public std::runtime_error {
 public:
  RuntimeLayoutException(RuntimeLayoutError code, const std::string &message);

  RuntimeLayoutError code() const noexcept;

 private:
  RuntimeLayoutError code_;
};

const char *runtimeLayoutErrorName(RuntimeLayoutError error) noexcept;

struct RuntimeLayout {
  std::filesystem::path addon_library;
  std::filesystem::path ffi_library;
  std::filesystem::path rime_shared_data_dir;
};

RuntimeLayout runtimeLayoutFromAddonLibrary(
    const std::filesystem::path &addon_library);
RuntimeLayout resolveLoadedRuntimeLayout();
void validateRuntimeLayout(const RuntimeLayout &layout);

}  // namespace radishlex::linux_platform

#endif
