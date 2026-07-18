#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/macos-imk"
cleanup_script="${platform_dir}/cleanup-user-install.sh"
cleanup_wrapper="${repo_root}/scripts/cleanup-macos-imk.sh"
stop_wrapper="${repo_root}/scripts/stop-macos-imk-process.sh"
cleanup_sources=("${cleanup_script}" "${cleanup_wrapper}" "${stop_wrapper}")
privacy_script="${platform_dir}/privacy-mode.sh"
privacy_wrapper="${repo_root}/scripts/manage-macos-imk-privacy-mode.sh"
privacy_tool_source="${platform_dir}/Tools/privacy_mode_control.m"
privacy_contract="${platform_dir}/Tests/privacy_mode_contract.sh"
r01b_userdb_cleanup="${platform_dir}/cleanup-r01b-test-userdb.sh"
r01b_userdb_cleanup_wrapper="${repo_root}/scripts/cleanup-macos-imk-r01b-test-userdb.sh"
r01b_userdb_cleanup_helper="${platform_dir}/Tools/test_data_cleanup.c"
r01b_userdb_helper_contract="${platform_dir}/Tests/r01b_test_userdb_cleanup_helper_contract.sh"
r01b_userdb_orchestration_contract="${platform_dir}/Tests/r01b_test_userdb_cleanup_orchestration_contract.sh"
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
bash -n "${cleanup_script}" \
  "${cleanup_wrapper}" \
  "${stop_wrapper}" \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh" \
  "${privacy_script}" \
  "${privacy_wrapper}" \
  "${privacy_contract}" \
  "${r01b_userdb_cleanup}" \
  "${r01b_userdb_cleanup_wrapper}" \
  "${r01b_userdb_helper_contract}" \
  "${r01b_userdb_orchestration_contract}" \
  "${platform_dir}/ValidationHost/build-bundle.sh" \
  "${platform_dir}/ValidationHost/check.sh"
if rg -n '/((usr/)?bin|usr/sbin|sbin)/' "${cleanup_sources[@]}" | \
  rg -v ':1:#!/usr/bin/env bash$'; then
  echo "macOS cleanup must not bypass the isolated contract with absolute executables." >&2
  exit 1
fi
if rg -n "(^|[;|()={[:space:]])[\"']?/[^/[:space:]\"']" \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not contain literal absolute paths outside its shebang." >&2
  exit 1
fi
if rg -n "&[[:space:]]*([\"']/|/[^/[:space:]\"']+/)" \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not invoke a literal absolute path after a control operator." >&2
  exit 1
fi
if rg -n '(^|[;&|(){}[:space:]])(ash|bash|csh|dash|fish|ksh|pwsh|sh|tcsh|xonsh|zsh)([;&|(){}[:space:]]|$)' \
  "${cleanup_sources[@]}" | rg -v ':1:#!/usr/bin/env bash$'; then
  echo "macOS cleanup must not invoke another shell inside the isolated contract." >&2
  exit 1
fi
if rg -n '\$(\{)?(SHELL|BASH)(\}|([^A-Za-z0-9_])|$)' \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not invoke a shell through SHELL or BASH variables." >&2
  exit 1
fi
if rg -n 'RADISHLEX_CLEANUP_CONTRACT_' "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not read or replace contract-owned expectations." >&2
  exit 1
fi
if rg -n '(^|[;&|()[:space:]])(HOME|PATH|BASH_ENV|ENV|SHELLOPTS)[[:space:]]*(\+)?=' \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not replace the isolated contract environment." >&2
  exit 1
fi
if rg -n '(^|[;&|()[:space:]])(unset|local|declare|typeset|export|readonly)([;&|()[:space:]][^;&|()]*)?[;&|()[:space:]](HOME|PATH|BASH_ENV|ENV|SHELLOPTS)([;&|()[:space:]]|$)' \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup must not unset, shadow or redeclare the isolated contract environment." >&2
  exit 1
