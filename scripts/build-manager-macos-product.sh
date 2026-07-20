#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
app_bundle="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "RadishLex manager macOS product build requires macOS." >&2
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

codesign --verify --deep --strict "${app_bundle}"
file "${native_library}"
otool -L "${native_library}"
echo "RadishLex manager product bundle: ${app_bundle}"
