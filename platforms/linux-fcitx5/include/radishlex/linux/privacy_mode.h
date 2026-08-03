#ifndef RADISHLEX_LINUX_PRIVACY_MODE_H
#define RADISHLEX_LINUX_PRIVACY_MODE_H

#include <stdexcept>
#include <string>

#include "radishlex/linux/xdg_paths.h"

namespace radishlex::linux_platform {

enum class PrivacyModeError {
  InvalidFormat,
  UnsafeFile,
  IoFailure,
};

class PrivacyModeException final : public std::runtime_error {
 public:
  PrivacyModeException(PrivacyModeError code, const std::string &message);

  PrivacyModeError code() const noexcept;

 private:
  PrivacyModeError code_;
};

struct PrivacyModeState {
  bool present = false;
  bool enabled = false;

  bool operator==(const PrivacyModeState &other) const noexcept {
    return present == other.present && enabled == other.enabled;
  }
};

PrivacyModeState readPrivacyMode(const XdgPaths &paths);
void writePrivacyMode(const XdgPaths &paths, bool enabled);
void restorePrivacyMode(const XdgPaths &paths, PrivacyModeState state);

}  // namespace radishlex::linux_platform

#endif
