#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

usage() {
  echo "usage: $0 --rootfs ABSOLUTE_PATH --output-dir ABSOLUTE_PATH" >&2
}

if [[ $# -ne 4 || "$1" != "--rootfs" || "$3" != "--output-dir" ]]; then
  usage
  exit 2
fi

product_rootfs="$2"
output_dir="$4"
if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "The Debian artifact builder requires ARM64 Linux." >&2
  exit 1
fi
for command in dpkg-deb dpkg-shlibdeps python3; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "${command} is required by the Debian artifact builder." >&2
    exit 1
  fi
done

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/rootfs.py" verify \
  --rootfs "${product_rootfs}"

multiarch="$(PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" field multiarch_tuple)"
package_name="$(PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" field package_name)"
librime_package="$(PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" field librime_package)"
package_version="$(PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" field package_version)"
architecture="$(PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" field debian_architecture)"

manager_root="${product_rootfs}/usr/lib/${multiarch}/radishlex/manager"
addon_root="${product_rootfs}/usr/lib/${multiarch}/fcitx5"
payload_binaries=(
  "${manager_root}/radishlex_manager"
  "${manager_root}/lib/libapp.so"
  "${manager_root}/lib/libflutter_linux_gtk.so"
  "${manager_root}/lib/libradishlex_ime_ffi.so"
  "${addon_root}/radishlex.so"
  "${addon_root}/libradishlex_ime_ffi.so"
)
for binary in "${payload_binaries[@]}"; do
  if [[ ! -f "${binary}" || -L "${binary}" ]]; then
    echo "Debian artifact ELF input is unavailable: ${binary}" >&2
    exit 1
  fi
done

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-deb-artifact.XXXXXX")"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT
shlibs_evidence="${temp_dir}/shlibs-depends.txt"
(
  cd "${temp_dir}"
  DPKG_COLORS=never DPKG_NLS=0 dpkg-shlibdeps \
    --warnings=0 \
    -O \
    -x"${package_name}" \
    -x"${librime_package}" \
    -l"${manager_root}/lib" \
    -l"${addon_root}" \
    "${payload_binaries[@]}"
) >"${shlibs_evidence}"

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/deb_artifact.py" build \
  --rootfs "${product_rootfs}" \
  --shlibs-depends "${shlibs_evidence}" \
  --output-dir "${output_dir}"

package_path="${output_dir}/${package_name}_${package_version}_${architecture}.deb"
evidence_path="${package_path}.evidence.json"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/deb_artifact.py" verify \
  --package "${package_path}" \
  --evidence "${evidence_path}"
dpkg-deb --info "${package_path}" >/dev/null
dpkg-deb --contents "${package_path}" >/dev/null

echo "Deterministic Debian ARM64 artifact gate passed without installation."
