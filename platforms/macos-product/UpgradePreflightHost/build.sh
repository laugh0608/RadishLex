#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
output_root="${repo_root}/target/macos-product/upgrade-preflight"
module_cache="${output_root}/clang-module-cache"
executable="${output_root}/RadishLexUpgradePreflightHost"

if [[ $# -ne 0 ]]; then
  echo "upgrade preflight host build does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required to build the upgrade preflight host" >&2
  exit 2
fi

manager_bundle_id="$(python3 "${product_tool}" field manager_bundle_id)"
input_method_bundle_id="$(python3 "${product_tool}" field input_method_bundle_id)"
minimum_macos="$(python3 "${product_tool}" field minimum_macos)"
manager_define="-DRLX_MANAGER_BUNDLE_ID=${manager_bundle_id}"
input_method_define="-DRLX_INPUT_METHOD_BUNDLE_ID=${input_method_bundle_id}"

mkdir -p "${output_root}" "${module_cache}"
CLANG_MODULE_CACHE_PATH="${module_cache}" clang \
  -fobjc-arc -fmodules -Wall -Wextra -Werror \
  "-mmacosx-version-min=${minimum_macos}" \
  "${manager_define}" "${input_method_define}" \
  -I "${script_dir}/Sources" \
  "${script_dir}/Sources/RLXUpgradePreflight.m" \
  "${script_dir}/Sources/main.m" \
  -framework Cocoa \
  -o "${executable}"

echo "RadishLex upgrade preflight host: ${executable}"
