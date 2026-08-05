#include "fcitx_candidate_key.h"

#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>

#include <cstdlib>
#include <iostream>

namespace {

using fcitx::Key;
using fcitx::KeyState;
using fcitx::KeyStates;
using radishlex::linux_fcitx5::candidateListHandlesKey;

void require(bool condition, const char *message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}

void testCandidateKeysUseFcitxMatchingSemantics() {
  require(candidateListHandlesKey(Key{FcitxKey_space}),
          "plain Space must select the visible candidate");
  require(candidateListHandlesKey(Key{FcitxKey_1}),
          "plain digit must select its visible candidate");
  require(candidateListHandlesKey(Key{FcitxKey_Up}),
          "plain Up must move the visible candidate cursor");
  require(candidateListHandlesKey(Key{FcitxKey_Down}),
          "plain Down must move the visible candidate cursor");
  require(!candidateListHandlesKey(Key{FcitxKey_a}),
          "ordinary composition keys must bypass candidate-list handling");
}

void testLockAndInternalStatesRemainCandidateKeys() {
  const KeyState tolerated_states[] = {
      KeyState::CapsLock, KeyState::NumLock, KeyState::HandledMask,
      KeyState::IgnoredMask, KeyState::Virtual, KeyState::Repeat,
  };
  for (const KeyState state : tolerated_states) {
    require(candidateListHandlesKey(
                Key{FcitxKey_space, KeyStates{state}}),
            "lock and internal delivery states must not bypass Space selection");
  }
}

void testCommandModifiersDoNotSelectCandidates() {
  const KeyState command_modifiers[] = {
      KeyState::Shift, KeyState::Ctrl, KeyState::Alt,
      KeyState::Hyper, KeyState::Hyper2, KeyState::Super,
      KeyState::Super2, KeyState::Meta,
  };
  for (const KeyState state : command_modifiers) {
    if (candidateListHandlesKey(Key{FcitxKey_space, KeyStates{state}})) {
      std::cerr << "command-modified Space must bypass candidate-list handling"
                << " state=" << static_cast<unsigned int>(state) << '\n';
      std::exit(1);
    }
  }
}

}  // namespace

int main() {
  testCandidateKeysUseFcitxMatchingSemantics();
  testLockAndInternalStatesRemainCandidateKeys();
  testCommandModifiersDoNotSelectCandidates();
  return 0;
}
