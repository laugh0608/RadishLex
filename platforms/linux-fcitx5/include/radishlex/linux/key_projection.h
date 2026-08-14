#ifndef RADISHLEX_LINUX_KEY_PROJECTION_H
#define RADISHLEX_LINUX_KEY_PROJECTION_H

#include <cstdint>
#include <optional>
#include <unordered_set>

#include "radishlex_input.h"

namespace radishlex::linux_platform {

enum class PlatformNamedKey {
  None,
  Space,
  Enter,
  Backspace,
  Escape,
  Tab,
  ArrowUp,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  PageUp,
  PageDown,
  Shift,
  Control,
  Alt,
  Meta,
};

struct PlatformKeyInput {
  std::optional<std::uint32_t> codepoint;
  PlatformNamedKey named_key = PlatformNamedKey::None;
  std::uint32_t modifiers = 0;
  bool release = false;
  bool platform_reserved = false;
};

class ConsumedPressTracker {
 public:
  void recordPress(std::uint32_t key_symbol);
  bool consumeRelease(std::uint32_t key_symbol);
  void clear();

 private:
  std::unordered_set<std::uint32_t> pressed_key_symbols_;
};

std::optional<RadishLexKeyEvent> projectKeyEvent(
    const PlatformKeyInput &input);

}  // namespace radishlex::linux_platform

#endif
