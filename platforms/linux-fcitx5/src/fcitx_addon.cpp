#include "fcitx_addon.h"

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputpanel.h>
#include <fcitx/userinterface.h>

#include <array>
#include <optional>
#include <string>
#include <utility>

#include "radishlex/linux/key_projection.h"

namespace radishlex::linux_fcitx5 {
namespace {

using radishlex::linux_platform::CandidateProjection;
using radishlex::linux_platform::ClientPreeditProjection;
using radishlex::linux_platform::KeyResultProjection;
using radishlex::linux_platform::LearningContextProjection;
using radishlex::linux_platform::PlatformKeyInput;
using radishlex::linux_platform::PlatformNamedKey;
using radishlex::linux_platform::ProjectionError;
using radishlex::linux_platform::SnapshotProjection;

const std::array<fcitx::Key, 10> kSelectionKeys{
    fcitx::Key{FcitxKey_1}, fcitx::Key{FcitxKey_2},
    fcitx::Key{FcitxKey_3}, fcitx::Key{FcitxKey_4},
    fcitx::Key{FcitxKey_5}, fcitx::Key{FcitxKey_6},
    fcitx::Key{FcitxKey_7}, fcitx::Key{FcitxKey_8},
    fcitx::Key{FcitxKey_9}, fcitx::Key{FcitxKey_0},
};

PlatformNamedKey namedKey(fcitx::KeySym symbol) {
  switch (symbol) {
    case FcitxKey_space:
      return PlatformNamedKey::Space;
    case FcitxKey_Return:
    case FcitxKey_KP_Enter:
      return PlatformNamedKey::Enter;
    case FcitxKey_BackSpace:
      return PlatformNamedKey::Backspace;
    case FcitxKey_Escape:
      return PlatformNamedKey::Escape;
    case FcitxKey_Tab:
    case FcitxKey_ISO_Left_Tab:
      return PlatformNamedKey::Tab;
    case FcitxKey_Up:
      return PlatformNamedKey::ArrowUp;
    case FcitxKey_Down:
      return PlatformNamedKey::ArrowDown;
    case FcitxKey_Left:
      return PlatformNamedKey::ArrowLeft;
    case FcitxKey_Right:
      return PlatformNamedKey::ArrowRight;
    case FcitxKey_Page_Up:
      return PlatformNamedKey::PageUp;
    case FcitxKey_Page_Down:
      return PlatformNamedKey::PageDown;
    case FcitxKey_Shift_L:
    case FcitxKey_Shift_R:
      return PlatformNamedKey::Shift;
    case FcitxKey_Control_L:
    case FcitxKey_Control_R:
      return PlatformNamedKey::Control;
    case FcitxKey_Alt_L:
    case FcitxKey_Alt_R:
      return PlatformNamedKey::Alt;
    case FcitxKey_Meta_L:
    case FcitxKey_Meta_R:
    case FcitxKey_Super_L:
    case FcitxKey_Super_R:
      return PlatformNamedKey::Meta;
    default:
      return PlatformNamedKey::None;
  }
}

std::optional<RadishLexKeyEvent> normalizeKeyEvent(
    const fcitx::KeyEvent &event) {
  const fcitx::Key key = event.key();
  const std::uint32_t states = key.states().toInteger();
  constexpr std::uint32_t kAcceptedStates =
      static_cast<std::uint32_t>(fcitx::KeyState::Shift) |
      static_cast<std::uint32_t>(fcitx::KeyState::CapsLock) |
      static_cast<std::uint32_t>(fcitx::KeyState::Ctrl) |
      static_cast<std::uint32_t>(fcitx::KeyState::Alt) |
      static_cast<std::uint32_t>(fcitx::KeyState::NumLock) |
      static_cast<std::uint32_t>(fcitx::KeyState::Super) |
      static_cast<std::uint32_t>(fcitx::KeyState::Super2) |
      static_cast<std::uint32_t>(fcitx::KeyState::Meta) |
      static_cast<std::uint32_t>(fcitx::KeyState::Repeat);

  PlatformKeyInput input;
  input.release = event.isRelease();
  input.platform_reserved = (states & ~kAcceptedStates) != 0;
  if (key.states().test(fcitx::KeyState::Shift)) {
    input.modifiers |= RADISHLEX_KEY_MOD_SHIFT;
  }
  if (key.states().test(fcitx::KeyState::Ctrl)) {
    input.modifiers |= RADISHLEX_KEY_MOD_CONTROL;
  }
  if (key.states().test(fcitx::KeyState::Alt)) {
    input.modifiers |= RADISHLEX_KEY_MOD_ALT;
  }
  if (key.states().testAny(fcitx::KeyStates{
          fcitx::KeyState::Super, fcitx::KeyState::Super2,
          fcitx::KeyState::Meta})) {
    input.modifiers |= RADISHLEX_KEY_MOD_META;
  }

  input.named_key = namedKey(key.sym());
  if (input.named_key == PlatformNamedKey::None) {
    const std::uint32_t codepoint = fcitx::Key::keySymToUnicode(key.sym());
    if (codepoint == 0) {
      return std::nullopt;
    }
    input.codepoint = codepoint;
  }
  return radishlex::linux_platform::projectKeyEvent(input);
}

class CandidateWord final : public fcitx::CandidateWord {
 public:
  CandidateWord(InputContextState &state, const CandidateProjection &candidate)
      : state_(state), display_index_(candidate.display_index) {
    setText(fcitx::Text(candidate.text));
    std::string comment;
    if (candidate.reading.has_value()) {
      comment = *candidate.reading;
    }
    if (candidate.annotation.has_value()) {
      if (!comment.empty()) {
        comment += " · ";
      }
      comment += *candidate.annotation;
    }
    if (!comment.empty()) {
      setComment(fcitx::Text(std::move(comment)));
    }
  }

