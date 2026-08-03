#ifndef RADISHLEX_MANAGER_LINUX_MANAGER_RUNTIME_BRIDGE_H_
#define RADISHLEX_MANAGER_LINUX_MANAGER_RUNTIME_BRIDGE_H_

#include <flutter_linux/flutter_linux.h>

namespace radishlex::manager_linux {

class ManagerRuntimeBridge final {
 public:
  explicit ManagerRuntimeBridge(FlBinaryMessenger *messenger);
  ~ManagerRuntimeBridge();

  ManagerRuntimeBridge(const ManagerRuntimeBridge &) = delete;
  ManagerRuntimeBridge &operator=(const ManagerRuntimeBridge &) = delete;

 private:
  static void handleMethodCall(FlMethodChannel *channel,
                               FlMethodCall *method_call,
                               gpointer user_data);

  FlMethodResponse *handle(FlMethodCall *method_call);

  FlMethodChannel *channel_;
};

}  // namespace radishlex::manager_linux

#endif
