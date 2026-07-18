#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
app_bundle="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-manager-product-smoke.XXXXXX")"

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

if [ "$(uname -s)" != "Darwin" ]; then
  echo "RadishLex manager product smoke requires macOS." >&2
  exit 1
fi

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