fi
if rg -n '(^|[;&|()[:space:]])(alias|builtin|command|enable|env|eval|function|hash|nice|nohup|source|unset|kill|killall|unlink|osascript|launchctl|xargs|python3?|perl|ruby|node|swift|ln|mv|cp|install|rsync|ditto)([;&|()[:space:]]|$)|(^|[;&|()[:space:]])\.[[:space:]]+|-(exec|delete)([[:space:]]|$)' \
  "${cleanup_sources[@]}"; then
  echo "macOS cleanup contains a command that bypasses fixed-path contract interception." >&2
  exit 1
fi
if rg -n '(^|[;&|(){}[:space:]])exec[[:space:]]+' "${cleanup_sources[@]}" | \
  rg -v '^[^:]+:[0-9]+:[[:space:]]*exec "\$\{status_tool\}" --monitor$|^[^:]+:[0-9]+:exec "\$\{repo_root\}/platforms/macos-imk/cleanup-user-install\.sh" "\$@"$|^[^:]+:[0-9]+:exec "\$\{repo_root\}/platforms/macos-imk/cleanup-user-install\.sh" \\$'; then
  echo "macOS cleanup may only exec the isolated status tool or cleanup wrapper target." >&2
  exit 1
fi
rg -Fxq '  exec "${status_tool}" --monitor' "${cleanup_script}"
rg -Fxq 'exec "${repo_root}/platforms/macos-imk/cleanup-user-install.sh" "$@"' \
  "${cleanup_wrapper}"
rg -Fq 'if [[ $# -ne 1 || "${1}" != "--authorized-stop-process" ]]; then' \
  "${stop_wrapper}"
rg -Fxq 'exec "${repo_root}/platforms/macos-imk/cleanup-user-install.sh" \' \
  "${stop_wrapper}"
rg -Fxq '  --authorized-stop-process' "${stop_wrapper}"
if rg -n '^[[:space:]]*rm[[:space:]]' \
  "${cleanup_sources[@]}" | \
  rg -v 'rm -rf "\$\{(installed_bundle|runtime_data)\}"$'; then
  echo "macOS cleanup may only remove the fixed bundle and Rime runtime paths." >&2
  exit 1
fi
rg -Fq 'pids="$(pgrep -x "${process_name}" 2>/dev/null)"' "${cleanup_script}"
rg -Fq '! command_line="$(ps -ww -p "${pid}" -o command= 2>/dev/null)"; then' \
  "${cleanup_script}"
rg -Fq 'pkill -f "${process_pattern}" 2>/dev/null || true' "${cleanup_script}"
if [[ "$(rg -o '\b(pgrep|ps|pkill)\b' "${cleanup_sources[@]}" | wc -l | tr -d ' ')" != "3" ]]; then
  echo "macOS cleanup process inspection/termination commands changed unexpectedly." >&2
  exit 1
fi
rg -Fq 'PATH="${fake_bin}" \' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fq 'cd "${contract_root}"' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fq 'for command_name in clang dirname find grep mkdir pgrep ps pkill rm sed sleep stat; do' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fq 'rm stub received a path outside the fixed cleanup targets' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
if rg -n 'ln -s /((usr/)?bin|usr/sbin|sbin)/(dirname|find|grep|mkdir|sed|stat)' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"; then
  echo "macOS cleanup contract must intercept path-observing commands with argument checks." >&2
  exit 1
fi
rg -Fq 'set_enabled_mode_then_zero_tis_state' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fq 'set_zero_then_residual_tis_state' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fq 'assert_two_tis_calls' \
  "${platform_dir}/Tests/cleanup_user_install_contract.sh"
if rg -n 'TIS(Select|Disable|Enable|Register|Deregister)InputSource|CFPreferencesSet|NSUserDefaults|com\.apple\.HIToolbox|defaults (write|delete)' \
  "${cleanup_script}" \
  "${platform_dir}/Tools/tis_source_status.m"; then
  echo "macOS cleanup must not mutate TIS or HIToolbox private state." >&2
  exit 1
