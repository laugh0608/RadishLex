#include "radishlex_input.h"

#include <stdio.h>

static int require_flag(const char *name, uint32_t actual, uint32_t expected) {
  if (actual == expected) {
    return 0;
  }
  fprintf(stderr, "Apple P-256 product status mismatch: %s\n", name);
  return 1;
}

int main(void) {
  RadishLexAppleP256ProductStatus status = {0};
  if (radishlex_apple_p256_product_status(&status) !=
      RADISHLEX_APPLE_P256_SMOKE_PASSED) {
    fputs("Apple P-256 product status query failed\n", stderr);
    return 1;
  }

  int failed = 0;
  failed |= require_flag("version", status.version,
                         RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION);
  failed |= require_flag("compiled", status.compiled, 1u);
  failed |= require_flag("runtime_available", status.runtime_available, 1u);
  failed |= require_flag("can_create_signing_keys",
                         status.can_create_signing_keys, 1u);
  failed |= require_flag("can_sign", status.can_sign, 1u);
  failed |= require_flag("product_qualified", status.product_qualified, 0u);
  failed |= require_flag("user_sync_enabled", status.user_sync_enabled, 0u);
  failed |= require_flag("exportable", status.exportable, 0u);
  failed |= require_flag("hardware_backed", status.hardware_backed, 0u);
  failed |= require_flag("user_presence_required",
                         status.user_presence_required, 0u);
  failed |= require_flag("backup_migratable", status.backup_migratable, 0u);
  if (failed != 0) {
    return 1;
  }

  puts("Apple P-256 product status is runtime-capable and product-gated");
  return 0;
}
