#include "radishlex/linux/xdg_paths.h"

#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <pwd.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include <array>
#include <limits>
#include <vector>

namespace radishlex::linux_platform {
namespace {

constexpr mode_t kPrivateDirectoryMode = 0700;
constexpr mode_t kPrivateFileMode = 0600;
constexpr const char *kProductDirectory = "radishlex";

struct ResolverEnvironment {
  std::filesystem::path home;
  std::optional<std::string> data_home;
  std::optional<std::string> config_home;
  std::optional<std::string> state_home;
  std::optional<std::string> cache_home;
  std::uint32_t owner_id;
};

std::optional<std::string> readEnvironment(const char *name) {
  const char *value = std::getenv(name);
  if (value == nullptr || value[0] == '\0') {
    return std::nullopt;
  }
  return std::string(value);
}

ResolverEnvironment productionEnvironment() {
  const uid_t owner_id = geteuid();
  long suggested = sysconf(_SC_GETPW_R_SIZE_MAX);
  std::size_t buffer_size =
      suggested > 0 ? static_cast<std::size_t>(suggested) : 16384U;
  std::vector<char> buffer(buffer_size);
  passwd user{};
  passwd *result = nullptr;
  const int status =
      getpwuid_r(owner_id, &user, buffer.data(), buffer.size(), &result);
  if (status != 0 || result == nullptr || user.pw_dir == nullptr ||
      user.pw_dir[0] == '\0') {
    throw XdgPathException(XdgPathError::UserLookupFailed,
                           "unable to resolve effective user home");
  }
  return ResolverEnvironment{
      std::filesystem::path(user.pw_dir),
      readEnvironment("XDG_DATA_HOME"),
      readEnvironment("XDG_CONFIG_HOME"),
      readEnvironment("XDG_STATE_HOME"),
      readEnvironment("XDG_CACHE_HOME"),
      static_cast<std::uint32_t>(owner_id),
  };
}

void validateAbsoluteInput(const std::filesystem::path &path,
                           const char *field) {
  if (path.empty() || !path.is_absolute()) {
    throw XdgPathException(XdgPathError::InvalidEnvironment,
                           std::string(field) + " must be absolute");
  }
  for (const auto &component : path) {
    if (component == "." || component == "..") {
      throw XdgPathException(
          XdgPathError::InvalidEnvironment,
          std::string(field) + " must not contain dot components");
    }
  }
}

std::filesystem::path xdgRoot(
    const std::optional<std::string> &configured,
    const std::filesystem::path &fallback, const char *field) {
  const std::filesystem::path root =
      configured.has_value() ? std::filesystem::path(*configured) : fallback;
  validateAbsoluteInput(root, field);
  return root.lexically_normal();
}

XdgPaths resolve(const ResolverEnvironment &environment) {
  validateAbsoluteInput(environment.home, "effective user home");
  const std::filesystem::path data_home =
      xdgRoot(environment.data_home, environment.home / ".local" / "share",
              "XDG_DATA_HOME");
  const std::filesystem::path config_home =
      xdgRoot(environment.config_home, environment.home / ".config",
              "XDG_CONFIG_HOME");
  const std::filesystem::path state_home =
      xdgRoot(environment.state_home, environment.home / ".local" / "state",
              "XDG_STATE_HOME");
  const std::filesystem::path cache_home =
      xdgRoot(environment.cache_home, environment.home / ".cache",
              "XDG_CACHE_HOME");

  const std::filesystem::path data_root = data_home / kProductDirectory;
  const std::filesystem::path config_root = config_home / kProductDirectory;
  const std::filesystem::path state_root = state_home / kProductDirectory;
  const std::filesystem::path cache_root = cache_home / kProductDirectory;
  return XdgPaths{
      data_root,
      config_root,
      state_root,
      cache_root,
      data_root / "rime",
      data_root / "userdb.sqlite3",
      config_root / "settings.json",
      config_root / "privacy-mode.json",
      environment.owner_id,
  };
}

struct FileStatus {
  struct stat value;
  bool exists;
};

FileStatus noFollowStatus(const std::filesystem::path &path) {
  struct stat value {};
  if (lstat(path.c_str(), &value) == 0) {
    return FileStatus{value, true};
  }
  if (errno == ENOENT) {
    return FileStatus{value, false};
  }
  throw XdgPathException(
      XdgPathError::IoFailure,
      "unable to inspect XDG path: " + std::string(std::strerror(errno)));
}

void requireOwner(const std::filesystem::path &path, const struct stat &status,
                  std::uint32_t owner_id) {
  if (status.st_uid != static_cast<uid_t>(owner_id)) {
    throw XdgPathException(XdgPathError::WrongOwner,
                           "XDG path has an unexpected owner: " +
                               path.filename().string());
  }
}

void requireNoSymlink(const std::filesystem::path &path,
                      const struct stat &status) {
  if (S_ISLNK(status.st_mode)) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "XDG path contains a symlink: " +
                               path.filename().string());
  }
}

void validateExistingComponents(const std::filesystem::path &path) {
  std::filesystem::path current = path.root_path();
  for (const auto &component : path.relative_path()) {
    current /= component;
    const FileStatus status = noFollowStatus(current);
    if (!status.exists) {
      break;
    }
    requireNoSymlink(current, status.value);
    if (!S_ISDIR(status.value.st_mode)) {
      throw XdgPathException(XdgPathError::UnsafePath,
                             "XDG path component is not a directory: " +
                                 current.filename().string());
    }
  }
}

