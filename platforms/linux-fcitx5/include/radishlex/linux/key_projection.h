#ifndef RADISHLEX_LINUX_KEY_PROJECTION_H
#define RADISHLEX_LINUX_KEY_PROJECTION_H

#include <cstdint>
#include <optional>

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

std::optional<RadishLexKeyEvent> projectKeyEvent(
    const PlatformKeyInput &input);

}  // namespace radishlex::linux_platform

#endif
