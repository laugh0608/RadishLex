#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" != "--authorized-product-keychain-smoke" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 --authorized-product-keychain-smoke" >&2
  echo "This launches the manager product process and touches the local macOS Keychain." >&2
  exit 2
fi

if [ "$(uname -s)" != "Darwin" ]; then
  echo "manager Apple P-256 product smoke requires macOS." >&2
  exit 1
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
app_bundle="${repo_root}/apps/radishlex-manager/build/macos/Build/Products/Release/radishlex_manager.app"
product_binary="${app_bundle}/Contents/MacOS/radishlex_manager"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
go_server_dir="${repo_root}/server/sync-server"

if [ ! -x "${product_binary}" ] || [ ! -f "${native_library}" ]; then
  echo "build and verify the manager Release product before running this smoke." >&2
  exit 1
fi
if [ ! -f "${go_server_dir}/go.mod" ] || ! command -v go >/dev/null 2>&1; then
  echo "Go verifier prerequisites are unavailable." >&2
  exit 1
fi

codesign --verify --deep --strict "${app_bundle}"
for symbol in \
  _radishlex_apple_p256_product_status \
  _radishlex_apple_p256_product_smoke; do
  if ! nm -gU "${native_library}" | grep -Eq "(^|[[:space:]])${symbol}$"; then
    echo "manager product native library is missing required symbol: ${symbol}" >&2
    exit 1
  fi
done

exec env \
  RADISHLEX_RUN_MANAGER_APPLE_KEYCHAIN_P256_SMOKE=1 \
  RADISHLEX_MANAGER_APPLE_P256_GO_SERVER_DIR="${go_server_dir}" \
  "${product_binary}" --radishlex-apple-p256-product-smoke
