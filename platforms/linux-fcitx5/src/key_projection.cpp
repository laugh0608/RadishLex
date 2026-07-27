#include "radishlex/linux/key_projection.h"

namespace radishlex::linux_platform {
namespace {

constexpr std::uint32_t kAllModifiers =
    RADISHLEX_KEY_MOD_SHIFT | RADISHLEX_KEY_MOD_CONTROL |
    RADISHLEX_KEY_MOD_ALT | RADISHLEX_KEY_MOD_META;

bool isUnicodeScalar(std::uint32_t codepoint) {
  return codepoint <= 0x10ffffU &&
         !(codepoint >= 0xd800U && codepoint <= 0xdfffU);
}

std::uint32_t namedKeyCode(PlatformNamedKey key) {
  switch (key) {
    case PlatformNamedKey::Space:
      return RADISHLEX_NAMED_KEY_SPACE;
    case PlatformNamedKey::Enter:
      return RADISHLEX_NAMED_KEY_ENTER;
    case PlatformNamedKey::Backspace:
      return RADISHLEX_NAMED_KEY_BACKSPACE;
    case PlatformNamedKey::Escape:
      return RADISHLEX_NAMED_KEY_ESCAPE;
    case PlatformNamedKey::Tab:
      return RADISHLEX_NAMED_KEY_TAB;
    case PlatformNamedKey::ArrowUp:
      return RADISHLEX_NAMED_KEY_ARROW_UP;
    case PlatformNamedKey::ArrowDown:
      return RADISHLEX_NAMED_KEY_ARROW_DOWN;
    case PlatformNamedKey::ArrowLeft:
      return RADISHLEX_NAMED_KEY_ARROW_LEFT;
    case PlatformNamedKey::ArrowRight:
      return RADISHLEX_NAMED_KEY_ARROW_RIGHT;
    case PlatformNamedKey::PageUp:
      return RADISHLEX_NAMED_KEY_PAGE_UP;
    case PlatformNamedKey::PageDown:
      return RADISHLEX_NAMED_KEY_PAGE_DOWN;
    case PlatformNamedKey::Shift:
      return RADISHLEX_NAMED_KEY_SHIFT;
    case PlatformNamedKey::Control:
      return RADISHLEX_NAMED_KEY_CONTROL;
    case PlatformNamedKey::Alt:
      return RADISHLEX_NAMED_KEY_ALT;
    case PlatformNamedKey::Meta:
      return RADISHLEX_NAMED_KEY_META;
    case PlatformNamedKey::None:
      return 0;
  }
  return 0;
}

}  // namespace

std::optional<RadishLexKeyEvent> projectKeyEvent(
    const PlatformKeyInput &input) {
  if (input.platform_reserved || (input.modifiers & ~kAllModifiers) != 0) {
    return std::nullopt;
  }
  const bool has_codepoint = input.codepoint.has_value();
  const bool has_named_key = input.named_key != PlatformNamedKey::None;
  if (has_codepoint == has_named_key) {
    return std::nullopt;
  }

  RadishLexKeyEvent output{};
  output.modifiers = input.modifiers;
  output.phase =
      input.release ? RADISHLEX_KEY_PHASE_RELEASE : RADISHLEX_KEY_PHASE_PRESS;
  if (has_codepoint) {
    if (!isUnicodeScalar(*input.codepoint)) {
      return std::nullopt;
    }
    output.key_kind = RADISHLEX_KEY_KIND_CHAR;
    output.codepoint = *input.codepoint;
  } else {
    output.key_kind = RADISHLEX_KEY_KIND_NAMED;
    output.named_key = namedKeyCode(input.named_key);
    if (output.named_key == 0) {
      return std::nullopt;
    }
  }
  return output;
}

}  // namespace radishlex::linux_platform
