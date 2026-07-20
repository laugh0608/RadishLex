#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 AUTHORIZED_SCENARIO" >&2
  echo "AUTHORIZED_SCENARIO must be exactly one of:" >&2
  echo "  --authorized-key-agreement-product-smoke" >&2
  echo "  --authorized-key-agreement-denied-probe" >&2
  echo "  --authorized-key-agreement-locked-prepare" >&2
  echo "  --authorized-key-agreement-locked-probe" >&2
  echo "  --authorized-key-agreement-locked-cleanup" >&2
  echo "  --authorized-key-agreement-unsupported-probe" >&2
}

if [ "$#" -ne 1 ]; then
  usage
  exit 2
fi

wait_for_device_lock=0
locked_probe_delay_seconds=20

case "$1" in
  --authorized-key-agreement-product-smoke)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-product-smoke"
    ;;
  --authorized-key-agreement-denied-probe)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-denied-probe"
    ;;
  --authorized-key-agreement-locked-prepare)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-locked-prepare"
    echo "The prepare scenario leaves one synthetic Secure Enclave key-agreement item for the locked probe." >&2
    ;;
  --authorized-key-agreement-locked-probe)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-locked-probe"
    wait_for_device_lock=1
    ;;
  --authorized-key-agreement-locked-cleanup)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-locked-cleanup"
    ;;
  --authorized-key-agreement-unsupported-probe)
    product_argument="--radishlex-apple-secure-enclave-key-agreement-unsupported-probe"
    ;;
  *)
    usage
    exit 2
    ;;
esac

echo "This launches the manager product process and creates, reads, derives with, or deletes a Secure Enclave/Keychain key-agreement item." >&2
echo "The script does not lock, unlock, or reconfigure any Keychain." >&2

if [ "$(uname -s)" != "Darwin" ]; then
  echo "manager Apple Secure Enclave key-agreement product smoke requires macOS." >&2
  exit 1
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"
product_binary="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app/Contents/MacOS/radishlex_manager"
native_library="${manager_dir}/build/macos/Build/Products/Release/radishlex_manager.app/Contents/Frameworks/libradishlex_ime_ffi.dylib"

for required_path in "${product_binary}" "${native_library}"; do
  if [ ! -e "${required_path}" ]; then
    echo "required product smoke input is missing: ${required_path}" >&2
    exit 1
  fi
done

if [ "${wait_for_device_lock}" -eq 1 ]; then
  echo "The device-locked ECDH probe starts in ${locked_probe_delay_seconds} seconds." >&2
  echo "During this delay, manually lock the screen without sleeping or closing the lid." >&2
  sleep "${locked_probe_delay_seconds}"
fi

exec env \
  RADISHLEX_RUN_MANAGER_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SMOKE=1 \
  "${product_binary}" "${product_argument}"
