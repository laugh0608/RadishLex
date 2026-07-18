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

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

if [ "$(uname -s)" != "Darwin" ]; then
  echo "RadishLex manager product smoke requires macOS." >&2
  exit 1
fi

bash -n "${m2_cleanup}" "${m2_cleanup_wrapper}" \
  "${m2_cleanup_helper_contract}" "${m2_cleanup_orchestration_contract}"
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
(
  cd "${manager_dir}"
  dart run tool/ffi_bridge_smoke.dart \
    --library "${native_library}" \
    --work-dir "${smoke_dir}"
)

echo "RadishLex manager product bundle smoke passed without launching the GUI."
