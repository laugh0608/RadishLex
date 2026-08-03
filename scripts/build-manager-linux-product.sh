#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"

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
    flutter build linux \
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
if ldd "${executable}" "${bundled_ffi}" | rg -n 'not found'; then
  echo "Linux Manager staged bundle has an unresolved dynamic dependency." >&2
  exit 1
fi
readelf -d "${executable}" | rg -q '\$ORIGIN/lib'

for symbol in \
  radishlex_ffi_contract \
  radishlex_userdb_list_terms \
  radishlex_userdb_learning_status \
  radishlex_userdb_rank_explain \
  radishlex_manager_sync_product_status \
  radishlex_manager_sync_qualification_start; do
  if ! nm -D --defined-only "${bundled_ffi}" | \
      rg -q "[[:space:]]${symbol}$"; then
    echo "Linux Manager native library is missing symbol: ${symbol}" >&2
    exit 1
  fi
done

echo "RadishLex Manager Linux staged bundle: ${bundle}"
