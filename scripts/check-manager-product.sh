#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
app_bundle="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-manager-product-smoke.XXXXXX")"
m2_cleanup="${repo_root}/platforms/macos-imk/cleanup-m2-manager-test-data.sh"
m2_cleanup_wrapper="${repo_root}/scripts/cleanup-macos-m2-manager-test-data.sh"
m2_cleanup_source="${repo_root}/platforms/macos-imk/Tools/test_data_cleanup.c"
m2_cleanup_helper_contract="${repo_root}/platforms/macos-imk/Tests/m2_manager_test_data_cleanup_helper_contract.sh"
m2_cleanup_orchestration_contract="${repo_root}/platforms/macos-imk/Tests/m2_manager_test_data_cleanup_orchestration_contract.sh"
manager_window="${manager_dir}/macos/Runner/MainFlutterWindow.swift"
apple_status_smoke="${manager_dir}/tool/apple_p256_product_status_smoke.c"
apple_status_smoke_binary="${smoke_dir}/apple-p256-product-status-smoke"
apple_product_smoke="${repo_root}/scripts/run-manager-apple-keychain-p256-product-smoke.sh"
secure_enclave_product_smoke="${repo_root}/scripts/run-manager-apple-secure-enclave-p256-product-smoke.sh"
apple_qualified_build="${repo_root}/scripts/build-manager-macos-dpk-qualified-product.sh"
apple_qualified_entitlements="${manager_dir}/macos/Runner/DPKQualification.entitlements"
manager_app_delegate="${manager_dir}/macos/Runner/AppDelegate.swift"

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

if [ "$(uname -s)" != "Darwin" ]; then
  echo "RadishLex manager product smoke requires macOS." >&2
  exit 1
fi

bash -n "${m2_cleanup}" "${m2_cleanup_wrapper}" "${apple_product_smoke}" \
  "${secure_enclave_product_smoke}" \
  "${apple_qualified_build}" \
  "${m2_cleanup_helper_contract}" "${m2_cleanup_orchestration_contract}"
rg -Fq '"${1:-}" != "--authorized-apple-development-provisioning-build"' \
  "${apple_qualified_build}"
rg -Fq -- '-allowProvisioningUpdates' "${apple_qualified_build}"
rg -Fq 'embedded.provisionprofile' "${apple_qualified_build}"
rg -Fq 'com.apple.application-identifier' "${apple_qualified_build}"
rg -Fq 'CODE_SIGN_ENTITLEMENTS="Runner/DPKQualification.entitlements"' \
  "${apple_qualified_build}"
plutil -lint "${apple_qualified_entitlements}" >/dev/null
rg -Fq '<key>keychain-access-groups</key>' "${apple_qualified_entitlements}"
rg -Fq '$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)' \
  "${apple_qualified_entitlements}"
rg -Fq 'RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE=1' "${apple_product_smoke}"
rg -Fq 'RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_P256_SMOKE=1' \
  "${secure_enclave_product_smoke}"
rg -Fq 'locked_probe_delay_seconds=20' "${secure_enclave_product_smoke}"
rg -Fq 'sleep "${locked_probe_delay_seconds}"' "${secure_enclave_product_smoke}"
for authorization_argument in \
  --authorized-product-keychain-smoke \
  --authorized-product-keychain-denied-probe \
  --authorized-product-keychain-locked-prepare \
  --authorized-product-keychain-locked-probe \
  --authorized-product-keychain-locked-cleanup; do
  rg -Fq -- "${authorization_argument}" "${apple_product_smoke}"
done
for authorization_argument in \
  --authorized-secure-enclave-product-smoke \
  --authorized-secure-enclave-denied-probe \
  --authorized-secure-enclave-locked-prepare \
  --authorized-secure-enclave-locked-probe \
  --authorized-secure-enclave-locked-cleanup \
  --authorized-secure-enclave-unsupported-probe; do
  rg -Fq -- "${authorization_argument}" "${secure_enclave_product_smoke}"
done
for product_argument in \
  --radishlex-apple-p256-product-smoke \
  --radishlex-apple-p256-denied-probe \
  --radishlex-apple-p256-locked-prepare \
  --radishlex-apple-p256-locked-probe \
  --radishlex-apple-p256-locked-cleanup; do
  rg -Fq -- "${product_argument}" "${apple_product_smoke}" "${manager_app_delegate}"
