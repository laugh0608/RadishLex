#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-manager-ffi-smoke.XXXXXX")"

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build radishlex-ime-ffi." >&2
  exit 1
fi

if ! command -v dart >/dev/null 2>&1; then
  echo "dart is required to run RadishLex manager FFI smoke." >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin)
    ffi_library="${target_dir}/debug/libradishlex_ime_ffi.dylib"
    ;;
  Linux)
    ffi_library="${target_dir}/debug/libradishlex_ime_ffi.so"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    ffi_library="${target_dir}/debug/radishlex_ime_ffi.dll"
    ;;
  *)
    echo "unsupported OS for manager FFI smoke: $(uname -s)" >&2
    exit 1
    ;;
esac

(
  cd "${repo_root}"
  cargo build -p radishlex-ime-ffi
)

if [ ! -f "${ffi_library}" ]; then
  echo "radishlex-ime-ffi dynamic library not found: ${ffi_library}" >&2
  exit 1
fi

(
  cd "${manager_dir}"
  dart run tool/ffi_bridge_smoke.dart \
    --library "${ffi_library}" \
    --work-dir "${smoke_dir}"
)
