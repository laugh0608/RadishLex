#include "radishlex/linux/runtime_layout.h"

#include <dlfcn.h>

#include <filesystem>
#include <iostream>
#include <string>

namespace {

class LoadedLibrary {
 public:
  explicit LoadedLibrary(void *handle) : handle_(handle) {}

  ~LoadedLibrary() {
    if (handle_ != nullptr) {
      dlclose(handle_);
    }
  }

  LoadedLibrary(const LoadedLibrary &) = delete;
  LoadedLibrary &operator=(const LoadedLibrary &) = delete;

 private:
  void *handle_;
};

}  // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: radishlex_runtime_probe <absolute-radishlex.so>\n";
    return 2;
  }

  try {
    const auto layout =
        radishlex::linux_platform::runtimeLayoutFromAddonLibrary(
            std::filesystem::path(argv[1]));
    radishlex::linux_platform::validateRuntimeLayout(layout);

    dlerror();
    void *handle =
        dlopen(layout.addon_library.c_str(), RTLD_NOW | RTLD_LOCAL);
    if (handle == nullptr) {
      (void)dlerror();
      std::cerr << "runtime_probe=failed reason=loader_failure\n";
      return 1;
    }
    LoadedLibrary library(handle);
    std::cout << "runtime_probe=ok addon=radishlex.so "
                 "ffi=libradishlex_ime_ffi.so rime_assets=3\n";
    return 0;
  } catch (const radishlex::linux_platform::RuntimeLayoutException &error) {
    std::cerr << "runtime_probe=failed reason="
              << radishlex::linux_platform::runtimeLayoutErrorName(
                     error.code())
              << "\n";
    return 1;
  }
}
