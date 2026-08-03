#include "radishlex/linux/privacy_mode.h"

#include <cerrno>
#include <cstdint>
#include <cstring>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include <array>
#include <filesystem>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace radishlex::linux_platform {
namespace {

constexpr std::size_t kMaximumPrivacyFileBytes = 512;
constexpr mode_t kPrivateFileMode = 0600;

[[noreturn]] void invalidFormat(const char *message) {
  throw PrivacyModeException(PrivacyModeError::InvalidFormat, message);
}

[[noreturn]] void ioFailure(const char *message) {
  throw PrivacyModeException(PrivacyModeError::IoFailure, message);
}

class JsonCursor final {
 public:
  explicit JsonCursor(std::string_view input) : input_(input) {}

  void skipWhitespace() {
    while (offset_ < input_.size()) {
      const char value = input_[offset_];
      if (value != ' ' && value != '\n' && value != '\r' && value != '\t') {
        break;
      }
      ++offset_;
    }
  }

  bool consume(char expected) {
    skipWhitespace();
    if (offset_ >= input_.size() || input_[offset_] != expected) {
      return false;
    }
    ++offset_;
    return true;
  }

  void expect(char expected) {
    if (!consume(expected)) {
      invalidFormat("privacy JSON has an unexpected token");
    }
  }

  std::string key() {
    skipWhitespace();
    if (offset_ >= input_.size() || input_[offset_] != '"') {
      invalidFormat("privacy JSON key must be a string");
    }
    ++offset_;
    const std::size_t start = offset_;
    while (offset_ < input_.size() && input_[offset_] != '"') {
      const char value = input_[offset_];
      const bool allowed =
          (value >= 'a' && value <= 'z') || value == '_';
      if (!allowed) {
        invalidFormat("privacy JSON key contains an unsupported character");
      }
      ++offset_;
    }
    if (offset_ >= input_.size() || offset_ == start) {
      invalidFormat("privacy JSON key is incomplete");
    }
    const std::string result(input_.substr(start, offset_ - start));
    ++offset_;
    return result;
  }

  void versionOne() {
    skipWhitespace();
    if (offset_ >= input_.size() || input_[offset_] != '1') {
      invalidFormat("privacy format version is unsupported");
    }
    ++offset_;
    if (offset_ < input_.size()) {
      const char next = input_[offset_];
      if (next >= '0' && next <= '9') {
        invalidFormat("privacy format version is unsupported");
      }
    }
  }

  bool boolean() {
    skipWhitespace();
    if (input_.substr(offset_, 4) == "true") {
      offset_ += 4;
      return true;
    }
    if (input_.substr(offset_, 5) == "false") {
      offset_ += 5;
      return false;
    }
    invalidFormat("privacy_mode must be a boolean");
  }

  bool atEnd() {
    skipWhitespace();
    return offset_ == input_.size();
  }

