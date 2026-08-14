#ifndef RADISHLEX_LINUX_PRODUCT_STARTUP_H
#define RADISHLEX_LINUX_PRODUCT_STARTUP_H

#include <cstdint>
#include <stdexcept>
#include <string>

#include "radishlex_input.h"

namespace radishlex::linux_platform {

enum class StartupBuildIdentity : std::uint32_t {
  DevelopmentStaged = RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED,
  DebianSystemProduct = RADISHLEX_LINUX_STARTUP_BUILD_DEBIAN_SYSTEM_PRODUCT,
};

enum class StartupComponent : std::uint32_t {
  Manager = RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER,
  FcitxAddon = RADISHLEX_LINUX_STARTUP_COMPONENT_FCITX_ADDON,
};

struct StartupGateApi {
  decltype(&radishlex_linux_product_startup_gate) startup_gate;
  decltype(&radishlex_error_message) error_message;
  decltype(&radishlex_error_free) error_free;
};

class StartupError final : public std::runtime_error {
 public:
  StartupError(RadishLexStatusCode status, std::uint32_t decision,
               std::uint32_t reason, std::uint32_t receipt_state,
               const std::string &message);

  RadishLexStatusCode status() const noexcept;
  std::uint32_t decision() const noexcept;
  std::uint32_t reason() const noexcept;
  std::uint32_t receiptState() const noexcept;

 private:
  RadishLexStatusCode status_;
  std::uint32_t decision_;
  std::uint32_t reason_;
  std::uint32_t receipt_state_;
};

class StartupPermit final {
 public:
  StartupPermit(const StartupPermit &) = delete;
  StartupPermit &operator=(const StartupPermit &) = delete;
  StartupPermit(StartupPermit &&) noexcept = default;
  StartupPermit &operator=(StartupPermit &&) noexcept = default;

 private:
  StartupPermit() = default;

  friend StartupPermit authorizeStartup(StartupComponent component,
                                         const std::string &component_path,
                                         const StartupGateApi &api);
};

StartupBuildIdentity compiledStartupBuildIdentity() noexcept;
const char *compiledStartupBuildIdentityName() noexcept;

StartupPermit authorizeStartup(StartupComponent component,
                               const std::string &component_path,
                               const StartupGateApi &api);

StartupPermit authorizeLinkedStartup(StartupComponent component);

}  // namespace radishlex::linux_platform

#endif
