#include "manager_runtime_bridge.h"

#include <cstring>
#include <exception>
#include <string>

#include "radishlex/linux/manager_runtime.h"
#include "radishlex/linux/privacy_mode.h"
#include "radishlex/linux/xdg_paths.h"

namespace radishlex::manager_linux {
namespace {

constexpr const char *kChannelName = "dev.radishlex.manager/runtime";

FlMethodResponse *success(FlValue *value = nullptr) {
  return FL_METHOD_RESPONSE(fl_method_success_response_new(value));
}

FlMethodResponse *failure(const char *code, const char *message) {
  return FL_METHOD_RESPONSE(
      fl_method_error_response_new(code, message, nullptr));
}

FlValue *privacyResult(
    const radishlex::linux_platform::PrivacyModeState &state) {
  FlValue *value = fl_value_new_map();
  fl_value_set_string_take(value, "present", fl_value_new_bool(state.present));
  fl_value_set_string_take(value, "enabled", fl_value_new_bool(state.enabled));
  return value;
}

radishlex::linux_platform::XdgPaths preparedXdgPaths() {
  auto paths = radishlex::linux_platform::resolveProductionXdgPaths();
  radishlex::linux_platform::secureManagerLocalFiles(paths);
  return paths;
}

bool requiredBoolean(FlMethodCall *method_call, const char *key, bool *value) {
  FlValue *arguments = fl_method_call_get_args(method_call);
  if (arguments == nullptr || fl_value_get_type(arguments) != FL_VALUE_TYPE_MAP) {
    return false;
  }
  FlValue *entry = fl_value_lookup_string(arguments, key);
  if (entry == nullptr || fl_value_get_type(entry) != FL_VALUE_TYPE_BOOL) {
    return false;
  }
  *value = fl_value_get_bool(entry);
  return true;
}

FlMethodResponse *privacyFailureFor(const char *method) {
  if (std::strcmp(method, "writePrivacyMode") == 0) {
    return failure("privacy_write_failed",
                   "Linux privacy setting could not be updated");
  }
  if (std::strcmp(method, "restorePrivacyModeState") == 0) {
    return failure("privacy_rollback_failed",
                   "Linux privacy setting could not be restored");
  }
  return failure("privacy_read_failed",
                 "Linux privacy setting could not be read");
}

}  // namespace

ManagerRuntimeBridge::ManagerRuntimeBridge(FlBinaryMessenger *messenger)
    : channel_(nullptr) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  channel_ = fl_method_channel_new(messenger, kChannelName,
                                   FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      channel_, ManagerRuntimeBridge::handleMethodCall, this, nullptr);
}

ManagerRuntimeBridge::~ManagerRuntimeBridge() {
  if (channel_ != nullptr) {
    fl_method_channel_set_method_call_handler(channel_, nullptr, nullptr,
                                              nullptr);
    g_clear_object(&channel_);
  }
}

void ManagerRuntimeBridge::handleMethodCall(FlMethodChannel *channel,
                                            FlMethodCall *method_call,
                                            gpointer user_data) {
  static_cast<void>(channel);
  auto *self = static_cast<ManagerRuntimeBridge *>(user_data);
  g_autoptr(FlMethodResponse) response = self->handle(method_call);
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond(method_call, response, &error)) {
    g_warning("RadishLex Linux runtime bridge response failed");
  }
}

FlMethodResponse *ManagerRuntimeBridge::handle(FlMethodCall *method_call) {
  const char *method = fl_method_call_get_name(method_call);
  try {
    if (std::strcmp(method, "resolveProductPaths") == 0) {
      const auto paths =
          radishlex::linux_platform::resolveManagerRuntimePaths();
      g_autoptr(FlValue) result = fl_value_new_map();
      fl_value_set_string_take(
          result, "userDbPath",
          fl_value_new_string(paths.xdg.userdb_path.c_str()));
      fl_value_set_string_take(
          result, "settingsFilePath",
          fl_value_new_string(paths.xdg.settings_path.c_str()));
      fl_value_set_string_take(
          result, "nativeLibraryPath",
          fl_value_new_string(paths.native_library_path.c_str()));
      return success(result);
    }

    if (std::strcmp(method, "readPrivacyModeState") == 0) {
      const auto state =
          radishlex::linux_platform::readPrivacyMode(preparedXdgPaths());
      g_autoptr(FlValue) result = privacyResult(state);
      return success(result);
    }

    if (std::strcmp(method, "writePrivacyMode") == 0) {
      bool enabled = false;
      if (!requiredBoolean(method_call, "enabled", &enabled)) {
        return failure("invalid_argument",
                       "privacy mode requires a boolean enabled value");
      }
      const auto paths = preparedXdgPaths();
      radishlex::linux_platform::writePrivacyMode(paths, enabled);
      g_autoptr(FlValue) result = privacyResult(
          radishlex::linux_platform::readPrivacyMode(paths));
      return success(result);
    }

    if (std::strcmp(method, "restorePrivacyModeState") == 0) {
      bool present = false;
      bool enabled = false;
      if (!requiredBoolean(method_call, "present", &present) ||
          !requiredBoolean(method_call, "enabled", &enabled) ||
          (!present && enabled)) {
        return failure("invalid_argument",
                       "privacy rollback requires a consistent prior state");
      }
      const auto paths = preparedXdgPaths();
      radishlex::linux_platform::restorePrivacyMode(
          paths, radishlex::linux_platform::PrivacyModeState{present, enabled});
      g_autoptr(FlValue) result = privacyResult(
          radishlex::linux_platform::readPrivacyMode(paths));
      return success(result);
    }

    if (std::strcmp(method, "secureLocalFiles") == 0) {
      static_cast<void>(preparedXdgPaths());
      return success();
    }

    return FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  } catch (const radishlex::linux_platform::ManagerRuntimeException &error) {
    if (error.code() ==
        radishlex::linux_platform::ManagerRuntimeError::NativeLibraryMissing) {
      return failure("ffi_library_load_failed",
                     "bundled RadishLex native library is missing");
    }
    return failure("platform_paths_unavailable",
                   "Linux product paths are unavailable");
  } catch (const radishlex::linux_platform::PrivacyModeException &) {
    return privacyFailureFor(method);
  } catch (const radishlex::linux_platform::XdgPathException &) {
    return failure("local_file_permissions_failed",
                   "Linux product paths or permissions are unsafe");
  } catch (const std::exception &) {
    return failure("platform_bridge_error",
                   "Linux manager runtime operation failed");
  }
}

}  // namespace radishlex::manager_linux
