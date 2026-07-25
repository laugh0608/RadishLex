#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"
identity_tool="${repo_root}/scripts/macos-product/release_identity.py"
community_tool="${repo_root}/scripts/macos-product/community_release.py"

if [[ $# -ne 0 ]]; then
  echo "macOS release DMG build does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the release DMG build" >&2
  exit 2
fi
product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"
release_root="${repo_root}/target/macos-release/${product_version}-${product_build}"
installer_bundle="${release_root}/RadishLex Installer.app"
product_root="${release_root}/Product"
payload_root="${release_root}/InstallPayload"
manager_bundle="${product_root}/Components/radishlex_manager.app"
input_method_bundle="${product_root}/Components/RadishLexInputMethod.app"
carrier_name="RadishLex-${product_version}-${product_build}.dmg"
carrier="${release_root}/${carrier_name}"
evidence="${release_root}/CommunityReleaseEvidence.json"

if [[ ! -d "${release_root}" || -L "${release_root}" ]]; then
  echo "signed release Installer root is required" >&2
  exit 1
fi
if [[ -e "${carrier}" || -L "${carrier}" || -e "${evidence}" || -L "${evidence}" ]]; then
  echo "release DMG output already exists" >&2
  exit 2
fi

python3 "${product_tool}" verify \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --license "${product_root}/LICENSE" \
  --manifest "${product_root}/ProductManifest.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${payload_root}"
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" verify \
  --installer-bundle "${installer_bundle}" \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --identity "${installer_bundle}/Contents/Resources/ReleaseIdentity.json"
codesign --verify --deep --strict --verbose=2 "${installer_bundle}"

staging="$(mktemp -d "${release_root}/.release-dmg.XXXXXX")"
mount_point="${staging}/mount"
source_root="${staging}/source"
temporary_carrier="${staging}/${carrier_name}"
mkdir -p "${source_root}" "${mount_point}"
attached=false
cleanup() {
  if [[ "${attached}" == true ]]; then
    hdiutil detach "${mount_point}" >/dev/null 2>&1 || true
  fi
  rm -rf -- "${staging}"
}
trap cleanup EXIT

ditto "${installer_bundle}" "${source_root}/RadishLex Installer.app"
hdiutil create \
  -srcfolder "${source_root}" \
  -format UDZO \
  -fs APFS \
  -volname "RadishLex Installer" \
  -nospotlight \
  "${temporary_carrier}" >/dev/null
hdiutil verify "${temporary_carrier}" >/dev/null

hdiutil attach \
  -readonly \
  -nobrowse \
  -noautoopen \
  -mountpoint "${mount_point}" \
  "${temporary_carrier}" >/dev/null
attached=true
shopt -s nullglob
mounted_entries=(
  "${mount_point}"/*
  "${mount_point}"/.[!.]*
  "${mount_point}"/..?*
)
shopt -u nullglob
if [[ ${#mounted_entries[@]} -ne 1 || \
  "${mounted_entries[0]}" != "${mount_point}/RadishLex Installer.app" ]]; then
  echo "release DMG root contents changed" >&2
  exit 1
fi
mounted_installer="${mount_point}/RadishLex Installer.app"
codesign --verify --deep --strict --verbose=2 "${mounted_installer}"
mounted_product="${mounted_installer}/Contents/Resources/InstallPayload/Product"
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" verify \
  --installer-bundle "${mounted_installer}" \
  --manager-bundle \
    "${mounted_product}/Components/radishlex_manager.app" \
  --input-method-bundle \
    "${mounted_product}/Components/RadishLexInputMethod.app" \
  --identity "${mounted_installer}/Contents/Resources/ReleaseIdentity.json"
hdiutil detach "${mount_point}" >/dev/null
attached=false

mv "${temporary_carrier}" "${carrier}"
PYTHONDONTWRITEBYTECODE=1 python3 "${community_tool}" create \
  --carrier "${carrier}" \
  --identity "${installer_bundle}/Contents/Resources/ReleaseIdentity.json" \
  --output "${evidence}"
PYTHONDONTWRITEBYTECODE=1 python3 "${community_tool}" verify \
  --carrier "${carrier}" \
  --identity "${installer_bundle}/Contents/Resources/ReleaseIdentity.json" \
  --evidence "${evidence}"
trap - EXIT
rm -rf -- "${staging}"
echo "RadishLex unnotarized community DMG and SHA-256 evidence are ready."
