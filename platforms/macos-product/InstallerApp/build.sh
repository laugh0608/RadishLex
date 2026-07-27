#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"
output_root="${repo_root}/target/macos-product/installer-app"
module_cache="${output_root}/clang-module-cache"
bundle="${output_root}/RadishLex Installer.app"
contents="${bundle}/Contents"
executable="${contents}/MacOS/RadishLex Installer"
plist="${contents}/Info.plist"

if [[ $# -ne 0 ]]; then
  echo "Installer app build does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required to build the Installer app" >&2
  exit 2
fi

installer_bundle_id="$(python3 "${layout_tool}" field installer_bundle_id)"
product_version="$(python3 "${product_tool}" field product_version)"
build_number="$(python3 "${product_tool}" field build_number)"
minimum_macos="$(python3 "${product_tool}" field minimum_macos)"
bridge_library="${repo_root}/target/release/libradishlex_macos_installer_bridge.a"
payload_root="${RADISHLEX_INSTALLER_PAYLOAD_ROOT:-${repo_root}/target/macos-install-payload/${product_version}-${build_number}}"
codesign_identity="${RADISHLEX_INSTALLER_CODESIGN_IDENTITY:--}"

env MACOSX_DEPLOYMENT_TARGET="${minimum_macos}" \
  cargo build --locked --release -p radishlex-macos-installer-bridge \
  --manifest-path "${repo_root}/Cargo.toml"
if [[ ! -d "${payload_root}" ]]; then
  "${repo_root}/scripts/build-macos-install-payload.sh"
fi
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${payload_root}"

rm -rf -- "${bundle}"
mkdir -p "${contents}/MacOS" "${contents}/Resources" "${module_cache}"
sed \
  -e "s/__INSTALLER_BUNDLE_ID__/${installer_bundle_id}/g" \
  -e "s/__PRODUCT_VERSION__/${product_version}/g" \
  -e "s/__BUILD_NUMBER__/${build_number}/g" \
  -e "s/__MINIMUM_MACOS__/${minimum_macos}/g" \
  "${script_dir}/Resources/Info.plist.in" >"${plist}"
plutil -lint "${plist}" >/dev/null
install -m 644 "${repo_root}/packaging/macos/install-layout.json" \
  "${contents}/Resources/InstallLayout.json"
ditto "${payload_root}" "${contents}/Resources/InstallPayload"

CLANG_MODULE_CACHE_PATH="${module_cache}" clang \
  -fobjc-arc -fblocks -fmodules -Wall -Wextra -Werror \
  "-mmacosx-version-min=${minimum_macos}" \
  -I "${script_dir}/Sources" \
  -I "${repo_root}/platforms/macos-product/InstallerBridge/include" \
  "${script_dir}/Sources/RLXInstallerApplicationMenu.m" \
  "${script_dir}/Sources/RLXInstallerBridge.m" \
  "${script_dir}/Sources/RLXInstallerPresentation.m" \
  "${script_dir}/Sources/main.m" \
  "${bridge_library}" \
  -framework Cocoa \
  -framework Security \
  -o "${executable}"
if [[ "${codesign_identity}" == "-" ]]; then
  codesign --force --sign - --timestamp=none "${bundle}"
else
  codesign --force --sign "${codesign_identity}" --timestamp --options runtime "${bundle}"
fi
codesign --verify --deep --strict "${bundle}"

echo "RadishLex Installer app: ${bundle}"