fi
"${platform_dir}/Tests/cleanup_user_install_contract.sh"
rg -Fxq 'exec "${repo_root}/platforms/macos-imk/privacy-mode.sh" "$@"' \
  "${privacy_wrapper}"
rg -Fq 'org.radishlex.inputmethod.macos' "${privacy_tool_source}"
rg -Fq 'RadishLexPrivacyMode' "${privacy_tool_source}"
rg -Fq 'CFPreferencesCopyValue' "${privacy_tool_source}"
rg -Fq 'CFPreferencesSetValue' "${privacy_tool_source}"
rg -Fq 'CFPreferencesSynchronize' "${privacy_tool_source}"
rg -Fq 'RLX_PRIVACY_STATE_DIR' "${privacy_tool_source}"
rg -Fq 'openat(' "${privacy_tool_source}"
rg -Fq 'fstatat(' "${privacy_tool_source}"
rg -Fq 'unlinkat(' "${privacy_tool_source}"
rg -Fq 'device=%llu' "${privacy_tool_source}"
rg -Fq 'inode=%llu' "${privacy_tool_source}"
rg -Fq 'privacy baseline already exists or is unsafe' "${privacy_tool_source}"
rg -Fq 'state_dir_define="-DRLX_PRIVACY_STATE_DIR=\"${state_dir}\""' \
  "${privacy_script}"
if rg -n 'CFPreferences(Copy|Set)AppValue|(^|[[:space:]])defaults([[:space:]]|$)|com\.apple\.HIToolbox|TIS(Select|Disable|Enable|Register|Deregister)InputSource' \
  "${privacy_script}" "${privacy_wrapper}" "${privacy_tool_source}"; then
  echo "macOS privacy control must use the fixed exact preference layer only." >&2
  exit 1
fi
clang -fobjc-arc -fmodules -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_PRIVACY_CONTRACT=0 \
  '-DRLX_PRIVACY_STATE_DIR="/private/tmp/radishlex-privacy-contract-state"' \
  "${privacy_tool_source}"
"${privacy_contract}"
rg -Fq 'if [[ $# -ne 1 || \' "${r01b_userdb_cleanup_wrapper}"
rg -Fq '"${1}" != "--capture-baseline"' "${r01b_userdb_cleanup_wrapper}"
rg -Fq '"${1}" != "--authorized-delete-r01b-test-userdb"' \
  "${r01b_userdb_cleanup_wrapper}"
rg -Fxq 'exec "${repo_root}/platforms/macos-imk/cleanup-r01b-test-userdb.sh" "$1"' \
  "${r01b_userdb_cleanup_wrapper}"
rg -Fq 'userdb_files=(' "${r01b_userdb_cleanup}"
rg -Fq 'lsof_output="$(lsof -n -P -- "${path}" 2>&1)"' \
  "${r01b_userdb_cleanup}"
rg -Fq 'require_status_value "matches" "0 enabled=0 selected=0"' \
  "${r01b_userdb_cleanup}"
rg -Fq 'require_status_value "application_support_parent_kind" "empty_directory"' \
  "${r01b_userdb_cleanup}"
rg -Fq 'require_status_value "application_support_parent_mode" "755"' \
  "${r01b_userdb_cleanup}"
if rg -n '^[[:space:]]*rm[[:space:]]|rm -rf|find .*-(delete|exec)|(^|[[:space:]])xargs([[:space:]]|$)' \
  "${r01b_userdb_cleanup}" "${r01b_userdb_cleanup_wrapper}"; then
  echo "R01B test userdb cleanup must not expose general path deletion." >&2
  exit 1
