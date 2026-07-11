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

bundle="${repo_root}/target/macos-imk/contract/RadishLex.inputmethod"
test -x "${bundle}/Contents/MacOS/RadishLex"
test -f "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib"
plutil -lint "${bundle}/Contents/Info.plist" >/dev/null
otool -L "${bundle}/Contents/MacOS/RadishLex" | grep -q \
  "@rpath/libradishlex_ime_ffi.dylib"
nm -gU "${bundle}/Contents/Frameworks/libradishlex_ime_ffi.dylib" | grep -q \
  "_radishlex_session_handle_key_event"

echo "macOS InputMethodKit bundle and wrapper contract checks passed."
