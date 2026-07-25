#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"
identity_tool="${repo_root}/scripts/macos-product/release_identity.py"
codesign_identity="${RADISHLEX_DEVELOPER_ID_APPLICATION:-}"

if [[ $# -ne 0 ]]; then
  echo "macOS release Installer build does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the release Installer build" >&2
  exit 2
fi
if [[ -z "${codesign_identity}" || "${codesign_identity}" == "-" ]]; then
  echo "RADISHLEX_DEVELOPER_ID_APPLICATION must name a Developer ID Application identity" >&2
  exit 2
fi
valid_identities="$(security find-identity -v -p codesigning)"
if ! grep -Fq "\"${codesign_identity}\"" <<<"${valid_identities}"; then
  echo "RADISHLEX_DEVELOPER_ID_APPLICATION is not a valid local code-signing identity" >&2
  exit 2
fi
if [[ "${codesign_identity}" != Developer\ ID\ Application:* ]]; then
  echo "release signing identity must be Developer ID Application" >&2
  exit 2
fi

product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"
source_product="${repo_root}/target/macos-product/${product_version}-${product_build}"
release_parent="${repo_root}/target/macos-release"
release_root="${release_parent}/${product_version}-${product_build}"

if [[ ! -d "${source_product}" || -L "${source_product}" ]]; then
  echo "verified macOS product assembly is required: ${source_product}" >&2
  exit 1
fi
if [[ -e "${release_root}" || -L "${release_root}" ]]; then
  echo "release Installer output already exists: ${release_root}" >&2
  exit 2
fi

manager_source="${source_product}/Components/radishlex_manager.app"
input_method_source="${source_product}/Components/RadishLexInputMethod.app"
python3 "${product_tool}" verify \
  --manager-bundle "${manager_source}" \
  --input-method-bundle "${input_method_source}" \
  --license "${source_product}/LICENSE" \
  --manifest "${source_product}/ProductManifest.json"

mkdir -p "${release_parent}"
staging="$(mktemp -d "${release_parent}/.release-installer.XXXXXX")"
cleanup() {
  rm -rf -- "${staging}"
}
trap cleanup EXIT

product_root="${staging}/Product"
payload_root="${staging}/InstallPayload"
installer_bundle="${staging}/RadishLex Installer.app"
ditto "${source_product}" "${product_root}"
manager_bundle="${product_root}/Components/radishlex_manager.app"
input_method_bundle="${product_root}/Components/RadishLexInputMethod.app"

sign_nested_macho() {
  local bundle="$1"
  local nested=""
  while IFS= read -r -d '' candidate; do
    if file -b "${candidate}" | grep -Fq "Mach-O"; then
      codesign --force --sign "${codesign_identity}" --timestamp --options runtime \
        "${candidate}"
    fi
  done < <(find "${bundle}" -type f -print0 | sort -z)
  while IFS= read -r -d '' nested; do
    if [[ "${nested}" != "${bundle}" ]]; then
      codesign --force --sign "${codesign_identity}" --timestamp --options runtime \
        "${nested}"
    fi
  done < <(
    find "${bundle}" -type d \
      \( -name '*.framework' -o -name '*.app' -o -name '*.xpc' -o -name '*.appex' \) \
      -print0 | sort -zr
  )
  codesign --force --sign "${codesign_identity}" --timestamp --options runtime "${bundle}"
  codesign --verify --deep --strict --verbose=2 "${bundle}"
}

sign_nested_macho "${manager_bundle}"
sign_nested_macho "${input_method_bundle}"
rm -f -- "${product_root}/ProductManifest.json"
python3 "${product_tool}" create \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --license "${product_root}/LICENSE" \
  --manifest "${product_root}/ProductManifest.json"
python3 "${product_tool}" verify \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --license "${product_root}/LICENSE" \
  --manifest "${product_root}/ProductManifest.json"

PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" assemble \
  --product-root "${product_root}" \
  --output "${payload_root}"
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${payload_root}"

env \
  RADISHLEX_INSTALLER_PAYLOAD_ROOT="${payload_root}" \
  RADISHLEX_INSTALLER_CODESIGN_IDENTITY="${codesign_identity}" \
  "${repo_root}/platforms/macos-product/InstallerApp/build.sh"
ditto \
  "${repo_root}/target/macos-product/installer-app/RadishLex Installer.app" \
  "${installer_bundle}"

release_identity="${installer_bundle}/Contents/Resources/ReleaseIdentity.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" create \
  --installer-bundle "${installer_bundle}" \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --output "${release_identity}"
codesign --force --sign "${codesign_identity}" --timestamp --options runtime \
  "${installer_bundle}"
codesign --verify --deep --strict --verbose=2 "${installer_bundle}"
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" verify \
  --installer-bundle "${installer_bundle}" \
  --manager-bundle "${manager_bundle}" \
  --input-method-bundle "${input_method_bundle}" \
  --identity "${release_identity}"

for bundle in "${manager_bundle}" "${input_method_bundle}" "${installer_bundle}"; do
  if ! codesign -d --verbose=4 "${bundle}" 2>&1 | grep -Eq \
    'CodeDirectory .* flags=.*\\(runtime\\)'; then
    echo "Hardened Runtime is unavailable: ${bundle}" >&2
    exit 1
  fi
  while IFS= read -r -d '' candidate; do
    if file -b "${candidate}" | grep -Fq "Mach-O"; then
      codesign --verify --strict "${candidate}"
      if ! codesign -d --verbose=4 "${candidate}" 2>&1 | grep -Eq \
        'CodeDirectory .* flags=.*\\(runtime\\)'; then
        echo "nested executable lacks Hardened Runtime: ${candidate}" >&2
        exit 1
      fi
    fi
  done < <(find "${bundle}" -type f -print0 | sort -z)
done

mv "${staging}" "${release_root}"
trap - EXIT
echo "RadishLex signed release Installer: ${release_root}"
