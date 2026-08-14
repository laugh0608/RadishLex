#ifndef RADISHLEX_LINUX_PRIVACY_MONITOR_H
#define RADISHLEX_LINUX_PRIVACY_MONITOR_H

#include <string>

#include "radishlex/linux/privacy_mode.h"

namespace radishlex::linux_platform {

class PrivacyModeMonitor final {
 public:
  explicit PrivacyModeMonitor(const XdgPaths &paths);
  ~PrivacyModeMonitor();

  PrivacyModeMonitor(const PrivacyModeMonitor &) = delete;
  PrivacyModeMonitor &operator=(const PrivacyModeMonitor &) = delete;
  PrivacyModeMonitor(PrivacyModeMonitor &&) = delete;
  PrivacyModeMonitor &operator=(PrivacyModeMonitor &&) = delete;

  int descriptor() const noexcept;
  bool active() const noexcept;
  bool consumeEvents() noexcept;
  void markUnavailable() noexcept;
  const PrivacyModeRuntimeSnapshot &snapshot() const noexcept;

 private:
  void deactivate() noexcept;

  PrivacyModeRuntime runtime_;
  std::string filename_;
  int descriptor_ = -1;
  int watch_descriptor_ = -1;
  bool active_ = false;
};

}  // namespace radishlex::linux_platform

#endif
