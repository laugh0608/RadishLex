#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
identity_tool="${repo_root}/scripts/macos-product/release_identity.py"
carrier_tool="${repo_root}/scripts/macos-product/release_carrier.py"
keychain_profile="${RADISHLEX_NOTARY_KEYCHAIN_PROFILE:-}"

if [[ $# -ne 0 ]]; then
  echo "macOS release notarization does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for release notarization" >&2
  exit 2
fi
if [[ "$(python3 "${product_tool}" field distribution_identity)" == \
  "community-adhoc-v1" ]]; then
  echo "community-adhoc-v1 is intentionally unsigned and cannot be notarized" >&2
  exit 2
fi
if [[ -z "${keychain_profile}" || \
  ! "${keychain_profile}" =~ ^[A-Za-z0-9][A-Za-z0-9._\ -]{0,127}$ ]]; then
  echo "RADISHLEX_NOTARY_KEYCHAIN_PROFILE must name a stored Keychain profile" >&2
  exit 2
fi

product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"
release_root="${repo_root}/target/macos-release/${product_version}-${product_build}"
installer_bundle="${release_root}/RadishLex Installer.app"
embedded_product="${installer_bundle}/Contents/Resources/InstallPayload/Product"
carrier="${release_root}/RadishLex-${product_version}-${product_build}.dmg"
submission_receipt="${release_root}/NotarizationSubmission.json"
qualification="${release_root}/ReleaseQualification.json"

if [[ ! -d "${release_root}" || -L "${release_root}" || \
  ! -f "${carrier}" || -L "${carrier}" || \
  ! -d "${installer_bundle}" || -L "${installer_bundle}" ]]; then
  echo "frozen signed release carrier is required" >&2
  exit 1
fi
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" verify \
  --installer-bundle "${installer_bundle}" \
  --manager-bundle "${embedded_product}/Components/radishlex_manager.app" \
  --input-method-bundle \
    "${embedded_product}/Components/RadishLexInputMethod.app" \
  --identity "${installer_bundle}/Contents/Resources/ReleaseIdentity.json"
codesign --verify --strict --verbose=2 "${carrier}"
PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" verify-carrier-team \
  --carrier "${carrier}" \
  --identity "${installer_bundle}/Contents/Resources/ReleaseIdentity.json"
hdiutil verify "${carrier}" >/dev/null

if [[ -f "${qualification}" && ! -L "${qualification}" ]]; then
  PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" verify-qualification \
    --receipt "${submission_receipt}" \
    --carrier "${carrier}" \
    --installer-bundle "${installer_bundle}" \
    --qualification "${qualification}"
  xcrun stapler validate "${carrier}" >/dev/null 2>&1
  echo "RadishLex notarized release DMG qualification is already complete."
  exit 0
fi
if [[ -e "${qualification}" || -L "${qualification}" ]]; then
  echo "release qualification output is unsafe" >&2
  exit 1
fi

temporary="$(mktemp -d "${release_root}/.notarization.XXXXXX")"
mount_point="${temporary}/mount"
mkdir -p "${mount_point}"
attached=false
cleanup() {
  if [[ "${attached}" == true ]]; then
    hdiutil detach "${mount_point}" >/dev/null 2>&1 || true
  fi
  rm -rf -- "${temporary}"
}
trap cleanup EXIT

if [[ ! -e "${submission_receipt}" && ! -L "${submission_receipt}" ]]; then
  submission_output="${temporary}/submission.json"
  xcrun notarytool submit \
    "${carrier}" \
    --keychain-profile "${keychain_profile}" \
    --wait \
    --timeout 30m \
    --no-progress \
    --output-format json >"${submission_output}"
  PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" parse-submission \
    --input "${submission_output}" \
    --carrier "${carrier}" \
    --installer-bundle "${installer_bundle}" \
    --output "${submission_receipt}"
fi
if [[ ! -f "${submission_receipt}" || -L "${submission_receipt}" ]]; then
  echo "notary submission receipt is unsafe" >&2
  exit 1
fi

submission_id="$(
  PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" print-submission-id \
    --receipt "${submission_receipt}"
)"
notary_log="${temporary}/notary-log.json"
xcrun notarytool log \
  "${submission_id}" \
  "${notary_log}" \
  --keychain-profile "${keychain_profile}" \
  --no-progress >/dev/null
PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" verify-log \
  --receipt "${submission_receipt}" \
  --log "${notary_log}"

if ! xcrun stapler validate "${carrier}" >/dev/null 2>&1; then
  PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" \
    verify-submission-artifacts \
    --receipt "${submission_receipt}" \
    --carrier "${carrier}" \
    --installer-bundle "${installer_bundle}"
  xcrun stapler staple "${carrier}" >/dev/null
fi
xcrun stapler validate "${carrier}" >/dev/null
codesign --verify --strict --verbose=2 "${carrier}"
hdiutil verify "${carrier}" >/dev/null
spctl --assess \
  --type open \
  --context context:primary-signature \
  -v \
  "${carrier}" >/dev/null 2>&1

hdiutil attach \
  -readonly \
  -nobrowse \
  -noautoopen \
  -mountpoint "${mount_point}" \
  "${carrier}" >/dev/null
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
  echo "notarized DMG root contents changed" >&2
  exit 1
fi
mounted_installer="${mount_point}/RadishLex Installer.app"
PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" verify-installer \
  --expected-bundle "${installer_bundle}" \
  --actual-bundle "${mounted_installer}"
codesign --verify --deep --strict --verbose=2 "${mounted_installer}"
mounted_product="${mounted_installer}/Contents/Resources/InstallPayload/Product"
PYTHONDONTWRITEBYTECODE=1 python3 "${identity_tool}" verify \
  --installer-bundle "${mounted_installer}" \
  --manager-bundle \
    "${mounted_product}/Components/radishlex_manager.app" \
  --input-method-bundle \
    "${mounted_product}/Components/RadishLexInputMethod.app" \
  --identity "${mounted_installer}/Contents/Resources/ReleaseIdentity.json"
spctl --assess --type execute -v "${mounted_installer}" >/dev/null 2>&1
hdiutil detach "${mount_point}" >/dev/null
attached=false

PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" create-qualification \
  --receipt "${submission_receipt}" \
  --carrier "${carrier}" \
  --installer-bundle "${installer_bundle}" \
  --output "${qualification}"
PYTHONDONTWRITEBYTECODE=1 python3 "${carrier_tool}" verify-qualification \
  --receipt "${submission_receipt}" \
  --carrier "${carrier}" \
  --installer-bundle "${installer_bundle}" \
  --qualification "${qualification}"

trap - EXIT
rm -rf -- "${temporary}"
echo "RadishLex notarized release DMG qualification is complete."
