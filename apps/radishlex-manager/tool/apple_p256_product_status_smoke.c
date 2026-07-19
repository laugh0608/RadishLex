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
  RadishLexAppleP256ProductStatus secure_enclave_status = {0};
  RadishLexAppleSecureEnclaveKeyAgreementProductStatus key_agreement_status = {0};
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
  failed |= require_flag("exportable", status.exportable, 1u);
  failed |= require_flag("hardware_backed", status.hardware_backed, 0u);
  failed |= require_flag("user_presence_required",
                         status.user_presence_required, 0u);
  failed |= require_flag("backup_migratable", status.backup_migratable, 0u);
  if (failed != 0) {
    return 1;
  }

  if (radishlex_apple_secure_enclave_p256_product_status(
          &secure_enclave_status) != RADISHLEX_APPLE_P256_SMOKE_PASSED) {
    fputs("Apple Secure Enclave P-256 product status query failed\n", stderr);
    return 1;
  }
  failed |= require_flag(
      "secure_enclave.version", secure_enclave_status.version,
      RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_STATUS_VERSION);
  failed |= require_flag("secure_enclave.compiled",
                         secure_enclave_status.compiled, 1u);
  failed |= require_flag("secure_enclave.runtime_available",
                         secure_enclave_status.runtime_available, 1u);
  failed |= require_flag("secure_enclave.can_create_signing_keys",
                         secure_enclave_status.can_create_signing_keys, 1u);
  failed |= require_flag("secure_enclave.can_sign",
                         secure_enclave_status.can_sign, 1u);
  failed |= require_flag("secure_enclave.product_qualified",
                         secure_enclave_status.product_qualified, 0u);
  failed |= require_flag("secure_enclave.user_sync_enabled",
                         secure_enclave_status.user_sync_enabled, 0u);
  failed |= require_flag("secure_enclave.exportable",
                         secure_enclave_status.exportable, 0u);
  failed |= require_flag("secure_enclave.hardware_backed",
                         secure_enclave_status.hardware_backed, 1u);
  failed |= require_flag("secure_enclave.user_presence_required",
                         secure_enclave_status.user_presence_required, 0u);
  failed |= require_flag("secure_enclave.backup_migratable",
                         secure_enclave_status.backup_migratable, 0u);
  if (failed != 0) {
    return 1;
  }

  if (radishlex_apple_secure_enclave_key_agreement_product_status(
          &key_agreement_status) != RADISHLEX_APPLE_P256_SMOKE_PASSED) {
    fputs("Apple Secure Enclave key-agreement product status query failed\n",
          stderr);
    return 1;
  }
  failed |= require_flag(
      "key_agreement.version", key_agreement_status.version,
      RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_STATUS_VERSION);
  failed |= require_flag("key_agreement.compiled",
                         key_agreement_status.compiled, 1u);
  failed |= require_flag("key_agreement.runtime_qualified",
                         key_agreement_status.runtime_qualified, 0u);
  failed |= require_flag("key_agreement.product_qualified",
                         key_agreement_status.product_qualified, 0u);
  failed |= require_flag("key_agreement.user_sync_enabled",
                         key_agreement_status.user_sync_enabled, 0u);
  failed |= require_flag("key_agreement.exportable",
                         key_agreement_status.exportable, 0u);
  failed |= require_flag("key_agreement.hardware_backed",
                         key_agreement_status.hardware_backed, 0u);
  failed |= require_flag("key_agreement.user_presence_required",
                         key_agreement_status.user_presence_required, 0u);
  failed |= require_flag("key_agreement.backup_migratable",
                         key_agreement_status.backup_migratable, 0u);
  if (failed != 0) {
    return 1;
  }

  puts("Apple P-256 signing and key-agreement product status gates passed");
  return 0;
}
