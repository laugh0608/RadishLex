#include "my_application.h"

#include <cstdio>
#include <sys/stat.h>

#include "radishlex/linux/product_startup.h"

int main(int argc, char** argv) {
  umask(0077);
  try {
    auto startup_permit =
        radishlex::linux_platform::authorizeLinkedStartup(
            radishlex::linux_platform::StartupComponent::Manager);
    (void)startup_permit;
  } catch (const radishlex::linux_platform::StartupError& error) {
    std::fprintf(stderr,
                 "radishlex_linux_startup_blocked status=%u decision=%u "
                 "reason=%u receipt_state=%u\n",
                 static_cast<unsigned int>(error.status()), error.decision(),
                 error.reason(), error.receiptState());
    return 1;
  }
  g_autoptr(MyApplication) app = my_application_new();
  return g_application_run(G_APPLICATION(app), argc, argv);
}
