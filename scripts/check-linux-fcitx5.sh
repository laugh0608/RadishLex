#!/usr/bin/env bash
set -euo pipefail

# Keep staged runtime resources non-writable by group or other regardless of
# the invoking developer account's default umask.
umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/linux-fcitx5"
manager_linux_dir="${repo_root}/apps/radishlex-manager/linux"
firefox_evidence_fixture="${platform_dir}/evidence/firefox-context.html"
evidence_log_pattern='browser_candidate_[0-9]'
evidence_log_pattern+='|radishlex_application_evidence'
evidence_log_pattern+='|radishlex_capability_evidence'
evidence_log_pattern+='|password_program_unread'
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
  -DRADISHLEX_APPLICATION_CONTEXT_TESTING=1 \
  "${platform_dir}/tests/application_context_test.cpp" \
  "${platform_dir}/src/application_context.cpp" \
  "${platform_dir}/src/application_evidence.cpp" \
  -o "${temp_dir}/application_context_test"
"${temp_dir}/application_context_test"

"${cxx}" "${common_flags[@]}" \
  "${platform_dir}/tests/ffi_projection_test.cpp" \
  "${platform_dir}/src/ffi_projection.cpp" \
  "${platform_dir}/src/key_projection.cpp" \
  -o "${temp_dir}/ffi_projection_test"
"${temp_dir}/ffi_projection_test"

for startup_identity in 1 2; do
  "${cxx}" "${common_flags[@]}" \
    "-DRADISHLEX_LINUX_STARTUP_BUILD_IDENTITY=${startup_identity}" \
    "${platform_dir}/tests/product_startup_test.cpp" \
    "${platform_dir}/src/product_startup.cpp" \
    -o "${temp_dir}/product_startup_test_${startup_identity}"
  "${temp_dir}/product_startup_test_${startup_identity}"
done

"${cxx}" "${common_flags[@]}" -DRADISHLEX_XDG_TESTING=1 \
  "${platform_dir}/tests/xdg_paths_test.cpp" \
  "${platform_dir}/src/xdg_paths.cpp" \
  -o "${temp_dir}/xdg_paths_test"
"${temp_dir}/xdg_paths_test"

"${cxx}" "${common_flags[@]}" \
  -DRADISHLEX_MANAGER_RUNTIME_TESTING=1 \
  -DRADISHLEX_XDG_TESTING=1 \
  "${platform_dir}/tests/manager_runtime_test.cpp" \
  "${platform_dir}/src/manager_runtime.cpp" \
  "${platform_dir}/src/privacy_mode.cpp" \
  "${platform_dir}/src/xdg_paths.cpp" \
  -o "${temp_dir}/manager_runtime_test"
"${temp_dir}/manager_runtime_test"

if [[ "$(uname -s)" == "Linux" ]]; then
  "${cxx}" "${common_flags[@]}" -DRADISHLEX_XDG_TESTING=1 \
    "${platform_dir}/tests/privacy_monitor_test.cpp" \
    "${platform_dir}/src/privacy_monitor.cpp" \
    "${platform_dir}/src/privacy_mode.cpp" \
    "${platform_dir}/src/xdg_paths.cpp" \
    -o "${temp_dir}/privacy_monitor_test"
  "${temp_dir}/privacy_monitor_test"
fi

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

"${cxx}" "${common_flags[@]}" \
  '-DRADISHLEX_SYSTEM_RIME_DATA_DIR="/usr/share/radishlex/rime"' \
  "${platform_dir}/tests/system_runtime_layout_test.cpp" \
  "${platform_dir}/src/runtime_layout.cpp" \
  -o "${temp_dir}/system_runtime_layout_test"
"${temp_dir}/system_runtime_layout_test"

"${cxx}" "${common_flags[@]}" -c \
  "${platform_dir}/src/linked_ffi_api.cpp" \
  -o "${temp_dir}/linked_ffi_api.o"
