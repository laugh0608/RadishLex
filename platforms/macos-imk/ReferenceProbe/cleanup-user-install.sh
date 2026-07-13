#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--authorized-after-settings-removal" ]]; then
  echo "Refusing cleanup without explicit authorization." >&2
  echo "First remove the reference probe in System Settings, then rerun with:" >&2
  echo "  $0 --authorized-after-settings-removal" >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/reference-probe-mode/tools"
status_tool="${tool_dir}/tis-source-status"
export CLANG_MODULE_CACHE_PATH="${tool_dir}/clang-module-cache"
source_prefix="org.radishlex.inputmethod.macos.reference-probe-mode"
installed_bundle="${HOME}/Library/Input Methods/RadishLexIMKModeReferenceProbe.app"
runtime_data="${HOME}/Library/Application Support/RadishLexIMKModeReferenceProbe"
process_name="RadishLexIMKModeReferenceProbe"

mkdir -p "${tool_dir}" "${CLANG_MODULE_CACHE_PATH}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "${script_dir}/Tools/tis_source_status.m" \
  -framework Carbon -framework Foundation \
  -o "${status_tool}"

set +e
before_status="$(${status_tool} "${source_prefix}" 2>&1)"
before_code=$?
set -e
printf '%s\n' "${before_status}"
if grep -Eq '^source_id=.* selected=1 ' <<<"${before_status}" ||
  grep -Eq '^source_id=.* enabled=1 selected=0 select_capable=0 ' \
    <<<"${before_status}"; then
  echo "Reference probe is still selected or its parent is enabled." >&2
  echo "Remove it in System Settings before deleting the bundle." >&2
  exit 3
fi
if [[ ${before_code} -ne 0 && ${before_code} -ne 4 ]]; then
  echo "Unable to inspect reference probe TIS state safely." >&2
  exit "${before_code}"
fi

rm -rf "${installed_bundle}"
rm -rf "${runtime_data}"
pkill -x "${process_name}" 2>/dev/null || true

if [[ -e "${installed_bundle}" || -e "${runtime_data}" ]]; then
  echo "Reference probe path cleanup is incomplete." >&2
  exit 5
fi
if pgrep -x "${process_name}" >/dev/null 2>&1; then
  echo "Reference probe process is still running." >&2
  exit 6
fi

set +e
after_status="$(${status_tool} "${source_prefix}" 2>&1)"
after_code=$?
set -e
printf '%s\n' "${after_status}"
if [[ ${after_code} -ne 0 ]] || ! grep -q '^matches=0 enabled=0 selected=0$' \
  <<<"${after_status}"; then
  echo "TIS still reports the reference probe." >&2
  echo "Reopen System Settings, remove any returned row, and rerun cleanup." >&2
  exit 7
fi

echo "Reference probe cleanup verified: settings removal precondition, TIS, paths and process."
