#include "radishlex/linux/product_startup.h"

#include <dlfcn.h>

#include <cstdint>
#include <filesystem>
#include <type_traits>

namespace radishlex::linux_platform {
namespace {

const int kStartupComponentAnchor = 0;

template <typename Function>
const void *functionAddress(Function function) {
  static_assert(std::is_pointer_v<Function>);
  return reinterpret_cast<const void *>(
      reinterpret_cast<std::uintptr_t>(function));
}

std::filesystem::path canonicalObjectPath(const void *address) {
  Dl_info info{};
  if (dladdr(address, &info) == 0 ||
      info.dli_fname == nullptr || info.dli_fname[0] == '\0') {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup object identity is unavailable");
  }
  std::error_code error;
  const std::filesystem::path canonical =
      std::filesystem::canonical(std::filesystem::path(info.dli_fname), error);
  if (error || canonical.empty()) {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup object identity is unavailable");
  }
  return canonical;
}

std::filesystem::path expectedFfiPath(
    StartupComponent component, const std::filesystem::path &component_path) {
  std::filesystem::path expected;
  switch (component) {
    case StartupComponent::Manager:
      if (component_path.filename() != "radishlex_manager") {
        throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                           "Linux startup component identity is unavailable");
      }
      expected = component_path.parent_path() / "lib" /
                 "libradishlex_ime_ffi.so";
      break;
    case StartupComponent::FcitxAddon:
      if (component_path.filename() != "radishlex.so") {
        throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                           "Linux startup component identity is unavailable");
      }
      expected = component_path.parent_path() / "libradishlex_ime_ffi.so";
      break;
  }
  std::error_code error;
  const std::filesystem::path canonical =
      std::filesystem::canonical(expected, error);
  if (error || canonical != expected) {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup FFI identity is unavailable");
  }
  return canonical;
}

template <typename Function>
void requireSymbolOrigin(Function function,
                         const std::filesystem::path &expected_ffi) {
  if (function == nullptr ||
      canonicalObjectPath(functionAddress(function)) != expected_ffi) {
    throw StartupError(RADISHLEX_STATUS_INVALID_STATE, 0, 0, 0,
                       "Linux startup FFI identity changed");
  }
}

void validateStartupApiOrigins(const StartupGateApi &api,
                               const std::filesystem::path &expected_ffi) {
  requireSymbolOrigin(api.startup_gate, expected_ffi);
  requireSymbolOrigin(api.error_message, expected_ffi);
  requireSymbolOrigin(api.error_free, expected_ffi);
}

void validateFcitxApiOrigins(const std::filesystem::path &expected_ffi) {
#define RADISHLEX_REQUIRE_FFI_ORIGIN(symbol) \
  requireSymbolOrigin(symbol, expected_ffi)
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_ffi_contract);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_new_personalized_rime);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_free);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_rime_runtime_shutdown);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_engine_kind);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_reset);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_set_learning_context);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_handle_key_event);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_session_select_candidate);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_version);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_consumed);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_commit);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_commit_present);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_learning_disposition);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_snapshot);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_key_result_free);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_schema);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_preedit);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_cursor);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_candidate_count);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_personalization_status);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_snapshot_candidate);
  RADISHLEX_REQUIRE_FFI_ORIGIN(radishlex_error_code);
#undef RADISHLEX_REQUIRE_FFI_ORIGIN
}

}  // namespace

StartupPermit authorizeLinkedStartup(StartupComponent component) {
  const std::filesystem::path component_path =
      canonicalObjectPath(static_cast<const void *>(&kStartupComponentAnchor));
  const std::filesystem::path expected_ffi =
      expectedFfiPath(component, component_path);
  const StartupGateApi api{
      radishlex_linux_product_startup_gate,
      radishlex_error_message,
      radishlex_error_free,
  };
  validateStartupApiOrigins(api, expected_ffi);
  if (component == StartupComponent::FcitxAddon) {
    validateFcitxApiOrigins(expected_ffi);
  }
  return authorizeStartup(component, component_path.string(), api);
}

}  // namespace radishlex::linux_platform
