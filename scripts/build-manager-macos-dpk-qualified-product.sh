#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" != "--authorized-apple-development-provisioning-build" ] || [ "$#" -ne 1 ]; then
  echo "usage: $0 --authorized-apple-development-provisioning-build" >&2
  echo "This may contact Apple Developer services and use the selected signing identity." >&2
  exit 2
fi

if [ "$(uname -s)" != "Darwin" ]; then
  echo "manager DPK qualification build requires macOS." >&2
  exit 1
fi

development_team="${RADISHLEX_MANAGER_DEVELOPMENT_TEAM:-}"
codesign_identity="${RADISHLEX_MANAGER_CODESIGN_IDENTITY:-}"
if ! [[ "${development_team}" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "RADISHLEX_MANAGER_DEVELOPMENT_TEAM must be a 10-character Team ID." >&2
  exit 2
fi
if [ -z "${codesign_identity}" ] || [ "${codesign_identity}" = "-" ]; then
  echo "RADISHLEX_MANAGER_CODESIGN_IDENTITY must name an Apple Development identity." >&2
  exit 2
fi
valid_identities="$(security find-identity -v -p codesigning)"
if ! grep -Fq "\"${codesign_identity}\"" <<<"${valid_identities}"; then
  echo "RADISHLEX_MANAGER_CODESIGN_IDENTITY is not a valid local code-signing identity." >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
workspace="${manager_dir}/macos/Runner.xcworkspace"
derived_data="${manager_dir}/build/macos"
app_bundle="${derived_data}/Build/Products/Release/radishlex_manager.app"
native_library="${app_bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
app_framework="${app_bundle}/Contents/Frameworks/App.framework"
flutter_framework="${app_bundle}/Contents/Frameworks/FlutterMacOS.framework"
xcode_entitlements="${derived_data}/Build/Intermediates.noindex/Runner.build/Release/Runner.build/radishlex_manager.app.xcent"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-manager-dpk-signing.XXXXXX")"
app_entitlements="${temporary_dir}/app-entitlements.plist"
profile_plist="${temporary_dir}/profile.plist"

cleanup() {
  rm -rf "${temporary_dir}"
}
trap cleanup EXIT

(
  cd "${manager_dir}"
  flutter build macos --release --dart-define=RADISHLEX_MANAGER_MODE=product
)

xcodebuild \
  -quiet \
  -workspace "${workspace}" \
  -scheme Runner \
  -configuration Release \
  -derivedDataPath "${derived_data}" \
  -destination "platform=macOS,arch=$(uname -m)" \
  -allowProvisioningUpdates \
  ARCHS="$(uname -m)" \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGN_STYLE=Automatic \
  CODE_SIGN_IDENTITY="Apple Development" \
  CODE_SIGN_ENTITLEMENTS="Runner/DPKQualification.entitlements" \
  DEVELOPMENT_TEAM="${development_team}" \
  build

if [ ! -f "${native_library}" ] || [ ! -d "${app_framework}" ] || \
  [ ! -d "${flutter_framework}" ] || [ ! -f "${xcode_entitlements}" ]; then
  echo "qualified manager bundle is missing a native component or generated entitlements." >&2
  exit 1
fi
if [ ! -f "${app_bundle}/Contents/embedded.provisionprofile" ]; then
  echo "qualified manager bundle is missing its embedded provisioning profile." >&2
  exit 1
fi

codesign --force --sign "${codesign_identity}" --timestamp=none --options runtime \
  "${app_framework}"
codesign --force --sign "${codesign_identity}" --timestamp=none --options runtime \
  "${flutter_framework}"
codesign --force --sign "${codesign_identity}" --timestamp=none --options runtime \
  "${native_library}"
codesign --force --sign "${codesign_identity}" --timestamp=none --options runtime \
  --entitlements "${xcode_entitlements}" "${app_bundle}"
codesign --verify --deep --strict --verbose=2 "${app_bundle}"
codesign -d --entitlements :- "${app_bundle}" >"${app_entitlements}" 2>/dev/null
openssl cms -verify -inform DER \
  -in "${app_bundle}/Contents/embedded.provisionprofile" \
  -noverify -out "${profile_plist}" 2>/dev/null

bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${app_bundle}/Contents/Info.plist")"
app_team="$(codesign -d --verbose=4 "${app_bundle}" 2>&1 | awk -F= '$1 == "TeamIdentifier" { print $2 }')"
app_authority="$(codesign -d --verbose=4 "${app_bundle}" 2>&1 | awk -F= '$1 == "Authority" && !seen { print $2; seen = 1 }')"
native_team="$(codesign -d --verbose=4 "${native_library}" 2>&1 | awk -F= '$1 == "TeamIdentifier" { print $2 }')"
application_identifier="$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "${app_entitlements}")"
entitlement_team="$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.team-identifier' "${app_entitlements}")"
keychain_access_group="$(/usr/libexec/PlistBuddy -c 'Print :keychain-access-groups:0' "${app_entitlements}")"
profile_team="$(/usr/libexec/PlistBuddy -c 'Print :TeamIdentifier:0' "${profile_plist}")"
profile_application_identifier="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.application-identifier' "${profile_plist}")"
profile_keychain_access_group="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:keychain-access-groups:0' "${profile_plist}")"
profile_uuid="$(/usr/libexec/PlistBuddy -c 'Print :UUID' "${profile_plist}")"
profile_expiration="$(/usr/libexec/PlistBuddy -c 'Print :ExpirationDate' "${profile_plist}")"
expected_application_identifier="${development_team}.${bundle_id}"

if [ "${app_team}" != "${development_team}" ] || [ "${native_team}" != "${development_team}" ]; then
  echo "qualified manager app and native library must use the requested Team ID." >&2
  exit 1
fi
if [ "${app_authority}" != "${codesign_identity}" ]; then
  echo "qualified manager app did not use the requested signing identity." >&2
  exit 1
fi
if [ "${entitlement_team}" != "${development_team}" ] || [ "${profile_team}" != "${development_team}" ]; then
  echo "qualified manager entitlements and profile must use the requested Team ID." >&2
  exit 1
fi
if [ "${application_identifier}" != "${expected_application_identifier}" ] || \
  [ "${profile_application_identifier}" != "${expected_application_identifier}" ]; then
  echo "qualified manager application identifier does not match Team ID plus Bundle ID." >&2
  exit 1
fi
if [ "${keychain_access_group}" != "${expected_application_identifier}" ]; then
  echo "qualified manager Keychain access group does not match its application identifier." >&2
  exit 1
fi
if [ "${profile_keychain_access_group}" != "${expected_application_identifier}" ] && \
  [ "${profile_keychain_access_group}" != "${development_team}.*" ]; then
  echo "qualified manager profile does not authorize its Keychain access group." >&2
  exit 1
fi
if codesign -d --entitlements :- "${app_bundle}" 2>&1 | grep -Fq "com.apple.security.app-sandbox"; then
  echo "DPK qualification must not silently change the accepted M2 App Sandbox boundary." >&2
  exit 1
fi

echo "RadishLex manager DPK qualification product built and verified."
echo "bundle_id=${bundle_id}"
echo "team_id=${app_team}"
echo "application_identifier=${application_identifier}"
echo "default_keychain_access_group=${keychain_access_group}"
echo "profile_uuid=${profile_uuid}"
echo "profile_expiration=${profile_expiration}"
