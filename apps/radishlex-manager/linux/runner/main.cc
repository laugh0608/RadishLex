#include "my_application.h"

#include <sys/stat.h>

int main(int argc, char** argv) {
  umask(0077);
  g_autoptr(MyApplication) app = my_application_new();
  return g_application_run(G_APPLICATION(app), argc, argv);
}