 private:
  std::string_view input_;
  std::size_t offset_ = 0;
};

PrivacyModeState parsePrivacyMode(std::string_view input) {
  JsonCursor cursor(input);
  cursor.expect('{');
  bool version_present = false;
  bool privacy_present = false;
  bool privacy_enabled = false;
  bool first_field = true;

  while (true) {
    if (cursor.consume('}')) {
      break;
    }
    if (!first_field) {
      cursor.expect(',');
      if (cursor.consume('}')) {
        invalidFormat("privacy JSON must not contain a trailing comma");
      }
    }
    const std::string key = cursor.key();
    cursor.expect(':');
    if (key == "format_version") {
      if (version_present) {
        invalidFormat("privacy format version is duplicated");
      }
      cursor.versionOne();
      version_present = true;
    } else if (key == "privacy_mode") {
      if (privacy_present) {
        invalidFormat("privacy_mode is duplicated");
      }
      privacy_enabled = cursor.boolean();
      privacy_present = true;
    } else {
      invalidFormat("privacy JSON contains an unknown field");
    }
    first_field = false;
  }

  if (!version_present || !privacy_present || !cursor.atEnd()) {
    invalidFormat("privacy JSON is incomplete");
  }
  return PrivacyModeState{true, privacy_enabled};
}

void validateOpenedPrivacyFile(int descriptor, std::uint32_t owner_id) {
  struct stat status {};
  if (fstat(descriptor, &status) != 0) {
    ioFailure("privacy file metadata could not be read");
  }
  if (!S_ISREG(status.st_mode) || status.st_uid != static_cast<uid_t>(owner_id) ||
      (status.st_mode & 0077) != 0 || status.st_nlink != 1) {
    throw PrivacyModeException(PrivacyModeError::UnsafeFile,
                               "privacy file identity is unsafe");
  }
}

std::string readAll(int descriptor) {
  std::array<char, kMaximumPrivacyFileBytes + 1> buffer{};
  std::size_t used = 0;
  while (used < buffer.size()) {
    const ssize_t count =
        read(descriptor, buffer.data() + used, buffer.size() - used);
    if (count < 0) {
      if (errno == EINTR) {
        continue;
      }
      ioFailure("privacy file could not be read");
    }
    if (count == 0) {
      break;
    }
    used += static_cast<std::size_t>(count);
  }
  if (used > kMaximumPrivacyFileBytes) {
    throw PrivacyModeException(PrivacyModeError::InvalidFormat,
                               "privacy file exceeds its size limit");
  }
  return std::string(buffer.data(), used);
}

void closeOrThrow(int &descriptor) {
  const int open_descriptor = descriptor;
  descriptor = -1;
  if (close(open_descriptor) != 0) {
    ioFailure("privacy file could not be closed");
  }
}

void closeIgnoringErrors(int &descriptor) noexcept {
  if (descriptor < 0) {
    return;
  }
  const int open_descriptor = descriptor;
  descriptor = -1;
  static_cast<void>(close(open_descriptor));
}

void writeAll(int descriptor, std::string_view contents) {
  std::size_t written = 0;
  while (written < contents.size()) {
    const ssize_t count =
        write(descriptor, contents.data() + written, contents.size() - written);
    if (count < 0) {
      if (errno == EINTR) {
        continue;
      }
      ioFailure("privacy file could not be written");
    }
    if (count == 0) {
      ioFailure("privacy file write made no progress");
    }
    written += static_cast<std::size_t>(count);
  }
}

void syncDirectory(const std::filesystem::path &directory) {
  int descriptor =
      open(directory.c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0) {
    ioFailure("privacy directory could not be opened");
  }
  if (fsync(descriptor) != 0) {
    const int saved_errno = errno;
    closeIgnoringErrors(descriptor);
    errno = saved_errno;
    ioFailure("privacy directory could not be synchronized");
  }
  closeOrThrow(descriptor);
}

std::string serializedPrivacyMode(bool enabled) {
  return std::string("{\n  \"format_version\": 1,\n  \"privacy_mode\": ") +
         (enabled ? "true" : "false") + "\n}\n";
}

}  // namespace

PrivacyModeException::PrivacyModeException(PrivacyModeError code,
                                           const std::string &message)
    : std::runtime_error(message), code_(code) {}

PrivacyModeError PrivacyModeException::code() const noexcept { return code_; }

PrivacyModeRuntime::PrivacyModeRuntime(XdgPaths paths)
    : paths_(std::move(paths)) {}

void PrivacyModeRuntime::refresh() noexcept {
  PrivacyModeRuntimeSnapshot next;
  try {
    const PrivacyModeState persisted = readPrivacyMode(paths_);
    next.enabled = persisted.enabled;
    next.status = PrivacyModeRuntimeStatus::Ready;
  } catch (const PrivacyModeException &error) {
    next.enabled = true;
    switch (error.code()) {
      case PrivacyModeError::InvalidFormat:
        next.status = PrivacyModeRuntimeStatus::InvalidFormat;
        break;
      case PrivacyModeError::UnsafeFile:
        next.status = PrivacyModeRuntimeStatus::UnsafeFile;
        break;
      case PrivacyModeError::IoFailure:
        next.status = PrivacyModeRuntimeStatus::IoFailure;
        break;
    }
  } catch (...) {
    next.enabled = true;
    next.status = PrivacyModeRuntimeStatus::IoFailure;
  }
  snapshot_ = next;
}

