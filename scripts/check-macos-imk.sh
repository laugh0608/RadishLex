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
bash -n "${platform_dir}/cleanup-user-install.sh" \
  "${repo_root}/scripts/cleanup-macos-imk.sh"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "${platform_dir}/Tools/tis_source_status.m" \
  -framework Carbon -framework Foundation \
  -o "${smoke_dir}/tis-source-status"
test "$("${smoke_dir}/tis-source-status" \
  org.radishlex.inputmethod.macos.contract-status-check)" = \
  "matches=0 enabled=0 selected=0"
rg -q 'kTISNotifySelectedKeyboardInputSourceChanged' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'TISCopyCurrentKeyboardInputSource' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'CFNotificationCenterGetDistributedCenter' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'CFNotificationCenterAddObserver' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'CFRunLoopRun\(\)' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'PrintCurrentSource\("initial"\)' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'PrintCurrentSource\("changed"\)' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q 'is_radishlex_pinyin' \
  "${platform_dir}/Tools/tis_source_status.m"
rg -q -- '--monitor' "${platform_dir}/cleanup-user-install.sh"
rg -q -- '--authorized-after-settings-removal' \
  "${platform_dir}/cleanup-user-install.sh"
rg -q 'org\.radishlex\.inputmethod\.macos' \
  "${platform_dir}/cleanup-user-install.sh"
rg -q 'Library/Input Methods/RadishLexInputMethod\.app' \
  "${platform_dir}/cleanup-user-install.sh"
rg -q 'Application Support/RadishLex/Rime' \
  "${platform_dir}/cleanup-user-install.sh"
if rg -n 'TIS(Select|Disable|Enable|Register|Deregister)InputSource|CFPreferencesSet|NSUserDefaults|com\.apple\.HIToolbox|defaults (write|delete)' \
  "${platform_dir}/cleanup-user-install.sh" \
  "${platform_dir}/Tools/tis_source_status.m"; then
  echo "macOS cleanup must not mutate TIS or HIToolbox private state." >&2
  exit 1
fi
clang -fobjc-arc -fmodules -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=0 \
  -I"${platform_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Sources/RadishLexInputController.m" \
  "${platform_dir}/Sources/main.m"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=1 \
  -I"${platform_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Tests/contract_smoke.m" \
  -L"${repo_root}/target/debug" -lradishlex_ime_ffi \
  -Wl,-rpath,"${repo_root}/target/debug" \
  -framework AppKit -framework Carbon -framework InputMethodKit \
  -o "${smoke_dir}/contract-smoke"
"${smoke_dir}/contract-smoke"

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=1 \
  -I"${platform_dir}/Sources" \
  -I"${platform_dir}/Tests" \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m" \
  "${platform_dir}/Tests/candidate_panel_contract.m" \
  -framework AppKit -framework InputMethodKit \
  -o "${smoke_dir}/candidate-panel-contract"
"${smoke_dir}/candidate-panel-contract"

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=1 \
  -I"${platform_dir}/Sources" \
  -I"${platform_dir}/Tests" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Sources/RadishLexInputController.m" \
  "${platform_dir}/Tests/input_controller_contract.m" \
  -L"${repo_root}/target/debug" -lradishlex_ime_ffi \
  -Wl,-rpath,"${repo_root}/target/debug" \
  -framework AppKit -framework Carbon -framework InputMethodKit \
  -o "${smoke_dir}/input-controller-contract"
"${smoke_dir}/input-controller-contract"

bundle="${repo_root}/target/macos-imk/contract/RadishLexInputMethod.app"
test -x "${bundle}/Contents/MacOS/RadishLex"
test -f "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
test -s "${bundle}/Contents/Resources/RadishLexInputIcon.tiff"
test "$(sips -g pixelWidth "${bundle}/Contents/Resources/RadishLexInputIcon.tiff" 2>/dev/null | awk '/pixelWidth:/ { print $2 }')" = "32"
test "$(sips -g pixelHeight "${bundle}/Contents/Resources/RadishLexInputIcon.tiff" 2>/dev/null | awk '/pixelHeight:/ { print $2 }')" = "32"
test "$(sips -g dpiWidth "${bundle}/Contents/Resources/RadishLexInputIcon.tiff" 2>/dev/null | awk '/dpiWidth:/ { print $2 }')" = "144.000"
test "$(sips -g dpiHeight "${bundle}/Contents/Resources/RadishLexInputIcon.tiff" 2>/dev/null | awk '/dpiHeight:/ { print $2 }')" = "144.000"
test -s "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"
test -s "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings"
test -s "${bundle}/Contents/Resources/zh-Hans.lproj/Localizable.strings"
test -s "${bundle}/Contents/Resources/en.lproj/Localizable.strings"
plutil -lint "${bundle}/Contents/Info.plist" >/dev/null
test "$(plutil -extract CFBundleVersion raw "${bundle}/Contents/Info.plist")" = "31"
plutil -lint "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" \
  "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings" \
  "${bundle}/Contents/Resources/zh-Hans.lproj/Localizable.strings" \
  "${bundle}/Contents/Resources/en.lproj/Localizable.strings" >/dev/null