  void select(fcitx::InputContext *) const override {
    state_.selectCandidate(display_index_);
  }

 private:
  InputContextState &state_;
  std::size_t display_index_;
};

std::unique_ptr<fcitx::CommonCandidateList> candidateList(
    InputContextState &state, const SnapshotProjection &snapshot) {
  auto list = std::make_unique<fcitx::CommonCandidateList>();
  list->setSelectionKey(
      fcitx::KeyList(kSelectionKeys.begin(), kSelectionKeys.end()));
  list->setPageSize(static_cast<int>(snapshot.candidates.size()));
  list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
  for (const auto &candidate : snapshot.candidates) {
    list->append<CandidateWord>(state, candidate);
  }
  if (!snapshot.candidates.empty()) {
    list->setGlobalCursorIndex(0);
  }
  return list;
}

}  // namespace

InputContextState::InputContextState(Engine &engine,
                                     fcitx::InputContext &input_context)
    : engine_(engine),
      input_context_(input_context),
      session_(engine.newSession()) {}

InputContextState::~InputContextState() {
  try {
    session_->reset();
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_session_destroy_reset_failed status="
                  << static_cast<int>(error.status());
  }
}

void InputContextState::handleKey(fcitx::KeyEvent &event) {
  const std::uint32_t key_symbol =
      static_cast<std::uint32_t>(event.key().sym());
  if (event.isRelease() &&
      consumed_presses_.consumeRelease(key_symbol)) {
    acceptKeyEvent(event);
    return;
  }

  if (!event.isRelease()) {
    const fcitx::Key key = event.key();
    const auto list = input_context_.inputPanel().candidateList();
    if (list && key.states().toInteger() == 0) {
      const int digit = key.digitSelection();
      if (digit >= 0 && digit < list->size()) {
        acceptKeyEvent(event);
        list->candidate(digit).select(&input_context_);
        return;
      }
      if (key.check(FcitxKey_Up) || key.check(FcitxKey_Down)) {
        if (moveVisibleCursor(key.check(FcitxKey_Down))) {
          acceptKeyEvent(event);
          return;
        }
      }
      if (key.check(FcitxKey_space) && selectVisibleCursor()) {
        acceptKeyEvent(event);
        return;
      }
    }
  }

  const auto normalized = normalizeKeyEvent(event);
  if (!normalized.has_value()) {
    return;
  }
  const bool had_composition = session_->hasComposition();
  try {
    const KeyResultProjection result =
        session_->handleKeyEvent(*normalized, learningContext());
    applyResult(result);
    if (result.consumed) {
      acceptKeyEvent(event);
    }
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_key_projection_failed status="
                  << static_cast<int>(error.status());
    handleProjectionFailure(had_composition, &event);
  }
}

void InputContextState::selectCandidate(std::size_t display_index) {
  const bool had_composition = session_->hasComposition();
  try {
    applyResult(session_->selectCandidate(display_index, learningContext()));
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_candidate_projection_failed status="
                  << static_cast<int>(error.status());
    handleProjectionFailure(had_composition, nullptr);
  }
}

void InputContextState::reset() {
  consumed_presses_.clear();
  try {
    session_->reset();
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_session_reset_failed status="
                  << static_cast<int>(error.status());
  }
  clearInputPanel();
}

LearningContextProjection InputContextState::learningContext() const {
  const fcitx::CapabilityFlags capabilities =
      input_context_.capabilityFlags();
  LearningContextProjection context;
  context.secure_input =
      capabilities.test(fcitx::CapabilityFlag::Password);
  context.sensitive_application =
      capabilities.test(fcitx::CapabilityFlag::Sensitive);
  context.privacy_mode = false;
  if (capabilities.test(fcitx::CapabilityFlag::Terminal)) {
    context.context_known = true;
    context.context_kind = "terminal";
  }
  return context;
}

void InputContextState::applyResult(const KeyResultProjection &result) {
  if (result.commit.has_value()) {
    input_context_.commitString(*result.commit);
  }
  updateInputPanel(result.snapshot);
}

void InputContextState::updateInputPanel(const SnapshotProjection &snapshot) {
  fcitx::InputPanel &panel = input_context_.inputPanel();
  panel.reset();
  if (!snapshot.candidates.empty()) {
    panel.setCandidateList(candidateList(*this, snapshot));
  }
  const ClientPreeditProjection projected_preedit =
      radishlex::linux_platform::projectClientPreedit(snapshot);
  fcitx::Text preedit(
      projected_preedit.text,
      projected_preedit.prevent_commit_on_unfocus
          ? fcitx::TextFormatFlag::DontCommit
          : fcitx::TextFormatFlag::NoFlag);
  preedit.setCursor(projected_preedit.cursor);
  if (input_context_.capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) {
    panel.setClientPreedit(preedit);
  } else {
    panel.setPreedit(preedit);
  }
  input_context_.updateUserInterface(
      fcitx::UserInterfaceComponent::InputPanel);
  input_context_.updatePreedit();
}

bool InputContextState::moveVisibleCursor(bool next) {
  const auto list = input_context_.inputPanel().candidateList();
  if (!list) {
    return false;
  }
  fcitx::CursorMovableCandidateList *cursor = list->toCursorMovable();
  if (cursor == nullptr || list->empty()) {
    return false;
  }
  if (next) {
    cursor->nextCandidate();
  } else {
    cursor->prevCandidate();
  }
  input_context_.updateUserInterface(
      fcitx::UserInterfaceComponent::InputPanel);
  return true;
}

bool InputContextState::selectVisibleCursor() {
  const auto list = input_context_.inputPanel().candidateList();
  if (!list || list->empty()) {
    return false;
  }
  const int cursor = list->cursorIndex();
  if (cursor < 0 || cursor >= list->size()) {
    return false;
  }
  list->candidate(cursor).select(&input_context_);
  return true;
}

void InputContextState::acceptKeyEvent(fcitx::KeyEvent &event) {
  if (!event.isRelease()) {
    consumed_presses_.recordPress(
        static_cast<std::uint32_t>(event.key().sym()));
  }
  event.filterAndAccept();
}

void InputContextState::clearInputPanel() {
  input_context_.inputPanel().reset();
  input_context_.updateUserInterface(
      fcitx::UserInterfaceComponent::InputPanel);
  input_context_.updatePreedit();
}

void InputContextState::handleProjectionFailure(bool had_composition,
                                                fcitx::KeyEvent *event) {
  consumed_presses_.clear();
  try {
    session_->reset();
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_failure_reset_failed status="
                  << static_cast<int>(error.status());
  }
  clearInputPanel();
  if (had_composition && event != nullptr) {
    acceptKeyEvent(*event);
  }
}

Engine::Engine(fcitx::Instance *instance)
    : instance_(instance),
      ffi_api_(radishlex::linux_platform::linkedFfiApi()),
      paths_(radishlex::linux_platform::resolveProductionXdgPaths()),
      runtime_layout_(
          radishlex::linux_platform::resolveLoadedRuntimeLayout()),
      state_factory_([this](fcitx::InputContext &input_context) {
        return new InputContextState(*this, input_context);
      }),
      next_session_id_(1) {
  radishlex::linux_platform::preparePrivateProductPaths(paths_);
  std::unique_ptr<radishlex::linux_platform::SessionProjection> probe(
      newSession());
  if (!instance_->inputContextManager().registerProperty(
          "radishlexLinuxSession", &state_factory_)) {
    throw std::runtime_error("unable to register RadishLex input context state");
  }
}

Engine::~Engine() {
  state_factory_.unregister();
  try {
    radishlex::linux_platform::shutdownRimeRuntime(ffi_api_);
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_runtime_shutdown_failed status="
                  << static_cast<int>(error.status());
  }
}

void Engine::keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) {
  event.inputContext()->propertyFor(&state_factory_)->handleKey(event);
}

