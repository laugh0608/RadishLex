#include "radishlex/linux/product_startup.h"

#include <cstdlib>
#include <iostream>
#include <string>

namespace {

using radishlex::linux_platform::StartupBuildIdentity;
using radishlex::linux_platform::StartupComponent;
using radishlex::linux_platform::StartupError;
using radishlex::linux_platform::StartupGateApi;

std::uint32_t decision = 0;
std::uint32_t reason = 0;
std::uint32_t receipt_state = 0;
std::uint32_t result_version = RADISHLEX_LINUX_PRODUCT_STARTUP_RESULT_VERSION;
std::size_t gate_calls = 0;
std::uint32_t observed_build_identity = 0;
std::uint32_t observed_component = 0;

RadishLexStatusCode fakeGate(
    const RadishLexLinuxProductStartupRequest *request,
    RadishLexLinuxProductStartupResult *result, RadishLexError **) {
  ++gate_calls;
  observed_build_identity = request->build_identity;
  observed_component = request->component;
  *result = RadishLexLinuxProductStartupResult{
      result_version,
      decision,
      reason,
      receipt_state,
  };
  return RADISHLEX_STATUS_OK;
}

const char *fakeErrorMessage(const RadishLexError *) { return "fake error"; }

void fakeErrorFree(RadishLexError *) {}

StartupGateApi fakeApi() {
  return StartupGateApi{fakeGate, fakeErrorMessage, fakeErrorFree};
}

void require(bool condition, const std::string &message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}

template <typename Callback>
void requireRejected(Callback callback, const std::string &message) {
  try {
    callback();
  } catch (const StartupError &) {
    return;
  }
  require(false, message);
}

}  // namespace

int main() {
  const StartupBuildIdentity build_identity =
      radishlex::linux_platform::compiledStartupBuildIdentity();
  const std::string build_identity_name =
      radishlex::linux_platform::compiledStartupBuildIdentityName();
  if (build_identity == StartupBuildIdentity::DevelopmentStaged) {
    decision = RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT;
    reason = RADISHLEX_LINUX_STARTUP_REASON_DEVELOPMENT_STATE_ABSENT;
    receipt_state = RADISHLEX_LINUX_STARTUP_RECEIPT_NONE;
  } else {
    decision = RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT;
    reason = RADISHLEX_LINUX_STARTUP_REASON_INSTALLED_RECEIPT_VERIFIED;
    receipt_state = RADISHLEX_LINUX_STARTUP_RECEIPT_COMPLETED;
  }
  require(
      build_identity_name ==
          (build_identity == StartupBuildIdentity::DevelopmentStaged
               ? "radishlex-linux-startup:development-staged-v1"
               : "radishlex-linux-startup:debian-system-product-v1"),
      "startup build identity name differs from compiled identity");

  auto permit = radishlex::linux_platform::authorizeStartup(
      StartupComponent::Manager, "/synthetic/component", fakeApi());
  (void)permit;
  require(gate_calls == 1, "startup gate must run exactly once");
  require(observed_build_identity == static_cast<std::uint32_t>(build_identity),
          "startup gate build identity differs from compiled identity");
  require(observed_component == RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER,
          "startup gate component identity differs");

  decision = RADISHLEX_LINUX_STARTUP_FAILED_CLOSED;
  reason = RADISHLEX_LINUX_STARTUP_REASON_COMPONENT_IDENTITY_CHANGED;
  requireRejected(
      [] {
        auto rejected = radishlex::linux_platform::authorizeStartup(
            StartupComponent::FcitxAddon, "/synthetic/component", fakeApi());
        (void)rejected;
      },
      "failed-closed startup decision must not create a permit");

  decision = build_identity == StartupBuildIdentity::DevelopmentStaged
                 ? RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT
                 : RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT;
  reason = build_identity == StartupBuildIdentity::DevelopmentStaged
               ? RADISHLEX_LINUX_STARTUP_REASON_INSTALLED_RECEIPT_VERIFIED
               : RADISHLEX_LINUX_STARTUP_REASON_DEVELOPMENT_STATE_ABSENT;
  receipt_state = build_identity == StartupBuildIdentity::DevelopmentStaged
                      ? RADISHLEX_LINUX_STARTUP_RECEIPT_COMPLETED
                      : RADISHLEX_LINUX_STARTUP_RECEIPT_NONE;
  requireRejected(
      [] {
        auto rejected = radishlex::linux_platform::authorizeStartup(
            StartupComponent::Manager, "/synthetic/component", fakeApi());
        (void)rejected;
      },
      "opposite build identity decision must not create a permit");

  result_version = 99;
  requireRejected(
      [] {
        auto rejected = radishlex::linux_platform::authorizeStartup(
            StartupComponent::Manager, "/synthetic/component", fakeApi());
        (void)rejected;
      },
      "unknown startup result version must fail closed");
  return 0;
}
