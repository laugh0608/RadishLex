#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd -P)"
pair_tool="${repo_root}/scripts/linux-product/l6_release_pair.py"

usage() {
  echo "usage: $0 --source-package ABSOLUTE_FILE --source-artifact-evidence ABSOLUTE_FILE --target-root ABSOLUTE_PATH --output ABSENT_ABSOLUTE_PATH" >&2
}

if [[ $# -ne 8 || "$1" != "--source-package" || "$3" != "--source-artifact-evidence" || "$5" != "--target-root" || "$7" != "--output" ]]; then
  usage
  exit 2
fi

source_package_input="$2"
source_artifact_evidence_input="$4"
target_root="$6"
output="$8"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "The L6 release pair builder requires Debian 13 ARM64 Linux." >&2
  exit 1
fi
for command in cargo cmake dpkg-deb dpkg-shlibdeps file flutter git python3 readelf; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "${command} is required by the L6 release pair builder." >&2
    exit 1
  fi
done
if [[ "${target_root}" != /* || ! -d "${target_root}" || -L "${target_root}" ]]; then
  echo "The target root must be an absolute real directory." >&2
  exit 1
fi
canonical_target_root="$(CDPATH= cd -- "${target_root}" && pwd -P)"
if [[ "${canonical_target_root}" != "${target_root}" ]]; then
  echo "The target root must not traverse symlinked components." >&2
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
  echo "--output must not traverse symlinked components." >&2
  exit 1
fi

PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" validate-contract
PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" validate-target \
  --target-root "${target_root}"
export CARGO_NET_OFFLINE=true

temporary="$(mktemp -d "${canonical_output_parent}/.radishlex-l6-release-pair.XXXXXX")"
cleanup() {
  rm -rf -- "${temporary}"
}
trap cleanup EXIT
staging="${temporary}/release-pair"
work_root="${temporary}/work"
mkdir -m 0755 "${staging}" "${work_root}"
mkdir -m 0755 "${staging}/source"
PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" stage-source \
  --source-package "${source_package_input}" \
  --source-artifact-evidence "${source_artifact_evidence_input}" \
  --target-root "${target_root}" \
  --output-dir "${staging}/source/artifacts"

build_target_release() {
  local root="$1"
  local role_work="${work_root}/target"
  local addon_stage="${role_work}/addon"
  local rootfs="${role_work}/rootfs"
  local artifacts="${staging}/target/artifacts"
  local manager_bundle="${root}/apps/radishlex-manager/build/linux/arm64/release/bundle"

  mkdir -m 0755 "${role_work}"
  mkdir -p -m 0755 "${artifacts}"
  "${root}/scripts/check-linux-product-metadata.sh"
  env \
    -u CPATH \
    -u C_INCLUDE_PATH \
    -u CPLUS_INCLUDE_PATH \
    -u OBJC_INCLUDE_PATH \
    -u CARGO_BUILD_RUSTC \
    -u CARGO_BUILD_TARGET \
    -u RUSTC_WORKSPACE_WRAPPER \
    -u RUSTC_WRAPPER \
    CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR="${root}/target" \
    "${root}/scripts/build-manager-linux-product.sh" --system-product
  "${root}/scripts/build-linux-product-addon-stage.sh" \
    --ffi-library "${manager_bundle}/lib/libradishlex_ime_ffi.so" \
    --output "${addon_stage}"
  PYTHONDONTWRITEBYTECODE=1 python3 \
    "${root}/scripts/linux-product/rootfs.py" assemble \
    --manager-bundle "${manager_bundle}" \
    --addon-stage "${addon_stage}" \
    --output "${rootfs}"
  PYTHONDONTWRITEBYTECODE=1 python3 \
    "${root}/scripts/linux-product/rootfs.py" verify \
    --rootfs "${rootfs}"
  "${root}/scripts/check-linux-product-layout.sh" \
    --manager-bundle "${manager_bundle}" \
    --addon-stage "${addon_stage}"
  "${root}/scripts/build-linux-deb-artifact.sh" \
    --rootfs "${rootfs}" \
    --output-dir "${artifacts}"
}

build_target_release "${target_root}"

cargo_home="${CARGO_HOME:-${HOME:-}/.cargo}"
if [[ "${cargo_home}" != /* ]]; then
  echo "The L6 release pair builder requires an absolute Cargo home." >&2
  exit 1
fi
rust_flag_separator=$'\x1f'
encoded_rustflags="--remap-path-prefix=${target_root}=/usr/src/radishlex"
encoded_rustflags+="${rust_flag_separator}--remap-path-prefix=${cargo_home}=/usr/src/cargo"
env \
  -u CARGO_BUILD_RUSTC \
  -u CARGO_BUILD_TARGET \
  -u RUSTC_WORKSPACE_WRAPPER \
  -u RUSTC_WRAPPER \
  -u RUSTFLAGS \
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="${target_root}/target" \
  CARGO_ENCODED_RUSTFLAGS="${encoded_rustflags}" \
  cargo build --manifest-path "${target_root}/Cargo.toml" \
  --locked --release --no-default-features \
  -p radishlex-linux-product-install \
  --bin radishlex-linux-maintenance \
  --bin radishlex-linux-artifact-verifier
cp "${target_root}/target/release/radishlex-linux-maintenance" \
  "${staging}/radishlex-linux-maintenance"
chmod 0755 "${staging}/radishlex-linux-maintenance"

env \
  -u CARGO_BUILD_RUSTC \
  -u CARGO_BUILD_TARGET \
  -u RUSTC_WORKSPACE_WRAPPER \
  -u RUSTC_WRAPPER \
  -u RUSTFLAGS \
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="${target_root}/target" \
  CARGO_ENCODED_RUSTFLAGS="${encoded_rustflags}" \
  cargo build --manifest-path "${target_root}/Cargo.toml" \
  --locked --release \
  -p radishlex-linux-l6-acceptance \
  --bin radishlex-linux-l6-acceptance
cp "${target_root}/target/release/radishlex-linux-l6-acceptance" \
  "${staging}/radishlex-linux-l6-acceptance"
chmod 0755 "${staging}/radishlex-linux-l6-acceptance"

PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" environment \
  --output "${staging}/build-environment.json"

metadata_field() {
  local root="$1"
  local field="$2"
  PYTHONDONTWRITEBYTECODE=1 python3 \
    "${root}/scripts/linux-product/product_metadata.py" field "${field}"
}
source_package="$(basename -- "${source_package_input}")"
target_package="$(metadata_field "${target_root}" package_name)_$(metadata_field "${target_root}" package_version)_$(metadata_field "${target_root}" debian_architecture).deb"

artifact_verifier="${target_root}/target/release/radishlex-linux-artifact-verifier"
verify_release_artifact() {
  local role="$1"
  local package="$2"
  "${artifact_verifier}" \
    --package "${staging}/${role}/artifacts/${package}" \
    --evidence "${staging}/${role}/artifacts/${package}.evidence.json"
}
verify_release_artifact source "${source_package}"
verify_release_artifact target "${target_package}"

PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" record \
  --source-package "${staging}/source/artifacts/${source_package}" \
  --source-artifact-evidence "${staging}/source/artifacts/${source_package}.evidence.json" \
  --target-root "${target_root}" \
  --target-package "${staging}/target/artifacts/${target_package}" \
  --target-artifact-evidence "${staging}/target/artifacts/${target_package}.evidence.json" \
  --environment "${staging}/build-environment.json" \
  --maintenance-executable "${staging}/radishlex-linux-maintenance" \
  --acceptance-executable "${staging}/radishlex-linux-l6-acceptance" \
  --output "${staging}/release-pair.evidence.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${pair_tool}" verify \
  --evidence "${staging}/release-pair.evidence.json"

mv "${staging}" "${canonical_output}"
echo "L6 ARM64 release pair: ${canonical_output}"
