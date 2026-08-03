#include "fcitx_addon.h"

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/event.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx/candidatelist.h>
#include <fcitx/event.h>
#include <fcitx/inputpanel.h>
#include <fcitx/userinterface.h>

#include <algorithm>
#include <array>
#include <exception>
#include <optional>
#include <string>
#include <utility>

#include "radishlex/linux/application_context.h"
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
#include "radishlex/linux/application_evidence.h"
#endif
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
      session_(engine.newSession()) {
  engine_.registerState(this);
}

InputContextState::~InputContextState() {
  engine_.unregisterState(this);
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
        session_->handleKeyEvent(*normalized,
                                 learningContext(
                                     engine_.synchronizedPrivacyMode()));
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
    applyResult(session_->selectCandidate(
        display_index, learningContext(engine_.synchronizedPrivacyMode())));
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

void InputContextState::applyPrivacyMode(bool enabled) {
  consumed_presses_.clear();
  const bool had_composition = session_->hasComposition();
  try {
    session_->updateLearningContext(learningContext(enabled));
    clearInputPanel();
  } catch (const ProjectionError &error) {
    FCITX_ERROR() << "radishlex_privacy_context_refresh_failed status="
                  << static_cast<int>(error.status());
    handleProjectionFailure(had_composition, nullptr);
  }
}

LearningContextProjection InputContextState::learningContext(
    bool privacy_mode) {
  const auto input = applicationContextInput(privacy_mode);
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  logApplicationEvidence(input);
#endif
  return radishlex::linux_platform::projectApplicationContext(input);
}

radishlex::linux_platform::ApplicationContextInput
InputContextState::applicationContextInput(bool privacy_mode) const {
  const fcitx::CapabilityFlags capabilities =
      input_context_.capabilityFlags();
  radishlex::linux_platform::ApplicationContextInput input;
  input.secure_input = capabilities.test(fcitx::CapabilityFlag::Password);
  input.sensitive_application =
      capabilities.test(fcitx::CapabilityFlag::Sensitive);
  input.terminal = capabilities.test(fcitx::CapabilityFlag::Terminal);
  input.privacy_mode = privacy_mode;
  if (!input.secure_input && !input.sensitive_application &&
      !input.terminal) {
    input.program = input_context_.program();
  }
  return input;
}

#if defined(RADISHLEX_APPLICATION_EVIDENCE)
void InputContextState::recordApplicationEvidence() {
  logApplicationEvidence(applicationContextInput(false));
}

void InputContextState::logApplicationEvidence(
    const radishlex::linux_platform::ApplicationContextInput &input) {
  std::string_view code;
  if (input.secure_input) {
    code = "password_program_unread";
  } else if (input.sensitive_application) {
    code = "sensitive_program_unread";
  } else if (input.terminal) {
    code = "terminal_program_unread";
  } else {
    code = radishlex::linux_platform::applicationEvidenceCandidateCode(
        radishlex::linux_platform::matchApplicationEvidenceCandidate(
            input.program));
  }
  if (logged_application_evidence_.has_value() &&
      *logged_application_evidence_ == code) {
    return;
  }
  logged_application_evidence_ = std::string(code);
  FCITX_INFO() << "radishlex_application_evidence state=" << code;
}
#endif

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
      privacy_monitor_(nullptr),
      privacy_event_source_(nullptr),
      state_factory_([this](fcitx::InputContext &input_context) {
        return new InputContextState(*this, input_context);
      }),
      next_session_id_(1) {
  radishlex::linux_platform::preparePrivateProductPaths(paths_);
  privacy_monitor_ =
      std::make_unique<radishlex::linux_platform::PrivacyModeMonitor>(paths_);
  logPrivacyStatus();
  if (privacy_monitor_->active()) {
    try {
      privacy_event_source_ = instance_->eventLoop().addIOEvent(
          privacy_monitor_->descriptor(),
          fcitx::IOEventFlags{fcitx::IOEventFlag::In,
                              fcitx::IOEventFlag::Err,
                              fcitx::IOEventFlag::Hup},
          [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags flags) {
            synchronizePrivacyMonitor(
                flags.testAny(fcitx::IOEventFlags{fcitx::IOEventFlag::Err,
                                                  fcitx::IOEventFlag::Hup}));
            return true;
          });
      if (!privacy_event_source_) {
        privacy_monitor_->markUnavailable();
        logPrivacyStatus();
      }
    } catch (...) {
      privacy_monitor_->markUnavailable();
      logPrivacyStatus();
    }
  }
  std::unique_ptr<radishlex::linux_platform::SessionProjection> probe(
      newSession());
  if (!instance_->inputContextManager().registerProperty(
          "radishlexLinuxSession", &state_factory_)) {
    throw std::runtime_error("unable to register RadishLex input context state");
  }
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  application_evidence_watcher_ = instance_->watchEvent(
      fcitx::EventType::InputContextCapabilityChanged,
      fcitx::EventWatcherPhase::Default, [](fcitx::Event &event) {
        const auto &capability_event =
            static_cast<const fcitx::CapabilityChangedEvent &>(event);
        const auto old_flags = capability_event.oldFlags();
        const auto new_flags = capability_event.newFlags();
        if (old_flags.test(fcitx::CapabilityFlag::Password) !=
            new_flags.test(fcitx::CapabilityFlag::Password)) {
          FCITX_INFO() << "radishlex_capability_evidence state=password_"
                       << (new_flags.test(fcitx::CapabilityFlag::Password)
                               ? "on"
                               : "off");
        }
        if (old_flags.test(fcitx::CapabilityFlag::Sensitive) !=
            new_flags.test(fcitx::CapabilityFlag::Sensitive)) {
          FCITX_INFO() << "radishlex_capability_evidence state=sensitive_"
                       << (new_flags.test(fcitx::CapabilityFlag::Sensitive)
                               ? "on"
                               : "off");
        }
        if (old_flags.test(fcitx::CapabilityFlag::Terminal) !=
            new_flags.test(fcitx::CapabilityFlag::Terminal)) {
          FCITX_INFO() << "radishlex_capability_evidence state=terminal_"
                       << (new_flags.test(fcitx::CapabilityFlag::Terminal)
                               ? "on"
                               : "off");
        }
      });
  if (!application_evidence_watcher_) {
    throw std::runtime_error(
        "unable to register application capability evidence watcher");
  }
#endif
}

Engine::~Engine() {
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  application_evidence_watcher_.reset();
#endif
  privacy_event_source_.reset();
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
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  auto *state = event.inputContext()->propertyFor(&state_factory_);
  state->recordApplicationEvidence();
#else
  event.inputContext()->propertyFor(&state_factory_);
#endif
}

void Engine::deactivate(const fcitx::InputMethodEntry &,
                        fcitx::InputContextEvent &event) {
  auto *state = event.inputContext()->propertyFor(&state_factory_);
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  state->recordApplicationEvidence();
#endif
  state->reset();
}

void Engine::reset(const fcitx::InputMethodEntry &,
                   fcitx::InputContextEvent &event) {
  auto *state = event.inputContext()->propertyFor(&state_factory_);
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  state->recordApplicationEvidence();
#endif
  state->reset();
}

radishlex::linux_platform::SessionProjection *Engine::newSession() {
  return new radishlex::linux_platform::SessionProjection(ffi_api_,
                                                           sessionConfig());
}

fcitx::FactoryFor<InputContextState> *Engine::stateFactory() {
  return &state_factory_;
}

bool Engine::synchronizedPrivacyMode() {
  synchronizePrivacyMonitor(false);
  if (!privacy_monitor_) {
    return true;
  }
  return privacy_monitor_->snapshot().enabled;
}

void Engine::registerState(InputContextState *state) {
  states_.push_back(state);
}

void Engine::unregisterState(InputContextState *state) {
  states_.erase(std::remove(states_.begin(), states_.end(), state),
                states_.end());
}

void Engine::synchronizePrivacyMonitor(bool event_source_failed) {
  if (!privacy_monitor_) {
    return;
  }
  const bool previous_enabled = privacy_monitor_->snapshot().enabled;
  if (event_source_failed) {
    privacy_monitor_->markUnavailable();
  } else {
    static_cast<void>(privacy_monitor_->consumeEvents());
  }
  if (!privacy_monitor_->active() && privacy_event_source_) {
    privacy_event_source_->setEnabled(false);
  }
  logPrivacyStatus();
  const bool enabled = privacy_monitor_->snapshot().enabled;
  if (previous_enabled == enabled) {
    return;
  }
  const std::vector<InputContextState *> current_states = states_;
  for (InputContextState *state : current_states) {
    if (std::find(states_.begin(), states_.end(), state) != states_.end()) {
      state->applyPrivacyMode(enabled);
    }
  }
}

void Engine::logPrivacyStatus() {
  if (!privacy_monitor_) {
    return;
  }
  const auto status = privacy_monitor_->snapshot().status;
  if (logged_privacy_status_ == status) {
    return;
  }
  const bool recovered = logged_privacy_status_.has_value() &&
                         status == radishlex::linux_platform::
                                       PrivacyModeRuntimeStatus::Ready;
  logged_privacy_status_ = status;
  if (status !=
      radishlex::linux_platform::PrivacyModeRuntimeStatus::Ready) {
    FCITX_ERROR() << "radishlex_privacy_state_failed category="
                  << radishlex::linux_platform::privacyModeRuntimeStatusCode(
                         status);
  } else if (recovered) {
    FCITX_INFO() << "radishlex_privacy_state_recovered category=ready";
  }
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
