#include "fcitx_candidate_key.h"

#include <fcitx-utils/keysym.h>

namespace radishlex::linux_fcitx5 {

bool candidateListHandlesKey(const fcitx::Key &key) {
  // Fcitx matching intentionally tolerates lock and internal delivery flags.
  // A raw states == 0 check would bypass candidate selection for valid GTK
  // events and send Space through the engine commit path without learning.
  constexpr fcitx::KeyStates kCommandModifiers{
      fcitx::KeyState::Shift,  fcitx::KeyState::Ctrl,
      fcitx::KeyState::Alt,    fcitx::KeyState::Hyper,
      fcitx::KeyState::Super,  fcitx::KeyState::Super2,
      fcitx::KeyState::Hyper2, fcitx::KeyState::Meta,
  };
  if (key.states().testAny(kCommandModifiers)) {
    return false;
  }
  return key.digitSelection() >= 0 || key.check(FcitxKey_Up) ||
         key.check(FcitxKey_Down) || key.check(FcitxKey_space);
}

}  // namespace radishlex::linux_fcitx5
