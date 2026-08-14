#ifndef RADISHLEX_LINUX_FCITX_ADDON_H
#define RADISHLEX_LINUX_FCITX_ADDON_H

#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx-utils/handlertable.h>
#include <fcitx-utils/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>

#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "radishlex/linux/application_context.h"
#include "radishlex/linux/ffi_projection.h"
#include "radishlex/linux/key_projection.h"
#include "radishlex/linux/privacy_monitor.h"
#include "radishlex/linux/product_startup.h"
#include "radishlex/linux/runtime_layout.h"
#include "radishlex/linux/xdg_paths.h"

namespace radishlex::linux_fcitx5 {

class Engine;

class InputContextState final : public fcitx::InputContextProperty {
 public:
  InputContextState(Engine &engine, fcitx::InputContext &input_context);
  ~InputContextState() override;

  void handleKey(fcitx::KeyEvent &event);
  void selectCandidate(std::size_t display_index);
  void reset();
  void applyPrivacyMode(bool enabled);
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  void recordApplicationEvidence();
#endif

 private:
  radishlex::linux_platform::LearningContextProjection learningContext(
      bool privacy_mode);
  radishlex::linux_platform::ApplicationContextInput applicationContextInput(
      bool privacy_mode) const;
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  void logApplicationEvidence(
      const radishlex::linux_platform::ApplicationContextInput &input);
#endif
  void applyResult(
      const radishlex::linux_platform::KeyResultProjection &result);
  void updateInputPanel(
      const radishlex::linux_platform::SnapshotProjection &snapshot);
  bool moveVisibleCursor(bool next);
  bool selectVisibleCursor();
  void acceptKeyEvent(fcitx::KeyEvent &event);
  void clearInputPanel();
  void handleProjectionFailure(bool had_composition, fcitx::KeyEvent *event);

  Engine &engine_;
  fcitx::InputContext &input_context_;
  std::unique_ptr<radishlex::linux_platform::SessionProjection> session_;
  radishlex::linux_platform::ConsumedPressTracker consumed_presses_;
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  std::optional<std::string> logged_application_evidence_;
#endif
};

class Engine final : public fcitx::InputMethodEngineV2 {
 public:
  Engine(fcitx::Instance *instance,
         radishlex::linux_platform::StartupPermit startup_permit);
  ~Engine() override;

  void keyEvent(const fcitx::InputMethodEntry &entry,
                fcitx::KeyEvent &event) override;
  void activate(const fcitx::InputMethodEntry &entry,
                fcitx::InputContextEvent &event) override;
  void deactivate(const fcitx::InputMethodEntry &entry,
                  fcitx::InputContextEvent &event) override;
  void reset(const fcitx::InputMethodEntry &entry,
             fcitx::InputContextEvent &event) override;

  radishlex::linux_platform::SessionProjection *newSession();
  fcitx::FactoryFor<InputContextState> *stateFactory();
  bool synchronizedPrivacyMode();
  void registerState(InputContextState *state);
  void unregisterState(InputContextState *state);

 private:
  radishlex::linux_platform::PersonalizedSessionConfig sessionConfig();
  void synchronizePrivacyMonitor(bool event_source_failed);
  void logPrivacyStatus();

  fcitx::Instance *instance_;
  radishlex::linux_platform::FfiApi ffi_api_;
  radishlex::linux_platform::XdgPaths paths_;
  radishlex::linux_platform::RuntimeLayout runtime_layout_;
  std::unique_ptr<radishlex::linux_platform::PrivacyModeMonitor>
      privacy_monitor_;
  std::unique_ptr<fcitx::EventSourceIO> privacy_event_source_;
  std::optional<radishlex::linux_platform::PrivacyModeRuntimeStatus>
      logged_privacy_status_;
  fcitx::FactoryFor<InputContextState> state_factory_;
  std::vector<InputContextState *> states_;
  std::uint64_t next_session_id_;
#if defined(RADISHLEX_APPLICATION_EVIDENCE)
  std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>>
      application_evidence_watcher_;
#endif
};

class EngineFactory final : public fcitx::AddonFactory {
  fcitx::AddonInstance *create(fcitx::AddonManager *manager) override;
};

}  // namespace radishlex::linux_fcitx5

#endif
