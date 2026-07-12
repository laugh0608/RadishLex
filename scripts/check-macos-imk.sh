#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/macos-imk"
export CLANG_MODULE_CACHE_PATH="${repo_root}/target/macos-imk/clang-module-cache"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the InputMethodKit contract smoke." >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-imk/test_native_manifest.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-imk/test_bundle_dylibs.py"
"${platform_dir}/build-bundle.sh" contract

smoke_dir="${repo_root}/target/macos-imk/contract-smoke"
mkdir -p "${smoke_dir}" "${CLANG_MODULE_CACHE_PATH}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=0 \
  -I"${platform_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Sources/RadishLexInputController.m" \
  "${platform_dir}/Sources/main.m"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=1 \
  -I"${platform_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Tests/contract_smoke.m" \
  -L"${repo_root}/target/debug" -lradishlex_ime_ffi \
  -Wl,-rpath,"${repo_root}/target/debug" \
  -framework AppKit -framework Carbon \
  -o "${smoke_dir}/contract-smoke"
"${smoke_dir}/contract-smoke"

bundle="${repo_root}/target/macos-imk/contract/RadishLex.app"
test -x "${bundle}/Contents/MacOS/RadishLex"
test -f "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
test -s "${bundle}/Contents/Resources/RadishLexInputIcon.tiff"
test -s "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"
test -s "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings"
plutil -lint "${bundle}/Contents/Info.plist" >/dev/null
plutil -lint "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" \
  "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings" >/dev/null
test "$(plutil -extract tsInputMethodCharacterRepertoireKey.0 raw \
  "${bundle}/Contents/Info.plist")" = "Hans"
test "$(plutil -extract tsInputMethodIconFileKey raw \
  "${bundle}/Contents/Info.plist")" = "RadishLexInputIcon.tiff"
test "$(plutil -extract TISInputSourceID raw \
  "${bundle}/Contents/Info.plist")" = "org.radishlex.inputmethod"
test "$(plutil -extract TISIntendedLanguage raw \
  "${bundle}/Contents/Info.plist")" = "zh-Hans"
mode_path=":ComponentInputModeDict:tsInputModeListKey:org.radishlex.inputmethod.Pinyin"
mode_id="$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISInputSourceID" \
  "${bundle}/Contents/Info.plist")"
test "${mode_id}" = "org.radishlex.inputmethod.Pinyin"
[[ "${mode_id}" =~ ^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$ ]]
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIntendedLanguage" \
  "${bundle}/Contents/Info.plist")" = "zh-Hans"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeIsVisibleKey" \
  "${bundle}/Contents/Info.plist")" = "true"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeScriptKey" \
  "${bundle}/Contents/Info.plist")" = "smSimpChinese"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeCharacterRepertoireKey:0" \
  "${bundle}/Contents/Info.plist")" = "Hans"
test "$(plutil -extract ComponentInputModeDict.tsVisibleInputModeOrderedArrayKey.0 raw \
  "${bundle}/Contents/Info.plist")" = "org.radishlex.inputmethod.Pinyin"
test "$(plutil -extract InputMethodServerDelegateClass raw \
  "${bundle}/Contents/Info.plist")" = "RadishLexInputController"
test "$(plutil -extract LSBackgroundOnly raw "${bundle}/Contents/Info.plist")" = "true"
test "$(/usr/libexec/PlistBuddy -c 'Print :org.radishlex.inputmethod.Pinyin' \
  "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings")" = "萝卜词核拼音"
otool -L "${bundle}/Contents/MacOS/RadishLex" | grep -q \
  "@rpath/libradishlex_ime_ffi.dylib"
nm -gU "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib" | grep -q \
  "_radishlex_session_handle_key_event"

echo "macOS InputMethodKit bundle and wrapper contract checks passed."