"${cxx}" "${common_flags[@]}" -c \
  "${platform_dir}/src/linked_product_startup.cpp" \
  -o "${temp_dir}/linked_product_startup.o"

for asset in default.yaml radishlex_pinyin.schema.yaml pinyin_simp.dict.yaml; do
  test -f "${repo_root}/packaging/rime/data/${asset}"
done

rg -q 'find_package\(Fcitx5Core 5\.1\.9 REQUIRED\)' \
  "${platform_dir}/CMakeLists.txt"
rg -Fq 'RADISHLEX_RUNTIME_LAYOUT_PROFILE "staged"' \
  "${platform_dir}/CMakeLists.txt"
rg -Fq 'RADISHLEX_SYSTEM_RIME_DATA_DIR="/usr/share/radishlex/rime"' \
  "${platform_dir}/CMakeLists.txt"
rg -Fq 'RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY=2' \
  "${platform_dir}/CMakeLists.txt"
rg -q 'Fcitx5::Core' "${platform_dir}/CMakeLists.txt"
rg -q 'radishlex_ime_ffi' "${platform_dir}/CMakeLists.txt"
rg -Fq 'umask(0077);' "${manager_linux_dir}/runner/main.cc"
rg -Fq 'authorizeLinkedStartup' "${manager_linux_dir}/runner/main.cc"
rg -Fq 'dev.radishlex.manager/runtime' \
  "${manager_linux_dir}/runner/manager_runtime_bridge.cc"
rg -Fq 'resolveManagerRuntimePaths' \
  "${manager_linux_dir}/runner/manager_runtime_bridge.cc"
rg -Fq 'RADISHLEX_MANAGER_FFI_LIBRARY' \
  "${manager_linux_dir}/CMakeLists.txt"
if rg -n 'RADISHLEX_MANAGER_FFI_LIBRARY|LD_LIBRARY_PATH' \
  "${repo_root}/apps/radishlex-manager/lib" \
  "${manager_linux_dir}/runner" \
  "${platform_dir}/src/manager_runtime.cpp"; then
  echo "Linux Manager runtime must not accept a native-library path override." >&2
  exit 1
fi
rg -Fq 'readlink("/proc/self/exe"' \
  "${platform_dir}/src/manager_runtime.cpp"
rg -Fq 'libradishlex_ime_ffi.so' \
  "${platform_dir}/src/manager_runtime.cpp"
rg -q 'inputPanel\(\)' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'candidateListHandlesKey' \
  "${platform_dir}/src/fcitx_addon.cpp" \
  "${platform_dir}/src/fcitx_candidate_key.cpp"
if rg -n 'states\(\)\.toInteger\(\)[[:space:]]*==[[:space:]]*0' \
  "${platform_dir}/src/fcitx_addon.cpp"; then
  echo "Fcitx candidate keys must use Fcitx matching semantics." >&2
  exit 1
fi
rg -q 'projectApplicationContext' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'addIOEvent' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'program\(\)' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'InputContextCapabilityChanged' "${platform_dir}/src/fcitx_addon.cpp"
rg -q 'radishlex_capability_evidence state=password_' \
  "${platform_dir}/src/fcitx_addon.cpp"
rg -Fq 'type="text"' "${firefox_evidence_fixture}"
rg -Fq 'type="password"' "${firefox_evidence_fixture}"
if rg -ni '<script|<form|https?://|src=|href=' "${firefox_evidence_fixture}"; then
  echo "Firefox context evidence fixture must remain offline and inert." >&2
  exit 1
fi
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
  if strings "${addon_library}" |
      rg -n "${evidence_log_pattern}"; then
    echo "Default addon must not contain application evidence logging." >&2
    exit 1
  fi
  ctest --test-dir "${temp_dir}/cmake-build" --output-on-failure
  echo "Linux Fcitx5 addon build, assembly, and contracts passed."
else
  echo "Linux platform-independent Fcitx5 contracts passed."
  echo "The real Fcitx5 addon target was not built; use --require-fcitx on Linux."
fi