void PrivacyModeRuntime::markMonitorUnavailable() noexcept {
  snapshot_ = PrivacyModeRuntimeSnapshot{
      true, PrivacyModeRuntimeStatus::MonitorUnavailable};
}

const PrivacyModeRuntimeSnapshot &PrivacyModeRuntime::snapshot()
    const noexcept {
  return snapshot_;
}

const char *privacyModeRuntimeStatusCode(
    PrivacyModeRuntimeStatus status) noexcept {
  switch (status) {
    case PrivacyModeRuntimeStatus::Ready:
      return "ready";
    case PrivacyModeRuntimeStatus::InvalidFormat:
      return "invalid_format";
    case PrivacyModeRuntimeStatus::UnsafeFile:
      return "unsafe_file";
    case PrivacyModeRuntimeStatus::IoFailure:
      return "io_failure";
    case PrivacyModeRuntimeStatus::MonitorUnavailable:
      return "monitor_unavailable";
  }
  return "io_failure";
}

PrivacyModeState readPrivacyMode(const XdgPaths &paths) {
  int descriptor =
      open(paths.privacy_path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0) {
    if (errno == ENOENT) {
      return PrivacyModeState{};
    }
    if (errno == ELOOP) {
      throw PrivacyModeException(PrivacyModeError::UnsafeFile,
                                 "privacy file must not be a symlink");
    }
    ioFailure("privacy file could not be opened");
  }

  try {
    validateOpenedPrivacyFile(descriptor, paths.owner_id);
    const std::string contents = readAll(descriptor);
    const PrivacyModeState state = parsePrivacyMode(contents);
    closeOrThrow(descriptor);
    return state;
  } catch (...) {
    closeIgnoringErrors(descriptor);
    throw;
  }
}

void writePrivacyMode(const XdgPaths &paths, bool enabled) {
  preparePrivateProductPaths(paths);
  static_cast<void>(readPrivacyMode(paths));

  std::string pattern =
      (paths.config_root / ".privacy-mode.json.tmp.XXXXXX").string();
  std::vector<char> pattern_buffer(pattern.begin(), pattern.end());
  pattern_buffer.push_back('\0');
  int descriptor = mkstemp(pattern_buffer.data());
  if (descriptor < 0) {
    ioFailure("privacy temporary file could not be created");
  }
  const std::filesystem::path temporary_path(pattern_buffer.data());
  bool temporary_exists = true;

  try {
    if (fchmod(descriptor, kPrivateFileMode) != 0) {
      ioFailure("privacy temporary file permissions could not be set");
    }
    const std::string contents = serializedPrivacyMode(enabled);
    writeAll(descriptor, contents);
    if (fsync(descriptor) != 0) {
      ioFailure("privacy temporary file could not be synchronized");
    }
    closeOrThrow(descriptor);
    if (rename(temporary_path.c_str(), paths.privacy_path.c_str()) != 0) {
      ioFailure("privacy file could not be replaced atomically");
    }
    temporary_exists = false;
    syncDirectory(paths.config_root);
  } catch (...) {
    closeIgnoringErrors(descriptor);
    if (temporary_exists) {
      unlink(temporary_path.c_str());
    }
    throw;
  }

  if (!(readPrivacyMode(paths) == PrivacyModeState{true, enabled})) {
    ioFailure("privacy file read-back did not match");
  }
}

void restorePrivacyMode(const XdgPaths &paths, PrivacyModeState state) {
  if (state.present) {
    writePrivacyMode(paths, state.enabled);
    return;
  }

  preparePrivateProductPaths(paths);
  const PrivacyModeState current = readPrivacyMode(paths);
  if (!current.present) {
    return;
  }
  if (unlink(paths.privacy_path.c_str()) != 0) {
    ioFailure("privacy file could not be removed during rollback");
  }
  syncDirectory(paths.config_root);
  if (readPrivacyMode(paths).present) {
    ioFailure("privacy file rollback did not persist");
  }
}

}  // namespace radishlex::linux_platform
