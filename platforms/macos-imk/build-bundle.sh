#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
mode="${1:-contract}"
cd "${repo_root}"

case "${mode}" in
  contract)
    cargo_profile="debug"
    schema="contract.demo"
    compile_mode=1
    cargo build -p radishlex-ime-ffi
    ;;
  native)
    : "${RIME_INCLUDE_DIR:?native bundle requires RIME_INCLUDE_DIR}"
    : "${RIME_LIB_DIR:?native bundle requires RIME_LIB_DIR}"
    : "${RADISHLEX_RIME_SHARED_DATA:?native bundle requires isolated RADISHLEX_RIME_SHARED_DATA}"
    : "${RADISHLEX_RIME_SCHEMA:?native bundle requires RADISHLEX_RIME_SCHEMA}"
    if [[ ! -d "${RADISHLEX_RIME_SHARED_DATA}" ]]; then
      echo "RADISHLEX_RIME_SHARED_DATA must be an existing isolated directory." >&2
      exit 2
    fi
    cargo_profile="release"
    schema="${RADISHLEX_RIME_SCHEMA}"
    compile_mode=0
    cargo build -p radishlex-ime-ffi --features native-rime --release
    ;;
  *)
    echo "usage: $0 [contract|native]" >&2
    exit 2
    ;;
esac

if [[ ! "${schema}" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "Rime schema id must contain only ASCII letters, digits, dot, underscore or hyphen." >&2
  exit 2
fi

build_root="${repo_root}/target/macos-imk/${mode}"
bundle="${build_root}/RadishLex.inputmethod"
contents="${bundle}/Contents"
macos_dir="${contents}/MacOS"
frameworks_dir="${contents}/Frameworks"
resources_dir="${contents}/Resources"
export CLANG_MODULE_CACHE_PATH="${repo_root}/target/macos-imk/clang-module-cache"

rm -rf "${bundle}"
mkdir -p "${macos_dir}" "${frameworks_dir}" "${resources_dir}" \
  "${CLANG_MODULE_CACHE_PATH}"

ffi_dylib="${repo_root}/target/${cargo_profile}/libradishlex_ime_ffi.dylib"
cp "${ffi_dylib}" "${frameworks_dir}/"
install_name_tool -id "@rpath/libradishlex_ime_ffi.dylib" \
  "${frameworks_dir}/libradishlex_ime_ffi.dylib"
if [[ "${mode}" == "native" ]]; then
  install_name_tool -add_rpath "${RIME_LIB_DIR}" \
    "${frameworks_dir}/libradishlex_ime_ffi.dylib"
fi

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE="${compile_mode}" \
  -I"${script_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${script_dir}/Sources/RadishLexBridge.m" \
  "${script_dir}/Sources/RadishLexRuntime.m" \
  "${script_dir}/Sources/RadishLexInputController.m" \
  "${script_dir}/Sources/main.m" \
  -L"${frameworks_dir}" -lradishlex_ime_ffi \
  -Wl,-rpath,@executable_path/../Frameworks \
  -framework Cocoa -framework Carbon -framework InputMethodKit \
  -o "${macos_dir}/RadishLex"

sed "s/__RADISHLEX_RIME_SCHEMA__/${schema}/g" \
  "${script_dir}/Resources/Info.plist.in" >"${contents}/Info.plist"
plutil -lint "${contents}/Info.plist" >/dev/null

if [[ "${mode}" == "native" ]]; then
  ditto "${RADISHLEX_RIME_SHARED_DATA}" "${resources_dir}/RimeData"
fi

echo "Built ${bundle}"