test "$(plutil -extract tsInputMethodCharacterRepertoireKey.0 raw \
  "${bundle}/Contents/Info.plist")" = "Hans"
test "$(plutil -extract TISIntendedLanguage raw \
  "${bundle}/Contents/Info.plist")" = "zh-Hans"
mode_path=":ComponentInputModeDict:tsInputModeListKey:org.radishlex.inputmethod.macos.Pinyin"
mode_id="$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISInputSourceID" \
  "${bundle}/Contents/Info.plist")"
test "${mode_id}" = "org.radishlex.inputmethod.macos.Pinyin"
[[ "${mode_id}" =~ ^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$ ]]
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIntendedLanguage" \
  "${bundle}/Contents/Info.plist")" = "zh-Hans"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeIsVisibleKey" \
  "${bundle}/Contents/Info.plist")" = "true"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeScriptKey" \
  "${bundle}/Contents/Info.plist")" = "smSimpChinese"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeCharacterRepertoireKey:0" \
  "${bundle}/Contents/Info.plist")" = "Hans"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeMenuIconFileKey" \
  "${bundle}/Contents/Info.plist")" = "RadishLexInputIcon.tiff"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModePaletteIconFileKey" \
  "${bundle}/Contents/Info.plist")" = "RadishLexInputIcon.tiff"
test "$(/usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIconLabels:Primary" \
  "${bundle}/Contents/Info.plist")" = "萝"
test "$(plutil -extract ComponentInputModeDict.tsVisibleInputModeOrderedArrayKey.0 raw \
  "${bundle}/Contents/Info.plist")" = "org.radishlex.inputmethod.macos.Pinyin"
if plutil -extract InputMethodServerDelegateClass raw \
  "${bundle}/Contents/Info.plist" >/dev/null 2>&1; then
  echo "macOS IMK bundle must not treat the input controller as a server delegate." >&2
  exit 1
fi
test "$(plutil -extract LSUIElement raw "${bundle}/Contents/Info.plist")" = "true"
if plutil -extract LSBackgroundOnly raw "${bundle}/Contents/Info.plist" >/dev/null 2>&1; then
  echo "macOS IMK bundle must use LSUIElement without LSBackgroundOnly." >&2
  exit 1
fi
test "$(/usr/libexec/PlistBuddy -c 'Print :org.radishlex.inputmethod.macos.Pinyin' \
  "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings")" = "萝卜词核拼音"
test "$(/usr/libexec/PlistBuddy -c 'Print :candidate_list' \
  "${bundle}/Contents/Resources/zh-Hans.lproj/Localizable.strings")" = "候选列表"
test "$(/usr/libexec/PlistBuddy -c 'Print :candidate_selected' \
  "${bundle}/Contents/Resources/en.lproj/Localizable.strings")" = "Selected"
otool -L "${bundle}/Contents/MacOS/RadishLex" | grep -q \
  "@rpath/libradishlex_ime_ffi.dylib"
nm -gU "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib" | grep -q \
  "_radishlex_session_handle_key_event"

product_sources=(
  "${platform_dir}/Sources/RadishLexInputController.m"
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
)
if rg -n 'IMKCandidates|selectCandidateWithIdentifier|candidateSelectionChanged:' \
  "${product_sources[@]}"; then
  echo "macOS production candidate flow must not depend on IMKCandidates selection state." >&2
  exit 1
fi
rg -q 'NSWindowStyleMaskNonactivatingPanel' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -Fq 'NSUInteger inlineCharacterIndex = 0;' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -Fq 'inlineCharacterIndex = MIN(cursorIndex, markedRange.length - 1)' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -Fq 'attributesForCharacterIndex:inlineCharacterIndex' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -Fq 'markedRange.location + MIN(cursorIndex, markedRange.length)' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -q '\[client windowLevel\].*\+ 1' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -q 'NSAccessibilityPostNotification' \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m"
rg -q '\[self\.candidatePanel setSelectedIndex:target owner:self\]' \
  "${platform_dir}/Sources/RadishLexInputController.m"
if rg -n 'NSMenu alloc|menuWithTitle|addItem' \
  "${platform_dir}/Sources/RadishLexInputController.m"; then
  echo "macOS M1 must not add placeholder input-method command menu items." >&2
  exit 1
fi

echo "macOS InputMethodKit bundle and wrapper contract checks passed."
