#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
output_root="${repo_root}/target/macos-product/upgrade-preflight"
module_cache="${output_root}/clang-module-cache"
test_executable="${output_root}/upgrade-preflight-contract"
synthetic_host="${output_root}/upgrade-preflight-synthetic-host"
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

# Exercise the real CLI/JSON boundary with compile-time synthetic bundle IDs
# and a private Foundation home. Never invoke either valid mode on the host
# built with production IDs/current-user paths in this repository gate.
CLANG_MODULE_CACHE_PATH="${module_cache}" clang \
  -fobjc-arc -fmodules -Wall -Wextra -Werror \
  "-mmacosx-version-min=${minimum_macos}" \
  -DRLX_MANAGER_BUNDLE_ID=org.radishlex.synthetic.manager \
  -DRLX_INPUT_METHOD_BUNDLE_ID=org.radishlex.synthetic.inputmethod \
  -I "${script_dir}/Sources" \
  "${script_dir}/Sources/RLXUpgradePreflight.m" \
  "${script_dir}/Sources/main.m" \
  -framework Cocoa -o "${synthetic_host}"
python3 - "${synthetic_host}" <<'PY'
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix="radishlex-cancel-native-") as temporary:
    home = Path(temporary).resolve()
    root = home / "Library/Application Support/RadishLex"
    root.mkdir(parents=True, mode=0o700)
    metadata = root.stat()
    environment = {
        key: value for key, value in os.environ.items()
        if not key.startswith("DYLD_") and key != "CFFIXED_USER_HOME"
    }
    environment["CFFIXED_USER_HOME"] = str(home)
    for cancellation in (False, True):
        args = [sys.argv[1]] + (["--cancellation-release"] if cancellation else [])
        result = subprocess.run(args, env=environment, capture_output=True, timeout=30, check=True)
        assert not result.stderr
        value = json.loads(result.stdout)
        keys = {"format", "result", "available_bytes", "quiescent"}
        expected_format = "radishlex-upgrade-preflight-v1"
        if cancellation:
            keys |= {"data_root", "manager_bundle_id", "input_method_bundle_id"}
            expected_format = "radishlex-cancellation-preflight-v1"
            assert value["data_root"] == {
                "device_id": metadata.st_dev, "inode": metadata.st_ino,
                "owner_id": metadata.st_uid, "mode": 0o700,
            }
            assert value["manager_bundle_id"] == "org.radishlex.synthetic.manager"
            assert value["input_method_bundle_id"] == "org.radishlex.synthetic.inputmethod"
        assert set(value) == keys and value["format"] == expected_format
        assert value["result"] == "ready" and value["quiescent"] is True
        assert value["available_bytes"] > 0 and list(root.iterdir()) == []
    invalid = subprocess.run(
        [sys.argv[1], "--cancellation-release", str(root)], env=environment,
        capture_output=True, timeout=30,
    )
    assert invalid.returncode == 2 and not invalid.stdout and not invalid.stderr
PY

set +e
"${host_executable}" --caller-path-is-forbidden >/dev/null 2>&1
argument_status=$?
set -e
if [[ ${argument_status} -ne 2 ]]; then
  echo "upgrade preflight production host must reject unrecognized arguments" >&2
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
rg -Fq 'if (argc != 1 && !cancellation)' "${script_dir}/Sources/main.m"
manager_bundle_id="$(python3 "${product_tool}" field manager_bundle_id)"
input_method_bundle_id="$(python3 "${product_tool}" field input_method_bundle_id)"
strings "${host_executable}" | grep -Fxq "${manager_bundle_id}"
strings "${host_executable}" | grep -Fxq "${input_method_bundle_id}"

echo "macOS upgrade preflight host contract passed."