void Engine::activate(const fcitx::InputMethodEntry &,
                      fcitx::InputContextEvent &event) {
  event.inputContext()->propertyFor(&state_factory_);
}

void Engine::deactivate(const fcitx::InputMethodEntry &,
                        fcitx::InputContextEvent &event) {
  event.inputContext()->propertyFor(&state_factory_)->reset();
}

void Engine::reset(const fcitx::InputMethodEntry &,
                   fcitx::InputContextEvent &event) {
  event.inputContext()->propertyFor(&state_factory_)->reset();
}

radishlex::linux_platform::SessionProjection *Engine::newSession() {
  return new radishlex::linux_platform::SessionProjection(ffi_api_,
                                                           sessionConfig());
}

fcitx::FactoryFor<InputContextState> *Engine::stateFactory() {
  return &state_factory_;
}

radishlex::linux_platform::PersonalizedSessionConfig Engine::sessionConfig() {
  return radishlex::linux_platform::PersonalizedSessionConfig{
      runtime_layout_.rime_shared_data_dir.string(),
      paths_.rime_user_data_dir.string(),
      RADISHLEX_RIME_SCHEMA,
      std::nullopt,
      true,
      paths_.userdb_path.string(),
      "linux-fcitx5-" + std::to_string(next_session_id_++),
  };
}

fcitx::AddonInstance *EngineFactory::create(fcitx::AddonManager *manager) {
  return new Engine(manager->instance());
}

}  // namespace radishlex::linux_fcitx5

FCITX_ADDON_FACTORY(radishlex::linux_fcitx5::EngineFactory);
