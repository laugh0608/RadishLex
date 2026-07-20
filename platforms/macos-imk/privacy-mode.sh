#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --status|--capture-baseline|--authorized-enable|--authorized-restore)
    action="${1}"
    ;;
  *)
    echo "usage: $0 --status|--capture-baseline|--authorized-enable|--authorized-restore" >&2
    echo "Enable and restore require explicit R01B authorization." >&2
    exit 2
    ;;
esac
if [[ $# -ne 1 ]]; then
  echo "privacy mode actions do not accept caller-provided paths or values" >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/tools"
state_dir="${repo_root}/target/macos-imk/r01b"
tool="${tool_dir}/privacy-mode-control"
export CLANG_MODULE_CACHE_PATH="${tool_dir}/clang-module-cache"

mkdir -p "${tool_dir}" "${CLANG_MODULE_CACHE_PATH}"
state_dir_define="-DRLX_PRIVACY_STATE_DIR=\"${state_dir}\""
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_PRIVACY_CONTRACT=0 \
  "${state_dir_define}" \
  "${script_dir}/Tools/privacy_mode_control.m" \
  -framework Foundation \
  -o "${tool}"

exec "${tool}" "${action}"
