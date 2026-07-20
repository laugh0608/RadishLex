#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
app_bundle="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "RadishLex manager macOS product build requires macOS." >&2
  exit 1
fi

python3 "${product_tool}" validate-source
product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"

(
  cd "${manager_dir}"
  flutter build macos --release \
    --build-name="${product_version}" \
    --build-number="${product_build}" \
    --dart-define=RADISHLEX_MANAGER_MODE=product
)

if [ ! -f "${native_library}" ]; then
  echo "manager product bundle is missing its native library." >&2
  exit 1
fi

codesign --verify --deep --strict "${app_bundle}"
file "${native_library}"
otool -L "${native_library}"
for symbol in \
  _radishlex_manager_sync_product_status \
  _radishlex_manager_sync_qualification_start \
  _radishlex_manager_sync_qualification_poll \
  _radishlex_manager_sync_qualification_cancel \
  _radishlex_manager_sync_qualification_free; do
  if ! nm -gU "${native_library}" | grep -Eq "(^|[[:space:]])${symbol}$"; then
    echo "manager product native library is missing required symbol: ${symbol}" >&2
    exit 1
  fi
done
echo "RadishLex manager product bundle: ${app_bundle}"
