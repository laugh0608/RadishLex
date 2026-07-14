#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --status)
    action="status"
    ;;
  --monitor)
    action="monitor"
    ;;
  --authorized-after-settings-removal)
    action="cleanup"
    ;;
  *)
    echo "usage: $0 --status|--monitor|--authorized-after-settings-removal" >&2
    echo "Cleanup requires prior removal in System Settings and explicit authorization." >&2
    exit 2
    ;;
esac

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/tools"
status_tool="${tool_dir}/tis-source-status"
export CLANG_MODULE_CACHE_PATH="${tool_dir}/clang-module-cache"
bundle_id="org.radishlex.inputmethod.macos"
installed_bundle="${HOME}/Library/Input Methods/RadishLexInputMethod.app"
runtime_data="${HOME}/Library/Application Support/RadishLex/Rime"
process_name="RadishLex"

mkdir -p "${tool_dir}" "${CLANG_MODULE_CACHE_PATH}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "${script_dir}/Tools/tis_source_status.m" \
  -framework Carbon -framework Foundation \
  -o "${status_tool}"

if [[ "${action}" == "monitor" ]]; then
  exec "${status_tool}" --monitor
fi

inspect_tis() {
  local output
  local code
  set +e
  output="$("${status_tool}" "${bundle_id}" 2>&1)"
  code=$?
  set -e
  printf '%s\n' "${output}"
  if [[ ${code} -ne 0 && ${code} -ne 4 ]]; then
    echo "Unable to inspect RadishLex TIS state safely." >&2
    return "${code}"
  fi
  TIS_OUTPUT="${output}"
}

print_path_status() {
  [[ -e "${installed_bundle}" ]] && echo "installed_bundle=present" || \
    echo "installed_bundle=absent"
  [[ -e "${runtime_data}" ]] && echo "runtime_data=present" || \
    echo "runtime_data=absent"
  pgrep -x "${process_name}" >/dev/null 2>&1 && echo "process=running" || \
    echo "process=stopped"
}

inspect_tis
print_path_status
if [[ "${action}" == "status" ]]; then
  exit 0
fi

if grep -Eq '^source_id=.* selected=1 ' <<<"${TIS_OUTPUT}" ||
  grep -Eq '^source_id=.* enabled=1 selected=0 select_capable=0 ' \
    <<<"${TIS_OUTPUT}"; then
  echo "RadishLex is still selected or its non-selectable parent is enabled." >&2
  echo "Switch away and remove it in System Settings before cleanup." >&2
  exit 3
fi

rm -rf "${installed_bundle}"
rm -rf "${runtime_data}"
pkill -x "${process_name}" 2>/dev/null || true

if [[ -e "${installed_bundle}" || -e "${runtime_data}" ]]; then
  echo "RadishLex path cleanup is incomplete." >&2
  exit 5
fi
if pgrep -x "${process_name}" >/dev/null 2>&1; then
  echo "RadishLex process is still running." >&2
  exit 6
fi

inspect_tis
if ! grep -q '^matches=0 enabled=0 selected=0$' <<<"${TIS_OUTPUT}"; then
  echo "TIS still reports RadishLex." >&2
  echo "Reopen System Settings, remove any returned row, and rerun cleanup." >&2
  exit 7
fi

echo "RadishLex cleanup verified: settings removal precondition, TIS, paths and process."
