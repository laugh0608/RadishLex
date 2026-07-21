#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
output_root="${repo_root}/target/macos-product/upgrade-preflight"
module_cache="${output_root}/clang-module-cache"
test_executable="${output_root}/upgrade-preflight-contract"
host_executable="${output_root}/RadishLexUpgradePreflightHost"
source_files=(
  "${script_dir}/Sources/RLXUpgradePreflight.h"
  "${script_dir}/Sources/RLXUpgradePreflight.m"
  "${script_dir}/Sources/main.m"
)

if [[ $# -ne 0 ]]; then
  echo "upgrade preflight host check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the upgrade preflight host check" >&2
  exit 2
fi

"${script_dir}/build.sh"
minimum_macos="$(python3 "${product_tool}" field minimum_macos)"
mkdir -p "${output_root}" "${module_cache}"
CLANG_MODULE_CACHE_PATH="${module_cache}" clang \
  -fobjc-arc -fmodules -Wall -Wextra -Werror \
  "-mmacosx-version-min=${minimum_macos}" \
  -I "${script_dir}/Sources" \
  "${script_dir}/Sources/RLXUpgradePreflight.m" \
  "${script_dir}/Tests/contract_smoke.m" \
  -framework Cocoa \
  -o "${test_executable}"
"${test_executable}"

set +e
"${host_executable}" --caller-path-is-forbidden >/dev/null 2>&1
argument_status=$?
set -e
if [[ ${argument_status} -ne 2 ]]; then
  echo "upgrade preflight production host must reject every argument" >&2
  exit 1
fi

if rg -n 'removeItem|createDirectory|createFile|writeTo(File|URL)|terminate|kill\(|pkill|unlink|rename' \
  "${source_files[@]}"; then
  echo "upgrade preflight production source must remain read-only" >&2
  exit 1
fi
rg -Fq 'NSURLVolumeAvailableCapacityForImportantUsageKey' \
  "${script_dir}/Sources/RLXUpgradePreflight.m"
rg -Fq 'runningApplicationsWithBundleIdentifier' \
  "${script_dir}/Sources/RLXUpgradePreflight.m"
rg -Fq '@"/usr/sbin/lsof"' "${script_dir}/Sources/RLXUpgradePreflight.m"
rg -Fq 'if (argc != 1)' "${script_dir}/Sources/main.m"
manager_bundle_id="$(python3 "${product_tool}" field manager_bundle_id)"
input_method_bundle_id="$(python3 "${product_tool}" field input_method_bundle_id)"
strings "${host_executable}" | grep -Fxq "${manager_bundle_id}"
strings "${host_executable}" | grep -Fxq "${input_method_bundle_id}"

echo "macOS upgrade preflight host contract passed."
