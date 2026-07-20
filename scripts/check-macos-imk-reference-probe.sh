#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
probe_dir="${repo_root}/platforms/macos-imk/ReferenceProbe"
build_root="${repo_root}/target/macos-imk/reference-probe-mode"
bundle="${build_root}/RadishLexIMKModeReferenceProbe.app"
plist="${bundle}/Contents/Info.plist"
executable="${bundle}/Contents/MacOS/RadishLexIMKModeReferenceProbe"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the InputMethodKit reference probe checks." >&2
  exit 2
fi

export CLANG_MODULE_CACHE_PATH="${build_root}/clang-module-cache"
mkdir -p "${build_root}/contract-smoke" "${CLANG_MODULE_CACHE_PATH}"

"${probe_dir}/build-bundle.sh"

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -I"${probe_dir}/Sources" \
  "${probe_dir}/Sources/ReferenceProbeState.m" \
  "${probe_dir}/Tests/contract_smoke.m" \
  -framework AppKit -framework Carbon \
  -o "${build_root}/contract-smoke/reference-probe-contract"
"${build_root}/contract-smoke/reference-probe-contract"

mkdir -p "${build_root}/tools"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "${repo_root}/platforms/macos-imk/Tools/tis_source_status.m" \
  -framework Carbon -framework Foundation \
  -o "${build_root}/tools/tis-source-status"

test -x "${executable}"
test -s "${bundle}/Contents/Resources/ReferenceProbeIcon.tiff"
test -s "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"
test -s "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings"
plutil -lint "${plist}" \
  "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings" \
  "${bundle}/Contents/Resources/en.lproj/InfoPlist.strings" >/dev/null

test "$(plutil -extract CFBundleIdentifier raw "${plist}")" = \
  "org.radishlex.inputmethod.macos.reference-probe-mode"
test "$(plutil -extract TISInputSourceID raw "${plist}")" = \
  "org.radishlex.inputmethod.macos.reference-probe-mode"
test "$(plutil -extract TISIntendedLanguage raw "${plist}")" = "zh-Hans"
test "$(plutil -extract tsInputMethodCharacterRepertoireKey.0 raw "${plist}")" = "Hans"
test "$(plutil -extract tsInputMethodIconFileKey raw "${plist}")" = \
  "ReferenceProbeIcon.tiff"
test "$(plutil -extract LSUIElement raw "${plist}")" = "true"
mode_path=":ComponentInputModeDict:tsInputModeListKey:org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:TISInputSourceID" \
  "${plist}"; })" = "org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIntendedLanguage" \
  "${plist}"; })" = "zh-Hans"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeIsVisibleKey" \
  "${plist}"; })" = "true"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeScriptKey" \
  "${plist}"; })" = "smSimpChinese"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:tsInputModeMenuIconFileKey" \
  "${plist}"; })" = "ReferenceProbeIcon.tiff"
test "$({ /usr/libexec/PlistBuddy -c "Print ${mode_path}:TISIconLabels:Primary" \
  "${plist}"; })" = "测"
test "$(plutil -extract ComponentInputModeDict.tsVisibleInputModeOrderedArrayKey.0 raw \
  "${plist}")" = "org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin"
test "$({ /usr/libexec/PlistBuddy -c \
  'Print :org.radishlex.inputmethod.macos.reference-probe-mode' \
  "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"; })" = \
  "萝卜词核模式探针"
test "$({ /usr/libexec/PlistBuddy -c \
  'Print :org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin' \
  "${bundle}/Contents/Resources/zh-Hans.lproj/InfoPlist.strings"; })" = \
  "萝卜词核模式探针拼音"

for forbidden_key in InputMethodServerDelegateClass LSBackgroundOnly; do
  if plutil -extract "${forbidden_key}" raw "${plist}" >/dev/null 2>&1; then
    echo "Reference probe plist must not contain ${forbidden_key}." >&2
    exit 1
  fi
done

test "$({ otool -L "${executable}" || true; } | grep -c 'librime\|radishlex_ime_ffi')" = "0"
if rg -n -i 'librime|rime|Library/Rime|Application Support/RadishLex' \
  "${probe_dir}/Sources" "${probe_dir}/Resources"; then
  echo "Reference probe must not depend on Rime or RadishLex user data." >&2
  exit 1
fi
rg -q 'kIMKSingleRowSteppingCandidatePanel' \
  "${probe_dir}/Sources/ReferenceProbeController.m"
rg -q 'IMKCandidatesSendServerKeyEventFirst' \
  "${probe_dir}/Sources/ReferenceProbeController.m"
rg -q '\[self\.candidatePanel updateCandidates\]' \
  "${probe_dir}/Sources/ReferenceProbeController.m"
if rg -n 'setCandidateData|clearSelection|selectCandidateWithIdentifier' \
  "${probe_dir}/Sources"; then
  echo "Reference probe must use the candidates/updateCandidates callback model." >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "${bundle}"
echo "macOS InputMethodKit reference probe checks passed without installation."
