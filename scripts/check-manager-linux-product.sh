#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-linux-manager-smoke.XXXXXX")"

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

"${repo_root}/scripts/build-manager-linux-product.sh"

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

bundle="${manager_dir}/build/linux/${flutter_arch}/release/bundle"
bundled_ffi="${bundle}/lib/libradishlex_ime_ffi.so"
rime_user_data="${smoke_dir}/rime-user"
mkdir -m 0700 "${rime_user_data}"

RADISHLEX_RIME_SHARED_DATA="${repo_root}/packaging/rime/data" \
RADISHLEX_RIME_USER_DATA="${rime_user_data}" \
RADISHLEX_RIME_SCHEMA="radishlex_pinyin" \
RADISHLEX_EXPECTED_CANDIDATE_PAGE_SIZE=5 \
  cargo test --manifest-path "${repo_root}/Cargo.toml" \
    --locked \
    -p radishlex-ime-ffi \
    --features native-rime \
    rime_session_native_smoke_uses_ffi_entrypoint \
    -- \
    --ignored

(
  cd "${manager_dir}"
  dart run tool/ffi_bridge_smoke.dart \
    --library "${bundled_ffi}" \
    --work-dir "${smoke_dir}"
)

echo "RadishLex Manager Linux product bundle smoke passed without launching the GUI."
