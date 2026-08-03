#include "radishlex/linux/privacy_monitor.h"

#include <cerrno>
#include <cstddef>
#include <cstdint>
#include <sys/inotify.h>
#include <unistd.h>

#include <array>
#include <cstring>

namespace radishlex::linux_platform {
namespace {

constexpr std::uint32_t kDirectoryWatchMask =
    IN_ATTRIB | IN_CLOSE_WRITE | IN_CREATE | IN_DELETE | IN_MOVED_FROM |
    IN_MOVED_TO | IN_DELETE_SELF | IN_MOVE_SELF;
constexpr std::uint32_t kTargetChangeMask =
    IN_ATTRIB | IN_CLOSE_WRITE | IN_CREATE | IN_DELETE | IN_MOVED_FROM |
    IN_MOVED_TO;
constexpr std::uint32_t kMonitorFailureMask =
    IN_DELETE_SELF | IN_MOVE_SELF | IN_IGNORED | IN_Q_OVERFLOW;

}  // namespace

PrivacyModeMonitor::PrivacyModeMonitor(const XdgPaths &paths)
    : runtime_(paths), filename_(paths.privacy_path.filename().string()) {
  descriptor_ = inotify_init1(IN_NONBLOCK | IN_CLOEXEC);
  if (descriptor_ < 0) {
    runtime_.markMonitorUnavailable();
    return;
  }
  watch_descriptor_ =
      inotify_add_watch(descriptor_, paths.config_root.c_str(),
                        static_cast<std::uint32_t>(kDirectoryWatchMask));
  if (watch_descriptor_ < 0) {
    static_cast<void>(close(descriptor_));
    descriptor_ = -1;
    runtime_.markMonitorUnavailable();
    return;
  }
  active_ = true;
  // Install the directory watch before reading the initial snapshot. A change
  // that races with startup is therefore either included in this read or left
  // queued for consumeEvents(), rather than being missed between read/watch.
  runtime_.refresh();
}

PrivacyModeMonitor::~PrivacyModeMonitor() {
  if (descriptor_ < 0) {
    return;
  }
  if (watch_descriptor_ >= 0) {
    static_cast<void>(inotify_rm_watch(descriptor_, watch_descriptor_));
  }
  static_cast<void>(close(descriptor_));
}

int PrivacyModeMonitor::descriptor() const noexcept { return descriptor_; }

bool PrivacyModeMonitor::active() const noexcept { return active_; }

bool PrivacyModeMonitor::consumeEvents() noexcept {
  if (!active_) {
    return false;
  }

  alignas(struct inotify_event) std::array<char, 4096> buffer{};
  bool refresh_required = false;
  while (true) {
    const ssize_t count = read(descriptor_, buffer.data(), buffer.size());
    if (count < 0) {
      if (errno == EINTR) {
        continue;
      }
      if (errno == EAGAIN || errno == EWOULDBLOCK) {
        break;
      }
      deactivate();
      return false;
    }
    if (count == 0) {
      deactivate();
      return false;
    }

    std::size_t offset = 0;
    const std::size_t bytes = static_cast<std::size_t>(count);
    while (offset < bytes) {
      if (bytes - offset < sizeof(struct inotify_event)) {
        deactivate();
        return false;
      }
      const auto *event = reinterpret_cast<const struct inotify_event *>(
          buffer.data() + offset);
      const std::size_t event_size = sizeof(struct inotify_event) + event->len;
      if (event_size > bytes - offset) {
        deactivate();
        return false;
      }
      if ((event->mask & kMonitorFailureMask) != 0) {
        deactivate();
        return false;
      }
      if (event->len == 0 && (event->mask & IN_ATTRIB) != 0) {
        deactivate();
        return false;
      }
      if (event->len > 0 && (event->mask & kTargetChangeMask) != 0 &&
          std::strcmp(event->name, filename_.c_str()) == 0) {
        refresh_required = true;
      }
      offset += event_size;
    }
  }

  if (refresh_required) {
    runtime_.refresh();
  }
  return true;
}

void PrivacyModeMonitor::markUnavailable() noexcept { deactivate(); }

const PrivacyModeRuntimeSnapshot &PrivacyModeMonitor::snapshot()
    const noexcept {
  return runtime_.snapshot();
}

void PrivacyModeMonitor::deactivate() noexcept {
  active_ = false;
  runtime_.markMonitorUnavailable();
}

}  // namespace radishlex::linux_platform
