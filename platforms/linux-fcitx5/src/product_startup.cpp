#include "radishlex/linux/product_startup.h"

#include <utility>

#ifndef RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY
#error "RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY must be defined"
#endif

#if RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY != 1 && \
    RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY != 2
#error "RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY must be 1 or 2"
#endif

namespace radishlex::linux_platform {
namespace {

#if RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY == 1
constexpr const char *kStartupBuildIdentityName =
    "radishlex-linux-startup:development-staged-v1";
#else
constexpr const char *kStartupBuildIdentityName =
    "radishlex-linux-startup:debian-system-product-v1";
#endif

class OwnedError final {
 public:
  explicit OwnedError(const StartupGateApi &api) : api_(api) {}
  ~OwnedError() {
    if (value_ != nullptr) {
      api_.error_free(value_);
    }
  }

  RadishLexError **out() { return &value_; }

  std::string message() const {
    if (value_ == nullptr) {
      return "Linux startup gate failed";
    }
    const char *message = api_.error_message(value_);
    return message == nullptr ? "Linux startup gate failed" : message;
  }

 private:
  const StartupGateApi &api_;
  RadishLexError *value_ = nullptr;
};

void validateApi(const StartupGateApi &api) {
  if (api.startup_gate == nullptr || api.error_message == nullptr ||
      api.error_free == nullptr) {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup gate API is incomplete");
  }
}

bool isExpectedAllow(const RadishLexLinuxProductStartupResult &result) {
  switch (compiledStartupBuildIdentity()) {
    case StartupBuildIdentity::DevelopmentStaged:
      return result.decision == RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT &&
             result.reason ==
                 RADISHLEX_LINUX_STARTUP_REASON_DEVELOPMENT_STATE_ABSENT &&
             result.receipt_state == RADISHLEX_LINUX_STARTUP_RECEIPT_NONE;
    case StartupBuildIdentity::DebianSystemProduct:
      return result.decision == RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT &&
             result.reason ==
                 RADISHLEX_LINUX_STARTUP_REASON_INSTALLED_RECEIPT_VERIFIED &&
             (result.receipt_state ==
                  RADISHLEX_LINUX_STARTUP_RECEIPT_COMPLETED ||
              result.receipt_state ==
                  RADISHLEX_LINUX_STARTUP_RECEIPT_ABORTED_PRESERVED ||
              result.receipt_state ==
                  RADISHLEX_LINUX_STARTUP_RECEIPT_ROLLED_BACK);
  }
  return false;
}

}  // namespace

StartupError::StartupError(RadishLexStatusCode status, std::uint32_t decision,
                           std::uint32_t reason,
                           std::uint32_t receipt_state,
                           const std::string &message)
    : std::runtime_error(message),
      status_(status),
      decision_(decision),
      reason_(reason),
      receipt_state_(receipt_state) {}

RadishLexStatusCode StartupError::status() const noexcept { return status_; }

std::uint32_t StartupError::decision() const noexcept { return decision_; }

std::uint32_t StartupError::reason() const noexcept { return reason_; }

std::uint32_t StartupError::receiptState() const noexcept {
  return receipt_state_;
}

StartupBuildIdentity compiledStartupBuildIdentity() noexcept {
#if RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY == 1
  return StartupBuildIdentity::DevelopmentStaged;
#else
  return StartupBuildIdentity::DebianSystemProduct;
#endif
}

const char *compiledStartupBuildIdentityName() noexcept {
  return kStartupBuildIdentityName;
}

StartupPermit authorizeStartup(StartupComponent component,
                               const std::string &component_path,
                               const StartupGateApi &api) {
  validateApi(api);
  if (component_path.empty() || compiledStartupBuildIdentityName()[0] == '\0') {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup component path is unavailable");
  }
  const RadishLexLinuxProductStartupRequest request{
      RADISHLEX_LINUX_PRODUCT_STARTUP_REQUEST_VERSION,
      static_cast<std::uint32_t>(compiledStartupBuildIdentity()),
      static_cast<std::uint32_t>(component),
      component_path.c_str(),
  };
  RadishLexLinuxProductStartupResult result{};
  OwnedError error(api);
  const RadishLexStatusCode status =
      api.startup_gate(&request, &result, error.out());
  if (status != RADISHLEX_STATUS_OK) {
    throw StartupError(status, result.decision, result.reason,
                       result.receipt_state, error.message());
  }
  if (result.version != RADISHLEX_LINUX_PRODUCT_STARTUP_RESULT_VERSION ||
      !isExpectedAllow(result)) {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, result.decision,
                       result.reason, result.receipt_state,
                       "Linux startup decision rejected");
  }
  return StartupPermit{};
}

}  // namespace radishlex::linux_platform
