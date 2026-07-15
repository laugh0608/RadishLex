#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/macos-imk"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the native InputMethodKit bundle check." >&2
  exit 2
fi

: "${RIME_INCLUDE_DIR:?native check requires RIME_INCLUDE_DIR}"
: "${RIME_LIB_DIR:?native check requires RIME_LIB_DIR}"
: "${RADISHLEX_RIME_SHARED_DATA:?native check requires isolated RADISHLEX_RIME_SHARED_DATA}"
: "${RADISHLEX_RIME_SCHEMA:?native check requires RADISHLEX_RIME_SCHEMA}"
: "${RADISHLEX_RIME_DATA_LICENSE:?native check requires RADISHLEX_RIME_DATA_LICENSE}"

shared_data="$(CDPATH= cd -- "${RADISHLEX_RIME_SHARED_DATA}" && pwd -P)"
case "${shared_data}" in
  "${HOME}/Library/Rime"|"${HOME}/Library/Rime/"*|\
  "${HOME}/Library/Input Methods"|"${HOME}/Library/Input Methods/"*|\
  "${HOME}/Library/Application Support/Squirrel"|\
  "${HOME}/Library/Application Support/Squirrel/"*|\
  "${HOME}/Library/Application Support/RadishLex/Rime"|\
  "${HOME}/Library/Application Support/RadishLex/Rime/"*)
    echo "native check refuses a real or runtime user data directory: ${shared_data}" >&2
    exit 2
    ;;
esac

"${platform_dir}/build-bundle.sh" native

bundle="${repo_root}/target/macos-imk/native/RadishLexInputMethod.app"
contents="${bundle}/Contents"
executable="${contents}/MacOS/RadishLex"
ffi_dylib="${contents}/Frameworks/libradishlex_ime_ffi.dylib"
resources="${contents}/Resources"
icon="${resources}/RadishLexInputIcon.tiff"
manifest="${resources}/RimeData.manifest.plist"
native_licenses="${resources}/NativeLicenses"
native_manifest="${resources}/NativeLibraries.manifest.plist"
architecture="$(uname -m)"
deploy_on_start="${RADISHLEX_RIME_DEPLOY_ON_START:-1}"

