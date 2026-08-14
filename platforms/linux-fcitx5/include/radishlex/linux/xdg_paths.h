#ifndef RADISHLEX_LINUX_XDG_PATHS_H
#define RADISHLEX_LINUX_XDG_PATHS_H

#include <cstdint>
#include <filesystem>
#include <optional>
#include <stdexcept>
#include <string>

namespace radishlex::linux_platform {

enum class XdgPathError {
  InvalidEnvironment,
  UserLookupFailed,
  UnsafePath,
  WrongOwner,
  UnsafePermissions,
  IoFailure,
};

class XdgPathException final : public std::runtime_error {
 public:
  XdgPathException(XdgPathError code, const std::string &message);

  XdgPathError code() const noexcept;

 private:
  XdgPathError code_;
};

struct XdgPaths {
  std::filesystem::path data_root;
  std::filesystem::path config_root;
  std::filesystem::path state_root;
  std::filesystem::path cache_root;
  std::filesystem::path rime_user_data_dir;
  std::filesystem::path userdb_path;
  std::filesystem::path settings_path;
  std::filesystem::path privacy_path;
  std::uint32_t owner_id;
};

XdgPaths resolveProductionXdgPaths();
void preparePrivateProductPaths(const XdgPaths &paths);
void validatePrivateRegularFile(const std::filesystem::path &path,
                                std::uint32_t expected_owner_id);
void validatePrivateRegularFileIfPresent(const std::filesystem::path &path,
                                         std::uint32_t expected_owner_id);

#if defined(RADISHLEX_XDG_TESTING)
struct XdgTestEnvironment {
  std::filesystem::path home;
  std::optional<std::string> data_home;
  std::optional<std::string> config_home;
  std::optional<std::string> state_home;
  std::optional<std::string> cache_home;
  std::uint32_t owner_id;
};

XdgPaths resolveXdgPathsForTesting(const XdgTestEnvironment &environment);
#endif

}  // namespace radishlex::linux_platform

#endif