done
for product_argument in \
  --radishlex-apple-secure-enclave-p256-product-smoke \
  --radishlex-apple-secure-enclave-p256-denied-probe \
  --radishlex-apple-secure-enclave-p256-locked-prepare \
  --radishlex-apple-secure-enclave-p256-locked-probe \
  --radishlex-apple-secure-enclave-p256-locked-cleanup \
  --radishlex-apple-secure-enclave-p256-unsupported-probe; do
  rg -Fq -- "${product_argument}" "${secure_enclave_product_smoke}" \
    "${manager_app_delegate}"
done
rg -Fq 'RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE' "${manager_app_delegate}"
rg -Fq 'RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_P256_SMOKE' \
  "${manager_app_delegate}"
if rg -n 'security[[:space:]]+(lock|unlock)-keychain|security[[:space:]]+list-keychains' \
  "${apple_product_smoke}" "${secure_enclave_product_smoke}" \
  "${apple_qualified_build}"; then
  echo "Apple P-256 product smoke must not change Keychain lock or search-list state." >&2
  exit 1
fi
rg -Fq '"${1}" != "--authorized-delete-m2-manager-test-data"' \
  "${m2_cleanup_wrapper}"
rg -Fxq 'exec "${repo_root}/platforms/macos-imk/cleanup-m2-manager-test-data.sh" "$1"' \
  "${m2_cleanup_wrapper}"
rg -Fq '"manager-settings.json"' "${m2_cleanup_source}"
rg -Fq '"manager-settings.json.tmp"' "${m2_cleanup_source}"
rg -Fq 'unlinkat(parent_fd, kTestDataNames[index], 0)' \
  "${m2_cleanup_source}"
rg -Fq '_ = umask(0o077)' "${manager_window}"
if rg -n '^[[:space:]]*rm[[:space:]]|rm -rf|find .*-(delete|exec)|(^|[[:space:]])xargs([[:space:]]|$)' \
  "${m2_cleanup}" "${m2_cleanup_wrapper}"; then
  echo "M2 manager test-data cleanup must not expose general path deletion." >&2
  exit 1
fi
clang -std=c11 -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  -DRLX_TEST_DATA_PROFILE_MANAGER=1 \
  '-DRLX_TEST_DATA_STATE_DIR="/private/tmp/radishlex-m2-manager-contract-state"' \
  "${m2_cleanup_source}"
"${m2_cleanup_helper_contract}"
"${m2_cleanup_orchestration_contract}"

(
  cd "${manager_dir}"
  flutter build macos --release --dart-define=RADISHLEX_MANAGER_MODE=product
)

if [ ! -f "${native_library}" ]; then
  echo "manager product bundle is missing its native library." >&2
  exit 1
fi

if codesign -d --entitlements :- "${app_bundle}" 2>&1 | grep -Fq "com.apple.security.app-sandbox"; then
  echo "M2 manager product bundle unexpectedly enables App Sandbox." >&2
  exit 1
fi

codesign --verify --deep --strict "${app_bundle}"
for symbol in \
  _radishlex_apple_p256_product_status \
  _radishlex_apple_p256_product_smoke \
  _radishlex_apple_secure_enclave_p256_product_status \
  _radishlex_apple_secure_enclave_p256_product_smoke; do
  if ! nm -gU "${native_library}" | grep -Eq "(^|[[:space:]])${symbol}$"; then
    echo "manager product native library is missing required symbol: ${symbol}" >&2
    exit 1
  fi
done
if rg -n 'radishlex_apple_(p256|secure_enclave_p256)_product_(smoke|status)' \
  "${manager_dir}/lib" "${manager_dir}/tool/ffi_bridge_smoke.dart"; then
  echo "Apple P-256 product validation ABI must not be bound by Dart." >&2
  exit 1
fi
clang -std=c11 -Wall -Wextra -Werror -pedantic \
  -I "${repo_root}/crates/ime-ffi/include" \
  "${apple_status_smoke}" "${native_library}" \
  -Wl,-rpath,"$(dirname "${native_library}")" \
  -o "${apple_status_smoke_binary}"
"${apple_status_smoke_binary}"
(
  cd "${manager_dir}"
  dart run tool/ffi_bridge_smoke.dart \
    --library "${native_library}" \
    --work-dir "${smoke_dir}"
)

echo "RadishLex manager product bundle smoke passed without launching the GUI."