check_dependencies() {
  local binary="$1"
  local dependency
  while IFS= read -r dependency; do
    case "${dependency}" in
      /System/*|/usr/lib/*)
        ;;
      @rpath/*)
        local name="${dependency#@rpath/}"
        if [[ ! -f "${contents}/Frameworks/${name}" ]]; then
          echo "unresolved @rpath dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      @loader_path/*)
        local loader_relative="${dependency#@loader_path/}"
        if [[ ! -f "$(dirname -- "${binary}")/${loader_relative}" ]]; then
          echo "unresolved @loader_path dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      @executable_path/*)
        local executable_relative="${dependency#@executable_path/}"
        if [[ ! -f "${contents}/MacOS/${executable_relative}" ]]; then
          echo "unresolved @executable_path dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      /*)
        echo "external absolute dependency for ${binary}: ${dependency}" >&2
        return 1
        ;;
      *)
        echo "unsupported dependency path for ${binary}: ${dependency}" >&2
        return 1
        ;;
    esac
  done < <(otool -L "${binary}" | tail -n +2 | awk '{print $1}')
}

test -x "${executable}"
test -f "${ffi_dylib}"
test -s "${icon}"
test "$(sips -g pixelWidth "${icon}" 2>/dev/null | awk '/pixelWidth:/ { print $2 }')" = "32"
test "$(sips -g pixelHeight "${icon}" 2>/dev/null | awk '/pixelHeight:/ { print $2 }')" = "32"
test "$(sips -g dpiWidth "${icon}" 2>/dev/null | awk '/dpiWidth:/ { print $2 }')" = "144.000"
test "$(sips -g dpiHeight "${icon}" 2>/dev/null | awk '/dpiHeight:/ { print $2 }')" = "144.000"
test -s "${resources}/zh-Hans.lproj/InfoPlist.strings"
test -s "${resources}/en.lproj/InfoPlist.strings"
codesign --verify --deep --strict --verbose=2 "${bundle}"
test -f "${resources}/RimeData/default.yaml"
cmp -s \
  <(sed "s/__RADISHLEX_RIME_SCHEMA__/${RADISHLEX_RIME_SCHEMA}/g" \
    "${platform_dir}/Resources/Rime/default.yaml.in") \
  "${resources}/RimeData/default.yaml"
grep -Fqx '  page_size: 5' "${resources}/RimeData/default.yaml"
test -f "${resources}/RimeData/${RADISHLEX_RIME_SCHEMA}.schema.yaml"
test -s "${resources}/RimeData.LICENSE"
test -s "${resources}/RadishLex.LICENSE"
test -d "${native_licenses}"
if strings "${executable}" | grep -Eq \
  'initForContractWithClient:|rlx_contract(Candidate|Owner|Selected|Window)'; then
  echo "native macOS product must not contain contract-only inspection APIs." >&2
  exit 1
fi
plutil -lint "${contents}/Info.plist" "${manifest}" "${native_manifest}" \
  "${resources}/zh-Hans.lproj/InfoPlist.strings" \
  "${resources}/en.lproj/InfoPlist.strings" >/dev/null
test "$(plutil -extract CFBundleVersion raw "${contents}/Info.plist")" = "32"
test "$(plutil -extract RadishLexRimeSchema raw "${contents}/Info.plist")" = \
  "${RADISHLEX_RIME_SCHEMA}"
test "$(plutil -extract schema_id raw "${manifest}")" = "${RADISHLEX_RIME_SCHEMA}"
test "$(plutil -extract tsInputMethodCharacterRepertoireKey.0 raw \
  "${contents}/Info.plist")" = "Hans"
test "$(plutil -extract TISIntendedLanguage raw \
  "${contents}/Info.plist")" = "zh-Hans"
mode_path=":ComponentInputModeDict:tsInputModeListKey:org.radishlex.inputmethod.macos.Pinyin"
mode_id="$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISInputSourceID" \
  "${contents}/Info.plist")"
test "${mode_id}" = "org.radishlex.inputmethod.macos.Pinyin"
[[ "${mode_id}" =~ ^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$ ]]
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIntendedLanguage" \
  "${contents}/Info.plist")" = "zh-Hans"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeIsVisibleKey" \
  "${contents}/Info.plist")" = "true"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeScriptKey" \
  "${contents}/Info.plist")" = "smSimpChinese"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeCharacterRepertoireKey:0" \
  "${contents}/Info.plist")" = "Hans"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeMenuIconFileKey" \
  "${contents}/Info.plist")" = "RadishLexInputIcon.tiff"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModePaletteIconFileKey" \
  "${contents}/Info.plist")" = "RadishLexInputIcon.tiff"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIconLabels:Primary" \
  "${contents}/Info.plist")" = "萝"
test "$(plutil -extract ComponentInputModeDict.tsVisibleInputModeOrderedArrayKey.0 raw \
  "${contents}/Info.plist")" = "org.radishlex.inputmethod.macos.Pinyin"
if plutil -extract InputMethodServerDelegateClass raw \
  "${contents}/Info.plist" >/dev/null 2>&1; then
  echo "macOS IMK bundle must not treat the input controller as a server delegate." >&2
  exit 1
fi
test "$(plutil -extract LSUIElement raw "${contents}/Info.plist")" = "true"
if plutil -extract LSBackgroundOnly raw "${contents}/Info.plist" >/dev/null 2>&1; then
  echo "macOS IMK bundle must use LSUIElement without LSBackgroundOnly." >&2
  exit 1
fi
test "$(/usr/libexec/PlistBuddy -c 'Print :org.radishlex.inputmethod.macos.Pinyin' \
  "${resources}/zh-Hans.lproj/InfoPlist.strings")" = "萝卜词核拼音"
test "$(plutil -extract RadishLexRimeDeployOnStart raw "${contents}/Info.plist")" = \
  "$([[ "${deploy_on_start}" == "1" ]] && echo true || echo false)"
test "$(plutil -extract deploy_on_start raw "${manifest}")" = \
  "$([[ "${deploy_on_start}" == "1" ]] && echo true || echo false)"

lipo -archs "${executable}" | tr ' ' '\n' | grep -qx "${architecture}"
lipo -archs "${ffi_dylib}" | tr ' ' '\n' | grep -qx "${architecture}"
otool -L "${executable}" | grep -q "@rpath/libradishlex_ime_ffi.dylib"
otool -L "${ffi_dylib}" | grep -q "@rpath/librime"

rime_dependency="$(otool -L "${ffi_dylib}" | awk '/librime/{print $1; exit}')"
case "${rime_dependency}" in
  @rpath/*) rime_binary="${contents}/Frameworks/${rime_dependency#@rpath/}" ;;
  *) echo "unsupported librime dependency path: ${rime_dependency}" >&2; exit 1 ;;
esac
test -f "${rime_binary}"
check_dependencies "${executable}"
while IFS= read -r -d '' dylib; do
  check_dependencies "${dylib}"
  codesign --verify --strict --verbose=2 "${dylib}"
  lipo -archs "${dylib}" | tr ' ' '\n' | grep -qx "${architecture}"
done < <(find "${contents}/Frameworks" -type f -name '*.dylib' -print0 | sort -z)

for symbol in session_new_rime session_handle_key_event rime_runtime_shutdown; do
  nm -gU "${ffi_dylib}" | grep -q "_radishlex_${symbol}$"
done

python3 "${repo_root}/scripts/macos-imk/native_manifest.py" verify \
  --data-dir "${resources}/RimeData" \
  --license "${resources}/RimeData.LICENSE" \
  --schema "${RADISHLEX_RIME_SCHEMA}" \
  --deploy-on-start "${deploy_on_start}" \
  --manifest "${manifest}"

python3 "${repo_root}/scripts/macos-imk/bundle_dylibs.py" verify \
  --frameworks-dir "${contents}/Frameworks" \
  --licenses-dir "${native_licenses}" \
  --manifest "${native_manifest}"

native_user_data="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-macos-imk-native.XXXXXX")"
trap 'rm -rf "${native_user_data}"' EXIT
(
  cd "${repo_root}"
  env \
    RIME_INCLUDE_DIR="${RIME_INCLUDE_DIR}" \
    RIME_LIB_DIR="${RIME_LIB_DIR}" \
    RADISHLEX_RIME_SHARED_DATA="${resources}/RimeData" \
    RADISHLEX_RIME_USER_DATA="${native_user_data}" \
    RADISHLEX_RIME_SCHEMA="${RADISHLEX_RIME_SCHEMA}" \
    RADISHLEX_EXPECTED_CANDIDATE_PAGE_SIZE=5 \
    cargo test -p radishlex-ime-ffi --features native-rime \
      rime_session_native_smoke_uses_ffi_entrypoint -- --ignored
)

echo "Native macOS InputMethodKit bundle checks passed without installation."
