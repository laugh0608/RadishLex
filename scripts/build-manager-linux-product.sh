#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
cargo_home="${CARGO_HOME:-${HOME:-}/.cargo}"
flutter_offline_arguments=()

case "${1:-}" in
  --development-staged)
    native_profile="development-staged"
    startup_identity_marker="development-staged"
    ;;
  --system-product)
    native_profile="system-product"
    startup_identity_marker="debian-system-product"
    ;;
  --system-product-offline)
    native_profile="system-product"
    startup_identity_marker="debian-system-product"
    flutter_offline_arguments=(--no-pub)
    export CARGO_NET_OFFLINE=true
    if [[ ! -f "${manager_dir}/.dart_tool/package_config.json" ]]; then
      echo "Offline Manager build requires pre-provisioned pub package configuration." >&2
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 --development-staged|--system-product|--system-product-offline" >&2
    exit 2
    ;;
esac
if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 --development-staged|--system-product|--system-product-offline" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "RadishLex Manager Linux product build requires Linux." >&2
  exit 1
fi

case "$(uname -m)" in
  x86_64)
    flutter_arch="x64"
    ;;
  aarch64|arm64)
    flutter_arch="arm64"
    ;;
  *)
    echo "Unsupported Linux Manager architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

if [[ "${cargo_home}" != /* ]]; then
  echo "Linux Manager product build requires an absolute Cargo home." >&2
  exit 1
fi

rust_flag_separator=$'\x1f'
encoded_rustflags="--remap-path-prefix=${repo_root}=/usr/src/radishlex"
encoded_rustflags+="${rust_flag_separator}--remap-path-prefix=${cargo_home}=/usr/src/cargo"

env -u RUSTFLAGS \
  CARGO_ENCODED_RUSTFLAGS="${encoded_rustflags}" \
  cargo build --manifest-path "${repo_root}/Cargo.toml" \
  --locked \
  -p radishlex-ime-ffi \
  --release \
  --features native-rime

ffi_library="${repo_root}/target/release/libradishlex_ime_ffi.so"
if [[ ! -f "${ffi_library}" ]]; then
  echo "Linux Manager native-rime library was not produced." >&2
  exit 1
fi

(
  cd "${manager_dir}"
  RADISHLEX_MANAGER_FFI_LIBRARY="${ffi_library}" \
  RADISHLEX_MANAGER_NATIVE_PROFILE="${native_profile}" \
    flutter build linux \
      "${flutter_offline_arguments[@]}" \
      --release \
      --dart-define=RADISHLEX_MANAGER_MODE=product
)

bundle="${manager_dir}/build/linux/${flutter_arch}/release/bundle"
executable="${bundle}/radishlex_manager"
bundled_ffi="${bundle}/lib/libradishlex_ime_ffi.so"
if [[ ! -x "${executable}" || ! -f "${bundled_ffi}" ]]; then
  echo "Linux Manager staged bundle is incomplete." >&2
  exit 1
fi

file "${executable}" "${bundled_ffi}"
manager_dynamic="$(LC_ALL=C readelf -d "${executable}")"
if ! rg -q '\(NEEDED\).*\[libradishlex_ime_ffi\.so\]$' <<<"${manager_dynamic}" ||
    rg -q '\(NEEDED\).*\[[^]]*/' <<<"${manager_dynamic}"; then
  echo "Linux Manager must name its FFI dependency by basename and use no path-based dependencies." >&2
  exit 1
fi
if ! rg -q '\((RUNPATH|RPATH)\).*\[\$ORIGIN/lib\]$' <<<"${manager_dynamic}"; then
  echo 'Linux Manager search path must be $ORIGIN/lib.' >&2
  exit 1
fi
if ! dependencies="$(env -u LD_LIBRARY_PATH -u LD_PRELOAD -u LD_AUDIT LC_ALL=C ldd "${executable}" "${bundled_ffi}" 2>&1)" ||
    rg -n 'not found' <<<"${dependencies}"; then
  echo "Linux Manager staged bundle has an unresolved dynamic dependency." >&2
  echo "${dependencies}" >&2
  exit 1
fi
if ! rg -Fq "libradishlex_ime_ffi.so => ${bundled_ffi} (" <<<"${dependencies}"; then
  echo "Linux Manager must resolve its FFI from the bundled lib directory." >&2
  exit 1
fi
strings "${executable}" | \
  rg -Fq "radishlex-linux-startup:${startup_identity_marker}-v1"

for symbol in \
  radishlex_ffi_contract \
  radishlex_userdb_terms_new \
  radishlex_userdb_learning_status \
  radishlex_userdb_rank_explain_new \
  radishlex_manager_sync_product_status \
  radishlex_linux_product_startup_gate \
  radishlex_manager_sync_qualification_start; do
  if ! nm -D --defined-only "${bundled_ffi}" | \
      rg -q "[[:space:]]${symbol}$"; then
    echo "Linux Manager native library is missing symbol: ${symbol}" >&2
    exit 1
  fi
done

echo "RadishLex Manager Linux ${native_profile} bundle: ${bundle}"