fi
rg -Fq '"userdb.sqlite3"' "${r01b_userdb_cleanup_helper}"
rg -Fq '"userdb.sqlite3-wal"' "${r01b_userdb_cleanup_helper}"
rg -Fq '"userdb.sqlite3-shm"' "${r01b_userdb_cleanup_helper}"
rg -Fq '"userdb.sqlite3-journal"' "${r01b_userdb_cleanup_helper}"
rg -Fq 'unlinkat(parent_fd, kTestDataNames[index], 0)' \
  "${r01b_userdb_cleanup_helper}"
rg -Fq 'AT_SYMLINK_NOFOLLOW' "${r01b_userdb_cleanup_helper}"
if rg -n '(^|[^A-Za-z])(remove|rename|system|popen)[[:space:]]*\(' \
  "${r01b_userdb_cleanup_helper}"; then
  echo "R01B test userdb helper contains a general path or process primitive." >&2
  exit 1
fi
clang -std=c11 -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  '-DRLX_TEST_DATA_STATE_DIR="/private/tmp/radishlex-r01b-contract-state"' \
  "${r01b_userdb_cleanup_helper}"
"${r01b_userdb_helper_contract}"
"${r01b_userdb_orchestration_contract}"
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
  "${cleanup_script}"
rg -q 'org\.radishlex\.inputmethod\.macos' \
  "${cleanup_script}"
rg -Fq 'input_methods_parent="${library_dir}/Input Methods"' "${cleanup_script}"
rg -Fq 'installed_bundle="${input_methods_parent}/RadishLexInputMethod.app"' \
  "${cleanup_script}"
rg -Fq 'application_support_root="${library_dir}/Application Support"' \
  "${cleanup_script}"
rg -Fq 'application_support_parent="${application_support_root}/RadishLex"' \
  "${cleanup_script}"
rg -Fq 'runtime_data="${application_support_parent}/Rime"' \
  "${cleanup_script}"
rg -q 'application_support_parent=' \
  "${cleanup_script}"
rg -q 'application_support_parent_kind=' \
  "${cleanup_script}"
rg -q 'application_support_parent_mode=' \
  "${cleanup_script}"
rg -q 'cleanup_path_ancestors=' "${cleanup_script}"
rg -q 'userdb=' "${cleanup_script}"
rg -q 'userdb_sidecars=' "${cleanup_script}"
if [[ "$(rg -c '^require_safe_cleanup_paths$' "${cleanup_script}")" != "2" ]]; then
  echo "macOS cleanup must recheck fixed-path ancestors before deletion." >&2
  exit 1
fi
clang -fobjc-arc -fmodules -Wall -Wextra -Werror -fsyntax-only \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE=0 \
  -I"${platform_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${platform_dir}/Sources/RadishLexBridge.m" \
  "${platform_dir}/Sources/RadishLexCandidatePanel.m" \
  "${platform_dir}/Sources/RadishLexLearningContext.m" \
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
  "${platform_dir}/Sources/RadishLexLearningContext.m" \
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
  "${platform_dir}/Sources/RadishLexLearningContext.m" \
  "${platform_dir}/Sources/RadishLexRuntime.m" \
  "${platform_dir}/Sources/RadishLexInputController.m" \
  "${platform_dir}/Tests/input_controller_contract.m" \
  -L"${repo_root}/target/debug" -lradishlex_ime_ffi \
  -Wl,-rpath,"${repo_root}/target/debug" \
  -framework AppKit -framework Carbon -framework InputMethodKit \
  -o "${smoke_dir}/input-controller-contract"
"${smoke_dir}/input-controller-contract"
"${platform_dir}/ValidationHost/check.sh"

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
grep -Fqx '  page_size: 5' \
  "${platform_dir}/Resources/Rime/default.yaml.in"
plutil -lint "${bundle}/Contents/Info.plist" >/dev/null
test "$(plutil -extract CFBundleVersion raw "${bundle}/Contents/Info.plist")" = "34"
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
  "${platform_dir}/Sources/RadishLexLearningContext.m"
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
