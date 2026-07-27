#include "radishlex/linux/runtime_layout.h"

#include <sys/stat.h>
#include <unistd.h>

#include <algorithm>
#include <array>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {

using radishlex::linux_platform::RuntimeLayoutError;
using radishlex::linux_platform::RuntimeLayoutException;

class TemporaryDirectory {
 public:
  TemporaryDirectory() {
    std::array<char, 64> pattern{};
    const std::string value = "/tmp/radishlex-runtime-layout.XXXXXX";
    std::copy(value.begin(), value.end(), pattern.begin());
    char *created = mkdtemp(pattern.data());
    if (created == nullptr) {
      throw std::runtime_error("mkdtemp failed");
    }
    path_ = std::filesystem::path(created).lexically_normal();
  }

  ~TemporaryDirectory() {
    std::error_code error;
    std::filesystem::remove_all(path_, error);
  }

  const std::filesystem::path &path() const { return path_; }

 private:
  std::filesystem::path path_;
};

void require(bool condition, const std::string &message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

void writeFile(const std::filesystem::path &path) {
  std::ofstream output(path);
  output << "synthetic runtime asset\n";
  output.close();
  if (!output) {
    throw std::runtime_error("unable to create synthetic runtime asset");
  }
  if (chmod(path.c_str(), 0644) != 0) {
    throw std::runtime_error("unable to set synthetic file mode");
  }
}

template <typename Callback>
void requireError(RuntimeLayoutError expected, Callback callback,
                  const std::string &message) {
  try {
    callback();
  } catch (const RuntimeLayoutException &error) {
    require(error.code() == expected, message + ": wrong error");
    return;
  }
  throw std::runtime_error(message + ": expected failure");
}

}  // namespace

int main() {
  TemporaryDirectory temporary;
  const std::filesystem::path addon =
      temporary.path() / "lib" / "fcitx5" / "radishlex.so";
  std::filesystem::create_directories(addon.parent_path());
  if (chmod((temporary.path() / "lib").c_str(), 0755) != 0 ||
      chmod(addon.parent_path().c_str(), 0755) != 0) {
    throw std::runtime_error("unable to set synthetic directory mode");
  }
  writeFile(addon);

  auto layout =
      radishlex::linux_platform::runtimeLayoutFromAddonLibrary(addon);
  require(layout.ffi_library ==
              addon.parent_path() / "libradishlex_ime_ffi.so",
          "FFI library must be addon-relative");
  require(layout.rime_shared_data_dir ==
              addon.parent_path() / "radishlex-rime",
          "RimeData must be addon-relative");

  writeFile(layout.ffi_library);
  std::filesystem::create_directory(layout.rime_shared_data_dir);
  if (chmod(layout.rime_shared_data_dir.c_str(), 0755) != 0) {
    throw std::runtime_error("unable to set RimeData directory mode");
  }
  for (const char *asset :
       {"default.yaml", "radishlex_pinyin.schema.yaml",
        "pinyin_simp.dict.yaml"}) {
    writeFile(layout.rime_shared_data_dir / asset);
  }
  radishlex::linux_platform::validateRuntimeLayout(layout);
  require(std::string(radishlex::linux_platform::runtimeLayoutErrorName(
              RuntimeLayoutError::LoaderFailure)) == "loader_failure",
          "runtime layout error names must be stable");

  std::filesystem::remove(layout.ffi_library);
  requireError(
      RuntimeLayoutError::MissingResource,
      [&] { radishlex::linux_platform::validateRuntimeLayout(layout); },
      "missing FFI library must fail");
  std::filesystem::create_symlink(addon, layout.ffi_library);
  requireError(
      RuntimeLayoutError::UnsafeResource,
      [&] { radishlex::linux_platform::validateRuntimeLayout(layout); },
      "symlinked FFI library must fail");
  std::filesystem::remove(layout.ffi_library);
  writeFile(layout.ffi_library);

  if (chmod(addon.parent_path().c_str(), 0775) != 0) {
    throw std::runtime_error("unable to widen synthetic addon directory mode");
  }
  requireError(
      RuntimeLayoutError::UnsafePermissions,
      [&] { radishlex::linux_platform::validateRuntimeLayout(layout); },
      "group-writable addon directory must fail");
  if (chmod(addon.parent_path().c_str(), 0755) != 0) {
    throw std::runtime_error("unable to restore synthetic addon directory");
  }

  const std::filesystem::path schema =
      layout.rime_shared_data_dir / "radishlex_pinyin.schema.yaml";
  if (chmod(schema.c_str(), 0664) != 0) {
    throw std::runtime_error("unable to widen synthetic schema mode");
  }
  requireError(
      RuntimeLayoutError::UnsafePermissions,
      [&] { radishlex::linux_platform::validateRuntimeLayout(layout); },
      "group-writable RimeData must fail");

  requireError(
      RuntimeLayoutError::InvalidAddonPath,
      [&] {
        radishlex::linux_platform::runtimeLayoutFromAddonLibrary(
            std::filesystem::path("radishlex.so"));
      },
      "relative addon path must fail");

  std::cout << "Linux runtime layout contract passed.\n";
  return 0;
}
