#!/usr/bin/env bash
set -euo pipefail

# Keep staged runtime resources non-writable by group or other regardless of
# the invoking developer account's default umask.
umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/linux-fcitx5"
require_fcitx=0

if [[ "${1:-}" == "--require-fcitx" ]]; then
  require_fcitx=1
elif [[ "$#" -ne 0 ]]; then
  echo "usage: $0 [--require-fcitx]" >&2
  exit 2
fi

cxx="${CXX:-c++}"
if ! command -v "${cxx}" >/dev/null 2>&1; then
  echo "A C++17 compiler is required for the Linux Fcitx5 contract." >&2
  exit 1
fi

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-linux-contract.XXXXXX")"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

common_flags=(
  -std=c++17
  -Wall
  -Wextra
  -Wpedantic
  -Werror
  "-I${platform_dir}/include"
  "-I${repo_root}/crates/ime-ffi/include"
)

"${cxx}" "${common_flags[@]}" \
  "${platform_dir}/tests/ffi_projection_test.cpp" \
  "${platform_dir}/src/ffi_projection.cpp" \
  "${platform_dir}/src/key_projection.cpp" \
  -o "${temp_dir}/ffi_projection_test"
"${temp_dir}/ffi_projection_test"

"${cxx}" "${common_flags[@]}" -DRADISHLEX_XDG_TESTING=1 \
  "${platform_dir}/tests/xdg_paths_test.cpp" \
  "${platform_dir}/src/xdg_paths.cpp" \
  -o "${temp_dir}/xdg_paths_test"
"${temp_dir}/xdg_paths_test"

if [[ "$(uname -s)" == "Linux" ]]; then
  "${cxx}" "${common_flags[@]}" \
    "${platform_dir}/tests/runtime_layout_test.cpp" \
    "${platform_dir}/src/runtime_layout.cpp" \
    -ldl \
    -o "${temp_dir}/runtime_layout_test"
else
  "${cxx}" "${common_flags[@]}" \
    "${platform_dir}/tests/runtime_layout_test.cpp" \
    "${platform_dir}/src/runtime_layout.cpp" \
    -o "${temp_dir}/runtime_layout_test"
fi
"${temp_dir}/runtime_layout_test"

"${cxx}" "${common_flags[@]}" -c \
  "${platform_dir}/src/linked_ffi_api.cpp" \
  -o "${temp_dir}/linked_ffi_api.o"

for asset in default.yaml radishlex_pinyin.schema.yaml pinyin_simp.dict.yaml; do
  test -f "${repo_root}/packaging/rime/data/${asset}"
done

rg -q 'find_package\(Fcitx5Core 5\.1\.9 REQUIRED\)' \
  "${platform_dir}/CMakeLists.txt"
rg -q 'Fcitx5::Core' "${platform_dir}/CMakeLists.txt"
rg -q 'radishlex_ime_ffi' "${platform_dir}/CMakeLists.txt"
rg -q 'inputPanel\(\)' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'resolveLoadedRuntimeLayout' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'session_select_candidate' \
  "${platform_dir}/src/ffi_projection.cpp" \
  "${platform_dir}/src/linked_ffi_api.cpp"
for source in runtime.rs session.rs; do
  rg -q 'use std::ffi::.*c_char' \
    "${repo_root}/crates/ime-engine-rime/src/${source}"
done

if rg -n '\*const i8|\*mut i8|0_i8' \
  "${repo_root}/crates/ime-engine-rime/src/runtime.rs" \
  "${repo_root}/crates/ime-engine-rime/src/session.rs"; then
  echo "Rime C char values must use std::ffi::c_char across targets." >&2
  exit 1
fi

if rg -n '#include[[:space:]]*[<"].*(rime|sqlite|curl)' \
  "${platform_dir}/src/fcitx_addon.cpp" \
  "${platform_dir}/src/fcitx_addon.h"; then
  echo "Fcitx5 addon must not include Rime, SQLite, or transport APIs." >&2
  exit 1
fi

if [[ "${require_fcitx}" -eq 1 ]]; then
  if [[ "$(uname -s)" != "Linux" ]]; then
    echo "--require-fcitx is only valid in a real Linux build environment." >&2
    exit 1
  fi
  if ! command -v cmake >/dev/null 2>&1; then
    echo "cmake is required for the real Fcitx5 addon build." >&2
    exit 1
  fi
  if [[ -z "${RADISHLEX_IME_FFI_LIBRARY:-}" ]]; then
    echo "RADISHLEX_IME_FFI_LIBRARY must point to the native-rime cdylib." >&2
    exit 1
  fi
  cmake -S "${platform_dir}" -B "${temp_dir}/cmake-build" \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DRADISHLEX_BUILD_FCITX_ADDON=ON \
    -DRADISHLEX_BUILD_CONTRACT_TESTS=ON \
    "-DRADISHLEX_IME_FFI_LIBRARY=${RADISHLEX_IME_FFI_LIBRARY}"
  cmake --build "${temp_dir}/cmake-build"
  test -f "${temp_dir}/cmake-build/radishlex.so"
  file "${temp_dir}/cmake-build/radishlex.so"
  ldd "${temp_dir}/cmake-build/radishlex.so"
  DESTDIR="${temp_dir}/stage" \
    cmake --install "${temp_dir}/cmake-build"
  addon_library="$(
    find "${temp_dir}/stage" -type f -name radishlex.so -print -quit
  )"
  test -n "${addon_library}"
  addon_directory="$(dirname "${addon_library}")"
  test -f "${addon_directory}/libradishlex_ime_ffi.so"
  for asset in default.yaml radishlex_pinyin.schema.yaml pinyin_simp.dict.yaml; do
    test -f "${addon_directory}/radishlex-rime/${asset}"
  done
  test -f "${temp_dir}/stage/usr/share/fcitx5/addon/radishlex.conf"
  test -f "${temp_dir}/stage/usr/share/fcitx5/inputmethod/radishlex.conf"
  file "${addon_library}"
  ldd "${addon_library}"
  "${temp_dir}/cmake-build/radishlex_runtime_probe" "${addon_library}"
  readelf -d "${addon_library}" | rg -q '\$ORIGIN'
  if readelf -d "${addon_library}" |
      rg -n '/tmp|/workspace|RadishLex'; then
    echo "Installed addon dynamic metadata contains a build path." >&2
    exit 1
  fi
  if strings "${addon_library}" |
      rg -n '/tmp/radishlex|/workspace/RadishLex'; then
    echo "Installed addon contains a temporary or repository path." >&2
    exit 1
  fi
  ctest --test-dir "${temp_dir}/cmake-build" --output-on-failure
  echo "Linux Fcitx5 addon build, assembly, and contracts passed."
else
  echo "Linux platform-independent Fcitx5 contracts passed."
  echo "The real Fcitx5 addon target was not built; use --require-fcitx on Linux."
fi