void ensureBaseDirectory(const std::filesystem::path &path,
                         std::uint32_t owner_id) {
  validateExistingComponents(path);
  std::error_code error;
  std::filesystem::create_directories(path, error);
  if (error) {
    throw XdgPathException(XdgPathError::IoFailure,
                           "unable to create XDG base directory");
  }
  validateExistingComponents(path);
  const FileStatus status = noFollowStatus(path);
  if (!status.exists || !S_ISDIR(status.value.st_mode)) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "XDG base is not a directory");
  }
  requireOwner(path, status.value, owner_id);
}

void ensurePrivateDirectory(const std::filesystem::path &path,
                            std::uint32_t owner_id) {
  validateExistingComponents(path.parent_path());
  FileStatus status = noFollowStatus(path);
  if (!status.exists) {
    if (mkdir(path.c_str(), kPrivateDirectoryMode) != 0) {
      throw XdgPathException(
          XdgPathError::IoFailure,
          "unable to create private product directory: " +
              std::string(std::strerror(errno)));
    }
    status = noFollowStatus(path);
  }
  requireNoSymlink(path, status.value);
  if (!S_ISDIR(status.value.st_mode)) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "private product path is not a directory");
  }
  requireOwner(path, status.value, owner_id);
  if ((status.value.st_mode & 0077) != 0) {
    throw XdgPathException(
        XdgPathError::UnsafePermissions,
        "private product directory grants group or other access");
  }
}

void ensurePrivateRegularFile(const std::filesystem::path &path,
                              std::uint32_t owner_id) {
  FileStatus status = noFollowStatus(path);
  if (!status.exists) {
    const int descriptor =
        open(path.c_str(), O_CREAT | O_EXCL | O_RDWR | O_NOFOLLOW,
             kPrivateFileMode);
    if (descriptor < 0) {
      throw XdgPathException(
          XdgPathError::IoFailure,
          "unable to create private userdb: " +
              std::string(std::strerror(errno)));
    }
    if (close(descriptor) != 0) {
      throw XdgPathException(XdgPathError::IoFailure,
                             "unable to close private userdb");
    }
  }
  validatePrivateRegularFile(path, owner_id);
}

std::filesystem::path baseFromProductRoot(
    const std::filesystem::path &product_root) {
  if (product_root.filename() != kProductDirectory) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "product directory name is not fixed");
  }
  return product_root.parent_path();
}

}  // namespace

XdgPathException::XdgPathException(XdgPathError code,
                                   const std::string &message)
    : std::runtime_error(message), code_(code) {}

XdgPathError XdgPathException::code() const noexcept { return code_; }

XdgPaths resolveProductionXdgPaths() {
  return resolve(productionEnvironment());
}

void preparePrivateProductPaths(const XdgPaths &paths) {
  const std::array<std::filesystem::path, 4> product_roots{
      paths.data_root,
      paths.config_root,
      paths.state_root,
      paths.cache_root,
  };
  for (const auto &product_root : product_roots) {
    ensureBaseDirectory(baseFromProductRoot(product_root), paths.owner_id);
    ensurePrivateDirectory(product_root, paths.owner_id);
  }
  if (paths.rime_user_data_dir.parent_path() != paths.data_root ||
      paths.userdb_path.parent_path() != paths.data_root ||
      paths.settings_path.parent_path() != paths.config_root ||
      paths.privacy_path.parent_path() != paths.config_root) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "derived XDG product paths are inconsistent");
  }
  ensurePrivateDirectory(paths.rime_user_data_dir, paths.owner_id);
  ensurePrivateRegularFile(paths.userdb_path, paths.owner_id);
}

void validatePrivateRegularFile(const std::filesystem::path &path,
                                std::uint32_t expected_owner_id) {
  const FileStatus status = noFollowStatus(path);
  if (!status.exists) {
    throw XdgPathException(XdgPathError::IoFailure,
                           "private product file does not exist");
  }
  requireNoSymlink(path, status.value);
  if (!S_ISREG(status.value.st_mode)) {
    throw XdgPathException(XdgPathError::UnsafePath,
                           "private product file is not regular");
  }
  requireOwner(path, status.value, expected_owner_id);
  if ((status.value.st_mode & 0077) != 0) {
    throw XdgPathException(XdgPathError::UnsafePermissions,
                           "private product file grants group or other access");
  }
}

void validatePrivateRegularFileIfPresent(
    const std::filesystem::path &path, std::uint32_t expected_owner_id) {
  if (!noFollowStatus(path).exists) {
    return;
  }
  validatePrivateRegularFile(path, expected_owner_id);
}

#if defined(RADISHLEX_XDG_TESTING)
XdgPaths resolveXdgPathsForTesting(const XdgTestEnvironment &environment) {
  return resolve(ResolverEnvironment{
      environment.home,
      environment.data_home,
      environment.config_home,
      environment.state_home,
      environment.cache_home,
      environment.owner_id,
  });
}
#endif

}  // namespace radishlex::linux_platform
