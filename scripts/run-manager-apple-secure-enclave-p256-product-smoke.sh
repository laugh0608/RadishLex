#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 AUTHORIZED_SCENARIO" >&2
  echo "AUTHORIZED_SCENARIO must be exactly one of:" >&2
  echo "  --authorized-secure-enclave-product-smoke" >&2
  echo "  --authorized-secure-enclave-denied-probe" >&2
  echo "  --authorized-secure-enclave-locked-prepare" >&2
  echo "  --authorized-secure-enclave-locked-probe" >&2
  echo "  --authorized-secure-enclave-locked-cleanup" >&2
  echo "  --authorized-secure-enclave-unsupported-probe" >&2
}

if [ "$#" -ne 1 ]; then
  usage
  exit 2
fi

case "$1" in
  --authorized-secure-enclave-product-smoke)
    product_argument="--radishlex-apple-secure-enclave-p256-product-smoke"
    ;;
  --authorized-secure-enclave-denied-probe)
    product_argument="--radishlex-apple-secure-enclave-p256-denied-probe"
    ;;
  --authorized-secure-enclave-locked-prepare)
    product_argument="--radishlex-apple-secure-enclave-p256-locked-prepare"
    echo "The prepare scenario leaves one synthetic Secure Enclave item for the locked probe." >&2
    ;;
  --authorized-secure-enclave-locked-probe)
    product_argument="--radishlex-apple-secure-enclave-p256-locked-probe"
    ;;
  --authorized-secure-enclave-locked-cleanup)
    product_argument="--radishlex-apple-secure-enclave-p256-locked-cleanup"
    ;;
  --authorized-secure-enclave-unsupported-probe)
    product_argument="--radishlex-apple-secure-enclave-p256-unsupported-probe"
    ;;
  *)
    usage
    exit 2
    ;;
esac

echo "This launches the manager product process and touches Secure Enclave/Keychain state." >&2
echo "The script does not lock, unlock, or reconfigure any Keychain." >&2

if [ "$(uname -s)" != "Darwin" ]; then
  echo "manager Apple Secure Enclave P-256 product smoke requires macOS." >&2
  exit 1
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
product_binary="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app/Contents/MacOS/radishlex_manager"
native_library="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app/Contents/Frameworks/libradishlex_ime_ffi.dylib"
go_server_dir="${repo_root}/server/sync-server"

for required_path in "${product_binary}" "${native_library}" "${go_server_dir}/go.mod"; do
  if [ ! -e "${required_path}" ]; then
    echo "required product smoke input is missing: ${required_path}" >&2
    exit 1
  fi
done

exec env \
  RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_P256_SMOKE=1 \
  RADISHLEX_MANAGER_APPLE_P256_GO_SERVER_DIR="${go_server_dir}" \
  "${product_binary}" "${product_argument}"
