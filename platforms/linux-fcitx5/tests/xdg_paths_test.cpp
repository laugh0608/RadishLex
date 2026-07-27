#include "radishlex/linux/xdg_paths.h"

#include <sys/stat.h>
#include <unistd.h>

#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

namespace {

using radishlex::linux_platform::XdgPathError;
using radishlex::linux_platform::XdgPathException;
using radishlex::linux_platform::XdgPaths;
using radishlex::linux_platform::XdgTestEnvironment;

class TemporaryDirectory final {
 public:
  TemporaryDirectory() {
    const std::filesystem::path base =
        std::filesystem::canonical(std::filesystem::temp_directory_path());
    std::string pattern = (base / "radishlex-xdg.XXXXXX").string();
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

XdgTestEnvironment environment(const std::filesystem::path &home) {
  return XdgTestEnvironment{
      home,
      std::nullopt,
      std::nullopt,
      std::nullopt,
      std::nullopt,
      static_cast<std::uint32_t>(geteuid()),
  };
}

template <typename Callback>
void requireError(XdgPathError expected, Callback callback,
                  const char *message) {
  try {
    callback();
  } catch (const XdgPathException &error) {
    require(error.code() == expected, message);
    return;
  }
  require(false, message);
}

void testDefaultsAndPermissions() {
  TemporaryDirectory temporary;
  const std::filesystem::path home = temporary.path() / "home";
  std::filesystem::create_directory(home);
  chmod(home.c_str(), 0700);
  const XdgPaths paths =
      radishlex::linux_platform::resolveXdgPathsForTesting(environment(home));

  require(paths.data_root == home / ".local/share/radishlex",
          "default data root must follow XDG");
  require(paths.config_root == home / ".config/radishlex",
          "default config root must follow XDG");
  require(paths.state_root == home / ".local/state/radishlex",
          "default state root must follow XDG");
  require(paths.cache_root == home / ".cache/radishlex",
          "default cache root must follow XDG");
  require(paths.userdb_path == paths.data_root / "userdb.sqlite3",
          "userdb path must be fixed");

  radishlex::linux_platform::preparePrivateProductPaths(paths);
  require(permissions(paths.data_root) == 0700,
          "product data directory must be private");
  require(permissions(paths.config_root) == 0700,
          "product config directory must be private");
  require(permissions(paths.rime_user_data_dir) == 0700,
          "Rime user directory must be private");
  require(permissions(paths.userdb_path) == 0600,
          "userdb must be private");
}

void testCustomRootsAndInvalidInput() {
  TemporaryDirectory temporary;
  const std::filesystem::path home = temporary.path() / "home";
  std::filesystem::create_directory(home);
  XdgTestEnvironment custom = environment(home);
  custom.data_home = (home / "data").string();
  custom.config_home = (home / "config").string();
  custom.state_home = (home / "state").string();
  custom.cache_home = (home / "cache").string();
  const XdgPaths paths =
      radishlex::linux_platform::resolveXdgPathsForTesting(custom);
  require(paths.data_root == home / "data/radishlex",
          "absolute data override must be honored in test resolver");

  XdgTestEnvironment relative = environment(home);
  relative.data_home = "relative/data";
  requireError(
      XdgPathError::InvalidEnvironment,
      [&] {
        radishlex::linux_platform::resolveXdgPathsForTesting(relative);
      },
      "relative XDG root must be rejected");

  XdgTestEnvironment dotted = environment(home);
  dotted.config_home = (home / "config/../escaped").string();
  requireError(
      XdgPathError::InvalidEnvironment,
      [&] {
        radishlex::linux_platform::resolveXdgPathsForTesting(dotted);
      },
      "dot components in XDG roots must be rejected");
}

void testSymlinkAndPermissionRejection() {
  TemporaryDirectory temporary;
  const std::filesystem::path home = temporary.path() / "home";
  std::filesystem::create_directory(home);
  chmod(home.c_str(), 0700);

  const std::filesystem::path actual = home / "actual-data";
  std::filesystem::create_directory(actual);
  const std::filesystem::path link = home / "linked-data";
  std::filesystem::create_directory_symlink(actual, link);
  XdgTestEnvironment linked = environment(home);
  linked.data_home = link.string();
  const XdgPaths linked_paths =
      radishlex::linux_platform::resolveXdgPathsForTesting(linked);
  requireError(
      XdgPathError::UnsafePath,
      [&] {
        radishlex::linux_platform::preparePrivateProductPaths(linked_paths);
      },
      "symlink XDG root must be rejected");

  const XdgPaths paths =
      radishlex::linux_platform::resolveXdgPathsForTesting(environment(home));
  radishlex::linux_platform::preparePrivateProductPaths(paths);
  chmod(paths.userdb_path.c_str(), 0644);
  requireError(
      XdgPathError::UnsafePermissions,
      [&] {
        radishlex::linux_platform::validatePrivateRegularFile(
            paths.userdb_path, paths.owner_id);
      },
      "broad userdb permissions must be rejected");

  chmod(paths.userdb_path.c_str(), 0600);
  requireError(
      XdgPathError::WrongOwner,
      [&] {
        radishlex::linux_platform::validatePrivateRegularFile(
            paths.userdb_path, paths.owner_id + 1);
      },
      "unexpected userdb owner must be rejected");

  chmod(paths.data_root.c_str(), 0755);
  requireError(
      XdgPathError::UnsafePermissions,
      [&] {
        radishlex::linux_platform::preparePrivateProductPaths(paths);
      },
      "broad product directory permissions must be rejected");
}

void testProductionIgnoresTestOverride() {
  TemporaryDirectory temporary;
  const std::string fake_home = temporary.path().string();
  const char *old_home = std::getenv("HOME");
  const std::optional<std::string> saved_home =
      old_home == nullptr ? std::nullopt : std::optional<std::string>(old_home);
  setenv("HOME", fake_home.c_str(), 1);
  setenv("RADISHLEX_XDG_TEST_DATA_HOME", fake_home.c_str(), 1);
  const XdgPaths production =
      radishlex::linux_platform::resolveProductionXdgPaths();
  require(production.data_root.string().find(fake_home) != 0,
          "production resolver must use effective user home, not test override");
  unsetenv("RADISHLEX_XDG_TEST_DATA_HOME");
  if (saved_home.has_value()) {
    setenv("HOME", saved_home->c_str(), 1);
  } else {
    unsetenv("HOME");
  }
}

}  // namespace

int main() {
  testDefaultsAndPermissions();
  testCustomRootsAndInvalidInput();
  testSymlinkAndPermissionRejection();
  testProductionIgnoresTestOverride();
  std::cout << "Linux XDG resolver contract passed.\n";
  return 0;
}
