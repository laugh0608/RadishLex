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

enum class PrivacyModeRuntimeStatus {
  Ready,
  InvalidFormat,
  UnsafeFile,
  IoFailure,
  MonitorUnavailable,
};

struct PrivacyModeRuntimeSnapshot {
  bool enabled = true;
  PrivacyModeRuntimeStatus status = PrivacyModeRuntimeStatus::IoFailure;
};

class PrivacyModeRuntime final {
 public:
  explicit PrivacyModeRuntime(XdgPaths paths);

  void refresh() noexcept;
  void markMonitorUnavailable() noexcept;
  const PrivacyModeRuntimeSnapshot &snapshot() const noexcept;

 private:
  XdgPaths paths_;
  PrivacyModeRuntimeSnapshot snapshot_;
};

PrivacyModeState readPrivacyMode(const XdgPaths &paths);
void writePrivacyMode(const XdgPaths &paths, bool enabled);
void restorePrivacyMode(const XdgPaths &paths, PrivacyModeState state);
const char *privacyModeRuntimeStatusCode(
    PrivacyModeRuntimeStatus status) noexcept;

}  // namespace radishlex::linux_platform

#endif
