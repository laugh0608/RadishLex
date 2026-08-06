#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

usage() {
  echo "usage: $0 --ffi-library ABSOLUTE_PATH --output ABSOLUTE_PATH" >&2
}

if [[ $# -ne 4 || "$1" != "--ffi-library" || "$3" != "--output" ]]; then
  usage
  exit 2
fi

ffi_library="$2"
output="$4"
if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "Linux product addon stage requires Debian-compatible ARM64 Linux." >&2
  exit 1
fi
for command in cmake ctest file ldd readelf strings; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "${command} is required for the Linux product addon stage." >&2
    exit 1
  fi
done
if [[ "${ffi_library}" != /* || ! -f "${ffi_library}" || -L "${ffi_library}" ]]; then
  echo "--ffi-library must identify an absolute regular file." >&2
  exit 1
fi
ffi_parent="$(CDPATH= cd -- "$(dirname -- "${ffi_library}")" && pwd -P)"
canonical_ffi="${ffi_parent}/$(basename -- "${ffi_library}")"
if [[ "${canonical_ffi}" != "${ffi_library}" ]]; then
  echo "--ffi-library must not traverse symlinked path components." >&2
  exit 1
fi
if [[ "${output}" != /* || -e "${output}" || -L "${output}" ]]; then
  echo "--output must be an absent absolute path." >&2
  exit 1
fi
output_parent="$(dirname -- "${output}")"
if [[ ! -d "${output_parent}" || -L "${output_parent}" ]]; then
  echo "--output parent must be an existing real directory." >&2
  exit 1
fi
canonical_output_parent="$(CDPATH= cd -- "${output_parent}" && pwd -P)"
canonical_output="${canonical_output_parent}/$(basename -- "${output}")"
if [[ "${canonical_output}" != "${output}" ]]; then
  echo "--output must not traverse symlinked path components." >&2
  exit 1
fi

product_version="$(
  PYTHONDONTWRITEBYTECODE=1 python3 \
    "${repo_root}/scripts/linux-product/product_metadata.py" \
    field product_version
)"
temp_dir="$(mktemp -d "${canonical_output_parent}/.radishlex-addon.XXXXXX")"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

build_dir="${temp_dir}/build"
stage_dir="${temp_dir}/stage"
path_map_flags="-ffile-prefix-map=${repo_root}=/usr/src/radishlex"
path_map_flags+=" -ffile-prefix-map=${temp_dir}=/usr/src/radishlex-build"
cmake -S "${repo_root}/platforms/linux-fcitx5" -B "${build_dir}" \
  -DCMAKE_BUILD_TYPE=Release \
  "-DCMAKE_CXX_FLAGS=${path_map_flags}" \
  -DCMAKE_INSTALL_PREFIX=/usr \
  -DCMAKE_INSTALL_LIBDIR=lib/aarch64-linux-gnu \
  -DRADISHLEX_BUILD_FCITX_ADDON=ON \
  -DRADISHLEX_BUILD_CONTRACT_TESTS=ON \
  -DRADISHLEX_RUNTIME_LAYOUT_PROFILE=system \
  "-DRADISHLEX_PRODUCT_VERSION=${product_version}" \
  "-DRADISHLEX_IME_FFI_LIBRARY=${canonical_ffi}"
cmake --build "${build_dir}"
ctest --test-dir "${build_dir}" --output-on-failure
DESTDIR="${stage_dir}" cmake --install "${build_dir}"

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/rootfs.py" \
  validate-addon-stage --addon-stage "${stage_dir}"

addon_library="${stage_dir}/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so"
staged_ffi="${stage_dir}/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so"
for library in "${addon_library}" "${staged_ffi}"; do
  file "${library}" | rg -q 'ELF 64-bit.*ARM aarch64'
  if ! dependency_output="$(ldd "${library}" 2>&1)"; then
    echo "Linux product addon stage dependency inspection failed." >&2
    echo "${dependency_output}" >&2
    exit 1
  fi
  if rg -n 'not found' <<<"${dependency_output}"; then
    echo "Linux product addon stage has an unresolved dependency." >&2
    exit 1
  fi
done
readelf -d "${addon_library}" | rg -q '\[\$ORIGIN\]'
if readelf -d "${addon_library}" | rg -n '/tmp|/workspace|RadishLex'; then
  echo "Linux product addon contains a build path in dynamic metadata." >&2
  exit 1
fi
strings "${addon_library}" | rg -F '/usr/share/radishlex/rime' >/dev/null
for library in "${addon_library}" "${staged_ffi}"; do
  if strings "${library}" | rg -F "${repo_root}" >/dev/null; then
    echo "Linux product addon stage contains the repository path." >&2
    exit 1
  fi
  if strings "${library}" | rg -F "${temp_dir}" >/dev/null; then
    echo "Linux product addon stage contains its temporary build path." >&2
    exit 1
  fi
  if [[ "${HOME:-}" == /* ]] && \
      strings "${library}" | rg -F "${HOME}/" >/dev/null; then
    echo "Linux product addon stage contains the build home path." >&2
    exit 1
  fi
done
if find "${stage_dir}" -type d -name radishlex-rime -print -quit | rg -q .; then
  echo "Linux product addon stage must not carry staged-profile RimeData." >&2
  exit 1
fi

mv "${stage_dir}" "${canonical_output}"
echo "Linux product addon stage: ${canonical_output}"
