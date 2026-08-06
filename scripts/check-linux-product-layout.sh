#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

usage() {
  echo "usage: $0 [--manager-bundle ABSOLUTE_PATH --addon-stage ABSOLUTE_PATH]" >&2
}

manager_bundle=""
addon_stage=""
if [[ $# -eq 4 && "$1" == "--manager-bundle" && "$3" == "--addon-stage" ]]; then
  manager_bundle="$2"
  addon_stage="$4"
elif [[ $# -ne 0 ]]; then
  usage
  exit 2
fi

"${repo_root}/scripts/check-linux-product-metadata.sh"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_rootfs.py"
bash -n "${repo_root}/scripts/build-linux-product-addon-stage.sh"

if [[ -z "${manager_bundle}" ]]; then
  echo "Linux product rootfs contract passed with synthetic payloads."
  echo "Provide real Manager and addon stages for the ARM64 payload gate."
  exit 0
fi

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "The real Linux product payload gate requires ARM64 Linux." >&2
  exit 1
fi
for command in fc-match file ldd nm readelf strings; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "${command} is required for the real Linux product payload gate." >&2
    exit 1
  fi
done

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-linux-layout.XXXXXX")"
temp_dir="$(CDPATH= cd -- "${temp_dir}" && pwd -P)"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT
assembled_rootfs="${temp_dir}/rootfs"

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/rootfs.py" assemble \
  --manager-bundle "${manager_bundle}" \
  --addon-stage "${addon_stage}" \
  --output "${assembled_rootfs}"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/rootfs.py" verify \
  --rootfs "${assembled_rootfs}"

manager_root="${assembled_rootfs}/usr/lib/aarch64-linux-gnu/radishlex/manager"
manager_executable="${manager_root}/radishlex_manager"
manager_ffi="${manager_root}/lib/libradishlex_ime_ffi.so"
addon_library="${assembled_rootfs}/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so"
addon_ffi="${assembled_rootfs}/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so"

for library in \
  "${manager_executable}" \
  "${manager_root}/lib/libapp.so" \
  "${manager_root}/lib/libflutter_linux_gtk.so" \
  "${manager_ffi}" \
  "${addon_library}" \
  "${addon_ffi}"; do
  file "${library}" | rg -q 'ELF 64-bit.*ARM aarch64'
  if ! dependency_output="$(ldd "${library}" 2>&1)"; then
    echo "Linux product dependency inspection failed: ${library}" >&2
    echo "${dependency_output}" >&2
    exit 1
  fi
  if rg -n 'not found' <<<"${dependency_output}"; then
    echo "Linux product payload has an unresolved dependency: ${library}" >&2
    exit 1
  fi
done

readelf -d "${manager_executable}" | rg -q '\[\$ORIGIN/lib\]'
readelf -d "${addon_library}" | rg -q '\[\$ORIGIN\]'
strings "${addon_library}" | rg -F '/usr/share/radishlex/rime' >/dev/null
for binary in "${manager_executable}" "${addon_library}"; do
  if readelf -d "${binary}" | rg -n '/tmp|/workspace|RadishLex'; then
    echo "Linux product payload contains a build path in dynamic metadata." >&2
    exit 1
  fi
done
for binary in "${manager_executable}" "${manager_ffi}" "${addon_library}"; do
  if strings "${binary}" | rg -F "${repo_root}" >/dev/null; then
    echo "Linux product payload contains the repository path." >&2
    exit 1
  fi
  if strings "${binary}" | rg -F "${manager_bundle}" >/dev/null; then
    echo "Linux product payload contains the Manager staging path." >&2
    exit 1
  fi
  if strings "${binary}" | rg -F "${addon_stage}" >/dev/null; then
    echo "Linux product payload contains the addon staging path." >&2
    exit 1
  fi
done
for symbol in \
  radishlex_ffi_contract \
  radishlex_userdb_terms_new \
  radishlex_userdb_learning_status \
  radishlex_userdb_rank_explain_new \
  radishlex_manager_sync_product_status \
  radishlex_manager_sync_qualification_start; do
  if ! nm -D --defined-only "${manager_ffi}" | \
      rg "[[:space:]]${symbol}$" >/dev/null; then
    echo "Linux product FFI is missing symbol: ${symbol}" >&2
    exit 1
  fi
done

dejavu_family="$(fc-match --format '%{family}\n' 'DejaVu Sans')"
noto_family="$(fc-match --format '%{family}\n' 'Noto Sans CJK SC')"
if [[ "${dejavu_family}" != *"DejaVu Sans"* ]]; then
  echo "Debian text dependency did not resolve DejaVu Sans." >&2
  exit 1
fi
if [[ "${noto_family}" != *"Noto Sans CJK SC"* ]]; then
  echo "Debian text dependency did not resolve Noto Sans CJK SC." >&2
  exit 1
fi

echo "Linux ARM64 product rootfs payload gate passed without system installation."
