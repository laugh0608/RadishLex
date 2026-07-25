#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"
output_root="${repo_root}/target/macos-product/installer-app"
module_cache="${output_root}/clang-module-cache"
contract_test="${output_root}/installer-presentation-contract"
bundle="${output_root}/RadishLex Installer.app"
bridge_library="${repo_root}/target/release/libradishlex_macos_installer_bridge.a"
source_files=(
  "${script_dir}/Sources/RLXInstallerBridge.h"
  "${script_dir}/Sources/RLXInstallerBridge.m"
  "${script_dir}/Sources/RLXInstallerPresentation.h"
  "${script_dir}/Sources/RLXInstallerPresentation.m"
  "${script_dir}/Sources/main.m"
)

if [[ $# -ne 0 ]]; then
  echo "Installer app check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the Installer app check" >&2
  exit 2
fi

env -u RADISHLEX_INSTALLER_PAYLOAD_ROOT \
  -u RADISHLEX_INSTALLER_CODESIGN_IDENTITY \
  "${script_dir}/build.sh"
minimum_macos="$(python3 "${product_tool}" field minimum_macos)"
mkdir -p "${output_root}" "${module_cache}"
CLANG_MODULE_CACHE_PATH="${module_cache}" clang \
  -fobjc-arc -fblocks -fmodules -Wall -Wextra -Werror \
  "-mmacosx-version-min=${minimum_macos}" \
  -I "${script_dir}/Sources" \
  -I "${repo_root}/platforms/macos-product/InstallerBridge/include" \
  "${script_dir}/Sources/RLXInstallerBridge.m" \
  "${script_dir}/Sources/RLXInstallerPresentation.m" \
  "${script_dir}/Tests/presentation_contract.m" \
  "${bridge_library}" \
  -framework Foundation \
  -framework Security \
  -o "${contract_test}"
"${contract_test}"

expected_bundle_id="$(python3 "${layout_tool}" field installer_bundle_id)"
actual_bundle_id="$(plutil -extract CFBundleIdentifier raw "${bundle}/Contents/Info.plist")"
if [[ "${actual_bundle_id}" != "${expected_bundle_id}" ]]; then
  echo "Installer bundle ID differs from install layout" >&2
  exit 1
fi
cmp -s "${repo_root}/packaging/macos/install-layout.json" \
  "${bundle}/Contents/Resources/InstallLayout.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${bundle}/Contents/Resources/InstallPayload"
test ! -e "${bundle}/Contents/Resources/ReleaseIdentity.json"
codesign --verify --deep --strict "${bundle}"
exported_symbols="$(nm -gU "${bundle}/Contents/MacOS/RadishLex Installer")"
for symbol in \
  radishlex_installer_bridge_contract_version \
  radishlex_installer_bridge_snapshot_v1 \
  radishlex_installer_bridge_perform_v1; do
  rg -Fq "_${symbol}" <<<"${exported_symbols}"
done

set +e
boundary_matches="$(rg -n 'TISSelectInputSource|TISRegisterInputSource|kTISPropertyInputSourceIsEnabled|NSTask|Process\(|/bin/(rm|sh)|rm -rf|HOME|NSHomeDirectory|expandTilde|receipt\.json|operation_id' \
  "${source_files[@]}")"
boundary_status=$?
set -e
if [[ ${boundary_status} -eq 0 ]]; then
  echo "${boundary_matches}" >&2
  echo "Installer presentation source crossed the UI/driver boundary" >&2
  exit 1
fi
if [[ ${boundary_status} -ne 1 ]]; then
  echo "Installer presentation boundary scan failed" >&2
  exit 1
fi
rg -Fq 'Applications/RadishLex Manager.app' "${script_dir}/Sources/main.m"
rg -Fq 'Library/Input Methods/RadishLexInputMethod.app' "${script_dir}/Sources/main.m"
rg -Fq 'Library/Application Support/RadishLex' "${script_dir}/Sources/main.m"
rg -Fq 'if (argc != 1)' "${script_dir}/Sources/main.m"

set +e
"${bundle}/Contents/MacOS/RadishLex Installer" --path-is-forbidden >/dev/null 2>&1
argument_status=$?
set -e
if [[ ${argument_status} -ne 2 ]]; then
  echo "Installer app executable must reject every argument" >&2
  exit 1
fi

echo "macOS Installer app presentation contract passed."
