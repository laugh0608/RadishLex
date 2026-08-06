#include "radishlex/linux/runtime_layout.h"

#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {

void require(bool condition, const std::string &message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

}  // namespace

int main() {
  const std::filesystem::path addon =
      "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so";
  const auto layout =
      radishlex::linux_platform::runtimeLayoutFromAddonLibrary(addon);

  require(layout.addon_library == addon,
          "system addon path must remain unchanged");
  require(layout.ffi_library ==
              "/usr/lib/aarch64-linux-gnu/fcitx5/"
              "libradishlex_ime_ffi.so",
          "system FFI must remain addon-relative");
  require(layout.rime_shared_data_dir == "/usr/share/radishlex/rime",
          "system RimeData must use the fixed product path");

  bool rejected_relative = false;
  try {
    (void)radishlex::linux_platform::runtimeLayoutFromAddonLibrary(
        "radishlex.so");
  } catch (const radishlex::linux_platform::RuntimeLayoutException &error) {
    rejected_relative =
        error.code() ==
        radishlex::linux_platform::RuntimeLayoutError::InvalidAddonPath;
  }
  require(rejected_relative, "relative addon paths must remain rejected");

  std::cout << "Linux system runtime layout contract passed.\n";
  return 0;
}
