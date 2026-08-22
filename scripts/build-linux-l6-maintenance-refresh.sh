#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd -P)"
refresh_tool="${repo_root}/scripts/linux-product/l6_maintenance_refresh.py"

usage() {
  echo "usage: $0 --base-record ABSOLUTE_FILE --target-package ABSOLUTE_FILE --target-artifact-evidence ABSOLUTE_FILE --refresh-root ABSOLUTE_PATH --output ABSENT_ABSOLUTE_PATH" >&2
}

if [[ $# -ne 10 || "$1" != "--base-record" || "$3" != "--target-package" || "$5" != "--target-artifact-evidence" || "$7" != "--refresh-root" || "$9" != "--output" ]]; then
  usage
  exit 2
fi

base_record_input="$2"
target_package_input="$4"
target_artifact_evidence_input="$6"
refresh_root="$8"
output="${10}"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "The L6 maintenance refresh builder requires Debian 13 ARM64 Linux." >&2
  exit 1
fi
for command in cargo git python3; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "${command} is required by the L6 maintenance refresh builder." >&2
    exit 1
  fi
done
if [[ "${refresh_root}" != /* || ! -d "${refresh_root}" || -L "${refresh_root}" ]]; then
  echo "The refresh root must be an absolute real directory." >&2
  exit 1
fi
canonical_refresh_root="$(CDPATH= cd -- "${refresh_root}" && pwd -P)"
if [[ "${canonical_refresh_root}" != "${refresh_root}" ]]; then
  echo "The refresh root must not traverse symlinked components." >&2
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

PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" validate-contract
PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" validate-refresh \
  --refresh-root "${refresh_root}"
export CARGO_NET_OFFLINE=true

temporary="$(mktemp -d "${canonical_output_parent}/.radishlex-l6-maintenance-refresh.XXXXXX")"
cleanup() {
  rm -rf -- "${temporary}"
}
trap cleanup EXIT
staging="${temporary}/maintenance-refresh"
PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" stage-base \
  --base-record "${base_record_input}" \
  --target-package "${target_package_input}" \
  --target-artifact-evidence "${target_artifact_evidence_input}" \
  --refresh-root "${refresh_root}" \
  --output-dir "${staging}"

cargo_home="${CARGO_HOME:-${HOME:-}/.cargo}"
if [[ "${cargo_home}" != /* ]]; then
  echo "The L6 maintenance refresh builder requires an absolute Cargo home." >&2
  exit 1
fi
rust_flag_separator=$'\x1f'
encoded_rustflags="--remap-path-prefix=${refresh_root}=/usr/src/radishlex"
encoded_rustflags+="${rust_flag_separator}--remap-path-prefix=${cargo_home}=/usr/src/cargo"
env \
  -u CARGO_BUILD_RUSTC \
  -u CARGO_BUILD_TARGET \
  -u RUSTC_WORKSPACE_WRAPPER \
  -u RUSTC_WRAPPER \
  -u RUSTFLAGS \
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="${refresh_root}/target" \
  CARGO_ENCODED_RUSTFLAGS="${encoded_rustflags}" \
  cargo build --manifest-path "${refresh_root}/Cargo.toml" \
  --locked --release --no-default-features \
  -p radishlex-linux-product-install \
  --bin radishlex-linux-maintenance \
  --bin radishlex-linux-artifact-verifier

cp "${refresh_root}/target/release/radishlex-linux-maintenance" \
  "${staging}/radishlex-linux-maintenance"
chmod 0755 "${staging}/radishlex-linux-maintenance"

target_package="${staging}/target/artifacts/$(basename -- "${target_package_input}")"
target_artifact_evidence="${staging}/target/artifacts/$(basename -- "${target_artifact_evidence_input}")"
artifact_verifier="${refresh_root}/target/release/radishlex-linux-artifact-verifier"
"${artifact_verifier}" \
  --package "${target_package}" \
  --evidence "${target_artifact_evidence}"

PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" environment \
  --output "${staging}/build-environment.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" record \
  --base-record "${staging}/base/release-pair.evidence.json" \
  --target-package "${target_package}" \
  --target-artifact-evidence "${target_artifact_evidence}" \
  --refresh-root "${refresh_root}" \
  --environment "${staging}/build-environment.json" \
  --maintenance-executable "${staging}/radishlex-linux-maintenance" \
  --output "${staging}/maintenance-refresh.evidence.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" verify \
  --evidence "${staging}/maintenance-refresh.evidence.json"
PYTHONDONTWRITEBYTECODE=1 python3 "${refresh_tool}" publish \
  --staging "${staging}" \
  --output "${canonical_output}"

echo "L6 ARM64 maintenance refresh: ${canonical_output}"
